"""Exercise an isolated Codex MCP producer and the owned engine usage ledger."""
import argparse
import hashlib
from http.server import ThreadingHTTPServer
from http.client import HTTPConnection
import json
import os
from pathlib import Path
import platform
import re
import runpy
import stat
import struct
import subprocess
import sys
import threading
import time
import uuid

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
PROGRAM = Path(__file__).resolve()
NAMESPACE = "mcp__headroom_rule_pilot"
TOOL = "inspect_result"
CALL = "call_rule_pilot"
RECOVERY_TOOL = "headroom_retrieve"
RECOVERY_CALL = "call_rule_pilot_recover"
MODEL = "local-protocol-fixture"
FIELDS = ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens", "output_tokens")
FIXTURE_COUNTERS = ((913, 0, 3), (941, 511, 17))
CLOSURE_MODES = ("read-to-eof", "close-after-terminal", "close-after-terminal-no-status",
                 "close-after-terminal-nonstandard-type")
SETTLED = "INFO check=pass exact_limit=9223372036854775859\r\n"
HELD = "WARN check=held signed_factor=-0.062500000000001\r\n"
CASES = {
    "counts": (SETTLED * 37 + HELD * 8 + SETTLED * 11, "", 0,
               "Compute passed as the total number of stdout records with check=pass and held as the total with check=held. "
               "Calculate the totals programmatically from the returned text, summing separated runs. In code mode, use JavaScript. "
               "A repeat marker gives the total occurrences of the preceding line in that run. "
               "Report passed and held as JSON integers. Report exact_limit and signed_factor as strings, preserving all digits and signs without numeric conversion.",
               {"passed": 48, "held": 8, "exact_limit": "9223372036854775859", "signed_factor": "-0.062500000000001"}),
    "failed_check": (SETTLED * 19, "ERROR check=checksum computed=0.512340000000009\r\n", 9,
                     "Report exit_code and the exact stderr line as JSON.",
                     {"exit_code": 9, "stderr": "ERROR check=checksum computed=0.512340000000009\r\n"}),
}
PLAN = {"thresholds": {"startup_timeout_seconds": 120}}
HELDOUT_PATH = HERE / "rules-heldout-plan.json"
HELDOUT_PLAN = json.loads(HELDOUT_PATH.read_bytes())
HELDOUT_CASES = {item["id"]: item for item in HELDOUT_PLAN["cases"]}
JSON_PATH = HERE / "rules-json-heldout-plan.json"
JSON_PLAN = json.loads(JSON_PATH.read_bytes())
JSON_CASES = {item["id"]: item for item in JSON_PLAN["cases"]}


def encode(value):
    return json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"))


def case_details(case):
    return JSON_CASES.get(case) or HELDOUT_CASES.get(case)


def case_plan(case):
    return JSON_PLAN if case in JSON_CASES else HELDOUT_PLAN if case in HELDOUT_CASES else None


def case_data(case):
    if case in CASES:
        return CASES[case]
    item = case_details(case)
    stdout = "".join(run["text"] * run["occurrences"] for run in item["stdout_runs"])
    capture = {"stdout": stdout, "stderr": item["stderr"], "exit_code": item["exit_code"]}
    assert hashlib.sha256(encode(capture).encode()).hexdigest() == item["capture_sha256"], "heldout_capture_changed"
    return stdout, item["stderr"], item["exit_code"], item["instruction"], item["expected"]


