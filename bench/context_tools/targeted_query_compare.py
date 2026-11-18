"""Pair full Read and exact-ID query over one fixed report in the real Codex CLI."""
import argparse
from collections import Counter
import hashlib
from http.server import ThreadingHTTPServer
import json
import os
from pathlib import Path
import platform
from queue import Queue
import re
import runpy
import shutil
import stat
import subprocess
import sys
import threading
import time
import uuid


PROGRAM = Path(__file__).resolve()
HERE = PROGRAM.parent
ROOT = HERE.parents[1]
OPTIMIZATION = HERE.parent / "optimization"
REPLAY_PATH = HERE / "targeted_query_client_replay.py"
LIVE_PATH = OPTIMIZATION / "rules_live.py"
USAGE_PATH = ROOT / "tests/runtime/anthropic_usage_capture.py"
PROOF_ROOT = ROOT / "work/proofs/optimization/targeted-query-comparison"
SERVER = "headroom_rule_pilot"
NAMESPACE = "mcp__" + SERVER
CAPTURE, SELECT, READ = "read_structured_report", "select_report_record", "read_archived_report"
REPORT_TOOLS = (CAPTURE, SELECT, READ)
ARMS = {"full_read": (CAPTURE, READ), "targeted": (CAPTURE, SELECT, READ)}
TARGET_ID = "batch_037"
LIVE_MODEL, LIVE_EFFORT = "gpt-6-luna", "medium"
CLI_VERSION = "codex-cli 0.169.0-alpha.11.1"
ATTEMPTS_PER_ARM = 4
UPSTREAM = "https://chatgpt.com/backend-api/codex/responses"
PROMPT = (
    f"Call {CAPTURE} once. Use the archive_id it returns to find {TARGET_ID} and count all JSONL records. "
    f"If {SELECT} is available, call it with that archive_id and record_id={TARGET_ID}. "
    f"If {SELECT} is unavailable or its result is insufficient, call {READ} with the same archive_id, "
    "stream=stdout, and offset=0. Use only these report tools. "
    "Return one JSON object with id, record_count, units, ratio, and label. Preserve every digit and the sign of "
    "units and ratio as strings, keep label verbatim, and use a JSON integer for record_count."
)


def report_rows():
    for index in range(48):
        digest = hashlib.sha256(f"headroom-query-case-v1:{index:03}".encode()).hexdigest()
        units = 9007199254740993 + int(digest[:10], 16)
        ratio = f"-0.{int(digest[10:26], 16):020d}{int(digest[26:34], 16):010d}"
        row = f'{{"id":"batch_{index:03}","units":{units},"ratio":{ratio},"label":"Kuyruk {digest[34:46]} — λ"}}'
        yield row.encode("utf-8")


ROWS = tuple(report_rows())
REPORT = b"".join(row + (b"\r\n" if index % 2 else b"\n") if index < len(ROWS) - 1 else row
                  for index, row in enumerate(ROWS))
RECORD = next(row for row in REPORT.splitlines(keepends=True) if json.loads(row)["id"] == TARGET_ID)
ANSWER = {**json.loads(RECORD, parse_int=str, parse_float=str), "record_count": len(ROWS)}
SCHEMA = {"type": "object", "properties": {key: {"type": "integer" if type(value) is int else "string"}
    for key, value in ANSWER.items()}, "required": sorted(ANSWER), "additionalProperties": False}


def exact_answer(message):
    try:
        value = json.loads(message)
    except (TypeError, ValueError, RecursionError):
        return False
    return isinstance(value, dict) and value.keys() == ANSWER.keys() and all(
        type(value[key]) is type(wanted) and value[key] == wanted for key, wanted in ANSWER.items())


def read_args(reference):
    return {"archive_id": reference["archive_id"], "stream": "stdout", "offset": 0}


def select_args(reference):
    return {"archive_id": reference["archive_id"], "record_id": TARGET_ID}


def valid_reference(value):
    return (isinstance(value, dict) and set(value) == {"archive_id", "source_tool", "source_call_id"}
            and isinstance(value["archive_id"], str) and re.fullmatch(r"[a-f0-9]{32}", value["archive_id"]) is not None
            and value["source_tool"] == CAPTURE and isinstance(value["source_call_id"], str)
            and 4 < len(value["source_call_id"]) <= 128 and value["source_call_id"].startswith("mcp:"))


