"""Exercise the real packaged UI and observe only this QA app's owned processes."""

import ctypes
import csv
import hashlib
import io
import json
import os
from pathlib import Path
import plistlib
import signal
import socket
import stat
import subprocess
import sys
import time


ROOT = Path(__file__).resolve().parents[1]


class ProbeError(RuntimeError):
    pass


def require(condition, code):
    if not condition:
        raise ProbeError(code)


def wait_for(observe, code, seconds=20):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        result = observe()
        if result:
            return result
        time.sleep(0.15)
    raise ProbeError(code)


def process_table():
    result = subprocess.run(
        ["/bin/ps", "-axo", "pid=,ppid=,lstart="],
        capture_output=True, text=True, timeout=5, check=True,
    )
    require(len(result.stdout) < 1024 * 1024, "process_table_limit")
    rows = {}
    for line in result.stdout.splitlines():
        fields = line.split(maxsplit=2)
        if len(fields) == 3:
            rows[int(fields[0])] = (int(fields[1]), fields[2])
    return rows


def executable_path(pid):
    libproc = ctypes.CDLL("/usr/lib/libproc.dylib")
    function = libproc.proc_pidpath
    function.argtypes = (ctypes.c_int, ctypes.c_void_p, ctypes.c_uint32)
    function.restype = ctypes.c_int
    buffer = ctypes.create_string_buffer(4096)
    if function(pid, buffer, len(buffer)) <= 0:
        return None
    return Path(os.fsdecode(buffer.value))


def matches_process(owned):
    pid, stamp, executable = owned
    return process_table().get(pid) == stamp and executable_path(pid) == executable


def find_app(executable):
    matching = [(pid, stamp, executable) for pid, stamp in process_table().items()
                if executable_path(pid) == executable]
    require(len(matching) <= 1, "multiple_owned_app_processes")
    return matching[0] if matching else None


def find_engine(app, runtime):
    python = (runtime / "payload/python/bin/python3.13").resolve(strict=True)
    matching = [(pid, stamp, python) for pid, stamp in process_table().items()
                if stamp[0] == app[0] and executable_path(pid) == python]
    require(len(matching) <= 1, "multiple_owned_engines")
    return matching[0] if matching else None


def run_ui(action, *arguments):
    command = ["/usr/bin/osascript", str(ROOT / "scripts/macos-package-input.applescript"), action, *arguments]
    if os.environ.get("CIRCLECI") == "true":
        command = ["/bin/launchctl", "asuser", str(os.getuid()), *command]
    result = subprocess.run(
        command, capture_output=True, text=True, timeout=75,
    )
    for line in result.stderr.splitlines():
        if line.startswith("headroom-package-ui="):
            print(line[:120], flush=True)
    require(result.returncode == 0, f"ui_{action}")


