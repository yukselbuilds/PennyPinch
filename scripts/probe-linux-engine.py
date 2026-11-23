"""Check the pinned Linux engine with an owned Unix socket and local Responses fixture."""

from contextlib import contextmanager
from http.client import HTTPConnection
from importlib.metadata import version
from importlib.util import module_from_spec, spec_from_file_location
import json
import os
from pathlib import Path
import platform
import signal
import site
import socket
import struct
import subprocess
import sys
import tempfile
import time
import uuid


ENGINE_VERSION = "0.44.0"
MAX_BODY = 256 * 1024
PEER_CREDENTIALS = struct.Struct("iII")
BOOTSTRAP = """
import site, sys
site.addsitedir(sys.argv.pop(1))
from headroom.cli import main
try:
    main()
except KeyboardInterrupt:
    pass
"""


class ProbeError(RuntimeError):
    pass


class PeerMismatch(ProbeError):
    pass


def require(condition, message):
    if not condition:
        raise ProbeError(message)


@contextmanager
def owned_connection(path, pid, timeout=2):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as transport:
        transport.settimeout(timeout)
        transport.connect(str(path))
        credentials = transport.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, PEER_CREDENTIALS.size)
        peer_pid, peer_uid, _ = PEER_CREDENTIALS.unpack(credentials)
        if peer_pid != pid or peer_uid != os.geteuid():
            raise PeerMismatch("unexpected Unix socket peer")
        connection = HTTPConnection("localhost", timeout=timeout)
        connection.sock = transport
        try:
            yield connection
        finally:
            connection.close()


def owned_get(path, pid, route):
    with owned_connection(path, pid) as connection:
        connection.request("GET", route, headers={"Connection": "close"})
        response = connection.getresponse()
        if response.status != 200:
            return {}
        body = response.read(MAX_BODY + 1)
        require(len(body) <= MAX_BODY, "unexpected status body size")
        data = json.loads(body)
        require(isinstance(data, dict), "unexpected status body type")
        return data


def wait_ready(process, endpoint):
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        require(process.poll() is None, f"engine exited before readiness ({process.returncode})")
        try:
            data = owned_get(endpoint, process.pid, "/readyz")
        except (FileNotFoundError, ConnectionRefusedError, TimeoutError):
            data = {}
        if data.get("service") == "headroom-proxy" and data.get("ready") is True:
            require(data.get("version") == ENGINE_VERSION, "unexpected ready engine version")
            return
        time.sleep(0.1)
    raise ProbeError("owned engine readiness timed out")


def check_foreign_peer(state, engine_pid):
    path = state / "foreign"
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as listener:
        listener.settimeout(2)
        listener.bind(str(path))
        listener.listen(1)
        try:
            owned_get(path, engine_pid, "/readyz")
        except PeerMismatch:
            accepted, _ = listener.accept()
            with accepted:
                accepted.settimeout(2)
                require(accepted.recv(1) == b"", "foreign peer received HTTP")
        else:
            raise ProbeError("foreign peer was accepted")
    path.unlink()


def check_response(endpoint, pid, fixture):
    body = {"model": fixture.model, "reasoning": {"effort": fixture.reasoning_effort},
            "input": "Return the local fixture marker. " + fixture.nonce, "stream": True}
    fixture.release_terminal()
    with owned_connection(endpoint, pid, timeout=25) as connection:
        connection.request("POST", "/v1/responses", body=json.dumps(body), headers={
            "Authorization": "Bearer " + fixture.api_key,
            "Content-Type": "application/json",
            "Connection": "close",
        })
        response = connection.getresponse()
        require(response.status == 200, f"unexpected fixture HTTP status ({response.status})")
        require(response.getheader("Content-Type", "").startswith("text/event-stream"), "expected an SSE stream")
        payload = response.read(MAX_BODY + 1)
        require(len(payload) <= MAX_BODY, "fixture response exceeded its limit")
    events = [json.loads(line[6:]) for line in payload.splitlines()
              if line.startswith(b"data: ") and line != b"data: [DONE]"]
    terminal = [event["response"] for event in events if event.get("type") == "response.completed"]
    require(len(terminal) == 1, "expected one completed fixture response")
    result = terminal[0]
    require(result.get("model") == fixture.model and result.get("status") == "completed",
            "unexpected response model or status")
    text = [part.get("text") for item in result["output"] for part in item.get("content", [])]
    require(fixture.partial_marker + "\n" + fixture.final_marker in text, "fixture response text changed")
    usage = result["usage"]
    require(usage["input_tokens"] == 11 and usage["output_tokens"] == 7, "fixture usage changed")
    report = fixture.report()
    require(report["transport_counts"] == {"http_sse": 1} and report["accepted_response_count"] == 1
            and report["terminal_sent"] and not report["error_counts"], "unexpected fixture request count or result")