def wire_tool(entry, namespace=None):
    name = entry.get("name")
    if not isinstance(name, str):
        return "other"
    scope = namespace or entry.get("namespace")
    if entry.get("type") in ("custom", "custom_tool_call") and (
            name == "functions.exec" or (name == "exec" and scope == "functions")):
        return "functions.exec"
    prefix = NAMESPACE + "__"
    if name.startswith(prefix) and namespace is None:
        name = name[len(prefix):]
    elif scope != NAMESPACE:
        return "other"
    return name if name in REPORT_TOOLS else "other"


def intact_reference(output):
    if isinstance(output, list):
        output = "".join(part["text"] for part in output if isinstance(part, dict)
                         and part.get("type") in ("text", "input_text") and isinstance(part.get("text"), str))
    if not isinstance(output, str):
        return False
    output = re.sub(r"\AWall time: [0-9]+(?:\.[0-9]+)? seconds\r?\nOutput:\s*", "", output, count=1)
    try:
        return valid_reference(json.loads(output))
    except (ValueError, RecursionError):
        return False


def wire_observation(body):
    records = [item for item in body["input"] if isinstance(item, dict)] if isinstance(body.get("input"), list) else []
    declarations = list(body.get("tools") or [])
    for item in records:
        if item.get("type") == "additional_tools" and isinstance(item.get("tools"), list):
            declarations.extend(item["tools"])
    advertised = set()
    for entry in declarations:
        if not isinstance(entry, dict):
            continue
        if entry.get("type") == "namespace" and entry.get("name") in (NAMESPACE, "functions"):
            namespace = entry["name"]
            labels = {wire_tool(tool, namespace) for tool in entry.get("tools") or [] if isinstance(tool, dict)}
            advertised.update(labels if namespace == NAMESPACE else labels & {"functions.exec"})
        elif entry.get("type") in ("function", "custom"):
            label = wire_tool(entry)
            if label != "other":
                advertised.add(label)
    calls = {item["call_id"]: wire_tool(item) for item in records
             if item.get("type") in ("function_call", "custom_tool_call") and isinstance(item.get("call_id"), str)}
    outputs, intact = Counter(), 0
    for item in records:
        if item.get("type") in ("function_call_output", "custom_tool_call_output") and isinstance(item.get("call_id"), str):
            label = calls.get(item["call_id"], "unmatched")
            outputs[label] += 1
            intact += label == CAPTURE and intact_reference(item.get("output"))
    choice = body.get("tool_choice")
    return {"advertised": sorted(advertised.intersection(REPORT_TOOLS)), "calls": dict(Counter(calls.values())),
            "outputs": dict(outputs), "intact_archive_references": intact,
            "unexpected_namespaced_tool": "other" in advertised, "code_mode_exec": "functions.exec" in advertised,
            "tool_choice": choice if choice in ("auto", "none", "required") else "other"}


def observe_guarded_requests(counter):
    import httpx

    send = httpx.AsyncClient.send
    trace = Path(counter).with_name("wire-summary.jsonl")

    async def observed(client, request, **options):
        # The outer guard admits only bounded JSON model requests to the selected target.
        row = wire_observation(json.loads(request.content))
        with trace.open("a", encoding="ascii") as stream:
            stream.write(json.dumps(row) + "\n")
        return await send(client, request, **options)

    httpx.AsyncClient.send = observed


def delivered(tool, reference, value):
    if tool == SELECT:
        return value == {**reference, "status": "selected", "stdout": RECORD.decode(), "stderr": "", "exit_code": 0,
                         "record_count": len(ROWS), "matched_count": 1, "truncated": False}
    return value == {**reference, "stream": "stdout", "offset": 0, "total_bytes": len(REPORT),
                     "next_offset": len(REPORT), "exit_code": 0, "text": REPORT.decode()}


def fixture_call(tool, args, turn, *, code_mode):
    assert tool in REPORT_TOOLS, "fixture_tool_outside_scope"
    call_id = f"call_query_{turn}"
    arguments = json.dumps(args, ensure_ascii=True, separators=(",", ":"))
    if code_mode:
        return {"type": "custom_tool_call", "id": f"fc_query_{turn}", "call_id": call_id,
                "name": "exec", "namespace": "functions",
                "input": f"const report{turn} = await tools.{NAMESPACE}__{tool}({arguments}); text(report{turn});"}
    return {"type": "function_call", "id": f"fc_query_{turn}", "call_id": call_id,
            "name": tool, "namespace": NAMESPACE, "arguments": arguments}