def lock_available(path):
    import fcntl

    info = path.lstat()
    require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1
            and info.st_uid == os.geteuid(), "invalid_owned_lock")
    descriptor = os.open(path, os.O_RDWR | os.O_NOFOLLOW)
    try:
        opened = os.fstat(descriptor)
        require((opened.st_dev, opened.st_ino) == (info.st_dev, info.st_ino), "owned_lock_changed")
        try:
            fcntl.flock(descriptor, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return False
        fcntl.flock(descriptor, fcntl.LOCK_UN)
        return True
    finally:
        os.close(descriptor)


def endpoint_owned(app, state):
    record = state / "runtime/connection-endpoint.json"
    if not record.exists():
        return None
    require(record.stat().st_size < 8192, "endpoint_record_limit")
    value = json.loads(record.read_bytes())
    require(value["schema_version"] == 1 and value["bind_address"] == "127.0.0.1"
            and 1024 <= value["port"] <= 65535, "invalid_loopback_record")
    result = subprocess.run(
        ["/usr/sbin/lsof", "-nP", "-a", "-p", str(app[0]), "-iTCP", "-sTCP:LISTEN", "-Fn"],
        capture_output=True, text=True, timeout=5,
    )
    addresses = [line[1:] for line in result.stdout.splitlines() if line.startswith("n")]
    if result.returncode == 1 and not addresses:
        return None
    require(result.returncode == 0, "loopback_observation_failed")
    print(f"headroom-package-endpoint=observed descriptors={len(addresses)} "
          f"unique={len(set(addresses))}", flush=True)
    require(set(addresses) == {f'127.0.0.1:{value["port"]}'}, "loopback_owner_mismatch")
    require(not lock_available(state / "runtime/.ingress.lock"), "ingress_lock_unheld")
    return value["port"]


def port_released(port):
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
            listener.bind(("127.0.0.1", port))
    except OSError:
        return False
    return True


def bundle_inventory(bundle, runtime):
    result = subprocess.run(
        [sys.executable, "-I", "-B", str(ROOT / "scripts/verify-macos-bundle.py"),
         "--bundle", str(bundle), "--runtime", str(runtime), "--expected-manifest",
         os.environ["HEADROOM_MACOS_RUNTIME_MANIFEST_SHA256"]],
        capture_output=True, text=True, timeout=90, check=True,
    )
    return json.loads(result.stdout)


def exercise_reports(root):
    reports = root / "reports"
    reports.mkdir(mode=0o700)
    run_ui("export_cancel")
    require(not list(reports.iterdir()), "cancel_created_report")
    results = []
    for kind, extension in (("statistics_json", "json"), ("statistics_csv", "csv"), ("diagnostics_json", "json")):
        name = f"ölçüm 项目-{kind}.{extension}"
        run_ui("export_" + kind, str(reports), name)
        target = reports / name
        info = target.lstat()
        require(stat.S_ISREG(info.st_mode) and stat.S_IMODE(info.st_mode) == 0o600
                and info.st_uid == os.geteuid() and info.st_nlink == 1
                and 0 < info.st_size <= 262_144, "report_file_invalid")
        data = target.read_bytes()
        if extension == "json":
            value = json.loads(data)
            require(value["schema_version"] == 1, "report_schema_invalid")
            if kind == "diagnostics_json":
                require(value["kind"] == "diagnostics" and value["application"]["os"] == "macos"
                        and value["application"]["architecture"] == "aarch64", "diagnostic_platform_invalid")
                value = value["statistics"]
            require(value["source"] == "headroom_local_metrics" and value["scope"] == "observed_requests"
                    and value["component"] == "input", "report_scope_invalid")
        else:
            require(data.startswith(b"\xef\xbb\xbf"), "csv_encoding_invalid")
            rows = list(csv.reader(io.StringIO(data.decode("utf-8-sig")), delimiter=";"))
            require(len(rows) == 2 and all(len(row) == 22 for row in rows), "csv_shape_invalid")
        text = data.decode("utf-8-sig")
        require(str(root) not in text and "PRIVATE_" not in text, "report_private_data")
        results.append({"kind": kind, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(), "mode": "0600"})
    require(len(list(reports.iterdir())) == 3, "report_staging_leak")
    result = {"status": "pass", "cancelled_without_file": True, "artifacts": results,
              "source": "actual_private_app_metrics", "provider_traffic": "not_tested"}
    print("headroom-package-reports=" + json.dumps(result, separators=(",", ":")), flush=True)
    return result


def probe(root):
    ci_run = os.environ.get("GITHUB_ACTIONS") == "true" or os.environ.get("CIRCLECI") == "true"
    require(sys.platform == "darwin" and ci_run, "ci_macos_required")
    require(root.is_absolute() and root.resolve(strict=True) == root
            and root.parent == (ROOT / "work").resolve(strict=True)
            and root.name.startswith("macos-package."), "invalid_proof_root")
    require(root.stat().st_uid == os.geteuid() and stat.S_IMODE(root.stat().st_mode) == 0o700,
            "invalid_proof_owner")
    bundle = root / "PennyPinch.app"
    with (bundle / "Contents/Info.plist").open("rb") as stream:
        info = plistlib.load(stream)
    require(info["CFBundleIdentifier"] == "dev.headroom.desktop.h048c5", "invalid_qa_bundle")
    app_executable = (bundle / "Contents/MacOS/headroom-desktop").resolve(strict=True)
    runtime = bundle / "Contents/Resources/runtime"
    original_runtime = Path(os.environ["HEADROOM_MACOS_RUNTIME_ROOT"])
    state = root / "state"
    require(not list(state.iterdir()), "qa_state_not_clean")
    launch = root / "launch"
    launch.mkdir(mode=0o700)
    before_binary = hashlib.sha256(app_executable.read_bytes()).hexdigest()
    before_inventory = bundle_inventory(bundle, original_runtime)
    owned_apps, owned_engines, launchers = [], [], []
    steps = []
    reports = None
    try:
        for opening in range(2):
            require(find_app(app_executable) is None, "qa_app_already_open")
            launcher = subprocess.Popen(
                ["/usr/bin/open", "-n", "-W", "-a", str(bundle)], cwd=launch,
                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            )
            launchers.append(launcher)
            app = wait_for(lambda: find_app(app_executable), "app_launch_timeout")
            owned_apps.append(app)
            run_ui("open")
            port = wait_for(lambda: endpoint_owned(app, state), "ingress_ready_timeout")
            if steps:
                require(port == steps[0]["port"], "saved_endpoint_changed")
            require(find_engine(app, runtime) is None, "engine_started_without_ui_action")
            if opening == 0 and os.environ.get("HEADROOM_VERIFY_REPORT_EXPORT") == "true":
                reports = exercise_reports(root)
            run_ui("start")
            engine = wait_for(lambda: find_engine(app, runtime), "packaged_engine_missing")
            owned_engines.append(engine)
            require(not lock_available(state / "runtime/.engine.lock"), "engine_lock_unheld")
            steps.append({"opening": opening + 1, "action": "started", "app_pid": app[0],
                          "engine_pid": engine[0], "port": port})
            if opening == 0:
                run_ui("stop")
                wait_for(lambda: not matches_process(engine), "stopped_engine_alive")
                require(lock_available(state / "runtime/.engine.lock"), "stopped_engine_lock_held")
                run_ui("start")
                replacement = wait_for(lambda: find_engine(app, runtime), "restarted_engine_missing")
                require(replacement[0] != engine[0], "restart_reused_old_process")
                owned_engines.append(replacement)
                steps.append({"opening": 1, "action": "restarted", "app_pid": app[0],
                              "engine_pid": replacement[0], "port": endpoint_owned(app, state)})
            run_ui("exit")
            wait_for(lambda: not matches_process(app), "app_exit_timeout")
            for engine in owned_engines:
                wait_for(lambda: not matches_process(engine), "exited_engine_alive")
            wait_for(lambda: port_released(port), "endpoint_not_released")
            require(lock_available(state / "runtime/.ingress.lock")
                    and lock_available(state / "runtime/.engine.lock"), "exit_lock_held")
            require(launcher.wait(timeout=10) == 0, "application_launcher_failed")
            steps.append({"opening": opening + 1, "action": "clean_exit", "port": port})
        require(hashlib.sha256(app_executable.read_bytes()).hexdigest() == before_binary,
                "bundle_executable_changed")
        after_inventory = bundle_inventory(bundle, original_runtime)
        require(before_inventory == after_inventory, "bundle_runtime_changed")
        result = {"schema_version": 1, "status": "pass", "binary_sha256": before_binary,
                  "runtime": before_inventory, "steps": steps, "openings": 2,
                  "engine_processes": len(owned_engines), "clean_shutdown": True,
                  "signing": "ad_hoc", "live_provider_traffic": "not_tested",
                  "build_profile": {"release": True, "lto": False, "codegen_units": 16}}
        if reports is not None:
            result["report_export"] = reports
        (root / "lifecycle.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(result, separators=(",", ":")), flush=True)
    except ProbeError:
        live_apps = sum(matches_process(app) for app in owned_apps)
        for app in owned_apps:
            if matches_process(app):
                engine = find_engine(app, runtime)
                if engine and engine not in owned_engines:
                    owned_engines.append(engine)
        live_engines = sum(matches_process(engine) for engine in owned_engines)
        print(f"headroom-package-processes=on_failure apps={live_apps} engines={live_engines}", flush=True)
        raise
    finally:
        for owned in reversed(owned_apps + owned_engines):
            if matches_process(owned):
                os.kill(owned[0], signal.SIGTERM)
                try:
                    wait_for(lambda: not matches_process(owned), "cleanup_timeout", seconds=5)
                except ProbeError:
                    if matches_process(owned):
                        os.kill(owned[0], signal.SIGKILL)
        for launcher in launchers:
            try:
                launcher.wait(timeout=5)
            except subprocess.TimeoutExpired:
                launcher.terminate()


if __name__ == "__main__":
    try:
        require(len(sys.argv) == 2, "proof_root_required")
        probe(Path(sys.argv[1]))
    except ProbeError as error:
        print(f"headroom-package-error={error}", file=sys.stderr)
        sys.exit(2)
    except (OSError, ValueError, KeyError, subprocess.SubprocessError):
        print("headroom-package-error=probe_failed", file=sys.stderr)
        sys.exit(2)
