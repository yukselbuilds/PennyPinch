"""Pinned RTK-OSS command evaluation and paired four-arm local engine replay (H043)."""
from __future__ import annotations

import argparse
import ctypes
import hashlib
from http.server import ThreadingHTTPServer
import http.client
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import shutil
import site
import subprocess
import sys
import threading
import time
import uuid
import zipfile

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


base = load("rtk_engine_replay", ROOT / "bench/optimization/replay.py")
fixtures = load("rtk_command_fixtures", HERE / "fixtures.py")


def real_rtk_directories():
    paths = []
    for folder_id in (26, 28):  # Windows Known Folders, not redirectable APPDATA env values.
        buffer = ctypes.create_unicode_buffer(32768)
        base.require(ctypes.windll.shell32.SHGetFolderPathW(None, folder_id, None, 0, buffer) == 0,
                     "known_folder_lookup_failed")
        paths.append(Path(buffer.value) / "rtk")
    return paths


def require_no_user_rtk(paths):
    # shortcut: official Windows RTK has no config-path override; only evaluate while no real RTK profile exists.
    # Upgrade to an upstream path override before supporting coexistence with an existing Windows RTK installation.
    base.require(all(not os.path.lexists(path) for path in paths), "existing_user_rtk_profile_not_isolated")


def environment(run, git):
    keep = ("SystemRoot", "WINDIR", "COMSPEC", "PATHEXT", "NUMBER_OF_PROCESSORS", "PROCESSOR_ARCHITECTURE")
    env = {key: value for key, value in os.environ.items() if key.upper() in {k.upper() for k in keep}}
    state = run / "command-state"
    state.mkdir()
    for name in ("temp", "profile", "tee"):
        (state / name).mkdir()
    env.update(PATH=os.pathsep.join((str(git.parent), str(Path(sys.executable).parent),
                                    str(Path(os.environ["SystemRoot"]) / "System32"))),
               TEMP=str(state / "temp"), TMP=str(state / "temp"), USERPROFILE=str(state / "profile"),
               RTK_DB_PATH=str(state / "tracking.db"), RTK_RECALL_DB=str(state / "recall.db"),
               RTK_TEE_DIR=str(state / "tee"), RTK_TELEMETRY_DISABLED="1", RTK_NO_TOML="1",
               DO_NOT_TRACK="1", NO_COLOR="1", CLICOLOR="0", TERM="dumb", GIT_TERMINAL_PROMPT="0",
               GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_SYSTEM=os.devnull,
               GIT_OPTIONAL_LOCKS="0", PYTHONIOENCODING="utf-8")
    return env


def command(argv, workspace, env, timeout):
    started = time.perf_counter()
    result = subprocess.run(argv, cwd=workspace, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=timeout, creationflags=subprocess.CREATE_NO_WINDOW)
    return {"exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr,
            "total_ms": (time.perf_counter() - started) * 1000}


def metadata(result):
    return {"exit_code": result["exit_code"], "total_ms": result["total_ms"],
            **{f"{stream}_{key}": value for stream in ("stdout", "stderr")
               for key, value in (("bytes", len(result[stream])), ("sha256", hashlib.sha256(result[stream]).hexdigest()))}}


def envelope(result):
    return base.encoded({"exit_code": result["exit_code"], "stdout": result["stdout"].decode("utf-8"),
                         "stderr": result["stderr"].decode("utf-8")}).decode("utf-8")


