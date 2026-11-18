"""Check per-process report capabilities through real Codex CLI and a loopback-only provider."""
import argparse
from http.server import ThreadingHTTPServer
import json
import os
from pathlib import Path
import platform
import re
import runpy
import shutil
import stat
import subprocess
import sys
import threading
import uuid


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
pilot = runpy.run_path(str(HERE.parent / "optimization/rules_pilot.py"))
reported_error = runpy.run_path(str(HERE.parent / "optimization/rules_live.py"))["tool_reported_error"]
prototype = runpy.run_path(str(HERE / "targeted_query_replay.py"))
client = runpy.run_path(str(HERE / "targeted_query_client.py"))
encode = pilot["encode"]
MODEL, NAMESPACE = pilot["MODEL"], pilot["NAMESPACE"]
TOOLS = {"read_structured_report", "select_report_record", "Read"}
# Capture and select, then 18 stdout ranges and the empty stderr range.
SCENARIOS = {"authorized": 22, "foreign": 3, "disabled": 4, "failed_report": 5}
COUNTERS = tuple((1013 + i * 100, 0 if i == 0 else 511, 3 + i) for i in range(max(SCENARIOS.values())))
REPORT_ROWS = prototype["report_rows"]()
SOURCES = {"authorized": (b"".join(REPORT_ROWS), b"", 0), "failed_report": client["FAILURE_FIXTURE"]}
DASHBOARD_REQUEST_WINDOW = 25


def tool_text(body, turn):
    values = [item["output"] for item in body["input"]
              if item.get("type") == "function_call_output" and item.get("call_id") == f"call_query_{turn}"]
    assert len(values) == 1, "tool_output_identity_changed"
    value = values[0]
    if isinstance(value, str):
        text = value
    else:
        assert isinstance(value, list) and value and all(isinstance(part, dict)
            and part.get("type") in ("text", "input_text") and isinstance(part.get("text"), str) for part in value), "tool_output_shape_changed"
        text = "".join(part["text"] for part in value)
    header = re.match(r"\AWall time: [0-9]+(?:\.[0-9]+)? seconds\r?\nOutput:\s*", text)
    return text[header.end():] if header is not None else text


class Readback:
    def __init__(self, reference, stdout, stderr, exit_code):
        self.reference = reference
        self.expected = {"stdout": stdout, "stderr": stderr}
        self.exit_code = exit_code
        self.stream = "stdout"
        self.buffers = {"stdout": bytearray(), "stderr": bytearray()}
        self.parts = 0

    def arguments(self):
        if self.stream is None:
            return None
        return {"archive_id": self.reference["archive_id"], "stream": self.stream, "offset": len(self.buffers[self.stream])}

    def accept(self, part):
        assert self.stream is not None and isinstance(part, dict), "unexpected_read_part"
        keys = {*self.reference, "stream", "offset", "next_offset", "total_bytes", "exit_code", "text"}
        assert part.keys() == keys, "read_part_shape_changed"
        assert all(part[key] == value for key, value in self.reference.items()), "read_part_source_changed"
        assert type(part["exit_code"]) is int and part["exit_code"] == self.exit_code, "read_part_exit_changed"
        assert part["stream"] == self.stream and all(type(part[key]) is int for key in ("offset", "next_offset", "total_bytes")), "read_part_range_changed"
        data = part["text"].encode("utf-8") if isinstance(part["text"], str) else None
        buffer = self.buffers[self.stream]
        expected = self.expected[self.stream]
        start, end = len(buffer), part["next_offset"]
        assert (data is not None and part["offset"] == start and part["total_bytes"] == len(expected)
                and start + len(data) == end <= len(expected) and (data or end == len(expected))), "read_part_range_changed"
        assert data == expected[start:end], "read_part_bytes_changed"
        buffer.extend(data)
        self.parts += 1
        if end == len(expected):
            self.stream = "stderr" if self.stream == "stdout" else None

    def finish(self):
        assert self.stream is None, "readback_incomplete"
        return {key: bytes(value) for key, value in self.buffers.items()}


def check_selection(reference, result, *, failed=False):
    expected = {**reference, "status": "full_output_required"} if failed else {
        **reference, "status": "selected", "stdout": REPORT_ROWS[777].decode(), "stderr": "", "exit_code": 0,
        "record_count": 1000, "matched_count": 1, "truncated": False}
    assert result == expected, "selected_report_changed"