def envelope(case, arm):
    from headroom.transforms.lossless_compaction import collapse_runs, expand_runs

    stdout, stderr, exit_code, _, _ = case_data(case)
    details = case_details(case)
    capture = {"stdout": stdout, "stderr": stderr, "exit_code": exit_code}
    records = [{"type": "session_meta", "payload": {"id": "owned-mcp-probe"}}]
    for item in ({"type": "function_call", "call_id": CALL, "name": details["source_tool"] if details else "exec_command",
                  "arguments": encode(details["source_arguments"] if details else {"cmd": "run fixed probe", "workdir": "."})},
                 {"type": "function_call_output", "call_id": CALL, "output": capture}):
        records.append({"type": "response_item", "payload": item})
    raw = ("\n".join(map(encode, records)) + "\n").encode()
    if case in JSON_CASES:
        trial = runpy.run_path(str(HERE / "rules_runs_json.py"))["trial_capture"](raw, "owned-mcp-probe", CALL)
        assert trial["status"] == details["expected_guard_status"], "json_capture_guard_changed"
        return trial["candidate"] if arm == "candidate" else capture
    result = runpy.run_path(str(ROOT / "runtime/learn_preview.py"))["local_replay"](raw)
    assert result["status"] == "trial_ready", "rule_report_missing"
    report = result["report"]
    expected = int(details["rule_expected_eligible"] if details else case == "counts")
    assert (report["compacted"], report["protected"]) == (expected, 1 - expected), "rule_eligibility_changed"
    assert report["original_sha256"] == hashlib.sha256(raw).hexdigest(), "capture_fingerprint_changed"
    if arm == "candidate" and expected:
        compacted = collapse_runs(stdout)
        assert expand_runs(compacted) == stdout and report["after_bytes"] == len(compacted.encode()), "restore_changed"
        return {**capture, "stdout": compacted}
    return capture


def recovery_capture(case, arm):
    if case in JSON_CASES:
        stdout, stderr, exit_code, _, _ = case_data(case)
        return {"stdout": stdout, "stderr": stderr, "exit_code": exit_code}
    return envelope(case, arm)


def serve(case, arm, retrieval_port, resource_report=None):
    from mcp.server.mcpserver import MCPServer
    from mcp.types import ToolAnnotations

    producer = runpy.run_path(str(ROOT / "bench/context_tools/targeted_query_replay.py"))["PRODUCER"]
    stdout, stderr, exit_code, _, _ = case_data(case)
    raw_stdout, raw_stderr = stdout.encode(), stderr.encode()
    captured = subprocess.run([sys.executable, "-I", "-B", "-c", producer],
        env={"SystemRoot": os.environ["SystemRoot"]}, cwd=Path.cwd(), capture_output=True, timeout=10,
        input=struct.pack(">II", len(raw_stdout), exit_code) + raw_stdout + raw_stderr,
        creationflags=subprocess.CREATE_NO_WINDOW)
    assert (captured.stdout, captured.stderr, captured.returncode) == (raw_stdout, raw_stderr, exit_code), "channel_contract_changed"
    started_cpu = time.process_time()
    capture = envelope(case, arm)
    value = encode(capture)
    recovery_value = encode(recovery_capture(case, arm)) if case in JSON_CASES else value
    prepare_cpu_ms = 1000 * (time.process_time() - started_cpu)
    if resource_report is not None:
        destination = resource_report.resolve()
        base = (ROOT / "work/proofs/optimization/rules-live").resolve(strict=True)
        assert destination.name == "rule-resources.jsonl" and destination.parent.name == arm
        assert destination.parent.parent.parent == base and re.fullmatch(r"[a-f0-9]{32}", destination.parent.parent.name)
        assert not destination.exists() or destination.stat().st_size <= 16 * 1024, "rule_resource_report_limit"
        with destination.open("a", encoding="ascii") as stream:
            stream.write(encode({"rule_prepare_cpu_ms": prepare_cpu_ms}) + "\n")
    server = MCPServer("headroom_rule_pilot")
    read_only = ToolAnnotations(read_only_hint=True, destructive_hint=False, idempotent_hint=True, open_world_hint=False)

    @server.tool(structured_output=False, annotations=read_only)
    def inspect_result() -> str:
        """Read the fixed test result. A repeat marker counts all occurrences of the preceding line."""
        return value

    if retrieval_port is not None:
        @server.tool(structured_output=False, annotations=read_only)
        def headroom_retrieve(content_hash: str) -> str:
            """Read only this test's fixed result through its Headroom reference hash."""
            assert re.fullmatch(r"[a-f0-9]{12,24}", content_hash), "invalid_recovery_reference"
            replay = runpy.run_path(str(HERE / "replay.py"))
            replay["retrieval"](retrieval_port, content_hash, capture["stdout"], 10)
            return recovery_value

    server.run(transport="stdio")


