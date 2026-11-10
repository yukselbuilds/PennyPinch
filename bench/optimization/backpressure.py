"""Run the finite, instrumented Rust relay stress case and sample only its process."""
from pathlib import Path
import importlib.util
import json
import os
import queue
import subprocess
import sys
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("optimization_replay", HERE / "replay.py")
replay = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = replay
spec.loader.exec_module(replay)
TEST = "runtime::ingress_relay::tests::long_blocked_peer_reports_real_relay_backpressure_and_preserves_bytes"


def run():
    replay.require(os.name == "nt" and Path.cwd().resolve() == ROOT, "windows_workspace_required")
    plan = json.loads((HERE / "backpressure-plan.json").read_text())
    directory = ROOT / "work/proofs/optimization/backpressure" / str(uuid.uuid4())
    directory.mkdir(parents=True)
    sources = [Path(__file__), HERE / "replay.py", HERE / "fixtures.py", HERE / "backpressure-plan.json",
               ROOT / "src-tauri/src/runtime/ingress_relay.rs",
               ROOT / "src-tauri/src/runtime/ingress_relay_tests.rs",
               ROOT / "src-tauri/Cargo.toml", ROOT / "src-tauri/Cargo.lock", Path(sys.executable)]
    before = replay.fingerprints(sources)
    environment = os.environ.copy()
    environment.update(CARGO_HOME=str(ROOT / "work/caches/cargo-home"),
                       CARGO_TARGET_DIR=str(ROOT / "work/caches/cargo-target"))
    result = {"passed": False, "raw_output_saved": False}
    process, sampler = None, None
    try:
        compile_args = ["cargo", "test", "--offline", "--manifest-path", "src-tauri/Cargo.toml",
                        "--no-run", "--message-format=json"]
        compiled = subprocess.run(compile_args, cwd=ROOT, env=environment,
                                  capture_output=True, text=True, encoding="utf-8", errors="replace")
        messages = []
        for line in compiled.stdout.splitlines():
            if line.startswith("{"):
                messages.append(json.loads(line))
        if compiled.returncode:
            errors = [row["message"].get("code") for row in messages
                      if row.get("reason") == "compiler-message" and row["message"]["level"] == "error"]
            result["compile_error_codes"] = errors
        replay.require(compiled.returncode == 0, "native_compile_failed")
        executables = {row["executable"] for row in messages
                       if row.get("reason") == "compiler-artifact" and row.get("executable")
                       and row["target"]["name"] == "headroom-desktop" and row["profile"]["test"]}
        replay.require(len(executables) == 1, "test_binary_not_unique")
        executable = Path(executables.pop()).resolve(strict=True)
        replay.require(executable.is_relative_to(ROOT / "work/caches/cargo-target"), "unowned_test_binary")
        result["test_binary_sha256_before"] = replay.sha(executable)
        process = subprocess.Popen([str(executable), TEST, "--ignored", "--exact", "--nocapture", "--test-threads=1"],
                                   cwd=ROOT, env=environment, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                   stderr=subprocess.DEVNULL, creationflags=subprocess.CREATE_NO_WINDOW)
        output = queue.Queue()

        def collect():
            for line in iter(lambda: process.stdout.readline(65536), b""):
                output.put(line)
            output.put(None)

        threading.Thread(target=collect, daemon=True).start()
        deadline = time.monotonic() + plan["thresholds"]["test_timeout_seconds"]
        phases, measurements = [], {}
        baseline_private = None
        while time.monotonic() < deadline:
            try:
                line = output.get(timeout=.25)
            except queue.Empty:
                replay.require(process.poll() is None, "test_exited_without_output_end")
                continue
            if line is None:
                break
            marker = b"H041_BACKPRESSURE "
            if marker not in line:
                continue
            phase = json.loads(line.split(marker, 1)[1])
            phases.append(phase)
            if phase["phase"] == "blocked_begin":
                replay.require(sampler is None, "overlapping_measurements")
                sampler = replay.Resources([process.pid], 50)
                baseline_private = sampler.samples[0]["engine_private_bytes"]
            elif phase["phase"] == "blocked_end":
                replay.require(sampler is not None, "measurement_start_missing")
                resource = sampler.finish()
                sampler = None
                measurements[phase["arm"]] = {
                    **resource,
                    "one_core_cpu_percent": 100 * resource["engine_cpu_ms"] / phase["blocked_elapsed_ms"],
                    "private_growth_bytes": resource["observed_engine_private_bytes"] - baseline_private,
                    "blocked_elapsed_ms": phase["blocked_elapsed_ms"],
                }
        else:
            raise AssertionError("bounded_test_timeout")
        process.wait(timeout=5)
        result["native_exit_code"] = process.returncode
        result["phases"] = phases
        result["measurements"] = measurements
        result["test_binary_sha256_after"] = replay.sha(executable)
        completed = [row for row in phases if row["phase"] == "complete"]
        relay_end = [row for row in phases if row["phase"] == "blocked_end" and row["arm"] == "relay"]
        checks = {
            "native_test_passed": process.returncode == 0,
            "two_complete_arms": len(completed) == 2 and all(row["byte_exact"] and row["relay_drained"] for row in completed),
            "real_sustained_relay_backpressure": len(relay_end) == 1 and relay_end[0]["sustained_backpressure"] is True,
            "two_measured_arms": set(measurements) == {"direct", "relay"},
            "cpu_limit": bool(measurements) and all(row["one_core_cpu_percent"] <= plan["thresholds"]["one_core_cpu_percent"]
                                                    for row in measurements.values()),
            "private_growth_limit": bool(measurements) and all(row["private_growth_bytes"] <= plan["thresholds"]["private_growth_bytes"]
                                                               for row in measurements.values()),
            "binary_unchanged": result["test_binary_sha256_before"] == result["test_binary_sha256_after"],
        }
        result.update(checks=checks, passed=all(checks.values()))
    except BaseException as error:
        result.update(passed=False, error_class=type(error).__name__,
                      error_code=str(error) if isinstance(error, AssertionError) else "bounded_backpressure_probe_failed")
    finally:
        if sampler is not None:
            try:
                sampler.finish()
            except BaseException:
                pass
        if process is not None and process.poll() is None:
            process.kill()
            process.wait(timeout=10)
        result.update(before=before, after=replay.fingerprints(sources), plan=plan,
                      environment={"windows_build": list(sys.getwindowsversion()[:3]), "python": sys.version,
                                   "logical_cpus": os.cpu_count()},
                      scope="cfg(test) Rust relay plus fixture threads; current private bytes; kernel buffers excluded",
                      stderr_status="redirected_to_null_contents_unknown")
        result["inputs_unchanged"] = result["before"] == result["after"]
        result["passed"] = result["passed"] and result["inputs_unchanged"]
        replay.write_new(directory / "result.json", result)
    print(json.dumps({key: value for key, value in result.items() if key not in ("before", "after", "plan", "phases")},
                     ensure_ascii=True))
    print(json.dumps({"proof": (directory / "result.json").relative_to(ROOT).as_posix()}))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(run())