def read_start(handle):
    return {"archive_id": handle, "stream": "stdout", "offset": 0}


def dashboard_request_ids(engine, exchange):
    status, _, data = exchange(engine.port, "GET", "/stats?cached=1", {"X-Headroom-Desktop-Control": engine.control_token})
    assert status == 200, "fixture_stats_status"
    # The dashboard projection covers up to 25 requests. The raw diagnostic tail covers 10.
    rows = json.loads(data)["recent_requests"]
    assert isinstance(rows, list) and len(rows) <= DASHBOARD_REQUEST_WINDOW, "fixture_stats_window"
    assert all(isinstance(row, dict) and isinstance(row.get("request_id"), str) and row["request_id"] for row in rows), "fixture_stats_ids"
    ids = {row["request_id"] for row in rows}
    assert len(ids) == len(rows), "fixture_stats_ids"
    return ids


class Provider(pilot["Provider"]):
    turn_counts = SCENARIOS

    def next_item(self, body, turn):
        server, scenario = self.server, self.server.scenario
        tools = {tool["name"] for schema in body.get("tools", []) if schema.get("type") == "namespace"
                 and schema.get("name") == NAMESPACE for tool in schema.get("tools", [])}
        assert tools == TOOLS - ({"Read"} if scenario == "disabled" else set()), "tool_permission_catalog_changed"
        assert body["model"] == MODEL and body.get("reasoning", {}).get("effort") == "high", "fixture_model_effort_changed"
        tool, arguments = None, None
        if scenario == "foreign":
            if turn == 1:
                tool, arguments = "select_report_record", {"archive_id": server.previous_handle, "record_id": "row_0777"}
            else:
                denied = tool_text(body, turn - 1)
                assert "archive_unavailable" in denied and "18446744073709551617" not in denied, "foreign_archive_not_rejected"
                if turn == 2:
                    tool, arguments = "Read", read_start(server.previous_handle)
        elif turn == 1:
            tool, arguments = "read_structured_report", {}
        elif turn == 2:
            reference = json.loads(tool_text(body, 1))
            assert set(reference) == {"archive_id", "source_tool", "source_call_id"}, "capture_reference_shape_changed"
            assert re.fullmatch(r"[a-f0-9]{32}", reference["archive_id"]), "invalid_archive_reference"
            assert reference["source_tool"] == "read_structured_report" and reference["source_call_id"].startswith("mcp:"), "source_identity_missing"
            server.reference = reference
            if scenario == "disabled":
                tool, arguments = "Read", read_start(reference["archive_id"])
            else:
                tool, arguments = "select_report_record", {"archive_id": reference["archive_id"], "record_id": "row_0777"}
        elif scenario == "disabled" and turn == 3:
            denied = tool_text(body, 2)
            assert "Read" in denied and re.search(r"\b(error|unknown|unrecognized|disabled|unsupported)\b", denied, re.IGNORECASE), "disabled_tool_not_rejected"
            assert "18446744073709551617" not in denied, "disabled_tool_leaked_report"
            tool, arguments = "select_report_record", {"archive_id": server.reference["archive_id"], "record_id": "row_0777"}
        elif scenario == "disabled":
            check_selection(server.reference, json.loads(tool_text(body, turn - 1)))
        else:
            if turn == 3:
                check_selection(server.reference, json.loads(tool_text(body, turn - 1)), failed=scenario == "failed_report")
                server.readback = Readback(server.reference, *SOURCES[scenario])
            else:
                text = tool_text(body, turn - 1)
                size = len(text.encode())
                assert size <= client["READ_REPLY_BYTES"], "read_part_wire_budget"
                server.max_read_reply_bytes = max(server.max_read_reply_bytes, size)
                server.readback.accept(json.loads(text))
            arguments = server.readback.arguments()
            if arguments is None:
                restored = server.readback.finish()
                assert restored == server.readback.expected, "original_readback_changed"
                assert turn == SCENARIOS[scenario], "unexpected_read_part_count"
                server.readback_observation = {"parts": server.readback.parts, "stdout_bytes": len(restored["stdout"]),
                    "stderr_bytes": len(restored["stderr"]), "exit_code": server.readback.exit_code,
                    "max_encoded_reply_bytes": server.max_read_reply_bytes, "exact": True}
                if scenario == "authorized":
                    server.previous_handle = server.reference["archive_id"]
            else:
                tool = "Read"
        if tool is not None:
            return {"type": "function_call", "id": f"fc_query_{turn}", "call_id": f"call_query_{turn}",
                    "name": tool, "namespace": NAMESPACE, "arguments": encode(arguments)}
        return {"type": "message", "id": "msg_query_fixture", "role": "assistant", "status": "completed",
                "content": [{"type": "output_text", "text": '{"fixture_ok":true}', "annotations": []}]}

    def do_POST(self):
        server = self.server
        try:
            size = int(self.headers.get("Content-Length", "0"))
            assert self.path == "/v1/responses" and 0 < size <= 2 * 1024 * 1024, "fixture_scope"
            assert self.headers.get("Authorization") == server.auth, "fixture_auth_changed"
            assert server.total_requests < sum(self.turn_counts.values()), "fixture_request_budget"
            body = json.loads(self.rfile.read(size))
            server.requests.append(body)
            server.total_requests += 1
            for entry in body.get("input", []):
                if entry.get("type") == "function_call_output" and isinstance(entry.get("output"), list):
                    server.output_part_types.update(part.get("type") if part.get("type") in ("text", "input_text") else "other"
                        for part in entry["output"] if isinstance(part, dict))
            turn = len(server.requests)
            assert turn <= self.turn_counts[server.scenario], "unexpected_fixture_turn"
            item = self.next_item(body, turn)
            input_tokens, cached, output = COUNTERS[turn - 1]
            response = {"id": f"resp_query_{server.scenario}_{turn}", "object": "response", "status": "completed",
                "model": MODEL, "output": [item], "usage": {"input_tokens": input_tokens,
                    "input_tokens_details": {"cached_tokens": cached}, "output_tokens": output, "total_tokens": input_tokens + output}}
            frames = b"".join(self.render_sse(response))
        except Exception as error:
            server.failure = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__.lower()
            if isinstance(error, json.JSONDecodeError):
                server.parse_failure = {"document_chars": len(error.doc), "position": error.pos,
                    "truncation_marker_present": bool(re.search(r"\btruncated\b", error.doc, re.IGNORECASE)),
                    "ccr_marker_present": "<<ccr:" in error.doc}
            trace = error.__traceback__
            while trace.tb_next is not None:
                trace = trace.tb_next
            server.failure_site = {"file": Path(trace.tb_frame.f_code.co_filename).name, "line": trace.tb_lineno}
            self.send_error(400)
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Content-Length", str(len(frames)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(frames)
        self.close_connection = True


