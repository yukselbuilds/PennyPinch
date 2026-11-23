"""Exercise the pinned Apple Silicon engine in a disposable private directory."""

from http.client import HTTPConnection
from importlib.metadata import version
import json
import os
from pathlib import Path
import platform
import signal
import site
import socket
import subprocess
import sys
import tempfile
import time


ENGINE_VERSION = "0.45.0"
SOL_LOCAL = 0
LOCAL_PEERPID = 2
MAX_JSON_BYTES = 256 * 1024


class ProbeError(RuntimeError):
    pass


class PeerMismatch(ProbeError):
    pass


def owned_json(path, expected_pid, endpoint):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as transport:
        transport.settimeout(2)
        transport.connect(str(path))
        if transport.getsockopt(SOL_LOCAL, LOCAL_PEERPID) != expected_pid:
            raise PeerMismatch("unexpected Unix socket peer")
        connection = HTTPConnection("localhost", timeout=2)
        connection.sock = transport
        try:
            connection.request("GET", endpoint, headers={"Connection": "close"})
            response = connection.getresponse()
            if response.status != 200:
                return {}
            body = response.read(MAX_JSON_BYTES + 1)
            if len(body) > MAX_JSON_BYTES:
                raise ProbeError("unexpected readiness response size")
            result = json.loads(body)
            if not isinstance(result, dict):
                raise ProbeError("unexpected readiness response type")
            return result
        finally:
            connection.close()


def wait_ready(process, socket_path):
    deadline = time.monotonic() + 80
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise ProbeError(f"engine exited before readiness ({process.returncode})")
        try:
            ready = owned_json(socket_path, process.pid, "/readyz")
        except (FileNotFoundError, ConnectionRefusedError, TimeoutError):
            ready = {}
        if (
            ready.get("service") == "headroom-proxy"
            and ready.get("ready") is True
            and ready.get("version") == ENGINE_VERSION
        ):
            return
        time.sleep(0.1)
    raise ProbeError("owned engine readiness timed out")


def reject_foreign_peer(state, engine_pid):
    path = state / "foreign.sock"
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as listener:
        listener.bind(str(path))
        listener.listen(1)
        listener.settimeout(2)
        try:
            owned_json(path, engine_pid, "/readyz")
        except PeerMismatch:
            connection, _ = listener.accept()
            with connection:
                connection.settimeout(2)
                if connection.recv(1) == b"":
                    return
        raise ProbeError("foreign socket received HTTP or was accepted")


def probe(site_packages, proof_root):
    if sys.platform != "darwin" or platform.machine() != "arm64" or sys.version_info[:3] != (3, 13, 13):
        raise ProbeError("expected Apple Silicon and CPython 3.13.13")
    site.addsitedir(str(site_packages))
    if version("headroom-ai") != ENGINE_VERSION or version("onnxruntime") != "1.31.0":
        raise ProbeError("installed dependency pin mismatch")
    import onnxruntime

    if onnxruntime.__version__ != "1.31.0":
        raise ProbeError("native ONNX runtime did not load the pin")

    with tempfile.TemporaryDirectory(prefix="run-", dir=proof_root) as directory:
        state = Path(directory).resolve(strict=True)
        if state.stat().st_mode & 0o777 != 0o700:
            raise ProbeError("engine test state is not private")
        for name in ("home", "temp", "config", "workspace", "cache"):
            (state / name).mkdir(mode=0o700)
        endpoint = state / "proxy.sock"
        env = {
            "PATH": "/usr/bin:/bin",
            "HOME": str(state / "home"),
            "TMPDIR": str(state / "temp"),
            "XDG_CACHE_HOME": str(state / "cache"),
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
            "HF_HUB_OFFLINE": "1",
            "TRANSFORMERS_OFFLINE": "1",
        }
        bootstrap = (
            "import site, sys\n"
            "site.addsitedir(sys.argv.pop(1))\n"
            "from headroom.cli import main\n"
            "try:\n"
            "    main()\n"
            "except KeyboardInterrupt:\n"
            "    pass\n"
        )
        process = subprocess.Popen(
            [sys.executable, "-I", "-B", "-c", bootstrap, str(site_packages),
             "proxy", "--uds", str(endpoint), "--workers", "1", "--no-memory-tools", "--no-memory-context"],
            cwd=state, env=env, stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        try:
            wait_ready(process, endpoint)
            stats = owned_json(endpoint, process.pid, "/stats")
            telemetry = stats.get("telemetry")
            if (
                not isinstance(telemetry, dict)
                or telemetry.get("enabled") is not True
                or stats.get("anon_telemetry_shipping") is not False
            ):
                raise ProbeError("local telemetry or beacon state did not match")
            reject_foreign_peer(state, process.pid)
            process.send_signal(signal.SIGINT)
            if process.wait(timeout=20) != 0 or endpoint.exists() or endpoint.is_symlink():
                raise ProbeError("engine did not shut down cleanly")
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)
    print("macOS engine proof: arm64 CPython 3.13.13, Headroom 0.45.0, owned readiness, foreign peer rejected, beacon off, clean stop.")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit("usage: probe-macos-engine.py <site-packages> <private-proof-root>")
    try:
        probe(Path(sys.argv[1]).resolve(strict=True), Path(sys.argv[2]).resolve(strict=True))
    except ProbeError as error:
        sys.exit(f"macOS engine proof failed: {error}")
    except (OSError, ImportError, ValueError, subprocess.SubprocessError) as error:
        sys.exit(f"macOS engine proof failed: {type(error).__name__}")
