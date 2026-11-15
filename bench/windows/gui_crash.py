"""H006-c: kill one real packaged GUI owner, then reopen its isolated profile.

Run --prepare first, then --candidate <printed project-relative candidate>.
The observer keeps its cleanup Job open until all measured processes have exited.
"""
import ctypes as c
from ctypes import wintypes as w
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
import os
from pathlib import Path
import shutil
import socket
import struct
import subprocess
import sys
import threading
import time
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[2]
AREA = ROOT / "work/proofs/windows-gui-crash"
STAGE = ROOT / "work/build/windows/h010-retrieval-resources"
GATE = ROOT / "work/proofs/opencode-v2-binding/checks-9afd1583f8/final.json"
UI = Path(__file__).with_name("gui_crash_ui.mjs")
CRASH_SECONDS = 5
assert Path.cwd().resolve() == ROOT and str(ROOT) == r"D:\Projeler\Headroom" and os.name == "nt"


def sha(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def save(path, value):
    with path.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


gate = read(GATE)
package = read(STAGE / "package-inputs.json")
qa = module("gui_crash_quality", ROOT / "work/proofs/codex-current/quality.py")
node = Path(shutil.which("node")).resolve()
payload = {name: digest for name, digest in package["resources"].items() if name.startswith("runtime/")}
assert gate["inputs_unchanged"] and gate["before"] == gate["after"]
assert len(gate["commands"]) == 5 and all(row["exit_code"] == 0 for row in gate["commands"])
assert len(payload) > 22000


def environment():
    tools = {}
    for command in (["rustc", "--version"], ["cargo", "--version"], ["node", "--version"], ["pnpm.cmd", "--version"]):
        result = subprocess.run(command, cwd=ROOT, env=qa.ENV, capture_output=True, text=True, timeout=15)
        assert result.returncode == 0
        tools[command[0]] = result.stdout.strip()
    value = {"workspace": str(ROOT), "python": sys.version, "windows_build": list(sys.getwindowsversion()[:3])}
    assert tools == gate["toolchain"] and value == gate["environment"], "quality_environment_changed"
    return {**value, "tools": tools, "python_sha256": sha(Path(sys.executable)), "node_sha256": sha(node)}


def inputs(extra=()):
    paths = [ROOT / name for name in gate["after"]]
    paths += [STAGE / name for name in payload]
    paths += [ROOT / name for name in package["inputs"]]
    paths += [p for p in (ROOT / "dist").rglob("*") if p.is_file()]
    paths += [Path(__file__), UI, GATE, STAGE / "package-inputs.json", *extra]
    return {p.relative_to(ROOT).as_posix(): sha(p) for p in sorted(set(paths))}


def verify_inputs(value):
    assert all(value[name] == digest for name, digest in gate["after"].items()), "current_product_gate_invalidated"
    assert all(value[(ROOT / name).relative_to(ROOT).as_posix()] == digest
               for name, digest in package["inputs"].items()), "stage_inputs_changed"
    assert all(value[(STAGE / name).relative_to(ROOT).as_posix()] == digest
               for name, digest in payload.items()), "stage_payload_changed"


if sys.argv[1:] == ["--prepare"]:
    candidate = AREA / ("candidate-" + uuid.uuid4().hex[:10])
    candidate.mkdir(parents=True)
    identifier = "dev.headroom.desktop.gui-crash-" + candidate.name.removeprefix("candidate-")
    overlay = {"identifier": identifier, "app": {"windows": [{"label": "main", "title": "Headroom H006 GUI crash",
               "width": 1280, "height": 800, "minWidth": 760, "minHeight": 620, "center": True}]}}
    overlay_path = candidate / "tauri.json"
    save(overlay_path, overlay)
    driver_path = candidate / "prepare-driver.py"
    with driver_path.open("xb") as stream:
        stream.write(Path(__file__).read_bytes())
    before, env_before = inputs([overlay_path, driver_path]), environment()
    verify_inputs(before)
    command = ["cargo", "build", "--release", "--locked", "--offline", "--manifest-path", "src-tauri/Cargo.toml",
               "--features", "custom-protocol"]
    env = {**qa.ENV, "RUSTUP_TOOLCHAIN": "stable-x86_64-pc-windows-msvc", "TAURI_CONFIG": json.dumps(overlay)}
    print(json.dumps({"phase": "compiling", "candidate": candidate.relative_to(ROOT).as_posix()}), flush=True)
    build = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, timeout=600)
    after, env_after = inputs([overlay_path, driver_path]), environment()
    verify_inputs(after)
    proof = {"command": command, "identifier": identifier, "before": before, "after": after,
             "environment_before": env_before, "environment_after": env_after, "exit_code": build.returncode,
             "inputs_unchanged": before == after, "passed": False, "full_release_profile": True}
    if build.returncode == 0 and before == after and env_before == env_after:
        app = candidate / "app"
        app.mkdir()
        exe = app / "headroom-desktop.exe"
        shutil.copyfile(Path(env["CARGO_TARGET_DIR"]) / "release/headroom-desktop.exe", exe)
        shutil.copytree(STAGE / "runtime", app / "runtime", copy_function=os.link)
        assert all(os.path.samefile(STAGE / name, app / name) for name in payload)
        proof.update(passed=True, exe_sha256=sha(exe), packaged_files=len(payload))
    save(candidate / "build.json", proof)
    print(json.dumps({"candidate": candidate.relative_to(ROOT).as_posix(), "passed": proof["passed"],
                      "exit_code": build.returncode, "inputs": len(before)}), flush=True)
    raise SystemExit(0 if proof["passed"] else 1)