def run_cli(cli, engine, server, supervisor, directory, scenario, usage):
    transient, work = directory / "client", directory / "workspace"
    home, temp = transient / "home", transient / "temp"
    for path in (home, temp, work):
        path.mkdir(parents=True)
    mcp_args = ["-I", "-B", str(HERE / "targeted_query_client.py"), "--mcp"]
    if scenario == "failed_report":
        mcp_args.append("--failed-report")
    config = {"model_provider": "fixture", "model_providers.fixture.name": "Owned query fixture",
        "model_providers.fixture.base_url": f"http://127.0.0.1:{engine.port}/v1",
        "model_providers.fixture.wire_api": "responses", "model_providers.fixture.env_key": "RULES_PILOT_TOKEN",
        "model_providers.fixture.requires_openai_auth": False, "model_providers.fixture.supports_websockets": False,
        "mcp_servers.headroom_rule_pilot.args": mcp_args,
        "features.memories": False, "features.multi_agent": False, "features.multi_agent_v2": False,
        "project_doc_max_bytes": 0, "log_dir": str(transient / "logs")}
    if scenario == "disabled":
        config["mcp_servers.headroom_rule_pilot.disabled_tools"] = ["Read"]
    command = pilot["client_args"](cli, work, "counts", "control", MODEL, config,
        prompt="Use only the fixed synthetic report tools. This is a local transport and permission fixture.")
    assert not any(argument.startswith("tool_output_token_limit=") for argument in command), "fixture_needs_default_output_budget"
    env = pilot["client_environment"]()
    env.update(CODEX_HOME=str(home), CODEX_SQLITE_HOME=str(transient / "state"), TEMP=str(temp), TMP=str(temp),
               RULES_PILOT_TOKEN=server.auth.removeprefix("Bearer "))
    before = pilot["usage_ids"](engine, usage)
    server.scenario, server.requests, server.failure = scenario, [], None
    server.readback_observation = None
    server.max_read_reply_bytes = 0
    try:
        exit_code, stdout = pilot["run_client"](command, work, env, supervisor, 90 if scenario == "authorized" else 60)
    finally:
        assert transient.resolve(strict=True) == transient and directory.parent.is_relative_to(ROOT / "work/proofs/optimization/targeted-query-client"), "temporary_client_scope"
        for path in transient.rglob("*"):
            flags = path.lstat().st_file_attributes
            assert path.resolve().is_relative_to(transient) and not flags & stat.FILE_ATTRIBUTE_REPARSE_POINT, "temporary_client_reparse"
            if path.is_file() and flags & stat.FILE_ATTRIBUTE_READONLY:
                path.chmod(stat.S_IWRITE)
        shutil.rmtree(transient)
    events = [json.loads(line) for line in stdout.splitlines() if line.startswith(b"{")]
    assert server.failure is None, server.failure
    assert exit_code == 0 and len(server.requests) == SCENARIOS[scenario], "cli_fixture_incomplete"
    assert sum(event.get("type") == "turn.completed" for event in events) == 1, "cli_turn_incomplete"
    messages = [event["item"]["text"] for event in events if event.get("type") == "item.completed" and event["item"].get("type") == "agent_message"]
    assert messages == ['{"fixture_ok":true}'], "cli_fixture_final_changed"
    tools = [event["item"] for event in events if event.get("type") == "item.completed" and event["item"].get("type") == "mcp_tool_call"]
    server.cli_tool_metadata = [{"tool": item["tool"], "status": item["status"], "reported_error": reported_error(item),
        "error_field_present": item.get("error") is not None, "result_type": type(item.get("result")).__name__,
        "result_isError": (item.get("result") or {}).get("isError"), "result_is_error": (item.get("result") or {}).get("is_error"),
        "denial_reason_seen": any("archive_unavailable" in part.get("text", "") for part in (item.get("result") or {}).get("content", []))}
        for item in tools]
    server.mcp_event_counts = {kind: sum(event.get("type") == kind and event.get("item", {}).get("type") == "mcp_tool_call" for event in events)
        for kind in ("item.started", "item.completed", "item.failed")}
    assert all(item["server"] == "headroom_rule_pilot" for item in tools), "foreign_tool_server"
    if scenario in SOURCES:
        assert server.readback_observation is not None, "readback_incomplete"
        expected = ["read_structured_report", "select_report_record", *(["Read"] * (SCENARIOS[scenario] - 3))]
        assert [item["tool"] for item in tools] == expected, "authorized_tool_order"
        assert all(item["status"] == "completed" and not reported_error(item) for item in tools), "authorized_tool_failed"
    elif scenario == "foreign":
        assert [item["tool"] for item in tools] == ["select_report_record", "Read"], "foreign_tool_order"
        assert all(item["status"] == "failed" or reported_error(item) for item in tools), "foreign_tool_not_error"
    else:
        assert [item["tool"] for item in tools if item["status"] == "completed"] == ["read_structured_report", "select_report_record"], "disabled_tool_executed"
    measured = pilot["usage_since"](engine, usage, before, COUNTERS[:SCENARIOS[scenario]])
    assert measured["missing_usage_records"] == 0, "incomplete_readback_usage"
    return {"scenario": scenario, "cli_exit": exit_code, "transport_oracle": True,
        "original_readback": server.readback_observation,
        "normal_tools": [{"tool": item["tool"], "status": item["status"], "reported_error_field": reported_error(item)} for item in tools],
        **measured}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true", required=True)
    parser.add_argument("--codex", type=Path, required=True)
    args = parser.parse_args()
    assert os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace"
    cli = args.codex.resolve(strict=True)
    assert cli.name == "codex.exe" and cli.is_relative_to((Path(os.environ["LOCALAPPDATA"]) / "OpenAI/Codex/bin").resolve(strict=True)), "cli_scope"
    version = subprocess.run([str(cli), "--version"], capture_output=True, timeout=10, check=True).stdout.decode().strip()
    assert version == "codex-cli 0.169.0-alpha.11.1", "cli_pin_changed"
    os.environ["LITELLM_LOCAL_MODEL_COST_MAP"] = "True"
    usage = runpy.run_path(str(ROOT / "tests/runtime/anthropic_usage_capture.py"))
    exchange = usage["probe"]["exchange"]
    usage["probe"] = {**usage["probe"], "request_logs": lambda engine: dashboard_request_ids(engine, exchange)}
    assert max(SCENARIOS.values()) <= DASHBOARD_REQUEST_WINDOW, "fixture_exceeds_stats_window"
    replay = usage["replay"]
    controller = replay.load_module("query_client_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(ROOT / "runtime/manifest.json")
    assert replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "python_pin_changed"
    runner = payload / manifest["engine"]["runner"]
    supervisor = replay.load_module("query_client_supervisor", runner)
    run = ROOT / "work/proofs/optimization/targeted-query-client" / uuid.uuid4().hex
    run.mkdir(parents=True)
    staged = replay.prepare_fixture_runtime(run, payload)
    sources = json.loads((ROOT / "runtime/optimization-sources.json").read_bytes())
    files = [Path(__file__), HERE / "targeted_query_client.py", HERE / "targeted_query.py", HERE / "targeted_query_replay.py",
        ROOT / "tests/runtime/test_targeted_query_client.py", ROOT / "tests/runtime/anthropic_usage_capture.py",
        HERE.parent / "optimization/rules_pilot.py", HERE.parent / "optimization/rules_live.py",
        HERE.parent / "optimization/replay.py", HERE.parent / "optimization/anthropic_usage.py",
        ROOT / "runtime/manifest.json", runner, cli, Path(sys.executable), payload / "python313.dll", payload / "runtime-manifest.json",
        payload / "Lib/site-packages/mcp-2.3.0.dist-info/RECORD",
        payload / "Lib/site-packages/mcp/server/mcpserver/server.py", payload / "Lib/site-packages/mcp/server/mcpserver/context.py",
        *staged, *[ROOT / "runtime" / path.name for path in staged],
        *[payload / "Lib/site-packages/headroom" / name for name in sources]]
    fingerprints = lambda: {str(path): replay.sha(path) for path in files}
    before = fingerprints()
    setup = pilot["prepare_provider"]()
    Provider.render_sse = staticmethod(pilot["Provider"].render_sse)
    server = ThreadingHTTPServer(("127.0.0.1", 0), Provider)
    server.auth, server.total_requests = "Bearer " + replay.SENTINEL, 0
    server.output_part_types = set()
    threading.Thread(target=server.serve_forever, daemon=True).start()
    engine = usage["probe"]["UsageEngine"](controller, supervisor, payload, run / "engine", server.server_port, pilot["PLAN"])
    rows, failure, failure_site = [], None, None
    try:
        engine.start()
        for scenario in SCENARIOS:
            rows.append(run_cli(cli, engine, server, supervisor, run / scenario, scenario, usage))
            print(encode({"scenario": scenario, "passed": True}), flush=True)
    except Exception as error:
        failure = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__.lower()
        trace = error.__traceback__
        while trace.tb_next is not None:
            trace = trace.tb_next
        failure_site = getattr(server, "failure_site", None) or {"file": Path(trace.tb_frame.f_code.co_filename).name, "line": trace.tb_lineno}
    finally:
        try:
            engine.close()
        finally:
            server.shutdown()
            server.server_close()
    after = fingerprints()
    passed = failure is None and len(rows) == len(SCENARIOS) and server.total_requests == sum(SCENARIOS.values()) and before == after
    report = run / "result.json"
    replay.write_new(report, {"passed": passed, "failure": failure, "failure_site": failure_site, "rows": rows, "stop": engine.stopped,
        "parse_failure_metadata": getattr(server, "parse_failure", None),
        "last_readback_observation": getattr(server, "readback_observation", None),
        "last_cli_tool_metadata": getattr(server, "cli_tool_metadata", None),
        "last_mcp_event_counts": getattr(server, "mcp_event_counts", None),
        "cli": version, "engine": manifest["engine"]["version"], "os": platform.platform(), "local_upstream_requests": server.total_requests,
        "normal_output_part_types": sorted(server.output_part_types),
        "model_calls_outside_loopback": 0, "quality_and_usage_source": "scripted fixture, not provider benefit",
        "native_file_sandbox_inheritance_verified": False, "subagents_or_other_clients_verified": False,
        "fixture_tool_output_token_limit": None, "expected_scenario_requests": SCENARIOS,
        "stats_projection": "recent_requests", "stats_projection_max_rows": DASHBOARD_REQUEST_WINDOW,
        "fixture_setup": setup, "before": before, "after": after})
    print(encode({"passed": passed, "failure": failure, "proof": report.relative_to(ROOT).as_posix()}), flush=True)
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
