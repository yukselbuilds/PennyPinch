"""Measure one Learn path suggestion against the existing workspace rule in Codex."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import runpy
import subprocess
import sys
from typing import Literal
import uuid


PROGRAM = Path(__file__).resolve()
ROOT = PROGRAM.parents[2]
COMPARE = ROOT / "bench/context_tools/targeted_query_compare.py"
PREVIEW = ROOT / "runtime/learn_preview.py"
LABELS = ROOT / "src/features/settings/Learn.tsx"
POLICY = ROOT / "AGENTS.md"
ARMS = ("workspace_rule", "workspace_rule_and_learn")
WORKDIRS = ("checkout", "staged_package")
PATHS = ("reports/engine-start.json", "engine-start.json")
TOOL = "inspect_engine_report"
ANSWER = {"engine_version": "0.44.0", "p95_ms": "184.072500000000000109", "samples": 19}
STALE = {"engine_version": "0.43.0", "p95_ms": "197.11", "samples": 17}
SCHEMA = {"type": "object", "properties": {key: {"type": "integer" if type(value) is int else "string"}
    for key, value in ANSWER.items()}, "required": list(ANSWER), "additionalProperties": False}
TASK = (
    "Repository guidance: Work within the designated canonical workspace, this release smoke-test fixture.\n"
    "In the fixture, the checkout has a development report and the staged_package has the report from the "
    "release smoke test. Use inspect_engine_report to read reports/engine-start.json from the staged package. "
    "Return the engine_version and p95_ms as exact strings and samples as an integer, in one JSON object. "
    "Use only the fixture tool."
)


def inspect_report(fixture, action, working_directory, relative_path):
    assert action in ("exists", "read") and working_directory in WORKDIRS and relative_path in PATHS, "fixture_arguments"
    path = fixture / working_directory / relative_path
    assert path.resolve() == path, "fixture_path_redirected"
    if not path.is_file():
        return {"status": "missing"}
    if action == "exists":
        return {"status": "present"}
    return {"status": "read", "report": json.loads(path.read_bytes())}


def serve(fixture, proof_root):
    from mcp.server.mcpserver import MCPServer
    from mcp.types import ToolAnnotations

    assert fixture.resolve(strict=True) == fixture and fixture.name == "fixture" and fixture.parent.parent == proof_root
    assert re.fullmatch(r"learn-[0-9a-f]{32}", fixture.parent.name), "fixture_scope"
    server = MCPServer("headroom_rule_pilot")

    read_only = ToolAnnotations(read_only_hint=True, destructive_hint=False, idempotent_hint=True, open_world_hint=False)

    @server.tool(structured_output=False, annotations=read_only)
    def inspect_engine_report(action: Literal["exists", "read"], working_directory: Literal["checkout", "staged_package"],
                              relative_path: Literal["reports/engine-start.json", "engine-start.json"]) -> str:
        """Check existence or read JSON from the fixed release fixture, relative to the selected working directory."""
        return json.dumps(inspect_report(fixture, action, working_directory, relative_path))

    server.run(transport="stdio")


def verified_suggestion(replay, evidence):
    evidence = evidence.resolve(strict=True)
    assert evidence.parent.parent == ROOT / "work/proofs/optimization/learn-merit" and evidence.name == "result.json"
    raw = json.loads(evidence.read_bytes())
    advice = raw["displayed_guidance"]["en"]["text"]
    assert raw["passed"] and raw["before"] == raw["after"], "local_evidence_failed"
    assert raw["scenarios"]["codex"]["stable_failure_then_workdir_repair"] == [
        {"kind": "path", "failures": 3, "recoveries": 0, "first_line": 3}], "path_suggestion_missing"
    assert all(raw["after"][path.relative_to(ROOT).as_posix()] == replay.sha(path)
               for path in (PREVIEW, LABELS, POLICY)), "learn_preview_inputs_changed"
    assert f'      path: "{advice}",' in LABELS.read_text(encoding="utf-8").splitlines(), "learn_label_changed"
    assert POLICY.read_text(encoding="utf-8").splitlines().count(r"- The canonical workspace is `D:\Projeler\Headroom`.") == 1
    return advice, evidence


def observe_turn(stdout, exit_code, fixture, server, report_error):
    events = [json.loads(line) for line in stdout.splitlines() if line.startswith(b"{")]
    items = [event["item"] for event in events if event.get("type") in ("item.completed", "item.failed")]
    messages = [item["text"] for item in items if item.get("type") == "agent_message"]
    try:
        answer = json.loads(messages[-1]) if messages else None
    except (ValueError, RecursionError):
        answer = None
    exact = (isinstance(answer, dict) and answer.keys() == ANSWER.keys()
             and all(type(answer[key]) is type(value) and answer[key] == value for key, value in ANSWER.items()))
    calls, failed = [], 0
    for item in items:
        if item.get("type") != "mcp_tool_call":
            continue
        try:
            assert item["server"] == server and item["tool"] == TOOL and item["status"] == "completed" and not report_error(item)
            args = item["arguments"]
            expected = inspect_report(fixture, **args)
            assert json.loads(item["result"]["content"][0]["text"]) == expected
            calls.append({**args, "status": expected["status"]})
        except (AssertionError, KeyError, IndexError, TypeError, ValueError):
            failed += 1
    valid_read = {"action": "read", "working_directory": WORKDIRS[1], "relative_path": PATHS[0], "status": "read"}
    return {"cli_exit": exit_code, "turn_completed": exit_code == 0 and sum(e.get("type") == "turn.completed" for e in events) == 1
                and not any(e.get("type") in ("error", "turn.failed") for e in events),
            "exact_answer": exact, "exact_staged_read": valid_read in calls,
            "tool_calls": calls, "unverified_tool_calls": failed,
            "repeat_calls": len(calls) - len({tuple(sorted(call.items())) for call in calls}),
            "missing_or_stale_reads": sum(call["action"] == "read" and call != valid_read for call in calls)}


def run_arm(arm, prompt, cli, compare, pilot, rules, usage, supervisor, controller, payload, run, fixture,
            profile, files):
    replay = usage["replay"]
    model, effort = compare["LIVE_MODEL"], compare["LIVE_EFFORT"]
    directory = run / arm
    directory.mkdir()
    staged, counter = rules["stage_arm"](replay, directory, payload, compare["UPSTREAM"], model, effort,
        compare["ATTEMPTS_PER_ARM"])
    work = directory / "client/workspace"
    work.mkdir(parents=True)
    transient = work.parent
    env = pilot["client_environment"]()
    env.update(CODEX_HOME=str(profile), CODEX_SQLITE_HOME=str(transient / "state"), TMP=str(transient), TEMP=str(transient))
    engine = usage["probe"]["UsageEngine"](controller, supervisor, payload, directory / "engine", 1, pilot["PLAN"])
    server = compare["SERVER"]
    protected = [TOOL, f"mcp__{server}__{TOOL}", "exec", "functions.exec"]
    engine.environment["HEADROOM_DESKTOP_PROTECTED_TOOLS"] = json.dumps(controller.validate_protected_tools(sorted(protected)))
    fingerprints = sorted({*files, *staged, *(ROOT / "runtime" / path.name for path in staged)})
    before = {str(path): replay.sha(path) for path in fingerprints}
    rejections = directory / "guard-rejections.txt"

    def rejection_codes():
        return rejections.read_text(encoding="ascii").splitlines() if rejections.is_file() else []

    try:
        engine.start()
        assert pilot["usage_ids"](engine, usage) == (set(), set()), "engine_not_empty"
        startup_rejections = rejection_codes()
        config = {"model_provider": "pilot", "model_providers.pilot.name": "Headroom Learn Pilot",
            "model_providers.pilot.base_url": f"http://127.0.0.1:{engine.port}/v1",
            "model_providers.pilot.wire_api": "responses", "model_providers.pilot.supports_websockets": False,
            "model_providers.pilot.requires_openai_auth": True, "model_reasoning_effort": effort,
            "model_catalog_json": str(run / "models.json"), "cli_auth_credentials_store": "file",
            "log_dir": str(transient / "logs"), "project_doc_max_bytes": 0, "features.memories": False,
            "features.multi_agent": False, "features.multi_agent_v2": False,
            f"mcp_servers.{server}.args": ["-I", "-B", str(PROGRAM), "--mcp", str(fixture)],
            f"mcp_servers.{server}.enabled_tools": [TOOL]}
        command = pilot["client_args"](cli, work, "counts", arm, model, config, prompt=prompt,
                                       output_schema=run / "answer-schema.json")
        try:
            exit_code, stdout = pilot["run_client"](command, work, env, supervisor, 100)
        except subprocess.TimeoutExpired:
            exit_code, stdout = -1, b""
        sent = counter.read_bytes().count(b"\n") if counter.is_file() else 0
        row = {"arm": arm, "forwarded_http_attempts": sent, "guard_rejections": rejection_codes()[len(startup_rejections):],
               **observe_turn(stdout, exit_code, fixture, server, rules["tool_reported_error"]),
               **rules["measure"](engine, usage, sent)}
    finally:
        try:
            engine.close()
        finally:
            compare["remove_client_state"](transient)
    after = {str(path): replay.sha(path) for path in fingerprints}
    return {**row, "stop": engine.stopped, "fingerprints": {"before": before, "after": after, "matched": before == after}}


def main(compare):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--live", action="store_true", required=True)
    parser.add_argument("--codex", type=Path, required=True)
    parser.add_argument("--local-evidence", type=Path, required=True)
    args = parser.parse_args()
    assert os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace"
    cli = args.codex.resolve(strict=True)
    app_bin = (Path(os.environ["LOCALAPPDATA"]) / "OpenAI/Codex/bin").resolve(strict=True)
    assert cli.name == "codex.exe" and cli.is_relative_to(app_bin), "cli_scope"
    version = subprocess.run([str(cli), "--version"], capture_output=True, timeout=10, check=True).stdout.decode().strip()
    assert version == compare["CLI_VERSION"], "cli_pin_changed"
    os.environ["LITELLM_LOCAL_MODEL_COST_MAP"] = "True"
    pilot = runpy.run_path(str(PROGRAM.with_name("rules_pilot.py")))
    rules, usage = (runpy.run_path(str(compare[key])) for key in ("LIVE_PATH", "USAGE_PATH"))
    replay = usage["replay"]
    runtime = ROOT / "runtime"
    controller = replay.load_module("learn_live_controller", runtime / "desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(runtime / "manifest.json")
    assert replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "pinned_python_required"
    runner = payload / manifest["engine"]["runner"]
    supervisor = replay.load_module("learn_live_supervisor", runner)
    suggestion, evidence = verified_suggestion(replay, args.local_evidence)
    run = compare["PROOF_ROOT"].resolve(strict=True) / ("learn-" + uuid.uuid4().hex)
    assert run.is_relative_to(ROOT), "proof_scope"
    fixture = run / "fixture"
    run.mkdir()
    pair_counter = run / "pair-attempts.txt"
    pair_counter.write_bytes(b"")
    inputs = [fixture / workdir / PATHS[0] for workdir in WORKDIRS]
    for path, value in zip(inputs, (STALE, ANSWER), strict=True):
        path.parent.mkdir(parents=True)
        replay.write_new(path, value)
    assert inspect_report(fixture, "exists", WORKDIRS[1], PATHS[0]) == {"status": "present"}
    assert inspect_report(fixture, "read", WORKDIRS[1], PATHS[0])["report"] == ANSWER
    assert inspect_report(fixture, "read", WORKDIRS[0], PATHS[0])["report"] == STALE
    assert inspect_report(fixture, "read", WORKDIRS[1], PATHS[1]) == {"status": "missing"}
    prompts = (TASK, TASK + "\nLearn suggestion: " + suggestion)
    registration = {"experiment": "learn_path_suggestion", "os": platform.platform(), "engine": manifest["engine"]["version"],
        "cli": version, "account": "isolated ChatGPT Free", "model": compare["LIVE_MODEL"], "effort": compare["LIVE_EFFORT"],
        "arms": list(ARMS), "maximum_requests_per_arm": compare["ATTEMPTS_PER_ARM"],
        "maximum_requests_per_pair": rules["ATTEMPTS_PER_PAIR"], "required_first_cache_read_tokens": 0,
        "prompt_sha256": [hashlib.sha256(prompt.encode()).hexdigest() for prompt in prompts],
        "fixture_sha256": [replay.sha(path) for path in inputs], "suggestion": suggestion, "local_evidence_sha256": replay.sha(evidence),
        "quality_gate": "exact staged file read and exact typed answer in both completed turns",
        "benefit_gate": "same quality, fewer repeats or bad reads, no increase in either, and fewer provider tokens"}
    replay.write_new(run / "registration.json", registration)
    replay.write_new(run / "answer-schema.json", SCHEMA)
    catalog_path = run / "models.json"
    rows, failure = [], None
    try:
        profile, catalog = compare["account_catalog"](cli, run, pilot, rules, supervisor)
        catalog_path.write_bytes(catalog)
        packages = payload / "Lib/site-packages"
        patched = json.loads((runtime / "optimization-sources.json").read_bytes())
        files = [PROGRAM, COMPARE, evidence, LABELS, PREVIEW, POLICY, cli, runner, Path(sys.executable),
                 payload / "python313.dll", payload / "runtime-manifest.json", packages / "mcp-2.3.0.dist-info/RECORD",
                 *[packages / "headroom" / name for name in patched], *inputs, run / "registration.json",
                 run / "answer-schema.json", catalog_path, PROGRAM.with_name("rules_pilot.py"), PROGRAM.with_name("replay.py"),
                 PROGRAM.with_name("anthropic_usage.py"), compare["LIVE_PATH"], compare["USAGE_PATH"]]
        for arm, prompt in zip(ARMS, prompts, strict=True):
            row = run_arm(arm, prompt, cli, compare, pilot, rules, usage, supervisor, controller, payload, run,
                          fixture, profile, files)
            rows.append(row)
            assert row["fingerprints"]["matched"], "fingerprints_changed"
            assert not row["guard_rejections"] and row["total_usage"] is not None, "provider_capture_incomplete"
    except Exception as error:
        failure = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__
    finally:
        catalog_path.unlink(missing_ok=True)
    sent = pair_counter.stat().st_size
    measured = (failure is None and len(rows) == 2 and sent == sum(row["forwarded_http_attempts"] for row in rows)
                and all(2 <= row["request_count"] == row["forwarded_http_attempts"] <= compare["ATTEMPTS_PER_ARM"]
                        for row in rows))
    quality = measured and all(row["turn_completed"] and row["exact_answer"] and row["exact_staged_read"]
                               and row["unverified_tool_calls"] == 0 for row in rows)
    cold = measured and all(row["first_upstream_cache_read_input_tokens"] == 0 for row in rows)
    delta = None
    if measured:
        control, candidate = rows
        baseline = control["total_usage"]["total_tokens"]
        saved = baseline - candidate["total_usage"]["total_tokens"]
        delta = {"tokens_saved": saved, "percent_saved": round(100 * saved / baseline, 4),
                 "repeats_avoided": control["repeat_calls"] - candidate["repeat_calls"],
                 "missing_or_stale_reads_avoided": control["missing_or_stale_reads"] - candidate["missing_or_stale_reads"]}
    benefit = bool(quality and cold and delta["tokens_saved"] > 0 and delta["repeats_avoided"] >= 0
                   and delta["missing_or_stale_reads_avoided"] >= 0 and
                   (delta["repeats_avoided"] + delta["missing_or_stale_reads_avoided"]) > 0)
    summary = {"failure": failure, "measured": measured, "quality_gate_passed": quality, "cold_first_requests": cold,
               "benefit_observed": benefit, "upstream_attempts": sent, "observed_delta": delta, "dollar_cost_measured": False}
    replay.write_new(run / "result.json", {**registration, **summary, "rows": rows})
    print(json.dumps({**summary, "proof": (run / "result.json").relative_to(ROOT).as_posix()}), flush=True)
    return 0 if measured and quality and cold else 1


if __name__ == "__main__":
    compare = runpy.run_path(str(COMPARE))
    if len(sys.argv) == 3 and sys.argv[1] == "--mcp":
        serve(Path(sys.argv[2]), compare["PROOF_ROOT"])
    else:
        raise SystemExit(main(compare))