def fixture_provider(client_replay, *, code_mode=False, force_readback=False):
    pilot = client_replay["pilot"]
    sequences = {**ARMS, "targeted": (CAPTURE, SELECT)} if code_mode and not force_readback else ARMS

    class Provider(client_replay["Provider"]):
        turn_counts = {arm: len(tools) + 1 for arm, tools in sequences.items()}
        render_sse = staticmethod(pilot["Provider"].render_sse)

        def next_item(self, body, turn):
            server = self.server
            expected = sequences[server.scenario]
            wire = wire_observation(body)
            assert (wire["code_mode_exec"] if code_mode else wire["advertised"] == sorted(ARMS[server.scenario])), "tool_catalog_scope_changed"
            assert body.get("text", {}).get("format", {}).get("schema") == SCHEMA, "answer_schema_wire_mismatch"
            if turn > 1:
                value = json.loads(client_replay["tool_text"](body, turn - 1, code_mode=code_mode))
                if code_mode and isinstance(value, dict) and "content" in value:
                    assert not value.get("isError"), "fixture_code_mode_tool_error"
                    value = json.loads(value["content"][0]["text"])
                if turn == 2:
                    assert valid_reference(value), "fixture_reference_invalid"
                    server.reference = value
                else:
                    text = value.get("text", "") if isinstance(value, dict) else ""
                    server.last_delivery = {"text_bytes": len(text.encode()) if isinstance(text, str) else None,
                        "ccr_marker": "<<ccr:" in json.dumps(value, ensure_ascii=True),
                        "stdout_equals_original": text == REPORT.decode() if isinstance(text, str) else False}
                    assert delivered(expected[turn - 2], server.reference, value), "fixture_tool_bytes_changed"
            if turn > len(expected):
                return {"type": "message", "id": "msg_query_pair", "role": "assistant", "status": "completed",
                        "content": [{"type": "output_text", "text": pilot["encode"](ANSWER), "annotations": []}]}
            tool = expected[turn - 1]
            args = {} if tool == CAPTURE else (read_args(server.reference) if tool == READ else select_args(server.reference))
            return fixture_call(tool, args, turn, code_mode=code_mode)

    return Provider


def inspect_turn(stdout, exit_code, arm, reported_error):
    events = [json.loads(line) for line in stdout.splitlines() if line.startswith(b"{")]
    kinds = ("item.started", "item.completed", "item.failed", "turn.completed", "turn.failed", "error")
    event_counts = {kind: sum(event.get("type") == kind for event in events) for kind in kinds}
    items = [event["item"] for event in events if event.get("type") == "item.completed"]
    messages = [item["text"] for item in items if item.get("type") == "agent_message"]
    calls = [item for item in items if item.get("type") == "mcp_tool_call"]
    failed_mcp_events = sum(item.get("status") == "failed" or reported_error(item) for item in calls)
    failed_mcp_events += sum(event.get("type") == "item.failed" and event.get("item", {}).get("type") == "mcp_tool_call"
                            for event in events)
    expected = ARMS[arm]
    tools = [item.get("tool") for item in calls]
    valid_sequence = tools == list(expected) or (arm == "targeted" and tools == list(expected[:-1]))
    exact_delivery = valid_sequence and all(item.get("server") == SERVER and item.get("status") == "completed"
        and not reported_error(item) for item in calls)
    if exact_delivery:
        try:
            values = [json.loads(item["result"]["content"][0]["text"]) for item in calls]
            reference = values[0]
            exact_delivery = valid_reference(reference) and calls[0]["arguments"] == {} and all(
                item["arguments"] == (read_args(reference) if tool == READ else select_args(reference))
                and delivered(tool, reference, value) for item, tool, value in zip(calls[1:], tools[1:], values[1:]))
        except (KeyError, IndexError, TypeError, ValueError, AttributeError):
            exact_delivery = False
    return {"turn_completed": exit_code == 0 and event_counts["turn.completed"] == 1
            and not (event_counts["error"] or event_counts["turn.failed"]),
            "exact_answer": bool(messages) and exact_answer(messages[-1]), "tool_delivery_exact": exact_delivery,
            "call_count": len(calls), "read_calls": tools.count(READ), "query_calls": tools.count(SELECT),
            "event_counts": event_counts, "failed_mcp_events": failed_mcp_events}