def verify_recovery(port, original, forwarded, timeout):
    if original == forwarded:
        return {"verified": True, "retrievals": 0, "recovered_content_bytes": 0,
                "scope": "unchanged", "diagnostics_visible": True}
    hashes = set(re.findall(r"(?:Retrieve more: hash=|<<ccr:)([a-fA-F0-9]{12,24})(?=[^a-fA-F0-9]|$)", forwarded))
    contents = {}
    for content_hash in hashes:
        connection = http.client.HTTPConnection("127.0.0.1", port, timeout=timeout)
        try:
            connection.request("POST", "/v1/retrieve", base.encoded({"hash": content_hash}), {"Content-Type": "application/json"})
            response = connection.getresponse()
            base.require(response.status == 200, "ccr_retrieval_status")
            contents[content_hash] = json.loads(response.read(2 * 1024 * 1024))["original_content"]
        finally:
            connection.close()
    expected = json.loads(original)
    whole_recovered = original in contents.values()
    recovered_bytes = sum(len(content.encode("utf-8")) for content in contents.values())
    try:
        restored = json.loads(forwarded)
    except json.JSONDecodeError:
        restored = None
    if not isinstance(restored, dict) or set(restored) != set(expected):
        return {"verified": whole_recovered, "retrievals": len(contents), "recovered_content_bytes": recovered_bytes,
                "diagnostics_visible": False,
                "scope": "whole_tool_output" if whole_recovered else "envelope_shape_changed"}
    diagnostics_visible = (type(restored["exit_code"]) is int and restored["exit_code"] == expected["exit_code"]
                           and restored["stderr"] == expected["stderr"])
    for field in ("stdout", "stderr"):
        for content_hash, content in contents.items():
            if isinstance(restored.get(field), str) and content_hash in restored[field] and content == expected[field]:
                restored[field] = content
    return {"verified": whole_recovered or base.encoded(restored).decode("utf-8") == original,
            "retrievals": len(contents), "recovered_content_bytes": recovered_bytes, "diagnostics_visible": diagnostics_visible,
            "scope": "whole_tool_output" if whole_recovered else "exact_envelope_reconstruction"}


def exact_json(data):
    # Tag number lexemes so decimal rounding and number-to-string conversion are visible.
    def number(text):
        return (text,)
    return json.loads(data, parse_int=number, parse_float=number, object_pairs_hook=tuple)


def rejection_reasons(raw, filtered, case, recovered):
    reasons = []
    if raw["exit_code"] != filtered["exit_code"]:
        reasons.append("exit_code_changed")
    if raw["stderr"] != filtered["stderr"]:
        reasons.append("stderr_changed_or_moved")
    if case in ("git_diff", "source_read") and raw["stdout"] != filtered["stdout"]:
        reasons.append("source_bytes_changed")
    if case == "json_exact":
        try:
            if exact_json(raw["stdout"]) != exact_json(filtered["stdout"]):
                reasons.append("exact_json_values_or_structure_changed")
        except (json.JSONDecodeError, UnicodeDecodeError):
            reasons.append("filtered_output_not_json")
    if case in ("test_pass", "test_fail"):
        count = b"100 passed" in filtered["stdout"]
        failure = case != "test_fail" or b"1 failed" in filtered["stdout"]
        if not count or not failure:
            reasons.append("test_counts_missing_or_changed")
    if raw["stdout"] != filtered["stdout"] and not recovered:
        reasons.append("original_stdout_not_recoverable")
    for marker in (b"EXACT_FAILURE_9007199254740993", b"SyntaxError", b"UNIQUE_STDERR_9007199254740993"):
        if marker in raw["stdout"] + raw["stderr"] and marker not in filtered["stdout"] + filtered["stderr"]:
            reasons.append("unique_failure_hidden")
    return reasons


def self_check():
    sample = {"exit_code": 23, "stdout": b"original\n", "stderr": b"warning\n", "total_ms": 1}
    assert rejection_reasons(sample, sample, "stderr_exit", False) == []
    changed = {**sample, "exit_code": 0, "stdout": b"summary\n", "stderr": b""}
    assert set(rejection_reasons(sample, changed, "stderr_exit", False)) == {
        "exit_code_changed", "stderr_changed_or_moved", "original_stdout_not_recoverable"}
    assert "source_bytes_changed" in rejection_reasons(sample, changed, "source_read", True)
    numeric = {**sample, "stdout": b'{"n":18446744073709551617,"d":-17.2500}'}
    rounded = {**numeric, "stdout": b'{"n":1.8446744073709552e19,"d":-17.25}'}
    assert "exact_json_values_or_structure_changed" in rejection_reasons(numeric, rounded, "json_exact", True)
    assert exact_json(b'{"n":1}') != exact_json(b'{"n":"1"}')
    assert exact_json(b'{"n":1}') == exact_json(b'{  "n": 1  }\n')
    assert "test_counts_missing_or_changed" in rejection_reasons(sample, sample, "test_fail", False)
    assert json.loads(envelope(sample))["stderr"] == "warning\n"