def run_engine(state, payload, fixture, *, test_foreign_peer):
    endpoint = state / "s"
    require(not os.path.lexists(endpoint), "previous engine socket still exists")
    env = {
        "PATH": "/usr/bin:/bin",
        "HOME": str(state / "home"),
        "TMPDIR": str(state / "temp"),
        "XDG_CACHE_HOME": str(state / "cache"),
        "TIKTOKEN_CACHE_DIR": str(payload / "tiktoken-cache"),
        "HEADROOM_CONFIG_DIR": str(state / "config"),
        "HEADROOM_WORKSPACE_DIR": str(state / "workspace"),
        "HEADROOM_MODE": "cache",
        "HEADROOM_SAVINGS_PROFILE": "coding",
        "HEADROOM_CCR_TTL_SECONDS": "7200",
        "HEADROOM_TELEMETRY": "on",
        "HEADROOM_BEACON": "off",
        "HEADROOM_LOG_MESSAGES": "off",
        "HEADROOM_UPDATE_CHECK": "off",
        "HEADROOM_SKIP_UPSTREAM_CHECK": "1",
        "HEADROOM_MODEL_ROUTER_ENABLED": "0",
        "HEADROOM_OUTPUT_SHAPER": "0",
        "LITELLM_LOCAL_MODEL_COST_MAP": "True",
        "HF_HUB_OFFLINE": "1",
        "TRANSFORMERS_OFFLINE": "1",
        "OPENAI_TARGET_API_URL": fixture.origin,
    }
    process = subprocess.Popen(
        [sys.executable, "-I", "-B", "-c", BOOTSTRAP, str(payload / "site-packages"),
         "proxy", "--uds", str(endpoint), "--workers", "1", "--no-learn",
         "--no-memory-tools", "--no-memory-context"],
        cwd=state, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    try:
        wait_ready(process, endpoint)
        stats = owned_get(endpoint, process.pid, "/stats")
        telemetry = stats.get("telemetry")
        require(isinstance(telemetry, dict) and telemetry.get("enabled") is True
                and stats.get("anon_telemetry_shipping") is False, "unexpected telemetry settings")
        if test_foreign_peer:
            check_foreign_peer(state, process.pid)
        check_response(endpoint, process.pid, fixture)
        process.send_signal(signal.SIGINT)
        require(process.wait(timeout=20) == 0 and not os.path.lexists(endpoint), "engine shutdown was incomplete")
    finally:
        if process.poll() is None:
            process.kill()
            process.wait(timeout=5)


def require_isolated_linux():
    require(sys.platform == "linux" and platform.machine() == "x86_64"
            and sys.version_info[:3] == (3, 13, 13), "expected Linux x64 and CPython 3.13.13")
    require(os.geteuid() != 0 and {name for _, name in socket.if_nameindex()} == {"lo"},
            "expected an unprivileged process in a loopback-only network namespace")


def local_responses_fixture():
    module_spec = spec_from_file_location("local_responses_fixture", Path(__file__).with_name("macos-desktop-fixture.py"))
    module = module_from_spec(module_spec)
    module_spec.loader.exec_module(module)
    return module.DesktopResponsesFixture(
        "sk-local-fixture-" + uuid.uuid4().hex, "gpt-5.2-codex", "medium", uuid.uuid4().hex,
    )


def probe(root):
    require_isolated_linux()
    payload = root / "relocated"
    site.addsitedir(str(payload / "site-packages"))
    import onnxruntime

    require(version("headroom-ai") == ENGINE_VERSION and onnxruntime.__version__ == "1.31.0",
            "installed engine or ONNX runtime pin mismatch")
    with tempfile.TemporaryDirectory(prefix="run-", dir=root) as directory:
        state = Path(directory)
        require(state.stat().st_mode & 0o777 == 0o700, "expected private test state")
        for name in ("home", "temp", "config", "workspace", "cache"):
            (state / name).mkdir(mode=0o700)
        for index in range(2):
            fixture = local_responses_fixture()
            try:
                run_engine(state, payload, fixture, test_foreign_peer=index == 0)
            finally:
                require(fixture.close(), "fixture shutdown was incomplete")
    print("Linux engine proof: Headroom 0.44.0, CPython 3.13.13, two fixture streams, owned peer, foreign peer rejected, clean restart and stop.")


def probe_managed(root, test_binary):
    require_isolated_linux()
    required_tests = (
        "runtime::manager::macos_tests::managed_app_engine_lifecycle",
        "integrations::config_io::linux_config_tests::managed_engine_routes_an_isolated_connection_and_undoes_all_client_configs",
    )
    test_command = [
        str(test_binary), *required_tests,
        "runtime::ingress::tests::", "runtime::ingress_relay::tests::", "runtime::unix_backend::tests::",
        "state::store_tests::", "--include-ignored", "--test-threads=1",
    ]
    # Child diagnostics can interrupt an individual test's status line.
    listed = subprocess.check_output([*test_command, "--list"], text=True, timeout=10).splitlines()
    require(all(f"{name}: test" in listed for name in required_tests), "required managed tests not selected")
    fixture = local_responses_fixture()
    try:
        fixture.release_terminal()
        result = subprocess.run(
            test_command,
            env={
                "PATH": "/usr/bin:/bin",
                "XDG_RUNTIME_DIR": str(root / "socket"),
                "HEADROOM_LINUX_ENGINE_PROOF_ROOT": str(root),
                "HEADROOM_LINUX_PROOF_ROOT": str(root),
                "HEADROOM_LINUX_RUNTIME_ROOT": str(root / "runtime"),
                "HEADROOM_LINUX_FIXTURE_ORIGIN": fixture.origin,
                "HEADROOM_LINUX_FIXTURE_KEY": fixture.api_key,
                "HEADROOM_LINUX_FIXTURE_NONCE": fixture.nonce,
            },
            timeout=240,
        )
        require(result.returncode == 0, "managed engine or connection test failed")
        report = fixture.report()
        require(report["transport_counts"] == {"http_sse": 1}
                and report["accepted_response_count"] == 1 and report["terminal_sent"]
                and not report["error_counts"], "unexpected managed fixture request or result")
    finally:
        require(fixture.close(), "fixture shutdown was incomplete")
    print("Linux engine proof: Managed engine, local Responses stream, and three config Apply and Undo checks passed.")


if __name__ == "__main__":
    try:
        if len(sys.argv) == 2:
            probe(Path(sys.argv[1]).resolve(strict=True))
        elif len(sys.argv) == 4 and sys.argv[1] == "--managed":
            probe_managed(Path(sys.argv[2]).resolve(strict=True), Path(sys.argv[3]).resolve(strict=True))
        else:
            sys.exit("Usage: probe-linux-engine.py [--managed] <private-proof-root> [<test-binary>]")
    except ProbeError as error:
        sys.exit(f"Linux engine proof failed: {error}")
    except (OSError, ImportError, ValueError, KeyError, subprocess.SubprocessError) as error:
        sys.exit(f"Linux engine proof failed: {type(error).__name__}")