def accepted(row):
    usage = row["total_usage"] or {}
    counts = [usage.get(key) for key in ("total_input_tokens", "output_tokens", "cache_read_input_tokens", "total_tokens")]
    first_cache_read = row["first_upstream_cache_read_input_tokens"]
    return (row["turn_completed"] and row["exact_answer"] and row["tool_delivery_exact"]
            and type(first_cache_read) is int and first_cache_read == 0
            and 3 <= row["request_count"] == row["forwarded_http_attempts"] <= ATTEMPTS_PER_ARM
            and all(type(value) is int and value >= 0 for value in counts)
            and 0 < counts[0] + counts[1] == counts[3] and counts[2] <= counts[0])


def remove_client_state(directory):
    assert directory.resolve(strict=True) == directory and directory.is_relative_to(PROOF_ROOT), "client_state_scope"
    for child in directory.rglob("*"):
        flags = child.lstat().st_file_attributes
        assert child.resolve().is_relative_to(directory) and not flags & stat.FILE_ATTRIBUTE_REPARSE_POINT, "client_state_reparse"
        if child.is_file() and flags & stat.FILE_ATTRIBUTE_READONLY:
            child.chmod(stat.S_IWRITE)
    shutil.rmtree(directory)


def account_catalog(cli, run, pilot, rules, supervisor):
    """Read account metadata from the pre-existing Headroom-owned test profile."""
    marker = json.loads((ROOT / "work/proofs/codex-current/active-desktop-live-login.json").read_bytes())
    owned = Path(os.environ["LOCALAPPDATA"]).resolve(strict=True) / "dev.headroom.desktop/desktop-live-proof"
    profile = Path(marker["profile"]).resolve(strict=True)
    assert marker["status"] == "authenticated" and profile.parent == owned and profile.name == marker["run"], "owned_login_required"
    auth_file = profile / "auth.json"
    assert (str(uuid.UUID(profile.name)) == profile.name and auth_file.is_file()
            and not auth_file.lstat().st_file_attributes & stat.FILE_ATTRIBUTE_REPARSE_POINT), "owned_login_required"
    state = run / "account-state"
    state.mkdir()
    env = pilot["client_environment"]()
    env.update(CODEX_HOME=str(profile), CODEX_SQLITE_HOME=str(state), TEMP=str(state), TMP=str(state))
    command = [str(cli), "-c", 'cli_auth_credentials_store="file"', "-c", "log_dir=" + pilot["encode"](str(state))]
    job = supervisor.KillOnCloseJob()
    process = None
    try:
        process = subprocess.Popen([*command, "app-server"], cwd=state, env=env, stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, creationflags=subprocess.CREATE_NO_WINDOW)
        job.assign(process.pid)
        output = Queue()

        def read_lines():
            for line in process.stdout:
                output.put(line)
            output.put(b"")

        threading.Thread(target=read_lines, daemon=True).start()

        def rpc(message):
            process.stdin.write(json.dumps(message).encode() + b"\n")
            process.stdin.flush()
            deadline = time.monotonic() + 10
            for _ in range(25):
                line = output.get(timeout=max(0, deadline - time.monotonic()))
                assert line, "account_rpc_closed"
                result = json.loads(line)
                if result.get("id") == message["id"]:
                    assert "result" in result, "account_rpc_failed"
                    return result["result"]
            raise RuntimeError("account_rpc_missing")

        rpc({"id": 1, "method": "initialize", "params": {"clientInfo": {"name": "headroom_query_compare", "version": "1"}}})
        process.stdin.write(b'{"method":"initialized"}\n')
        process.stdin.flush()
        account = rpc({"id": 2, "method": "account/read", "params": {"refreshToken": False}}).get("account") or {}
        assert account.get("type") == "chatgpt" and account.get("planType") == "free", "free_chatgpt_login_required"
        process.stdin.close()
        process.wait(timeout=8)
        catalog = subprocess.run([*command, "debug", "models"], cwd=state, env=env, capture_output=True, timeout=20, check=True).stdout
        assert rules["account_catalog_allows"](catalog, LIVE_MODEL, LIVE_EFFORT), "matched_model_unavailable"
        return profile, catalog
    finally:
        job.close()
        if process is not None and process.poll() is None:
            process.wait(timeout=10)
        remove_client_state(state)


