"""Exercise file-bound report tools in real parent and child Codex threads on loopback."""
import argparse
from collections import Counter
from collections.abc import Mapping
from contextvars import ContextVar
import hashlib
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
TURN_HEADER = "x-codex-turn-metadata"
ACTOR_FIELDS = {"threadid", "sessionid", "agentid", "agentname", "agentpath", "parentthreadid"}
ORDINARY_CALL = "call_query_105"


def transport_thread(fields):
    values = [value for key, value in fields.items() if key.replace("_", "").lower() == "threadid"]
    if len(values) != 1 or not isinstance(values[0], str):
        return None
    try:
        thread = uuid.UUID(values[0])
    except ValueError:
        return None
    return str(thread) if thread.int and str(thread) == values[0].lower() else None


def request_metadata(ctx):
    fields = ctx.request_context.meta or {}
    return fields if isinstance(fields, Mapping) else fields.model_dump(exclude_none=True)


def thread_hash(thread):
    return hashlib.sha256(thread.encode("ascii")).hexdigest() if thread else None


def append_probe(path, value):
    with path.open("a", encoding="ascii") as stream:
        stream.write(json.dumps(value, ensure_ascii=True) + "\n")


def mcp_observer(path):
    connection = uuid.uuid4().hex

    def note(tool, ctx):
        fields = request_metadata(ctx)
        labels = {key.replace("_", "").lower() for key in fields}
        append_probe(path, {"tool": tool, "connection": connection, "metadata_fields": len(fields),
            "actor_metadata_fields": sorted(labels & ACTOR_FIELDS), "thread_hash": thread_hash(transport_thread(fields)),
            "transport_request_present": ctx.request_context.request is not None})

    return note


def install_owned_engine_probe(target, directory):
    import httpx
    from headroom.proxy import server

    ingress = Path(directory) / "ingress-summary.jsonl"
    egress = Path(directory) / "egress-summary.jsonl"
    owner = ContextVar("scope_fixture_thread", default=None)
    send = httpx.AsyncClient.send
    create = server.create_app
    remaining = sum(TURN_LIMITS.values())

    async def bounded(client, request, **options):
        nonlocal remaining
        thread = owner.get()
        if remaining == 0 or thread is None or request.method != "POST" or str(request.url) != target:
            return httpx.Response(400, request=request, json={"error": {"message": "Local scope fixture only"}})
        remaining -= 1
        append_probe(egress, {"thread_hash": thread, "body_hash": hashlib.sha256(request.content).hexdigest()})
        return await send(client, request, **{**options, "follow_redirects": False})

    class IngressObserver:
        def __init__(self, app):
            self.app = app

        async def __call__(self, scope, receive, respond):
            if scope["type"] != "http" or scope["path"] != "/v1/responses":
                return await self.app(scope, receive, respond)
            metadata = json.loads(dict(scope["headers"]).get(TURN_HEADER.encode("ascii"), b"{}"))
            thread = thread_hash(transport_thread(metadata))
            append_probe(ingress, {"thread_hash": thread})
            token = owner.set(thread)
            try:
                return await self.app(scope, receive, respond)
            finally:
                owner.reset(token)

    def create_observed(config=None):
        app = create(config)
        app.add_middleware(IngressObserver)
        return app

    httpx.AsyncClient.send = bounded
    server.create_app = create_observed
    return owner