assert len(sys.argv) == 3 and sys.argv[1] == "--candidate"
candidate = (ROOT / sys.argv[2]).resolve()
assert candidate.parent == AREA and candidate.name.startswith("candidate-")
build = read(candidate / "build.json")
BUILD_DRIVER = candidate / "prepare-driver.py"
assert build["passed"] and build["inputs_unchanged"] and build["before"] == build["after"]
exe = candidate / "app/headroom-desktop.exe"
runtime = module("gui_crash_runtime", candidate / "app/runtime/payload/headroom_runtime.py")
run = AREA / ("native-" + uuid.uuid4().hex[:10])
run.mkdir()
state = Path(os.environ["LOCALAPPDATA"]).resolve() / build["identifier"]
assert state.name.startswith("dev.headroom.desktop.gui-crash-") and not state.exists(), "fixture_state_already_exists"
snapshot = lambda: inputs([candidate / "build.json", candidate / "tauri.json", exe, BUILD_DRIVER])
print(json.dumps({"phase": "native_input_fingerprint", "run": run.relative_to(ROOT).as_posix()}), flush=True)
before, env_before = snapshot(), environment()
verify_inputs(before)
assert before[exe.relative_to(ROOT).as_posix()] == build["exe_sha256"]
for name, digest in build["after"].items():
    # The prepared executable is unchanged when only its observer's I/O is optimized.
    actual = before[BUILD_DRIVER.relative_to(ROOT).as_posix()] if name == Path(__file__).relative_to(ROOT).as_posix() else before[name]
    assert actual == digest, "candidate_inputs_changed"

k = c.WinDLL("kernel32", use_last_error=True)
iphlp = c.WinDLL("iphlpapi")
for name, args, result in (
    ("OpenProcess", [w.DWORD, w.BOOL, w.DWORD], w.HANDLE),
    ("CloseHandle", [w.HANDLE], w.BOOL),
    ("WaitForSingleObject", [w.HANDLE, w.DWORD], w.DWORD),
    ("IsProcessInJob", [w.HANDLE, w.HANDLE, c.POINTER(w.BOOL)], w.BOOL),
    ("QueryInformationJobObject", [w.HANDLE, c.c_int, c.c_void_p, w.DWORD, c.c_void_p], w.BOOL),
    ("QueryFullProcessImageNameW", [w.HANDLE, w.DWORD, w.LPWSTR, c.POINTER(w.DWORD)], w.BOOL),
    ("GetCurrentProcess", [], w.HANDLE),
):
    function = getattr(k, name)
    function.argtypes, function.restype = args, result


