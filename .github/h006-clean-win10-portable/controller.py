from ctypes import windll
from hashlib import sha256
from http.client import HTTPConnection
import json
from pathlib import Path
from queue import Queue
import runpy
import subprocess
import sys
from threading import Thread
from time import monotonic
from uuid import uuid4


def verify(bundle, receipt, result):
    assert not windll.shell32.IsUserAnAdmin(), "elevated_test_process"
    state = receipt.parent / f"pp-clean-controller-{uuid4().hex[:8]}"
    workspace, config = state / "workspace", state / "config"
    workspace.mkdir(parents=True)
    config.mkdir()
    script = bundle / "desktop_controller.py"
    control = runpy.run_path(str(script))
    payload = bundle / "payload"
    manifest = bundle / "manifest.json"
    metadata, observed, _ = control["load_manifest"](manifest)
    assert observed == payload and metadata["engine"]["version"] == "0.45.0", "unexpected_manifest"
    runtime = runpy.run_path(str(payload / metadata["engine"]["runner"]))
    tcp_table = runtime["windows_api"]()[2]
    files = [script, bundle / "desktop_helper.py", manifest, payload / "python.exe", bundle.parent / "package-inputs.json"]

    def fingerprints():
        return {p.name: sha256(p.read_bytes()).hexdigest() for p in files}

    initial = fingerprints()
    timer = monotonic()
    result.update(phase="start", normal_user=True)
    child = subprocess.Popen(
        [str(payload / "python.exe"), "-I", "-B", "-u", str(script), "--manifest", str(manifest),
         "--state-root", str(state), "--workspace", str(workspace), "--config", str(config)],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, cwd=payload,
        env=control["_safe_child_environment"](state), creationflags=subprocess.CREATE_NO_WINDOW,
    )
    frames = Queue()

    def read_frames():
        for line in iter(lambda: child.stdout.readline(8193), b""):
            frames.put(line)
        frames.put(None)

    def request(event, seconds, generation=None):
        req = uuid4().hex
        frame = {"event": event, "protocol_version": 1, "request_id": req}
        if generation is not None:
            frame["runtime_generation"] = generation
        child.stdin.write(json.dumps(frame).encode("ascii") + b"\n")
        child.stdin.flush()
        raw = frames.get(timeout=seconds)
        assert raw is not None, "controller_exited_without_reply"
        return control["decode_frame"](raw, expected_generation=generation, expected_request_id=req)

    Thread(target=read_frames, daemon=True).start()
    try:
        ready = request("start", 100)
        if ready["event"] != "ready":
            code = ready.get("error_code")
            if isinstance(code, str) and len(code) <= 40 and code.replace("_", "").isascii() and code.replace("_", "").isalnum():
                result["controller_error"] = code
            raise AssertionError("controller_did_not_reach_ready")
        result["ready_seconds"] = round(monotonic() - timer, 3)
        assert ready["engine_version"] == "0.45.0", "unexpected_engine_version"
        assert ready["manager_hidden_console"] and ready["engine_console_allocated"] and ready["controller_consoleless"], "unexpected_console_mode"
        port = ready["listener_port"]
        assert runtime["listener_pids"](port, tcp_table) == {"127.0.0.1": ready["engine_pid"]}, "unexpected_engine_listener"
        conn = HTTPConnection("127.0.0.1", port, timeout=3)
        try:
            conn.request("GET", "/readyz")
            answer = conn.getresponse()
            assert answer.status == 200 and json.loads(answer.read(65536))["ready"] is True, "readiness_check_failed"
        finally:
            conn.close()
        result.update(phase="stop", local_readiness_http=200)
        stopped = request("stop", 50, ready["runtime_generation"])
        assert stopped["event"] == "stopped" and stopped["owned_port_closed"], "shutdown_receipt_failed"
        assert stopped["shutdown"] == "graceful" and stopped["force_fallback"] is False, "engine_shutdown_not_graceful"
        assert child.wait(timeout=10) == 0, "controller_exit_failed"
        assert not runtime["listener_pids"](port, tcp_table), "engine_listener_remained"
        assert initial == fingerprints(), "package_inputs_changed"
        result.update(status="pass", phase="complete", engine=ready["engine_version"],
                      total_seconds=round(monotonic() - timer, 3), shutdown=stopped["shutdown"],
                      loopback_closed=True, model_requests=0, source_sha256_before_and_after=initial)
    finally:
        child.stdin.close()
        if child.poll() is None:
            try:
                child.wait(timeout=15)
            except subprocess.TimeoutExpired:
                child.terminate()
                child.wait(timeout=10)


bundle = Path(sys.argv[1]).resolve(strict=True)
receipt = Path(sys.argv[2]).resolve()
assert receipt.parent.is_dir() and not receipt.exists()
assert Path(sys.executable).resolve(strict=True) == bundle / "payload" / "python.exe"
result = {"status": "fail", "phase": "preflight"}
try:
    verify(bundle, receipt, result)
except Exception as exc:
    result.update(status="fail", error_type=type(exc).__name__)
    code = str(exc)
    if isinstance(exc, AssertionError) and 0 < len(code) <= 70 and code.replace("_", "").isalpha() and code.isascii():
        result["error_code"] = code
receipt.with_suffix(".tmp").write_text(json.dumps(result), encoding="utf-8")
receipt.with_suffix(".tmp").rename(receipt)
raise SystemExit(0 if result["status"] == "pass" else 1)