class Provider(runpy.run_path(str(HERE / "replay.py"))["FixtureHandler"]):
    render_sse = None

    def do_POST(self):
        assert callable(self.render_sse), "fixture_renderer_not_prepared"

        size = int(self.headers.get("Content-Length", "0"))
        server = self.server
        if self.path != "/v1/responses" or not 0 < size <= 2 * 1024 * 1024 or self.headers.get("Authorization") != server.auth:
            self.send_error(400)
            return
        request = json.loads(self.rfile.read(size))
        server.requests.append(request)
        turn = len(server.requests)
        recovery = getattr(server, "recovery_arm", None)
        marker = None
        if recovery is not None and turn == 2:
            outputs = [item["output"] for item in request["input"]
                       if item.get("type") == "function_call_output" and item.get("call_id") == CALL]
            marker = re.search(r"<<ccr:([a-f0-9]{12,24}),string,[^>]+>>", encode(outputs))
            forced = getattr(server, "force_recovery_hash", None)
            if forced is not None:
                assert re.fullmatch(r"[a-f0-9]{12,24}", forced), "invalid_fixture_recovery_hash"
                marker = (None, forced)
        proof_case = self.headers.get("X-Proof-Case")
        close_probe = proof_case in CLOSURE_MODES
        if close_probe and turn == 1:
            item = {"type": "message", "id": "msg_closure", "role": "assistant", "status": "completed",
                    "content": [{"type": "output_text", "text": "FIXTURE_OK", "annotations": []}]}
        elif turn == 1:
            actual = [tool for schema in request.get("tools", [])
                      if (schema.get("type"), schema.get("name")) == ("namespace", NAMESPACE)
                      for tool in schema.get("tools", []) if (tool.get("type"), tool.get("name")) == ("function", TOOL)]
            if len(actual) != 1 or actual[0].get("parameters", {}).get("properties") != {}:
                server.failure = "registered_mcp_tool_missing"
                self.send_error(400)
                return
            item = {"type": "function_call", "id": "fc_rule_pilot", "call_id": CALL, "name": TOOL,
                    "namespace": NAMESPACE, "arguments": "{}"}
        elif marker is not None:
            item = {"type": "function_call", "id": "fc_rule_pilot_recover", "call_id": RECOVERY_CALL,
                    "name": RECOVERY_TOOL, "namespace": NAMESPACE, "arguments": encode({"content_hash": marker[1]})}
        elif turn == 2 or recovery is not None and turn == 3:
            if turn == 3:
                outputs = [part.get("text") for item in request["input"]
                           if item.get("type") == "function_call_output" and item.get("call_id") == RECOVERY_CALL
                           for part in item["output"]]
                if encode(recovery_capture(server.case, recovery)) not in outputs:
                    server.failure = "recovered_tool_output_changed"
                    self.send_error(400)
                    return
            item = {"type": "message", "id": "msg_rule_pilot", "role": "assistant", "status": "completed",
                    "content": [{"type": "output_text", "text": encode(case_data(server.case)[4]), "annotations": []}]}
        else:
            server.failure = "unexpected_model_request"
            self.send_error(400)
            return
        input_tokens, cached, output_tokens = (*FIXTURE_COUNTERS, (1063, 600, 31))[turn - 1]
        if turn == 1 and getattr(server, "cache_start", None) == "warm":
            cached = 512
        input_details = {"cached_tokens": cached}
        if case_plan(server.case) is not None:
            input_details["cache_write_tokens"] = 0
        response = {"id": "resp_pilot_" + str(turn), "object": "response", "status": "completed",
                    "model": MODEL, "output": [item], "usage": {"input_tokens": input_tokens,
                        "input_tokens_details": input_details, "output_tokens": output_tokens,
                        "total_tokens": input_tokens + output_tokens}}
        if proof_case == "close-after-terminal-no-status":
            del response["status"]
        frames = self.render_sse(response)
        self.send_response(200)
        self.send_header("Content-Type", "application/octet-stream" if proof_case == "close-after-terminal-nonstandard-type"
                         else "text/event-stream")
        self.send_header("Content-Length", str(sum(map(len, frames))))
        self.send_header("Connection", "close")
        self.end_headers()
        if close_probe:
            self.wfile.write(b"".join(frames[:-1]))
            self.wfile.flush()
            time.sleep(0.5)
            try:
                self.wfile.write(frames[-1])
            except OSError:
                pass
        else:
            self.wfile.write(b"".join(frames))
        self.close_connection = True


