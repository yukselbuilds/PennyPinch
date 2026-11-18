"""Exercise file-bound report tools in real parent and child Codex threads on loopback."""
import argparse
from http.server import ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import runpy
import stat
import subprocess
import sys
import threading
import uuid


ENTRY = Path(__file__).resolve()
ROOT = ENTRY.parents[2]
compare = runpy.run_path(str(ENTRY.with_name("targeted_query_compare.py")))
PROOFS = compare["PROOF_ROOT"] / "file-agent-scope"
CAPTURE, READ = compare["CAPTURE"], compare["READ"]
REPORT = '{"id":"owned","units":9007199254740993,"label":"Yetkili — λ"}\r\n'.encode("utf-8")
OUTSIDE = b'{"id":"outside","label":"HEADROOM_OUTSIDE_SCOPE_CANARY"}\n'
CHILD_TASK = "report_scope_child"
CHILD_PROMPT = "Exercise report ownership in the isolated fixture."
PARENT_FINAL, CHILD_FINAL = "HEADROOM_PARENT_SCOPE_OK", "HEADROOM_CHILD_SCOPE_OK"
TURN_LIMITS = {"parent": 6, "child": 4}
OUTSIDE_PATH_ARGUMENT = "../outside.jsonl"


def approved_report(workspace):
    assert workspace.resolve(strict=True) == workspace and workspace.name == "workspace" and workspace.parent.parent == PROOFS, "workspace_outside_fixture"
    path = workspace / "approved.jsonl"
    metadata = path.lstat()
    assert path.resolve(strict=True) == path and stat.S_ISREG(metadata.st_mode) and metadata.st_nlink == 1, "report_not_owned"
    assert not getattr(metadata, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT, "report_is_reparse_point"
    with path.open("rb") as stream:
        current = os.fstat(stream.fileno())
        assert (current.st_dev, current.st_ino) == (metadata.st_dev, metadata.st_ino), "report_changed_before_open"
        data = stream.read(len(REPORT) + 1)
    assert data == REPORT, "report_fixture_changed"
    return data


def json_output(text):
    value = json.loads(text)
    if isinstance(value, dict) and "content" in value and not value.get("isError"):
        content = value["content"]
        assert len(content) == 1 and content[0].get("type") == "text", "report_content_shape"
        value = json.loads(content[0]["text"])
    return value


def child_notification(body, task):
    return any(item.get("type") == "agent_message" and item.get("author") == task
               and any(CHILD_FINAL in (part.get("text") or "") for part in item.get("content", [])) for item in body.get("input", []))


def read_exact(value, reference):
    return value == {**reference, "stream": "stdout", "offset": 0, "total_bytes": len(REPORT),
                     "next_offset": len(REPORT), "exit_code": 0, "text": REPORT.decode("utf-8")}


def foreign_read_outcome(text, reference):
    if "archive_unavailable" in text and "9007199254740993" not in text:
        return "denied"
    try:
        return "returned" if read_exact(json_output(text), reference) else "unknown"
    except (ValueError, TypeError, KeyError, AssertionError):
        return "unknown"


def collaboration_call(name, args, turn):
    return {"type": "function_call", "id": f"collab_{turn}", "call_id": f"call_query_{turn}",
            "name": name, "namespace": "collaboration", "arguments": json.dumps(args)}


def message(text):
    return {"type": "message", "id": "msg_" + text.lower(), "role": "assistant", "status": "completed",
            "content": [{"type": "output_text", "text": text, "annotations": []}]}


def scope_handler(replay):
    pilot = replay["pilot"]

    class Handler(pilot["Provider"]):
        def reply(self, body, actor, turn):
            server = self.server
            native = lambda name, args: collaboration_call(name, args, turn)
            report = lambda name, args: compare["fixture_call"](name, args, turn, code_mode=True)
            if actor == "parent":
                if turn == 1:
                    return report(CAPTURE, {"path": OUTSIDE_PATH_ARGUMENT})
                if turn == 2:
                    ref = json_output(replay["tool_text"](body, 1, code_mode=True))
                    assert compare["valid_reference"](ref), "parent_reference_invalid"
                    server.references[actor] = ref
                    return native("spawn_agent", {"task_name": CHILD_TASK, "message": CHILD_PROMPT, "fork_turns": "none"})
                if turn == 3:
                    spawned = json_output(replay["tool_text"](body, 2))
                    task = spawned.get("task_name")
                    assert task in (CHILD_TASK, "/root/" + CHILD_TASK), "spawn_task_unconfirmed"
                    server.spawned_task = task
                    return native("wait_agent", {"timeout_ms": 25000})
                if turn == 4:
                    wait = json_output(replay["tool_text"](body, 3))
                    assert server.observed["child_file_exact"] and server.child_thread != server.parent_thread, "child_route_unconfirmed"
                    assert wait.get("timed_out") is False and child_notification(body, server.spawned_task), "child_completion_unconfirmed"
                    server.observed["child_completion_confirmed"] = True
                    return report(READ, compare["read_args"](server.references["child"]))
                if turn == 5:
                    value = replay["tool_text"](body, 4, code_mode=True)
                    outcome = foreign_read_outcome(value, server.references["child"])
                    server.foreign_reads[actor] = outcome
                    assert outcome == "denied", "parent_denial_unconfirmed"
                    server.observed["parent_denied_child_archive"] = True
                    return report(READ, compare["read_args"](server.references[actor]))
                value = json_output(replay["tool_text"](body, 5, code_mode=True))
                assert read_exact(value, server.references[actor]), "parent_file_changed"
                server.observed["parent_file_exact"] = True
                server.observed["outside_path_override_ignored"] = True
                return message(PARENT_FINAL)
            if turn == 1:
                assert CHILD_PROMPT in json.dumps(body.get("input")), "child_fixture_prompt_unconfirmed"
                return report(READ, compare["read_args"](server.references["parent"]))
            if turn == 2:
                value = replay["tool_text"](body, 1, code_mode=True)
                outcome = foreign_read_outcome(value, server.references["parent"])
                server.foreign_reads[actor] = outcome
                assert outcome == "denied", "child_denial_unconfirmed"
                server.observed["child_denied_parent_archive"] = True
                return report(CAPTURE, {})
            if turn == 3:
                ref = json_output(replay["tool_text"](body, 2, code_mode=True))
                assert compare["valid_reference"](ref) and ref["archive_id"] != server.references["parent"]["archive_id"], "child_reference_invalid"
                server.references[actor] = ref
                return report(READ, compare["read_args"](ref))
            value = json_output(replay["tool_text"](body, 3, code_mode=True))
            assert read_exact(value, server.references[actor]), "child_file_changed"
            server.observed["child_file_exact"] = True
            return message(CHILD_FINAL)

        def do_POST(self):
            server = self.server
            try:
                size = int(self.headers.get("Content-Length", "0"))
                assert self.path == "/v1/responses" and self.headers.get("Authorization") == "Bearer scope-loopback", "request_scope"
                assert 0 < size <= 262144, "request_size"
                body = json.loads(self.rfile.read(size))
                assert body.get("model") == compare["LIVE_MODEL"] and compare["wire_observation"](body)["code_mode_exec"], "request_contract"
                meta = json.loads(self.headers["x-codex-turn-metadata"])
                thread = str(uuid.UUID(meta["thread_id"]))
                assert OUTSIDE.strip().decode() not in json.dumps(body) and "HEADROOM_OUTSIDE_SCOPE_CANARY" not in json.dumps(body), "outside_content_seen"
                with server.lock:
                    if server.parent_thread is None:
                        server.parent_thread = thread
                    actor = "parent" if thread == server.parent_thread else "child"
                    if actor == "child":
                        assert server.references.get("parent") is not None, "child_started_outside_fixture"
                        if server.child_thread is None:
                            server.child_thread = thread
                        assert thread == server.child_thread, "unexpected_child_thread"
                    server.turns[actor] += 1
                    turn = server.turns[actor]
                    assert turn <= TURN_LIMITS[actor], "request_budget"
                    item = self.reply(body, actor, turn)
                response = {"id": f"resp_scope_{actor}_{turn}", "model": compare["LIVE_MODEL"], "object": "response",
                    "status": "completed", "output": [item], "usage": {"input_tokens": 100, "output_tokens": 5,
                        "total_tokens": 105, "input_tokens_details": {"cached_tokens": 0}}}
                wire = b"".join(pilot["Provider"].render_sse(response))
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream")
                self.send_header("Content-Length", str(len(wire)))
                self.send_header("Connection", "close")
                self.end_headers()
                self.wfile.write(wire)
                self.close_connection = True
            except Exception as error:
                reason = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__.lower()
                server.failure = server.failure or reason
                self.send_error(400)

    return Handler


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--codex", required=True, type=Path)
    parser.add_argument("--catalog", required=True, type=Path)
    parser.add_argument("--share-mcp", action="store_true", help="Test the native MCP sharing feature in the isolated client")
    args = parser.parse_args()
    assert os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace"
    cli = args.codex.resolve(strict=True)
    bin_dir = (Path(os.environ["LOCALAPPDATA"]) / "OpenAI/Codex/bin").resolve(strict=True)
    assert cli.name == "codex.exe" and cli.is_relative_to(bin_dir), "cli_scope"
    version = subprocess.run([str(cli), "--version"], capture_output=True, timeout=10, check=True).stdout.decode().strip()
    assert version == compare["CLI_VERSION"], "cli_version"
    catalog = args.catalog.resolve(strict=True)
    assert catalog.name == "models.json" and catalog.parent.parent == compare["PROOF_ROOT"], "catalog_scope"
    assert json.loads(catalog.with_name("registration.json").read_bytes())["mode"] == "live", "catalog_origin"
    replay = runpy.run_path(str(compare["REPLAY_PATH"]))
    pilot = replay["pilot"]
    pilot["prepare_provider"]()
    core = runpy.run_path(str(ROOT / "tests/runtime/anthropic_usage_capture.py"))["replay"]
    runner = Path(sys.executable).with_name("headroom_runtime.py")
    supervisor = core.load_module("scope_supervisor", runner)
    run = PROOFS / uuid.uuid4().hex
    assert run.resolve().is_relative_to(ROOT), "owned_proof_root"
    work, state = run / "workspace", run / "client"
    work.mkdir(parents=True)
    for name in ("home", "temp"):
        (state / name).mkdir(parents=True)
    approved, outside = work / "approved.jsonl", run / "outside.jsonl"
    approved.write_bytes(REPORT)
    outside.write_bytes(OUTSIDE)
    files = (ENTRY, compare["PROGRAM"], catalog, cli, Path(sys.executable), runner, approved, outside,
             ENTRY.with_name("targeted_query_client.py"), compare["REPLAY_PATH"], ROOT / "bench/optimization/rules_pilot.py")
    before = {str(path): core.sha(path) for path in files}
    with ThreadingHTTPServer(("127.0.0.1", 0), scope_handler(replay)) as server:
        server.parent_thread, server.spawned_task, server.child_thread, server.failure = None, None, None, None
        server.references, server.observed, server.foreign_reads, server.turns = {}, {}, {}, dict.fromkeys(TURN_LIMITS, 0)
        server.lock = threading.Lock()
        threading.Thread(target=server.serve_forever, daemon=True).start()
        config = {"model_provider": "scope", "model_providers.scope.name": "Owned scope fixture",
            "model_providers.scope.base_url": f"http://127.0.0.1:{server.server_port}/v1", "model_providers.scope.wire_api": "responses",
            "model_providers.scope.env_key": "RULES_PILOT_TOKEN", "model_providers.scope.supports_websockets": False,
            "model_providers.scope.requires_openai_auth": False, "model_catalog_json": str(catalog),
            "model_reasoning_effort": compare["LIVE_EFFORT"], "features.multi_agent": True, "features.multi_agent_v2": True,
            "features.mcp_connection_sharing": args.share_mcp, "features.memories": False, "agents.max_threads": 2, "agents.max_depth": 1,
            "project_doc_max_bytes": 0, "log_dir": str(state / "logs"),
            "mcp_servers.headroom_rule_pilot.args": ["-I", "-B", str(ENTRY), "--mcp", str(work)],
            "mcp_servers.headroom_rule_pilot.enabled_tools": [CAPTURE, READ]}
        env = pilot["client_environment"]()
        env.update(CODEX_HOME=str(state / "home"), CODEX_SQLITE_HOME=str(state / "db"), TEMP=str(state / "temp"),
                   TMP=str(state / "temp"), RULES_PILOT_TOKEN="scope-loopback")
        command = pilot["client_args"](cli, work, "counts", "control", compare["LIVE_MODEL"], config,
            prompt="Exercise the native parent and child report scope in this fixed local fixture.")
        try:
            exit_code, output = pilot["run_client"](command, work, env, supervisor, 75)
        except subprocess.TimeoutExpired:
            exit_code, output = -1, b""
        finally:
            server.shutdown()
            compare["remove_client_state"](state)
    after = {str(path): core.sha(path) for path in files}
    records = [json.loads(line) for line in output.splitlines() if line.startswith(b"{")]
    parent_final = any(event.get("type") == "item.completed" and event.get("item", {}).get("text") == PARENT_FINAL for event in records)
    expected = {"parent_file_exact", "child_file_exact", "child_denied_parent_archive", "parent_denied_child_archive",
                "child_completion_confirmed", "outside_path_override_ignored"}
    passed = (exit_code == 0 and server.failure is None and server.turns == TURN_LIMITS and parent_final
              and set(server.observed) == expected and all(server.observed.values()) and before == after)
    result = {"passed": passed, "cli": version, "failure": server.failure, "cli_exit": exit_code, "turns": server.turns,
              "observed": server.observed, "parent_final": parent_final, "external_model_requests": 0,
              "requested_mcp_connection_sharing": args.share_mcp, "foreign_reads": server.foreign_reads,
              "fixture_file_bytes": len(REPORT), "before": before, "after": after}
    with (run / "result.json").open("x", encoding="utf-8") as stream:
        json.dump(result, stream, indent=2)
        stream.write("\n")
    print(json.dumps({**{key: result[key] for key in ("passed", "failure", "cli_exit", "turns", "observed", "foreign_reads")},
                      "proof": (run / "result.json").relative_to(ROOT).as_posix()}))
    return 0 if passed else 1


if __name__ == "__main__":
    if sys.argv[1:2] == ["--mcp"] and len(sys.argv) == 3:
        source = approved_report(Path(sys.argv[2]))
        runpy.run_path(str(ENTRY.with_name("targeted_query_client.py")))["serve"](source=(source, b"", 0), read_tool=READ)
    else:
        raise SystemExit(main())