def prepare_engine(core, usage, pilot, supervisor, run, upstream_port, *, preservation=None):
    runtime = ROOT / "runtime"
    controller = core.load_module("scope_engine_controller", runtime / "desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(runtime / "manifest.json")
    assert manifest["engine"]["version"] == "0.44.0" and core.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "engine_pin"
    staged = core.prepare_fixture_runtime(run, payload)
    helper = run / "fixture-runtime/desktop_helper.py"
    source = helper.read_bytes()
    anchor = b'"from headroom.cli import main; main()"'
    assert source.count(anchor) == 1, "engine_bootstrap_changed"
    target = f"http://127.0.0.1:{upstream_port}/v1/responses"
    hook = f"import runpy; scope_owner=runpy.run_path({str(ENTRY)!r})['install_owned_engine_probe']({target!r}, {str(run)!r}); "
    assert preservation in (None, "fixture", "mcp"), "unknown_preservation_mode"
    if preservation is not None:
        policy = ENTRY.with_name("targeted_query_preservation.py")
        hook += f"runpy.run_path({str(policy)!r})['install_preservation'](scope_owner, {str(run)!r}, {preservation!r}); "
    helper.write_bytes(source.replace(anchor, repr(hook).encode("ascii") + b" " + anchor, 1))
    engine = usage["probe"]["UsageEngine"](controller, supervisor, payload, run / "engine", upstream_port, pilot["PLAN"])
    files = [*staged, *[runtime / path.name for path in staged], payload / "runtime-manifest.json", payload / "python313.dll"]
    files.extend(payload / "Lib/site-packages/headroom" / name for name in json.loads((runtime / "optimization-sources.json").read_bytes()))
    return engine, files


def summary_rows(path):
    return [json.loads(line) for line in path.read_bytes().splitlines()] if path.is_file() else []


def approved_report(workspace, expected=REPORT):
    assert workspace.resolve(strict=True) == workspace and workspace.name == "workspace" and workspace.parent.parent == PROOFS, "workspace_outside_fixture"
    path = workspace / "approved.jsonl"
    metadata = path.lstat()
    assert path.resolve(strict=True) == path and stat.S_ISREG(metadata.st_mode) and metadata.st_nlink == 1, "report_not_owned"
    assert not getattr(metadata, "st_file_attributes", 0) & stat.FILE_ATTRIBUTE_REPARSE_POINT, "report_is_reparse_point"
    with path.open("rb") as stream:
        current = os.fstat(stream.fileno())
        assert (current.st_dev, current.st_ino) == (metadata.st_dev, metadata.st_ino), "report_changed_before_open"
        data = stream.read(len(expected) + 1)
    assert data == expected, "report_fixture_changed"
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


def read_exact(value, reference, expected=REPORT):
    return value == {**reference, "stream": "stdout", "offset": 0, "total_bytes": len(expected),
                     "next_offset": len(expected), "exit_code": 0, "text": expected.decode("utf-8")}


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
        def owned_output(self, body, actor, turn):
            server = self.server
            value = json_output(replay["tool_text"](body, turn, code_mode=True))
            assert read_exact(value, server.references[actor], server.source), f"{actor}_file_changed"
            if server.policy is not None:
                identity = f"call_query_{turn}"
                item = server.policy["forwarded_output"](body, identity)
                proof = [getattr(server, actor + "_thread"), identity, server.policy["digest"](item)]
                assert proof in summary_rows(server.run / "native-protected-digests.jsonl"), "native_wrapper_changed"
                server.observed[actor + "_wrapper_preserved"] = True
            server.observed[actor + "_file_exact"] = True

        def reply(self, body, actor, turn):
            server = self.server
            native = lambda name, args: collaboration_call(name, args, turn)

            def report(name, args):
                call = compare["fixture_call"](name, args, turn, code_mode=True)
                reference = server.references.get(actor)
                if server.policy is not None and name == READ and reference is not None and args == compare["read_args"](reference):
                    issued = server.policy["issued_read"](getattr(server, actor + "_thread"), call, args)
                    append_probe(server.run / "issued-reads.jsonl", issued)
                return call

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
                    assert server.observed["child_file_exact"], "child_route_unconfirmed"
                    assert wait.get("timed_out") is False and child_notification(body, server.spawned_task), "child_completion_unconfirmed"
                    server.observed["child_completion_confirmed"] = True
                    return report(READ, compare["read_args"](server.references["child"]))
                if turn == 5:
                    value = replay["tool_text"](body, 4, code_mode=True)
                    outcome = foreign_read_outcome(value, server.references["child"])
                    server.foreign_reads[actor] = outcome
                    assert outcome == "denied", "parent_denial_unconfirmed"
                    server.observed["parent_denied_child_archive"] = True
                    call = report(READ, compare["read_args"](server.references[actor]))
                    if server.policy is not None:
                        text = json.dumps({"text": server.source.decode("utf-8")}, ensure_ascii=True)
                        ordinary = {**call, "id": "fc_ordinary_native", "call_id": ORDINARY_CALL,
                            "input": "text(" + json.dumps({"content": [{"type": "text", "text": text}], "isError": False}) + ");"}
                        return [call, ordinary]
                    return call
                self.owned_output(body, actor, 5)
                if server.policy is not None:
                    other = server.policy["forwarded_output"](body, ORDINARY_CALL)
                    server.ordinary_ccr = server.policy["ccr_hash"](other)
                    server.observed["same_request_ordinary_compacted"] = True
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
            self.owned_output(body, actor, 3)
            return message(CHILD_FINAL)

        def do_POST(self):
            server = self.server
            try:
                size = int(self.headers.get("Content-Length", "0"))
                assert self.path == "/v1/responses" and self.headers.get("Authorization") == "Bearer scope-loopback", "request_scope"
                assert 0 < size <= 262144, "request_size"
                raw = self.rfile.read(size)
                body = json.loads(raw)
                assert body.get("model") == compare["LIVE_MODEL"] and compare["wire_observation"](body)["code_mode_exec"], "request_contract"
                if server.engine_routed:
                    assert TURN_HEADER not in self.headers, "unexpected_upstream_turn_header"
                    body_key = hashlib.sha256(raw).hexdigest()
                    matches = {row["thread_hash"] for row in summary_rows(server.run / "egress-summary.jsonl") if row["body_hash"] == body_key}
                    assert len(matches) == 1 and None not in matches, "upstream_owner_unconfirmed"
                    thread, = matches
                else:
                    thread = thread_hash(transport_thread(json.loads(self.headers[TURN_HEADER])))
                    assert thread is not None, "client_thread_unconfirmed"
                assert "HEADROOM_OUTSIDE_SCOPE_CANARY" not in json.dumps(body), "outside_content_seen"
                with server.lock:
                    if server.parent_thread is None:
                        server.parent_thread = thread
                    actor = "parent" if thread == server.parent_thread else "child"
                    if actor == "child":
                        if server.child_thread is None:
                            server.child_thread = thread
                        assert thread == server.child_thread, "unexpected_child_thread"
                    assert actor == "parent" or server.references.get("parent") is not None, "child_started_outside_fixture"
                    server.turns[actor] += 1
                    turn = server.turns[actor]
                    assert turn <= TURN_LIMITS[actor], "request_budget"
                    item = self.reply(body, actor, turn)
                response = {"id": f"resp_scope_{actor}_{turn}", "model": compare["LIVE_MODEL"], "object": "response",
                    "status": "completed", "output": item if isinstance(item, list) else [item], "usage": {"input_tokens": 100, "output_tokens": 5,
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
    parser.add_argument("--engine", action="store_true", help="Route through the pinned Headroom engine on loopback")
    parser.add_argument("--agent-scope", action="store_true", help="Partition report archives by the MCP request thread")
    parser.add_argument("--native-preservation", action="store_true", help="Trace a large read from MCP through selective engine preservation")
    args = parser.parse_args()
    if args.native_preservation and not (args.engine and args.share_mcp and args.agent_scope):
        parser.error("--native-preservation requires --engine --share-mcp --agent-scope")
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
    usage = runpy.run_path(str(ROOT / "tests/runtime/anthropic_usage_capture.py"))
    core = usage["replay"]
    runner = Path(sys.executable).with_name("headroom_runtime.py")
    supervisor = core.load_module("scope_supervisor", runner)
    run = PROOFS / uuid.uuid4().hex
    assert run.resolve().is_relative_to(ROOT), "owned_proof_root"
    work, state = run / "workspace", run / "client"
    work.mkdir(parents=True)
    for name in ("home", "temp"):
        (state / name).mkdir(parents=True)
    approved, outside = work / "approved.jsonl", run / "outside.jsonl"
    report_data = compare["REPORT"] if args.native_preservation else REPORT
    approved.write_bytes(report_data)
    outside.write_bytes(OUTSIDE)
    files = [ENTRY, compare["PROGRAM"], catalog, cli, Path(sys.executable), runner, approved, outside,
             ENTRY.with_name("targeted_query_client.py"), compare["REPLAY_PATH"], ROOT / "bench/optimization/rules_pilot.py"]
    engine, engine_counts, engine_stop = None, None, None
    policy_path = ENTRY.with_name("targeted_query_preservation.py")
    with ThreadingHTTPServer(("127.0.0.1", 0), scope_handler(replay)) as server:
        server.run, server.engine_routed = run, args.engine
        server.source, server.ordinary_ccr = report_data, None
        server.policy = runpy.run_path(str(policy_path)) if args.native_preservation else None
        server.parent_thread, server.spawned_task, server.child_thread, server.failure = None, None, None, None
        server.references, server.observed, server.foreign_reads, server.turns = {}, {}, {}, dict.fromkeys(TURN_LIMITS, 0)
        server.lock = threading.Lock()
        threading.Thread(target=server.serve_forever, daemon=True).start()
        if args.engine:
            engine, staged = prepare_engine(core, usage, pilot, supervisor, run, server.server_port,
                preservation="mcp" if args.native_preservation else None)
            files.extend(staged)
        if args.native_preservation:
            files.append(policy_path)
        before = {str(path): core.sha(path) for path in files}
        config = {"model_provider": "scope", "model_providers.scope.name": "Owned scope fixture",
            "model_providers.scope.base_url": f"http://127.0.0.1:{server.server_port}/v1", "model_providers.scope.wire_api": "responses",
            "model_providers.scope.env_key": "RULES_PILOT_TOKEN", "model_providers.scope.supports_websockets": False,
            "model_providers.scope.requires_openai_auth": False, "model_catalog_json": str(catalog),
            "model_reasoning_effort": compare["LIVE_EFFORT"], "features.multi_agent": True, "features.multi_agent_v2": True,
            "features.mcp_connection_sharing": args.share_mcp, "features.memories": False, "agents.max_threads": 2, "agents.max_depth": 1,
            "project_doc_max_bytes": 0, "log_dir": str(state / "logs"),
            "mcp_servers.headroom_rule_pilot.args": ["-I", "-B", str(ENTRY), "--mcp", str(work)],
            "mcp_servers.headroom_rule_pilot.enabled_tools": [CAPTURE, READ]}
        if args.agent_scope:
            config["mcp_servers.headroom_rule_pilot.args"].append("--agent-scope")
        if args.native_preservation:
            config["mcp_servers.headroom_rule_pilot.args"].append("--native-preservation")
        env = pilot["client_environment"]()
        env.update(CODEX_HOME=str(state / "home"), CODEX_SQLITE_HOME=str(state / "db"), TEMP=str(state / "temp"),
                   TMP=str(state / "temp"), RULES_PILOT_TOKEN="scope-loopback")
        try:
            if engine is not None:
                engine.start()
                config["model_providers.scope.base_url"] = f"http://127.0.0.1:{engine.port}/v1"
            command = pilot["client_args"](cli, work, "counts", "control", compare["LIVE_MODEL"], config,
                prompt="Exercise the native parent and child report scope in this fixed local fixture.")
            exit_code, output = pilot["run_client"](command, work, env, supervisor, 75)
            if engine is not None:
                engine_counts = {"request_logs": len(usage["probe"]["request_logs"](engine)), "usage_records": len(usage["records"](engine))}
                if server.ordinary_ccr is not None:
                    try:
                        core.retrieval(engine.port, server.ordinary_ccr, report_data.decode("utf-8"), 10)
                        server.observed["ordinary_retrieved_exact"] = True
                    except (AssertionError, KeyError, ValueError):
                        server.failure = "ordinary_retrieval_mismatch"
        except subprocess.TimeoutExpired:
            exit_code, output = -1, b""
        finally:
            try:
                if engine is not None:
                    engine.close()
                    engine_stop = engine.stopped
            finally:
                server.shutdown()
                compare["remove_client_state"](state)
    after = {str(path): core.sha(path) for path in files}
    records = [json.loads(line) for line in output.splitlines() if line.startswith(b"{")]
    parent_final = any(event.get("type") == "item.completed" and event.get("item", {}).get("text") == PARENT_FINAL for event in records)
    expected = {"parent_file_exact", "child_file_exact", "child_denied_parent_archive", "parent_denied_child_archive",
                "child_completion_confirmed", "outside_path_override_ignored"}
    if args.native_preservation:
        expected.update(("parent_wrapper_preserved", "child_wrapper_preserved", "same_request_ordinary_compacted", "ordinary_retrieved_exact"))
    context_rows = summary_rows(run / "mcp-context.jsonl")
    ingress = summary_rows(run / "ingress-summary.jsonl")
    threads = {"parent": server.parent_thread, "child": server.child_thread}
    actor_by_thread = {thread: actor for actor, thread in threads.items() if thread is not None}
    ingress_counts = dict(Counter(actor_by_thread.get(row["thread_hash"], "unknown") for row in ingress))
    has_identity = len(actor_by_thread) == 2
    context_matches = sum(row["thread_hash"] in actor_by_thread for row in context_rows)
    engine_verified = args.engine and has_identity and ingress_counts == TURN_LIMITS and engine_counts == {"request_logs": 10, "usage_records": 10}
    reads = summary_rows(run / "mcp-read-receipts.jsonl")
    native_receipts_verified = not args.native_preservation or len(reads) == 2 and {row[0] for row in reads} == actor_by_thread.keys()
    passed = (exit_code == 0 and server.failure is None and server.turns == TURN_LIMITS and parent_final and native_receipts_verified
              and has_identity and (not args.engine or engine_verified) and context_matches == len(context_rows) == 6
              and set(server.observed) == expected and all(server.observed.values()) and before == after)
    result = {"passed": passed, "cli": version, "failure": server.failure, "cli_exit": exit_code, "turns": server.turns,
              "observed": server.observed, "parent_final": parent_final, "external_model_requests": 0,
              "requested_mcp_connection_sharing": args.share_mcp, "foreign_reads": server.foreign_reads,
              "agent_scope": args.agent_scope,
              "engine_routed": args.engine, "engine_verified": engine_verified, "ingress_requests": ingress_counts,
              "distinct_ingress_threads": len({row["thread_hash"] for row in ingress if row["thread_hash"]}),
              "engine_counts": engine_counts, "engine_stop": engine_stop,
              "mcp_context": {"calls": len(context_rows), "connections": len({row["connection"] for row in context_rows}),
                  "calls_with_actor_metadata": sum(bool(row["actor_metadata_fields"]) for row in context_rows),
                  "calls_matching_client_thread": context_matches,
                  "calls_with_transport_request": sum(row["transport_request_present"] for row in context_rows)},
              "fixture_file_bytes": len(report_data), "native_preservation": args.native_preservation,
              "successful_mcp_read_receipts": len(reads), "before": before, "after": after}
    with (run / "result.json").open("x", encoding="utf-8") as stream:
        json.dump(result, stream, indent=2)
        stream.write("\n")
    print(json.dumps({**{key: result[key] for key in ("passed", "failure", "cli_exit", "turns", "foreign_reads",
                         "engine_verified", "ingress_requests", "mcp_context")},
                      "proof": (run / "result.json").relative_to(ROOT).as_posix()}))
    return 0 if passed else 1


if __name__ == "__main__":
    if len(sys.argv) >= 3 and sys.argv[1] == "--mcp" and sys.argv[3:] in ([], ["--agent-scope"], ["--agent-scope", "--native-preservation"]):
        workspace = Path(sys.argv[2])
        native_enabled = "--native-preservation" in sys.argv[3:]
        source = approved_report(workspace, compare["REPORT"] if native_enabled else REPORT)
        note = mcp_observer(workspace.parent / "mcp-context.jsonl")
        scope = (lambda ctx: transport_thread(request_metadata(ctx))) if "--agent-scope" in sys.argv[3:] else None
        record = None
        if native_enabled:
            digest = runpy.run_path(str(ENTRY.with_name("targeted_query_preservation.py")))["digest"]

            def record(ctx, value):
                thread = thread_hash(transport_thread(request_metadata(ctx)))
                append_probe(workspace.parent / "mcp-read-receipts.jsonl", [thread, digest(value)])

        runpy.run_path(str(ENTRY.with_name("targeted_query_client.py")))["serve"](
            source=(source, b"", 0), read_tool=READ, observe_request=note, request_scope=scope, record_read=record)
    else:
        raise SystemExit(main())