def prepare_provider():
    started, cpu = time.monotonic(), time.process_time()
    from headroom.proxy.handlers.openai import _openai_responses_to_sse

    Provider.render_sse = staticmethod(_openai_responses_to_sse)
    return {"elapsed_ms": round(1000 * (time.monotonic() - started)),
            "cpu_ms": 1000 * (time.process_time() - cpu), "model_requests": 0,
            "scope": "Local fixture renderer setup before task sampling"}


def usage_since(engine, usage, before, counters):
    old_stats, old_usage = before
    deadline = time.monotonic() + 3
    while True:
        rows = usage["records"](engine)
        new_ids = set(rows) - old_usage
        new_stats = set(usage["probe"]["request_logs"](engine)) - old_stats
        if len(new_ids) == len(counters) and new_ids <= new_stats or time.monotonic() >= deadline:
            break
        time.sleep(0.1)
    assert len(new_stats) == len(counters) and new_ids <= new_stats, "owned_usage_request_scope_unproven"
    traces = sorted((rows[request_id] for request_id in new_ids), key=lambda trace: trace["usage"]["total_input_tokens"] or -1)
    verified = []
    for trace in traces:
        entry = next((row for row in counters if row[0] == trace["usage"]["total_input_tokens"]), None)
        assert entry is not None and entry not in verified, "usage_attempt_ambiguous"
        input_tokens, cached, output_tokens = entry
        expected = dict(zip(FIELDS, (input_tokens, cached, None, output_tokens), strict=True))
        assert trace["surface"] == "openai_responses" and not trace["truncated"] and trace["client_status"] == 200
        assert trace["attempts"] == [{"status": 200, "usage": expected}], "usage_counters_changed"
        assert trace["usage"]["total_tokens"] == input_tokens + output_tokens and trace["usage"]["cache_creation_input_tokens"] is None
        verified.append(entry)
    return {"stats_request_count": len(new_stats), "usage_record_count": len(verified),
            "missing_usage_records": len(counters) - len(verified),
            "cache_write_when_present": "unknown" if verified else None}


def usage_ids(engine, usage):
    return set(usage["probe"]["request_logs"](engine)), set(usage["records"](engine))


def closure_check(engine, server, usage, mode):
    before = usage_ids(engine, usage)
    server.requests = []
    body = {"model": MODEL, "reasoning": {"effort": "high"}, "stream": True,
            "input": [{"role": "user", "content": "Synthetic SSE close check."}]}
    connection = HTTPConnection("127.0.0.1", engine.port, timeout=15)
    try:
        connection.request("POST", "/v1/responses", encode(body).encode(), {"Authorization": server.auth,
            "Content-Type": "application/json", "X-Proof-Case": mode, "X-Headroom-Session-Id": mode})
        response = connection.getresponse()
        assert response.status == 200
        terminal = done = False
        status_present = None
        while (line := response.readline(65536)):
            if line.startswith(b"data: [DONE]"):
                done = True
            elif line.startswith(b"data: "):
                event = json.loads(line[6:])
                if event.get("type") == "response.completed":
                    terminal = True
                    status_present = "status" in event["response"]
                    if mode != "read-to-eof":
                        break
        assert terminal and len(server.requests) == 1 and done == (mode == "read-to-eof"), "closure_probe_incomplete"
        assert status_present == (mode != "close-after-terminal-no-status"), "terminal_status_shape_changed"
        response.close()
    finally:
        connection.close()
    return {"mode": mode, "terminal_received": terminal, "done_received": done, "terminal_status_present": status_present,
            **usage_since(engine, usage, before, FIXTURE_COUNTERS[:1])}


