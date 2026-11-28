import ctypes
from ctypes import wintypes, windll
from hashlib import sha256
from http.client import HTTPConnection, HTTPException
import json
from pathlib import Path
from queue import Queue
import re
import runpy
import subprocess
import sys
from threading import Event, Thread
from time import monotonic
from uuid import uuid4


STARTUP_MARKERS = {
    b"registered compress-turn extension": "turn_extension_registered",
    b"event=rust_core_loaded": "rust_core_loaded",
    b"headroom proxy started": "proxy_startup_entered",
    b"eager preload exceeded 0s": "zero_preload_budget",
    b"anonymous upload beacon: disabled": "lifespan_near_end",
}


class ProcessEntry(ctypes.Structure):
    _fields_ = [
        ("size", wintypes.DWORD), ("usage", wintypes.DWORD), ("pid", wintypes.DWORD),
        ("heap", ctypes.c_size_t), ("module", wintypes.DWORD), ("threads", wintypes.DWORD),
        ("parent", wintypes.DWORD), ("priority", wintypes.LONG), ("flags", wintypes.DWORD),
        ("exe", wintypes.WCHAR * 260),
    ]


def python_parents(kernel):
    kernel.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL
    first, next_entry = kernel.Process32FirstW, kernel.Process32NextW
    for function in (first, next_entry):
        function.argtypes = [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)]
        function.restype = wintypes.BOOL
    snapshot = kernel.CreateToolhelp32Snapshot(2, 0)
    if snapshot == ctypes.c_void_p(-1).value:
        raise OSError("process_snapshot_failed")
    try:
        entry = ProcessEntry(size=ctypes.sizeof(ProcessEntry))
        if not first(snapshot, ctypes.byref(entry)):
            raise OSError("process_snapshot_empty")
        parents = {}
        while True:
            if entry.exe.casefold() == "python.exe":
                parents[entry.pid] = entry.parent
            if not next_entry(snapshot, ctypes.byref(entry)):
                return parents
    finally:
        kernel.CloseHandle(snapshot)


def readiness(port):
    connection = HTTPConnection("127.0.0.1", port, timeout=2)
    try:
        connection.request("GET", "/readyz")
        response = connection.getresponse()
        data = json.loads(response.read(65536))
        assert response.status in (200, 503) and data["service"] == "headroom-proxy"
        assert data["version"] == "0.45.0" and type(data["ready"]) is bool
        checks = data["checks"]
        return {
            "http": response.status, "ready": data["ready"],
            "upstream_check_enabled": checks["upstream"]["enabled"],
            "upstream_ok": checks["upstream"]["ready"],
            "local_startup_ok": checks["startup"]["ready"],
            "rust_core_loaded": data["rust_core"] == "loaded",
        }
    finally:
        connection.close()


def observe_startup(controller, workspace, runtime, timer, stop, timeline):
    kernel, _, tcp_table = runtime["windows_api"]()
    elapsed = lambda: round(monotonic() - timer, 3)
    marks = timeline["markers"]
    while not stop.is_set():
        try:
            parents = python_parents(kernel)
            helpers = {p for p, parent in parents.items() if parent == controller}
            engines = {p for p, parent in parents.items() if parent in helpers}
            if helpers:
                timeline.setdefault("helper_spawn_observed_seconds", elapsed())
            if engines:
                timeline.setdefault("engine_spawn_observed_seconds", elapsed())
            for path in (workspace / "logs").glob("proxy-*.log"):
                match = re.fullmatch(r"proxy-(\d{1,5})\.log", path.name)
                if not match:
                    continue
                with path.open("rb") as log:
                    data = log.read(65536).lower()
                for marker, name in STARTUP_MARKERS.items():
                    if marker in data:
                        marks.setdefault(name, elapsed())
                port = int(match[1])
                if not 1 <= port <= 65535:
                    continue
                owners = runtime["listener_pids"](port, tcp_table)
                if set(owners) != {"127.0.0.1"} or owners["127.0.0.1"] not in engines:
                    continue
                timeline.setdefault("owned_port_observed_seconds", elapsed())
                probe = readiness(port)
                if runtime["listener_pids"](port, tcp_table) != owners:
                    continue
                probe["observed_seconds"] = elapsed()
                timeline.setdefault("first_owned_readiness", probe)
                if probe["ready"]:
                    timeline["first_ready"] = probe
                    return
        except (AssertionError, KeyError, ValueError, OSError, HTTPException) as exc:
            timeline.setdefault("sample_error", type(exc).__name__)
        stop.wait(0.4)


def verify(bundle, receipt, result, settings=None):
    assert not windll.shell32.IsUserAnAdmin(), "elevated_test_process"
    state = receipt.parent / f"pp-clean-controller-{uuid4().hex[:8]}"
    workspace, config = state / "workspace", state / "config"
    workspace.mkdir(parents=True)
    config.mkdir()
    if settings is not None:
        (workspace / "settings.json").write_text(json.dumps(settings), encoding="utf-8")
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
    stop_observer = Event()
    timeline = {"markers": {}}
    observer = Thread(target=observe_startup, args=(child.pid, workspace, runtime, timer, stop_observer, timeline), daemon=True)
    observer.start()
    try:
        ready = request("start", 100)
        result["controller_reply_seconds"] = round(monotonic() - timer, 3)
        if ready["event"] != "ready":
            code = ready.get("error_code")
            if isinstance(code, str) and len(code) <= 40 and code.replace("_", "").isascii() and code.replace("_", "").isalnum():
                result["controller_error"] = code
            raise AssertionError("controller_did_not_reach_ready")
        result["ready_seconds"] = result["controller_reply_seconds"]
        assert ready["engine_version"] == "0.45.0", "unexpected_engine_version"
        assert ready["manager_hidden_console"] and ready["engine_console_allocated"] and ready["controller_consoleless"], "unexpected_console_mode"
        port = ready["listener_port"]
        assert runtime["listener_pids"](port, tcp_table) == {"127.0.0.1": ready["engine_pid"]}, "unexpected_engine_listener"
        probe = readiness(port)
        assert probe["http"] == 200 and probe["ready"], "readiness_check_failed"
        assert probe["upstream_check_enabled"] is False, "provider_check_in_local_readiness"
        assert probe["local_startup_ok"] and probe["rust_core_loaded"], "native_startup_unverified"
        result["readiness"] = probe
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
        stop_observer.set()
        observer.join(timeout=3)
        result["startup_timeline"] = timeline
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
    settings = json.loads(Path(sys.argv[3]).read_bytes()) if len(sys.argv) == 4 else None
    verify(bundle, receipt, result, settings)
except Exception as exc:
    result.update(status="fail", error_type=type(exc).__name__)
    code = str(exc)
    if isinstance(exc, AssertionError) and 0 < len(code) <= 70 and code.replace("_", "").isalpha() and code.isascii():
        result["error_code"] = code
receipt.with_suffix(".tmp").write_text(json.dumps(result), encoding="utf-8")
receipt.with_suffix(".tmp").rename(receipt)
raise SystemExit(0 if result["status"] == "pass" else 1)