def run_arm(arm, engine, directory, cli, supervisor, pilot, rules, counter, usage, fixture, profile):
    transient = directory / "client"
    work, temp = transient / "workspace", transient / "temp"
    work.mkdir(parents=True)
    temp.mkdir()
    run = directory.parent
    catalog = run / "models.json"
    model, effort = (LIVE_MODEL, LIVE_EFFORT) if catalog.is_file() else (pilot["MODEL"], "high")
    env = pilot["client_environment"]()
    env.update(TEMP=str(temp), TMP=str(temp), CODEX_SQLITE_HOME=str(transient / "state"))
    config = {"model_provider": "pilot", "model_providers.pilot.name": "Headroom Query Pilot",
        "model_providers.pilot.base_url": f"http://127.0.0.1:{engine.port}/v1", "model_providers.pilot.wire_api": "responses",
        "model_providers.pilot.supports_websockets": False, "model_providers.pilot.requires_openai_auth": fixture is None,
        "model_reasoning_effort": effort, "log_dir": str(transient / "logs"), "project_doc_max_bytes": 0,
        "features.memories": False, "features.multi_agent": False, "features.multi_agent_v2": False,
        f"mcp_servers.{SERVER}.args": ["-I", "-B", str(PROGRAM), "--mcp"], f"mcp_servers.{SERVER}.enabled_tools": list(ARMS[arm])}
    if fixture is None:
        env["CODEX_HOME"] = str(profile)
        config["cli_auth_credentials_store"] = "file"
    else:
        home = transient / "home"
        home.mkdir()
        env.update(CODEX_HOME=str(home), RULES_PILOT_TOKEN=fixture.auth.removeprefix("Bearer "))
        config["model_providers.pilot.env_key"] = "RULES_PILOT_TOKEN"
        fixture.scenario, fixture.requests, fixture.failure = arm, [], None
        fixture.last_delivery = None
    if catalog.is_file():
        config["model_catalog_json"] = str(catalog)
    command = pilot["client_args"](cli, work, "counts", arm, model, config, prompt=PROMPT,
        output_schema=run / "answer-schema.json")
    try:
        exit_code, stdout = pilot["run_client"](command, work, env, supervisor, 100)
    except subprocess.TimeoutExpired:
        exit_code, stdout = -1, b""
    finally:
        remove_client_state(transient)
    sent = counter.read_bytes().count(b"\n") if counter.exists() else 0
    trace = counter.with_name("wire-summary.jsonl")
    wire = [json.loads(line) for line in trace.read_bytes().splitlines()] if trace.is_file() else []
    # Code mode advertises its wrapper, while inspect_turn verifies the actual scoped MCP calls.
    wire_transport = bool(wire) and len(wire) == sent and all(not entry["unexpected_namespaced_tool"] and (
        entry["advertised"] == sorted(ARMS[arm]) or (entry["code_mode_exec"] and not entry["advertised"])) for entry in wire)
    row = {"arm": arm, "cli_exit": exit_code, "forwarded_http_attempts": sent,
        "wire": wire, "forwarded_tool_transport_recognized": wire_transport,
        **inspect_turn(stdout, exit_code, arm, rules["tool_reported_error"]), **rules["measure"](engine, usage, sent)}
    if fixture is not None:
        row["fixture_failure"] = fixture.failure
        row["fixture_payloads_exact"] = fixture.failure is None and len(fixture.requests) == fixture.RequestHandlerClass.turn_counts[arm]
        if fixture.failure:
            row["fixture_failure_site"] = getattr(fixture, "failure_site", None)
            row["fixture_parse_failure"] = getattr(fixture, "parse_failure", None)
            row["fixture_last_delivery"] = fixture.last_delivery
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", type=Path, required=True)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--live", action="store_true", help="One capped pair using the existing isolated Free ChatGPT profile")
    mode.add_argument("--code-mode-catalog", type=Path, help="Replay a previous isolated model catalog on loopback")
    parser.add_argument("--code-mode-readback", action="store_true", help="Rehearse the targeted arm with a full-read fallback on loopback")
    args = parser.parse_args()
    code_mode = args.code_mode_catalog is not None
    if args.code_mode_readback and not code_mode:
        parser.error("--code-mode-readback requires --code-mode-catalog")
    assert os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace"
    cli = args.codex.resolve(strict=True)
    app_bin = (Path(os.environ["LOCALAPPDATA"]) / "OpenAI/Codex/bin").resolve(strict=True)
    assert cli.name == "codex.exe" and cli.is_relative_to(app_bin), "cli_scope"
    version = subprocess.run([str(cli), "--version"], capture_output=True, timeout=10, check=True).stdout.decode().strip()
    assert version == CLI_VERSION, "cli_pin_changed"
    os.environ["LITELLM_LOCAL_MODEL_COST_MAP"] = "True"
    client_replay = runpy.run_path(str(REPLAY_PATH))
    pilot, rules = client_replay["pilot"], runpy.run_path(str(LIVE_PATH))
    usage = runpy.run_path(str(USAGE_PATH))
    replay = usage["replay"]
    runtime = ROOT / "runtime"
    controller = replay.load_module("query_compare_controller", runtime / "desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(runtime / "manifest.json")
    assert replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "python_pin_changed"
    runner = payload / manifest["engine"]["runner"]
    supervisor = replay.load_module("query_compare_supervisor", runner)
    run = PROOF_ROOT / uuid.uuid4().hex
    assert run.resolve().is_relative_to(ROOT), "proof_scope"
    run.mkdir(parents=True)
    pair_counter = run / "pair-attempts.txt"
    pair_counter.write_bytes(b"")
    replay.write_new(run / "answer-schema.json", SCHEMA)
    arms = ("targeted",) if args.code_mode_readback else tuple(ARMS)
    protect_exec = args.live or code_mode
    model, effort = (LIVE_MODEL, LIVE_EFFORT) if args.live or code_mode else (pilot["MODEL"], "high")
    registration = {"mode": "live" if args.live else "code_mode_loopback" if code_mode else "loopback",
        "cli": version, "engine": manifest["engine"]["version"],
        "os": platform.platform(), "model": model, "effort": effort, "arm_order": list(arms), "attempts_per_arm": ATTEMPTS_PER_ARM,
        "attempts_per_pair": rules["ATTEMPTS_PER_PAIR"], "first_upstream_cache_read": 0, "report_bytes": len(REPORT),
        "report_sha256": hashlib.sha256(REPORT).hexdigest(), "report_rows": len(ROWS), "default_cli_output_budget": True,
        "report_tools_by_arm": ARMS, "protect_code_mode_outputs": protect_exec, "force_readback": args.code_mode_readback,
        "gate": "exact answer and tool delivery, cold first requests, all attempts measured"}
    replay.write_new(run / "registration.json", registration)
    fixture, profile, rows, fingerprints, failure = None, None, [], {}, None
    try:
        if args.live:
            profile, catalog = account_catalog(cli, run, pilot, rules, supervisor)
            (run / "models.json").write_bytes(catalog)
            target, upstream_port = UPSTREAM, 1
        else:
            if code_mode:
                source_catalog = args.code_mode_catalog.resolve(strict=True)
                assert source_catalog.name == "models.json" and source_catalog.parent.parent == PROOF_ROOT, "catalog_scope"
                source_run = json.loads(source_catalog.with_name("registration.json").read_bytes())
                assert source_run["mode"] == "live" and source_run["model"] == model and source_run["cli"] == version, "catalog_origin"
                catalog = source_catalog.read_bytes()
                assert rules["account_catalog_allows"](catalog, model, effort), "catalog_model_effort"
                assert any(row.get("slug") == model and row.get("tool_mode") == "code_mode_only" and row.get("use_responses_lite") is True
                           for row in json.loads(catalog)["models"]), "catalog_code_mode"
                (run / "models.json").write_bytes(catalog)
            pilot["prepare_provider"]()
            provider = fixture_provider(client_replay, code_mode=code_mode, force_readback=args.code_mode_readback)
            fixture = ThreadingHTTPServer(("127.0.0.1", 0), provider)
            fixture.auth, fixture.total_requests, fixture.output_part_types = "Bearer " + replay.SENTINEL, 0, set()
            fixture.fixture_model = model
            upstream_port = fixture.server_port
            target = f"http://127.0.0.1:{upstream_port}/v1/responses"
            threading.Thread(target=fixture.serve_forever, daemon=True).start()
        for arm in arms:
            directory = run / arm
            directory.mkdir()
            staged, counter = rules["stage_arm"](replay, directory, payload, target, model, effort, ATTEMPTS_PER_ARM)
            helper = directory / "fixture-runtime/desktop_helper.py"
            source = helper.read_bytes()
            anchor = b'"import optimization_control; "'
            assert source.count(anchor) == 1, "engine_bootstrap_changed"
            observe = f"import runpy; runpy.run_path({str(PROGRAM)!r})['observe_guarded_requests']({str(counter)!r}); "
            helper.write_bytes(source.replace(anchor, repr(observe).encode("ascii") + b" " + anchor, 1))
            patched = json.loads((runtime / "optimization-sources.json").read_bytes())
            files = [PROGRAM, REPLAY_PATH, LIVE_PATH, USAGE_PATH, cli, runner, Path(sys.executable), payload / "python313.dll",
                payload / "runtime-manifest.json", runtime / "manifest.json", run / "answer-schema.json", run / "registration.json",
                *staged, *[runtime / path.name for path in staged],
                *[HERE / name for name in ("targeted_query.py", "targeted_query_replay.py", "targeted_query_client.py")],
                *[OPTIMIZATION / name for name in ("rules_pilot.py", "replay.py", "anthropic_usage.py", "ccr_policy.py")],
                payload / "Lib/site-packages/mcp-2.3.0.dist-info/RECORD",
                *[payload / "Lib/site-packages/headroom" / name for name in patched]]
            if args.live or code_mode:
                files.append(run / "models.json")
            before = {str(path): replay.sha(path) for path in files}
            engine = usage["probe"]["UsageEngine"](controller, supervisor, payload, directory / "engine", upstream_port, pilot["PLAN"])
            protected = [tool for name in REPORT_TOOLS for tool in (name, NAMESPACE + "__" + name)]
            if protect_exec:
                # The pinned engine sees the wrapper name, so keep report output intact in this benchmark.
                protected.extend(("exec", "functions.exec"))
            engine.environment["HEADROOM_DESKTOP_PROTECTED_TOOLS"] = json.dumps(controller.validate_protected_tools(sorted(protected)))
            rejections = directory / "guard-rejections.txt"
            try:
                engine.start()
                assert pilot["usage_ids"](engine, usage) == (set(), set()), "engine_not_empty"
                startup_rejections = rejections.read_text(encoding="ascii") if rejections.is_file() else ""
                row = run_arm(arm, engine, directory, cli, supervisor, pilot, rules, counter, usage, fixture, profile)
                row["blocked_during_startup"] = startup_rejections.splitlines()
                rows.append(row)
            finally:
                engine.close()
                after = {str(path): replay.sha(path) for path in files}
                fingerprints[arm] = {"before": before, "after": after, "matched": before == after}
            row["stop"] = engine.stopped
            assert before == after, "fingerprints_changed"
            assert fixture is None or row["fixture_payloads_exact"], "loopback_rehearsal_failed"
            assert row["forwarded_tool_transport_recognized"], "forwarded_tool_transport_unknown"
            denied = rejections.read_text(encoding="ascii") if rejections.is_file() else ""
            assert denied == startup_rejections, "client_request_rejected_by_guard"
            assert accepted(row), "pair_acceptance_failed"
    except Exception as error:
        failure = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__.lower()
    finally:
        if fixture is not None:
            fixture.shutdown()
            fixture.server_close()
    attempts = pair_counter.stat().st_size
    passed = failure is None and len(rows) == len(arms) and attempts == sum(row["forwarded_http_attempts"] for row in rows)
    savings = None
    if passed and args.live:
        baseline, candidate = (row["total_usage"]["total_tokens"] for row in rows)
        savings = {"tokens": baseline - candidate, "percent": round(100 * (baseline - candidate) / baseline, 2)}
    replay.write_new(run / "result.json", {**registration, "passed": passed, "failure": failure, "rows": rows,
        "fingerprints": fingerprints, "upstream_attempts": attempts, "provider_token_delta": savings,
        "dollar_cost_measured": False, "native_files_or_subagents_verified": False})
    print(pilot["encode"]({"passed": passed, "failure": failure, "upstream_attempts": attempts, "provider_token_delta": savings,
        "proof": (run / "result.json").relative_to(ROOT).as_posix()}), flush=True)
    return 0 if passed else 1


if __name__ == "__main__":
    if sys.argv[1:] == ["--mcp"]:
        runpy.run_path(str(HERE / "targeted_query_client.py"))["serve"](source=(REPORT, b"", 0), read_tool=READ)
    else:
        raise SystemExit(main())