class JobPids(c.Structure):
    _fields_ = [("assigned", w.DWORD), ("count", w.DWORD), ("pids", c.c_size_t * 256)]


def job_pids(job):
    value = JobPids()
    assert k.QueryInformationJobObject(job._handle, 3, c.byref(value), c.sizeof(value), None), "job_query_failed"
    assert value.count <= 256
    return list(value.pids[:value.count])


def contains(job, handle):
    value = w.BOOL()
    assert k.IsProcessInJob(handle, job._handle, c.byref(value)), "job_membership_query_failed"
    return bool(value.value)


def image(handle):
    buffer, size = c.create_unicode_buffer(32768), w.DWORD(32768)
    assert k.QueryFullProcessImageNameW(handle, 0, buffer, c.byref(size)), "process_image_query_failed"
    return Path(buffer.value).resolve()


def listeners(pids):
    size = w.ULONG()
    code = iphlp.GetExtendedTcpTable(None, c.byref(size), False, runtime.AF_INET, runtime.TCP_TABLE_OWNER_PID_LISTENER, 0)
    assert code in (0, 122) and size.value > 0, "listener_query_failed"
    buffer = c.create_string_buffer(size.value)
    assert iphlp.GetExtendedTcpTable(buffer, c.byref(size), False, runtime.AF_INET, runtime.TCP_TABLE_OWNER_PID_LISTENER, 0) == 0
    rows = []
    for index in range(struct.unpack_from("=I", buffer.raw)[0]):
        status, address, port, _, _, pid = struct.unpack_from("=IIIIII", buffer.raw, 4 + 24 * index)
        if status == 2 and pid in pids:
            rows.append((socket.inet_ntoa(struct.pack("=I", address)), socket.ntohs(port & 65535), pid))
    return rows


def wait_until(check, seconds, reason):
    deadline = time.monotonic() + seconds
    while not check():
        assert time.monotonic() < deadline, reason
        time.sleep(.02)


