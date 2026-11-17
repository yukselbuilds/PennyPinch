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
encode = pilot["encode"]
MODEL, NAMESPACE = pilot["MODEL"], pilot["NAMESPACE"]
TOOLS = {"read_structured_report", "select_report_record", "Read"}
COUNTERS = tuple((1013 + i * 100, 0 if i == 0 else 511, 3 + i) for i in range(4))
SCENARIOS = {"authorized": 4, "foreign": 3, "disabled": 4}
FIXTURE_OUTPUT_TOKEN_LIMIT = 65_536


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


class Provider(pilot["Provider"]):
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
                assert "archive_unavailable" in tool_text(body, turn - 1), "foreign_archive_not_rejected"
                if turn == 2:
                    tool, arguments = "Read", {"archive_id": server.previous_handle}
        elif turn == 1:
            tool, arguments = "read_structured_report", {}
        elif turn == 2:
            reference = json.loads(tool_text(body, 1))
            assert set(reference) == {"archive_id", "source_tool", "source_call_id"}, "capture_reference_shape_changed"
            assert re.fullmatch(r"[a-f0-9]{32}", reference["archive_id"]), "invalid_archive_reference"
            assert reference["source_tool"] == "read_structured_report" and reference["source_call_id"].startswith("mcp:"), "source_identity_missing"
            server.reference = reference
            tool = "select_report_record" if scenario == "authorized" else "Read"
            arguments = {"archive_id": reference["archive_id"]}
            if scenario == "authorized":
                arguments["record_id"] = "row_0777"
        elif scenario == "disabled" and turn == 3:
            denied = tool_text(body, 2)
            assert "Read" in denied and re.search(r"\b(error|unknown|unrecognized|disabled|unsupported)\b", denied, re.IGNORECASE), "disabled_tool_not_rejected"
            assert "18446744073709551617" not in denied, "disabled_tool_leaked_report"
            tool, arguments = "select_report_record", {"archive_id": server.reference["archive_id"], "record_id": "row_0777"}
        elif scenario == "authorized" and turn == 4:
            original = json.loads(tool_text(body, 3))
            expected = {"stdout": b"".join(prototype["report_rows"]()).decode(), "stderr": "", "exit_code": 0,
                "source_tool": server.reference["source_tool"], "source_call_id": server.reference["source_call_id"]}
            server.original_wire_comparison = {key: original.get(key) == value for key, value in expected.items()}
            server.original_wire_comparison["same_fields"] = original.keys() == expected.keys()
            assert original == expected, "original_report_changed"
            server.previous_handle = server.reference["archive_id"]
        else:
            selected = json.loads(tool_text(body, turn - 1))
            assert selected == {"status": "selected", "archive_id": server.reference["archive_id"],
                "source_tool": server.reference["source_tool"], "source_call_id": server.reference["source_call_id"],
                "stdout": prototype["report_rows"]()[777].decode(), "stderr": "", "exit_code": 0,
                "record_count": 1000, "matched_count": 1, "truncated": False}, "selected_report_changed"
            if scenario == "authorized":
                tool, arguments = "Read", {"archive_id": server.reference["archive_id"]}
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
            assert server.total_requests < sum(SCENARIOS.values()), "fixture_request_budget"
            body = json.loads(self.rfile.read(size))
            server.requests.append(body)
            server.total_requests += 1
            for entry in body.get("input", []):
                if entry.get("type") == "function_call_output" and isinstance(entry.get("output"), list):
                    server.output_part_types.update(part.get("type") if part.get("type") in ("text", "input_text") else "other"
                        for part in entry["output"] if isinstance(part, dict))
            turn = len(server.requests)
            assert turn <= SCENARIOS[server.scenario], "unexpected_fixture_turn"
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
    config = {"model_provider": "fixture", "model_providers.fixture.name": "Owned query fixture",
        "model_providers.fixture.base_url": f"http://127.0.0.1:{engine.port}/v1",
        "model_providers.fixture.wire_api": "responses", "model_providers.fixture.env_key": "RULES_PILOT_TOKEN",
        "model_providers.fixture.requires_openai_auth": False, "model_providers.fixture.supports_websockets": False,
        "mcp_servers.headroom_rule_pilot.args": ["-I", "-B", str(HERE / "targeted_query_client.py"), "--mcp"],
        "features.memories": False, "features.multi_agent": False, "features.multi_agent_v2": False,
        "project_doc_max_bytes": 0, "log_dir": str(transient / "logs"),
        "tool_output_token_limit": FIXTURE_OUTPUT_TOKEN_LIMIT}
    if scenario == "disabled":
        config["mcp_servers.headroom_rule_pilot.disabled_tools"] = ["Read"]
    command = pilot["client_args"](cli, work, "counts", "control", MODEL, config,
        prompt="Use only the fixed synthetic report tools. This is a local transport and permission fixture.")
    env = pilot["client_environment"]()
    env.update(CODEX_HOME=str(home), CODEX_SQLITE_HOME=str(transient / "state"), TEMP=str(temp), TMP=str(temp),
               RULES_PILOT_TOKEN=server.auth.removeprefix("Bearer "))
    before = pilot["usage_ids"](engine, usage)
    server.scenario, server.requests, server.failure = scenario, [], None
    try:
        exit_code, stdout = pilot["run_client"](command, work, env, supervisor, 60)
    finally:
        assert transient.resolve(strict=True) == transient and directory.parent.is_relative_to(ROOT / "work/proofs/optimization/targeted-query-client"), "temporary_client_scope"
        for path in transient.rglob("*"):
            flags = path.lstat().st_file_attributes
            assert path.resolve().is_relative_to(transient) and not flags & stat.FILE_ATTRIBUTE_REPARSE_POINT, "temporary_client_reparse"
            if path.is_file() and flags & stat.FILE_ATTRIBUTE_READONLY:
                path.chmod(stat.S_IWRITE)
        shutil.rmtree(transient)
    events = [json.loads(line) for line in stdout.splitlines() if line.startswith(b"{")]
    server.original_cli_delivery = None
    for event in events:
        item = event.get("item", {})
        if event.get("type") == "item.completed" and item.get("type") == "mcp_tool_call" and item.get("tool") == "Read":
            result = item.get("result") or {}
            texts = [part["text"] for part in result.get("content", []) if part.get("type") == "text"]
            captures = []
            for text in texts:
                try:
                    value = json.loads(text)
                except json.JSONDecodeError:
                    continue
                if isinstance(value, dict):
                    captures.append(value)
            server.original_cli_delivery = {"text_bytes": sum(len(text.encode()) for text in texts),
                "complete_original_seen": any(value.get("stdout") == b"".join(prototype["report_rows"]()).decode() for value in captures),
                "is_error": result.get("isError")}
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
    if scenario == "authorized":
        assert [item["tool"] for item in tools] == ["read_structured_report", "select_report_record", "Read"], "authorized_tool_order"
        assert all(item["status"] == "completed" and not reported_error(item) for item in tools), "authorized_tool_failed"
    elif scenario == "foreign":
        assert len(tools) == 2 and all(item["status"] == "failed" or reported_error(item) for item in tools), "foreign_tool_not_error"
    else:
        assert not any(item["tool"] == "Read" and item["status"] == "completed" and not reported_error(item) for item in tools), "disabled_tool_executed"
    return {"scenario": scenario, "cli_exit": exit_code, "transport_oracle": True,
        "original_cli_delivery_metadata": server.original_cli_delivery,
        "normal_tools": [{"tool": item["tool"], "status": item["status"], "reported_error_field": reported_error(item)} for item in tools],
        **pilot["usage_since"](engine, usage, before, COUNTERS[:SCENARIOS[scenario]])}


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
    passed = failure is None and len(rows) == 3 and server.total_requests == 11 and before == after
    report = run / "result.json"
    replay.write_new(report, {"passed": passed, "failure": failure, "failure_site": failure_site, "rows": rows, "stop": engine.stopped,
        "parse_failure_metadata": getattr(server, "parse_failure", None),
        "original_wire_comparison": getattr(server, "original_wire_comparison", None),
        "original_cli_delivery_metadata": getattr(server, "original_cli_delivery", None),
        "last_cli_tool_metadata": getattr(server, "cli_tool_metadata", None),
        "last_mcp_event_counts": getattr(server, "mcp_event_counts", None),
        "cli": version, "engine": manifest["engine"]["version"], "os": platform.platform(), "local_upstream_requests": server.total_requests,
        "normal_output_part_types": sorted(server.output_part_types),
        "model_calls_outside_loopback": 0, "quality_and_usage_source": "scripted fixture, not provider benefit",
        "native_file_sandbox_inheritance_verified": False, "subagents_or_other_clients_verified": False,
        "fixture_tool_output_token_limit": FIXTURE_OUTPUT_TOKEN_LIMIT,
        "fixture_setup": setup, "before": before, "after": after})
    print(encode({"passed": passed, "failure": failure, "proof": report.relative_to(ROOT).as_posix()}), flush=True)
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