def baseline_comparison(plan, env):
    proof = json.loads((ROOT / "docs/proofs/optimization-local-acceptance.json").read_text(encoding="utf-8"))["responses"]["evidence"]
    evidence = ROOT / proof["path"]
    comparison = {"evidence": proof, "reused": False, "reason": "new_command_outputs_and_request_bodies_not_H041_fixtures"}
    if not evidence.is_file():
        return {**comparison, "local_h041_receipt": "unavailable"}
    old = json.loads(evidence.read_text(encoding="utf-8"))
    base.require(base.sha(evidence) == proof["sha256"] and old["inputs_unchanged"] and old["passed"],
                 "h041_evidence_not_verified")
    matching, changed, unavailable = [], [], []
    for name, digest in old["after"].items():
        path = ROOT / name
        (unavailable if not path.is_file() else matching if base.sha(path) == digest else changed).append(name)
    return {**comparison, "matched_inputs": len(matching), "changed_inputs": changed,
            "unavailable_inputs": unavailable, "old_environment": old["environment"], "current_environment": env,
            "old_cases": json.loads((ROOT / "bench/optimization/plan.json").read_text(encoding="utf-8"))["cases"],
            "new_cases": plan["cases"]}


def replay(plan, run, env, rtk, git, controller, supervisor, payload, tokenizer):
    workspace = run / "command-workspace"
    fixtures.create(workspace, env, git)
    timeout = plan["thresholds"]["command_timeout_seconds"]
    version = command([str(rtk), "--version"], workspace, env, timeout)
    base.require(version["exit_code"] == 0 and version["stdout"].strip() == f"rtk {plan['version']}".encode(), "rtk_version_changed")
    no_engine = command([str(rtk), "methods"], workspace, env, timeout)
    base.require(no_engine["exit_code"] == 2, "oss_engine_command_should_be_absent")
    server = ThreadingHTTPServer(("127.0.0.1", 0), base.FixtureHandler)
    server.daemon_threads = True
    server.capture_lock, server.captured = threading.Lock(), {}
    threading.Thread(target=server.serve_forever, daemon=True).start()
    commands, rows, engines = [], [], []
    engine_plan = {**plan, "memory_sample_interval_ms": 50}
    try:
        for iteration in range(plan["process_runs"]):
            engine = base.Engine(controller, supervisor, payload, run / f"engine-{iteration}", server.server_port, engine_plan)
            engines.append({})
            try:
                engines[-1].update(engine.start())
                for round_number in range(plan["rounds"]):
                    for case in plan["cases"]:
                        direct, wrapped = fixtures.commands(case, workspace, git, rtk)
                        # Rotate execution order; compare both outputs from the same owned fixture event.
                        outcomes = {}
                        for name in (("control", "rtk") if (iteration + round_number) % 2 == 0 else ("rtk", "control")):
                            outcomes[name] = command(direct if name == "control" else wrapped, workspace, env, timeout)
                        raw, filtered = outcomes["control"], outcomes["rtk"]
                        base.require(raw["exit_code"] == fixtures.EXPECTED_EXIT[case] and raw["stdout"], "control_fixture_unexpected_result")
                        bypass = command([str(rtk), "bypass", *direct], workspace, env, timeout)
                        base.require(all(raw[key] == bypass[key] for key in ("exit_code", "stdout", "stderr")), "bypass_not_byte_exact")
                        recall_checks = []
                        for content_hash in set(re.findall(rb"rtk recall ([0-9a-f]{12,64})", filtered["stdout"] + filtered["stderr"])):
                            recalled = command([str(rtk), "recall", content_hash.decode(), "--full"], workspace, env, timeout)
                            recall_checks.append({"exit_code": recalled["exit_code"],
                                "exact_stdout": recalled["exit_code"] == 0 and recalled["stdout"] == raw["stdout"] and not recalled["stderr"],
                                "exact_merged_streams": recalled["exit_code"] == 0 and recalled["stdout"] in (
                                    raw["stdout"] + raw["stderr"], raw["stderr"] + raw["stdout"]) and not recalled["stderr"]})
                        recovered = any(check["exact_stdout"] for check in recall_checks)
                        command_bytes = lambda output: len(output["stdout"]) + len(output["stderr"])
                        original_bytes, filtered_bytes = command_bytes(raw), command_bytes(filtered)
                        event = f"r{iteration}-{case}-t{round_number}"
                        commands.append({"event": event, "case": case, "process_run": iteration, "round": round_number,
                            "control": metadata(raw), "rtk": metadata(filtered), "bypass_byte_exact": True,
                            "recall": recall_checks, "rejection_reasons": rejection_reasons(raw, filtered, case, recovered),
                            "original_bytes": original_bytes, "filtered_bytes": filtered_bytes,
                            "byte_saved_percent": 100 * (original_bytes - filtered_bytes) / original_bytes if original_bytes else 0,
                            "bytes_div_4_estimate": {"original": original_bytes / 4, "filtered": filtered_bytes / 4},
                            "added_ms": filtered["total_ms"] - raw["total_ms"],
                            "rtk_process_cpu_and_ram": "not_measured"})
                        base.write_new(run / f"command-{len(commands):03d}.json", commands[-1])
                        raw_body = fixtures.body(envelope(raw), case, iteration, round_number)
                        rtk_body = fixtures.body(envelope(filtered), case, iteration, round_number)
                        control_tokens = len(tokenizer.encode(base.encoded(raw_body).decode(), disallowed_special=()))
                        start_arm = (iteration + round_number) % len(plan["arms"])
                        arm_order = plan["arms"][start_arm:] + plan["arms"][:start_arm]
                        for arm in arm_order:
                            body = rtk_body if arm in ("rtk", "rtk_headroom") else raw_body
                            via_engine = arm in ("headroom", "rtk_headroom")
                            key = f"r{iteration}-{case}-{arm}-t{round_number}"
                            sampler = base.Resources([os.getpid()] + (engine.pids if via_engine else []), 50)
                            try:
                                outcome = base.request(engine.port if via_engine else server.server_port, body, key,
                                                       plan["thresholds"]["request_timeout_seconds"])
                            finally:
                                resource = sampler.finish()
                            with server.capture_lock:
                                captured = server.captured.pop(key, [])
                            base.require(outcome["status"] == 200 and len(captured) == 1 and captured[0]["auth_preserved"],
                                         "single_owned_upstream_success")
                            forwarded = captured[0]["body"]
                            base.validate_body(body, forwarded, None, case in ("json_exact", "source_read", "git_diff"))
                            if not via_engine:
                                base.require(forwarded == body, "direct_payload_changed")
                            recovery = []
                            for call_id, output in base.fixtures.outputs(forwarded).items():
                                recovery.append(verify_recovery(engine.port, base.fixtures.outputs(body)[call_id], output, timeout))
                            forwarded_tokens = len(tokenizer.encode(base.encoded(forwarded).decode(), disallowed_special=()))
                            rows.append({"event": event, "case": case, "process_run": iteration, "round": round_number, "arm": arm,
                                "input_sha256": hashlib.sha256(base.encoded(body)).hexdigest(),
                                "forwarded_sha256": hashlib.sha256(base.encoded(forwarded)).hexdigest(),
                                "control_tokens": control_tokens, "forwarded_tokens": forwarded_tokens,
                                "saved_percent": 100 * (control_tokens - forwarded_tokens) / control_tokens,
                                "recovery": recovery, "ccr_retrievals_verified": sum(item["retrievals"] for item in recovery if item["verified"]),
                                "protocol_integrity": "pass" if all(item["verified"] for item in recovery) else "recovery_failed", **outcome, **resource,
                                "command_plus_request_ms": outcomes["rtk" if arm in ("rtk", "rtk_headroom") else "control"]["total_ms"] + outcome["total_ms"]})
                            base.write_new(run / f"request-{len(rows):03d}.json", rows[-1])
                            base.require(all(item["verified"] for item in recovery), "engine_changed_without_exact_recovery")
            finally:
                engine.close()
                engines[-1]["stop"] = engine.stopped
    finally:
        server.shutdown()
        server.server_close()
    cases = {}
    for case in plan["cases"]:
        items = [item for item in commands if item["case"] == case]
        warm = [item for item in items if item["round"] > 0]
        reasons = sorted(set(reason for item in items for reason in item["rejection_reasons"]))
        if not all(item["byte_saved_percent"] >= 100 * plan["thresholds"]["minimum_command_saved_fraction"] for item in items):
            reasons.append("below_minimum_byte_benefit")
        command_p95 = base.percentile([item["added_ms"] for item in warm])
        pair_rows = {item["event"]: {} for item in warm}
        for row in rows:
            if row["event"] in pair_rows:
                pair_rows[row["event"]][row["arm"]] = row
        combined_p95 = base.percentile([pair["rtk_headroom"]["command_plus_request_ms"] - pair["control"]["command_plus_request_ms"]
                                        for pair in pair_rows.values()])
        if command_p95 > plan["thresholds"]["warm_command_added_p95_ms"]:
            reasons.append("command_latency_exceeded")
        if combined_p95 > plan["thresholds"]["warm_combined_added_p95_ms"]:
            reasons.append("combined_latency_exceeded")
        if not all(check["diagnostics_visible"] for row in rows if row["case"] == case and row["arm"] == "rtk_headroom"
                   for check in row["recovery"]):
            reasons.append("combined_immediate_diagnostics_changed")
        cases[case] = {"support": "rejected_for_this_fixture" if reasons else "local_fixture_only",
            "reasons": reasons, "warm_command_added_p95_ms": command_p95, "warm_combined_added_p95_ms": combined_p95,
            "minimum_byte_saved_percent": min(item["byte_saved_percent"] for item in items),
            "arms": {arm: {"minimum_saved_percent": min(row["saved_percent"] for row in rows if row["case"] == case and row["arm"] == arm),
                           "maximum_saved_percent": max(row["saved_percent"] for row in rows if row["case"] == case and row["arm"] == arm)} for arm in plan["arms"]}}
    return {"passed": len(rows) == plan["process_runs"] * plan["rounds"] * len(plan["cases"]) * 4,
            "cases": cases, "commands": commands, "rows": rows, "engines": engines,
            "external_provider_calls": 0, "provider_cache_discount": None, "invoice_savings": None,
            "response_quality": None, "model_ttft": None,
            "latency_scope": "Command wall time plus loopback first-delta/total; fresh request per event; OS cache uncontrolled",
            "resource_scope": "H041 owned harness/controller/helper/engine sampler; RTK child CPU/RAM not measured; Rust ingress excluded",
            "history_cache_scope": "Repeated fixture rounds with unique request identity, not appended multi-turn cache or provider billing"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-check", action="store_true")
    parser.add_argument("--run", action="store_true")
    parser.add_argument("--diagnose", action="store_true", help="One git-status event, never a full H043 acceptance")
    args = parser.parse_args()
    self_check()
    if args.self_check and not args.run:
        print(json.dumps({"passed": True, "network_calls": 0, "checks": "rejection, exact-number, byte and envelope contracts"}))
        return 0
    base.require(args.run and os.name == "nt" and Path.cwd().resolve() == ROOT and ROOT == Path(r"D:\Projeler\Headroom"), "explicit_canonical_windows_run_required")
    plan = json.loads((HERE / "plan.json").read_text(encoding="utf-8"))
    toolchain = ROOT / "work/proofs/rtk/toolchain" / plan["version"]
    receipt = json.loads((toolchain / "receipt.json").read_text(encoding="utf-8"))
    base.require(receipt["commit"] == plan["commit"] and receipt["edition"] == "rtk-oss"
                 and receipt["plan_sha256"] == base.sha(HERE / "plan.json")
                 and all(base.sha(toolchain / path) == digest for path, digest in receipt["files"].items()), "pinned_oss_files_changed")
    archive = toolchain / plan["asset"]
    base.require(base.sha(archive) == plan["asset_sha256"], "archive_digest_mismatch")
    with zipfile.ZipFile(archive) as package:
        member, = (info for info in package.infolist() if Path(info.filename).name == "rtk.exe")
        base.require(hashlib.sha256(package.read(member)).hexdigest() == base.sha(toolchain / "rtk.exe"), "executable_differs_from_pinned_archive")
    if args.diagnose:
        plan = {**plan, "process_runs": 1, "rounds": 2, "cases": ["git_status"], "diagnostic_only": True}
    paths = real_rtk_directories()
    require_no_user_rtk(paths)
    git_exe = shutil.which("git.exe")
    base.require(git_exe, "git_not_found")
    git = Path(git_exe).resolve()
    controller = load("rtk_benchmark_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(ROOT / "runtime/manifest.json")
    base.require(base.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "pinned_python_required")
    supervisor_path = payload / manifest["engine"]["runner"]
    supervisor = load("rtk_benchmark_supervisor", supervisor_path)
    cache = controller.verify_tokenizer_cache(payload)
    os.environ["TIKTOKEN_CACHE_DIR"] = str(cache)
    site.addsitedir(str(payload / "Lib/site-packages"))
    import tiktoken
    tokenizer = tiktoken.get_encoding(plan["token_encoding"])
    run = ROOT / "work/proofs/rtk/runs" / str(uuid.uuid4())
    run.mkdir(parents=True)
    env = environment(run, git)
    sources = [*HERE.glob("*.py"), HERE / "plan.json", Path(base.__file__), Path(base.fixtures.__file__),
               *sorted((ROOT / "runtime").glob("*.py")), ROOT / "runtime/manifest.json", ROOT / "runtime/optimization-sources.json",
               supervisor_path, Path(sys.executable), git, toolchain / "receipt.json",
               *(toolchain / path for path in receipt["files"])]
    sources.extend((payload / "Lib/site-packages/headroom").rglob("*.py"))
    for package in ("tiktoken", "tiktoken_ext"):
        sources.extend(path for path in (payload / "Lib/site-packages" / package).rglob("*")
                       if path.is_file() and "__pycache__" not in path.parts)
    sources.extend(path for path in cache.iterdir() if path.is_file())
    sources.extend(base.prepare_fixture_runtime(run, payload))
    fingerprint = lambda: {str(path): base.sha(path) for path in sorted(set(sources))}
    before = fingerprint()
    def environment_info():
        return {"windows_build": list(sys.getwindowsversion()[:3]), "python": sys.version,
                "machine": platform.machine(), "processor": platform.processor(), "logical_cpus": os.cpu_count()}
    environment_before = environment_info()
    baseline = baseline_comparison(plan, environment_before)
    base.write_new(run / "plan.json", plan)
    result = {"passed": False}
    try:
        result = replay(plan, run, env, toolchain / "rtk.exe", git, controller, supervisor, payload, tokenizer)
    except BaseException as error:
        result.update(passed=False, error_class=type(error).__name__,
                      error_code=str(error) if isinstance(error, AssertionError) else "bounded_replay_failed")
    finally:
        result.update(before=before, after=fingerprint(), environment=environment_before, environment_after=environment_info(), baseline=baseline,
                      run=run.relative_to(ROOT).as_posix(), token_basis=plan["token_basis"], token_encoding=plan["token_encoding"],
                      fixture_network=base.fixture_network_evidence(run), user_rtk_directories_absent=all(not os.path.lexists(path) for path in paths))
        result["diagnostic_only"] = args.diagnose
        result["inputs_unchanged"] = result["before"] == result["after"] and result["environment"] == result["environment_after"]
        result["passed"] = result["passed"] and result["inputs_unchanged"] and result["user_rtk_directories_absent"] and result["fixture_network"]["huggingface_hub_payload_bytes"] == 0
        base.write_new(run / "result.json", result)
    print(json.dumps({key: result[key] for key in ("passed", "run", "inputs_unchanged", "user_rtk_directories_absent")}, ensure_ascii=True))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