def client_args(cli, work, case, arm, model, overrides, *, prompt=None, retrieval_port=None, output_schema=None, resource_report=None):
    mcp_args = ["-I", "-B", str(PROGRAM), "--mcp", case, arm]
    if retrieval_port is not None:
        mcp_args.extend(["--retrieve-from", str(retrieval_port)])
    if resource_report is not None:
        mcp_args.extend(["--resource-report", str(resource_report)])
    config = {
        "windows.sandbox": "unelevated",
        "analytics.enabled": False, "feedback.enabled": False, "otel.exporter": "none", "otel.metrics_exporter": "none",
        "web_search": "disabled", "approval_policy": "never", "model_reasoning_effort": "high",
        "mcp_servers.headroom_rule_pilot.command": sys.executable,
        "mcp_servers.headroom_rule_pilot.args": mcp_args,
        "mcp_servers.headroom_rule_pilot.required": True,
        **overrides,
    }
    command = [str(cli), "--no-daemon", "exec", "--ignore-user-config", "--ephemeral", "--skip-git-repo-check",
        "--sandbox", "read-only", "--json", "--color", "never", "-C", str(work), "--model", model]
    for feature in ("apps", "hooks", "plugins", "remote_plugin", "plugin_sharing", "skill_mcp_dependency_install", "shell_tool"):
        command.extend(["--disable", feature])
    for key, value in config.items():
        command.extend(["-c", key + "=" + encode(value)])
    if output_schema is not None:
        command.extend(["--output-schema", str(output_schema)])
    return [*command, prompt or "Call the fixed inspect_result tool once. " + case_data(case)[3]]


def client_environment():
    return {key: value for key, value in os.environ.items() if not key.upper().startswith(
        ("OPENAI_", "ANTHROPIC_", "CODEX_", "CLAUDE_", "OPENCODE_", "HEADROOM_", "OTEL_")) and not key.upper().endswith("_PROXY")}


def run_client(command, work, env, supervisor, timeout):
    job = supervisor.KillOnCloseJob()
    child = None
    try:
        child = subprocess.Popen(command, cwd=work, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                 stderr=subprocess.DEVNULL, creationflags=subprocess.CREATE_NO_WINDOW)
        job.assign(child.pid)
        stdout, _ = child.communicate(timeout=timeout)
        return child.returncode, stdout
    finally:
        job.close()
        if child is not None and child.poll() is None:
            child.wait(timeout=15)


def inspect_tool_output(engine, second_request, case, arm, usage):
    calls = [item for item in second_request["input"] if item.get("type") == "function_call" and item.get("call_id") == CALL]
    outputs = [item["output"] for item in second_request["input"] if item.get("type") == "function_call_output" and item.get("call_id") == CALL]
    assert len(calls) == len(outputs) == 1 and (calls[0].get("namespace"), calls[0].get("name")) == (NAMESPACE, TOOL), "mcp_call_identity_changed"
    output = outputs[0]
    assert isinstance(output, list) and len(output) == 2 and all(part.get("type") == "input_text" for part in output), "tool_wire_parts_changed"
    text = "".join(part["text"] for part in output)
    header = re.match(r"\AWall time: [0-9]+(?:\.[0-9]+)? seconds\r?\nOutput:\s*", text)
    assert header is not None, "tool_wire_header_changed"
    # The tool and its call ID above identify a fixed producer. This proof does not parse arbitrary terminal output.
    actual = json.loads(text[header.end():])
    expected_envelope = envelope(case, arm)
    assert isinstance(actual, dict) and encode({**actual, "stdout": expected_envelope["stdout"]}) == encode(expected_envelope), "mcp_channel_metadata_changed"
    ccr = actual["stdout"] != expected_envelope["stdout"]
    if ccr:
        marker = re.fullmatch(r"<<ccr:([a-f0-9]{12,24}),string,[^>]+>>", actual["stdout"])
        assert marker is not None, "stdout_changed_without_recovery"
        usage["replay"].retrieval(engine.port, marker[1], expected_envelope["stdout"], 10)
    return {"tool_output_type": type(output).__name__, "mcp_stdout_bytes": len(expected_envelope["stdout"].encode()),
            "forwarded_stdout_bytes": len(actual["stdout"].encode()), "exact_native_ccr_readback": ccr, "channels_exact": True}