class RejectProvider(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_POST(self):
        self.server.model_calls += 1
        self.send_error(503)

    def do_GET(self):
        self.send_error(404)


provider = ThreadingHTTPServer(("127.0.0.1", 0), RejectProvider)
provider.daemon_threads, provider.model_calls = True, 0
threading.Thread(target=provider.serve_forever, daemon=True).start()
foreign = socket.socket()
foreign.bind(("127.0.0.1", 0))
foreign.listen(4)
foreign.settimeout(1)
settings = state / "runtime/workspace/settings.json"
settings.parent.mkdir(parents=True)
settings_bytes = json.dumps({"openai_base_url": f"http://127.0.0.1:{provider.server_port}/v1",
                             "anthropic_base_url": f"http://127.0.0.1:{provider.server_port}"}).encode()
settings.write_bytes(settings_bytes)
job, process, handles = None, None, []
proof = {"task": "H006-c", "passed": False, "cases": [], "ui": [], "failure": None,
         "crash_timeout_seconds": CRASH_SECONDS, "runtime_state_root": str(state), "real_provider_call": False,
         "build_driver_snapshot": {"path": BUILD_DRIVER.relative_to(ROOT).as_posix(), "sha256": sha(BUILD_DRIVER)},
         "observer_change_since_build": "deduplicate fingerprint reads and use pinned listener-table constants; same product, payload, UI and build command"
             if before[Path(__file__).relative_to(ROOT).as_posix()] != build["after"][Path(__file__).relative_to(ROOT).as_posix()] else "none"}
stage = "launch"
endpoint_identity = None


def witness():
    data = b"H006-foreign-ok"
    with socket.create_connection(foreign.getsockname(), timeout=1) as sender:
        receiver, _ = foreign.accept()
        with receiver:
            sender.sendall(data)
            assert receiver.recv(len(data)) == data, "foreign_listener_changed"


def ui(phase, port, env):
    owners = runtime.listener_pids(port, iphlp)
    assert list(owners) == ["127.0.0.1"] and owners["127.0.0.1"] in job_pids(job), "unowned_debug_listener"
    result = subprocess.run([str(node), str(UI), str(port), phase], cwd=ROOT, env=env, capture_output=True,
                            text=True, encoding="utf-8", timeout=165, creationflags=subprocess.CREATE_NO_WINDOW)
    try:
        value = json.loads(result.stdout)
    except ValueError:
        raise AssertionError("ui_reply_invalid") from None
    proof["ui"].append(value)
    assert result.returncode == 0 and value["passed"], "ui_action_failed"
    return value


try:
    for phase in ("crash", "reopen"):
        stage = phase + "_launch"
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            port = reservation.getsockname()[1]
        profile = run / phase
        profile.mkdir()
        env = {**os.environ, "PATH": str(Path(os.environ["SystemRoot"]) / "System32"),
               "CODEX_HOME": str(profile / "codex"), "CLAUDE_CONFIG_DIR": str(profile / "claude"),
               "XDG_CONFIG_HOME": str(profile / "config"), "WEBVIEW2_USER_DATA_FOLDER": str(profile / "webview"),
               "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS": f"--remote-debugging-address=127.0.0.1 --remote-debugging-port={port}"}
        job = runtime.KillOnCloseJob()
        assert not contains(job, k.GetCurrentProcess()), "observer_inside_fixture_job"
        process = subprocess.Popen([str(exe)], cwd=profile, env=env, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                   creationflags=subprocess.CREATE_NO_WINDOW)
        assert job._kernel32.AssignProcessToJobObject(job._handle, w.HANDLE(int(process._handle))), "gui_job_assignment_failed"
        wait_until(lambda: bool(runtime.listener_pids(port, iphlp)) or process.poll() is not None, 30, "webview_start_timeout")
        assert process.poll() is None, "gui_exited_before_start"
        stage = phase + "_ui_start"
        value = ui("start", port, env)
        current_endpoint = read(state / "runtime/connection-endpoint.json")
        endpoint = (current_endpoint["endpoint_id"], current_endpoint["port"])
        if endpoint_identity is None:
            endpoint_identity = endpoint
        assert endpoint == endpoint_identity, "stable_endpoint_changed_after_crash"
        assert runtime.listener_pids(endpoint[1], iphlp) == {"127.0.0.1": process.pid}, "unowned_stable_listener"
        python = (candidate / "app/runtime/payload/python.exe").resolve()
        for pid in job_pids(job):
            handle = k.OpenProcess(0x100000 | 0x1000, False, pid)
            assert handle, "owned_process_handle_unavailable"
            if contains(job, handle) and image(handle) == python:
                handles.append((pid, handle))
            else:
                k.CloseHandle(handle)
        assert len(handles) == 3 and all(k.WaitForSingleObject(handle, 0) == 258 for _, handle in handles), "engine_chain_not_live"
        ports = listeners({pid for pid, _ in handles})
        assert len(ports) == 1 and ports[0][0] == "127.0.0.1", "engine_listener_invalid"
        engine_port = ports[0][1]
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        with opener.open(f"http://127.0.0.1:{engine_port}/readyz", timeout=2) as response:
            ready = json.loads(response.read(4096))
        assert ready.get("version") == "0.41.0", "engine_version_changed"
        witness()
        stage = phase + "_observe"
        row = {"phase": phase, "gui_pid": process.pid, "engine_process_ids": [pid for pid, _ in handles],
               "engine_port": engine_port, "stable_port": endpoint[1], "initial_status": value["initial_status"],
               "runtime_generation": value["generation"], "engine_readiness": True}
        if phase == "crash":
            start = time.monotonic()
            process.kill()  # TerminateProcess uses this exact Popen child handle.
            assert process.wait(timeout=CRASH_SECONDS) != 0, "crash_exit_was_normal"
            wait_until(lambda: all(k.WaitForSingleObject(handle, 0) == 0 for _, handle in handles),
                       max(.001, CRASH_SECONDS - (time.monotonic() - start)), "engine_survived_gui_crash")
            row["engine_processes_closed_ms"] = round((time.monotonic() - start) * 1000, 3)
            wait_until(lambda: not runtime.listener_pids(engine_port, iphlp) and not runtime.listener_pids(endpoint[1], iphlp),
                       max(.001, CRASH_SECONDS - (time.monotonic() - start)), "ports_survived_gui_crash")
            row["ports_closed_ms"] = round((time.monotonic() - start) * 1000, 3)
            assert row["ports_closed_ms"] <= CRASH_SECONDS * 1000
        else:
            ui("stop-exit", port, env)
            assert process.wait(timeout=45) == 0, "normal_exit_failed"
            assert all(k.WaitForSingleObject(handle, 0) == 0 for _, handle in handles), "engine_survived_normal_exit"
            assert not runtime.listener_pids(engine_port, iphlp) and not runtime.listener_pids(endpoint[1], iphlp)
        witness()
        wait_until(lambda: not job_pids(job), 10, "owned_gui_job_not_empty")
        assert not runtime.listener_pids(port, iphlp), "webview_listener_survived"
        assert settings.read_bytes() == settings_bytes, "fixture_settings_changed"
        row.update(gui_exit_code=process.returncode, foreign_listener_preserved=True,
                   observer_job_open_during_measurement=True, owned_processes_after_exit=0,
                   owned_ports_closed=True, forced_cleanup_for_success=False)
        proof["cases"].append(row)
        for _, handle in handles:
            k.CloseHandle(handle)
        handles.clear()
        job.close()  # The empty cleanup Job cannot cause the observed success.
        job = None
        process = None
        print(json.dumps({"phase": phase, "passed": True}), flush=True)
    assert len(proof["cases"]) == 2 and proof["cases"][0]["runtime_generation"] != proof["cases"][1]["runtime_generation"]
    assert provider.model_calls == 0
    proof["passed"] = True
except Exception as error:
    proof["failure"] = {"stage": stage, "kind": type(error).__name__,
                        "reason": str(error) if isinstance(error, AssertionError) else "native_fixture_error"}
finally:
    proof["cleanup_job_needed"] = job is not None
    if job:
        job.close()
    if process and process.poll() is None:
        process.wait(timeout=10)
    for _, handle in handles:
        k.CloseHandle(handle)
    foreign.close()
    provider.shutdown()
    provider.server_close()
    print(json.dumps({"phase": "native_final_fingerprint", "cases": len(proof["cases"])}), flush=True)
    after, env_after = snapshot(), environment()
    proof.update(before=before, after=after, inputs_unchanged=before == after,
                 environment_before=env_before, environment_after=env_after,
                 provider_model_calls=provider.model_calls, exe_sha256=sha(exe), build_proof=candidate.relative_to(ROOT).as_posix() + "/build.json")
    proof["passed"] = proof["passed"] and before == after and env_before == env_after and not proof["cleanup_job_needed"]
    save(run / "result.json", proof)
    print(json.dumps({"passed": proof["passed"], "proof": (run / "result.json").relative_to(ROOT).as_posix(),
                      "failure": proof["failure"], "cleanup_job_needed": proof["cleanup_job_needed"]}), flush=True)
raise SystemExit(0 if proof["passed"] else 1)