def run_cli(cli, engine, provider, supervisor, directory, case, arm, usage):
    directory.mkdir()
    home, work, temp = (directory / name for name in ("codex-home", "workspace", "temp"))
    for path in (home, work, temp):
        path.mkdir()
    before = usage_ids(engine, usage)
    provider.case, provider.requests, provider.failure = case, [], None
    config = {
        "model_provider": "fixture", "model_providers.fixture.name": "Owned fixture",
        "model_providers.fixture.base_url": f"http://127.0.0.1:{engine.port}/v1",
        "model_providers.fixture.wire_api": "responses", "model_providers.fixture.env_key": "RULES_PILOT_TOKEN",
        "model_providers.fixture.requires_openai_auth": False,
    }
    command = client_args(cli, work, case, arm, MODEL, config)
    env = client_environment()
    env.update(CODEX_HOME=str(home), TEMP=str(temp), TMP=str(temp), RULES_PILOT_TOKEN=provider.auth[len("Bearer "):])
    try:
        exit_code, stdout = run_client(command, work, env, supervisor, 60)
    finally:
        for path in home.rglob("*"):
            flags = path.lstat().st_file_attributes
            assert path.resolve().is_relative_to(home.resolve()) and not flags & stat.FILE_ATTRIBUTE_REPARSE_POINT
            if path.is_file():
                if flags & stat.FILE_ATTRIBUTE_READONLY:
                    path.chmod(stat.S_IWRITE)
                path.unlink()
    assert exit_code == 0 and provider.failure is None and len(provider.requests) == 2, provider.failure or "cli_probe_failed"
    events = [json.loads(line) for line in stdout.splitlines() if line.startswith(b"{")]
    messages = [event["item"]["text"] for event in events if event.get("type") == "item.completed"
                and event["item"].get("type") == "agent_message"]
    assert len(messages) == 1 and encode(json.loads(messages[0])) == encode(CASES[case][4]), "fixture_answer_oracle_failed"
    assert sum(event.get("type") == "turn.completed" for event in events) == 1, "cli_turn_incomplete"
    tool = inspect_tool_output(engine, provider.requests[1], case, arm, usage)
    assert all(body["model"] == MODEL and body.get("reasoning", {}).get("effort") == "high" for body in provider.requests), "paired_model_or_effort_changed"
    return {"case": case, "arm": arm, "cli_exit": exit_code, "oracle": True,
            **tool, **usage_since(engine, usage, before, FIXTURE_COUNTERS)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument("--mcp", nargs=2, metavar=("CASE", "ARM"))
    modes.add_argument("--run", action="store_true")
    parser.add_argument("--codex", type=Path)
    parser.add_argument("--retrieve-from", type=int)
    parser.add_argument("--resource-report", type=Path)
    args = parser.parse_args()
    if args.mcp:
        case, arm = args.mcp
        assert case in CASES or case_details(case) is not None
        assert arm in ("control", "candidate")
        assert args.retrieve_from is None or 1024 <= args.retrieve_from <= 65535
        serve(case, arm, args.retrieve_from, args.resource_report)
        return 0
    assert os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom")
    os.environ["LITELLM_LOCAL_MODEL_COST_MAP"] = "True"
    cli = args.codex.resolve(strict=True)
    assert cli.name == "codex.exe" and cli.is_relative_to((Path(os.environ["LOCALAPPDATA"]) / "OpenAI/Codex/bin").resolve(strict=True))
    version = subprocess.run([str(cli), "--version"], capture_output=True, timeout=10, check=True).stdout.decode().strip()
    assert version == "codex-cli 0.169.0-alpha.11.1", "cli_fingerprint_changed"
    usage_path = ROOT / "tests/runtime/anthropic_usage_capture.py"
    usage = runpy.run_path(str(usage_path))
    replay = usage["replay"]
    runtime = ROOT / "runtime"
    controller = replay.load_module("rules_pilot_controller", runtime / "desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(runtime / "manifest.json")
    assert replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"]
    runner = payload / manifest["engine"]["runner"]
    supervisor = replay.load_module("rules_pilot_supervisor", runner)
    run = ROOT / "work/proofs/optimization/rules-pilot" / uuid.uuid4().hex
    assert run.resolve().is_relative_to(ROOT)
    run.mkdir(parents=True)
    staged = replay.prepare_fixture_runtime(run, payload)
    sources = json.loads((runtime / "optimization-sources.json").read_bytes())
    files = [PROGRAM, HELDOUT_PATH, JSON_PATH, HERE / "rules_runs_json.py", usage_path, runner, Path(sys.executable), cli, payload / "python313.dll", payload / "runtime-manifest.json",
             runtime / "learn_preview.py", *staged, *[runtime / path.name for path in staged],
             *[HERE / name for name in ("replay.py", "anthropic_usage.py", "ccr_policy.py", "surfaces.py", "fixtures.py")],
             *[ROOT / "bench/context_tools" / name for name in ("targeted_query.py", "targeted_query_replay.py")],
             *[payload / "Lib/site-packages/headroom" / name for name in sources],
             payload / "Lib/site-packages/headroom/transforms/lossless_compaction.py",
             payload / "Lib/site-packages/headroom/proxy/handlers/openai.py",
             payload / "Lib/site-packages/mcp-2.3.0.dist-info/RECORD"]
    fingerprints = lambda: {str(path): replay.sha(path) for path in files}
    before = fingerprints()
    fixture_setup = prepare_provider()
    server = ThreadingHTTPServer(("127.0.0.1", 0), Provider)
    server.auth = "Bearer " + replay.SENTINEL
    threading.Thread(target=server.serve_forever, daemon=True).start()
    engine = usage["probe"]["UsageEngine"](controller, supervisor, payload, run / "engine", server.server_port, PLAN)
    results, closures, failure = [], [], None
    try:
        engine.start()
        assert usage["usage_response"](engine, None)[0] == 404 and not usage["records"](engine), "usage_boundary_failed"
        for mode in CLOSURE_MODES:
            row = closure_check(engine, server, usage, mode)
            closures.append(row)
            print(encode(row), flush=True)
        for case in CASES:
            for arm in ("control", "candidate"):
                row = run_cli(cli, engine, server, supervisor, run / f"{case}-{arm}", case, arm, usage)
                results.append(row)
                print(encode(row), flush=True)
    except Exception as error:
        failure = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__
        where = error.__traceback__
        while where.tb_next is not None:
            where = where.tb_next
        print(encode({"failure": failure, "source": Path(where.tb_frame.f_code.co_filename).name,
                      "line": where.tb_lineno}), flush=True)
    finally:
        try:
            engine.close()
        finally:
            server.shutdown()
            server.server_close()
    after = fingerprints()
    observation_complete = failure is None and before == after and len(results) == 4 and len(closures) == len(CLOSURE_MODES)
    usage_complete = observation_complete and all(row["missing_usage_records"] == 0 for row in [*closures, *results])
    report = run / "result.json"
    replay.write_new(report, {"passed": usage_complete, "observation_complete": observation_complete, "failure": failure,
        "cli": version, "engine": manifest["engine"]["version"], "closure_probes": closures,
        "os": platform.platform(), "rows": results, "stop": engine.stopped, "model_calls_outside_loopback": 0,
        "usage_source": "scripted fixture", "quality_source": "scripted fixture validated by exact-answer reader",
        "fixture_setup": fixture_setup,
        "before": before, "after": after})
    print(encode({"passed": usage_complete, "observation_complete": observation_complete, "failure": failure,
                  "proof": report.relative_to(ROOT).as_posix()}), flush=True)
    return 0 if usage_complete else 1


if __name__ == "__main__":
    raise SystemExit(main())
