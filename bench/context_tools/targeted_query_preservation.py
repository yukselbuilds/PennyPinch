"""Prove byte-exact protection of one recognized code-mode result in the pinned engine."""
import argparse
from collections import Counter
from contextvars import ContextVar
from hashlib import sha256
from http.server import ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import runpy
import sys
import threading
import uuid


ENTRY = Path(__file__).resolve()
ROOT = ENTRY.parents[2]
scope = runpy.run_path(str(ENTRY.with_name("targeted_query_scope.py")))
compare = scope["compare"]
replay = runpy.run_path(str(compare["REPLAY_PATH"]))
REPORT = compare["REPORT"]
OWNED = "call_query_1"
ORDINARY = "call_ordinary_1"
SUMMARY = "HEADROOM_PRESERVATION_FIXTURE_OK"


def digest(value):
    encoded = json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":")).encode("ascii")
    return sha256(encoded).hexdigest()


def receipt(thread, call, output):
    return [thread, call["call_id"], digest(call), digest(output)]


def issued_read(thread, call, args):
    invocation = {key: call[key] for key in ("type", "namespace", "name", "input")}
    return [thread, call["call_id"], digest(invocation), digest(args)]


def native_read_value(output):
    identity = output.get("call_id")
    match = re.fullmatch(r"call_query_([1-9][0-9]{0,3})", identity) if isinstance(identity, str) else None
    if match is None:
        return None
    parts = output.get("output")
    if isinstance(parts, list) and (not 1 <= len(parts) <= 2
            or any(not isinstance(part, dict) or set(part) != {"type", "text"} for part in parts)):
        return None
    try:
        wrapper = json.loads(replay["tool_text"]({"input": [output]}, int(match[1]), code_mode=True))
        if not isinstance(wrapper, dict) or wrapper.keys() - {"content", "isError"} or wrapper.get("isError", False) is not False:
            return None
        content = wrapper["content"]
        if not isinstance(content, list) or len(content) != 1 or set(content[0]) != {"type", "text"} or content[0]["type"] != "text":
            return None
        value = json.loads(content[0]["text"])
    except (AssertionError, KeyError, TypeError, ValueError):
        return None
    return value if isinstance(value, dict) and {"archive_id", "stream", "offset"} <= value.keys() else None


def native_read_matches(thread, call, output, issued, reads):
    value = native_read_value(output)
    if value is None or [thread, digest(value)] not in reads:
        return False
    args = {key: value[key] for key in ("archive_id", "stream", "offset")}
    return issued_read(thread, call, args) in issued


def protected_scripts(body, thread, receipts, *, native_match=None):
    entries = body.get("input")
    if thread is None or not isinstance(entries, list):
        return set()
    accepted = {tuple(row) for row in receipts if row[0] == thread}
    items = [item for item in entries if isinstance(item, dict)]
    calls = [item for item in items if item.get("type") in ("function_call", "custom_tool_call")]
    outputs = [item for item in items if item.get("type") in ("function_call_output", "custom_tool_call_output")]
    call_counts = Counter(item.get("call_id") for item in calls if isinstance(item.get("call_id"), str))
    output_counts = Counter(item.get("call_id") for item in outputs if isinstance(item.get("call_id"), str))
    arguments = [item.get("arguments") or item.get("input") for item in calls]
    scripts = Counter(value for value in arguments if isinstance(value, str))
    by_id = {item["call_id"]: item for item in outputs if isinstance(item.get("call_id"), str)}
    protected = set()
    for call in calls:
        identity, script = call.get("call_id"), call.get("input")
        if (call.get("type") != "custom_tool_call" or call.get("namespace") != "functions" or call.get("name") != "exec"
                or not isinstance(identity, str) or not isinstance(script, str)):
            continue
        if call_counts[identity] != 1 or output_counts[identity] != 1 or scripts[script] != 1:
            continue
        output = by_id[identity]
        if output.get("type") == "custom_tool_call_output" and (tuple(receipt(thread, call, output)) in accepted
                or native_match is not None and native_match(thread, call, output)):
            protected.add(script)
    return protected


def install_preservation(owner, directory, mode):
    from headroom.proxy.handlers import openai

    path = Path(directory)
    rows = scope["summary_rows"]
    grants = rows(path / "owned-read-receipts.jsonl") if mode == "fixture" else []
    active = ContextVar("owned_read_scripts", default=frozenset())
    original = openai.OpenAIHandlerMixin._compress_openai_responses_payload
    unwrap = openai.unwrap_tool_call_name

    def payload(proxy, body, **kwargs):
        if mode == "mcp":
            issued, reads = rows(path / "issued-reads.jsonl"), rows(path / "mcp-read-receipts.jsonl")
            selected = protected_scripts(body, owner.get(), (), native_match=
                lambda thread, call, output: native_read_matches(thread, call, output, issued, reads))
            for call in body.get("input", []):
                if isinstance(call, dict) and isinstance(call.get("input"), str) and call["input"] in selected:
                    output = forwarded_output(body, call["call_id"])
                    scope["append_probe"](path / "native-protected-digests.jsonl", [owner.get(), call["call_id"], digest(output)])
        else:
            selected = protected_scripts(body, owner.get(), grants)
        token = active.set(frozenset(selected))
        try:
            return original(proxy, body, **kwargs)
        finally:
            active.reset(token)

    def tool_name(name, arguments):
        if name == "exec" and isinstance(arguments, str) and arguments in active.get():
            return "Read"
        return unwrap(name, arguments)

    openai.OpenAIHandlerMixin._compress_openai_responses_payload = payload
    openai.unwrap_tool_call_name = tool_name


def read_pair():
    client = runpy.run_path(str(ENTRY.with_name("targeted_query_client.py")))
    output = client["query"].ToolOutput(compare["CAPTURE"], "", REPORT, b"", 0)
    archive = client["ReportSession"](output)
    reference = archive.capture("mcp:owned-read-fixture")
    args = compare["read_args"](reference)
    value = archive.read_part(args["archive_id"], args["stream"], args["offset"])
    assert value["text"].encode("utf-8") == REPORT and value["next_offset"] == len(REPORT), "approved_read_is_incomplete"
    call = compare["fixture_call"](compare["READ"], args, 1, code_mode=True)
    text = client["encode"]({"content": [{"type": "text", "text": client["encode"](value)}], "isError": False})
    result = {"type": "custom_tool_call_output", "call_id": OWNED,
              "output": [{"type": "input_text", "text": "Sandbox execution\nOutput:\n" + text}]}
    return call, result


def request_body(call, output, case):
    tools = {"type": "additional_tools", "tools": [{"type": "namespace", "name": "functions", "tools": [
        {"type": "custom", "name": "exec", "format": {"type": "text"}, "description": "Fixed local fixture"}]}]}
    prompt = {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "Check the fixed synthetic report."}]}
    items = [tools, prompt, call, output]
    if case == "mixed":
        items.extend([{**call, "id": "fc_ordinary_1", "call_id": ORDINARY, "input": "text(fixture_ordinary_result);"},
                      {**output, "call_id": ORDINARY}])
    return {"model": compare["LIVE_MODEL"], "reasoning": {"effort": compare["LIVE_EFFORT"]}, "stream": True,
            "store": False, "tool_choice": "auto", "input": items, "prompt_cache_key": f"owned-preservation-{case}"}


def forwarded_output(body, call_id):
    values = [item for item in body["input"] if item.get("type") == "custom_tool_call_output" and item.get("call_id") == call_id]
    assert len(values) == 1, "unique_forwarded_result_required"
    return values[0]


def ccr_hash(item):
    markers = re.findall(r"<<ccr:([a-f0-9]{12,24}),[^>]*>>", json.dumps(item))
    assert len(markers) == 1, "one_ccr_reference_required"
    return markers[0]


def handler(pilot):
    class Fixture(pilot["Provider"]):
        def do_POST(self):
            size = int(self.headers.get("Content-Length", "0"))
            if self.path != "/v1/responses" or self.headers.get("Authorization") != "Bearer scope-loopback" or not 0 < size <= 262144:
                self.send_error(400)
                return
            if scope["TURN_HEADER"] in self.headers or len(self.server.bodies) >= 3:
                self.send_error(400)
                return
            body = json.loads(self.rfile.read(size))
            self.server.bodies.append(body)
            model = body["model"]
            response = {"id": f"resp_preservation_{len(self.server.bodies)}", "model": model, "object": "response", "status": "completed",
                        "output": [scope["message"](SUMMARY)], "usage": {"input_tokens": 100, "output_tokens": 5,
                            "total_tokens": 105, "input_tokens_details": {"cached_tokens": 0}}}
            frames = b"".join(pilot["Provider"].render_sse(response))
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(frames)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(frames)
            self.close_connection = True

    return Fixture


def main():
    argparse.ArgumentParser(description=__doc__).parse_args()
    assert os.name == "nt" and Path.cwd().resolve() == ROOT == Path(r"D:\Projeler\Headroom"), "canonical_workspace"
    usage_path = ROOT / "tests/runtime/anthropic_usage_capture.py"
    usage = runpy.run_path(str(usage_path))
    core = usage["replay"]
    pilot_path = ROOT / "bench/optimization/rules_pilot.py"
    pilot = runpy.run_path(str(pilot_path))
    pilot["prepare_provider"]()
    runner = Path(sys.executable).with_name("headroom_runtime.py")
    supervisor = core.load_module("preservation_supervisor", runner)
    run = scope["PROOFS"] / ("preservation-" + uuid.uuid4().hex)
    assert run.resolve().is_relative_to(ROOT), "owned_proof_root"
    run.mkdir(parents=True)
    owner, other = str(uuid.uuid4()), str(uuid.uuid4())
    call, output = read_pair()
    grants = run / "owned-read-receipts.jsonl"
    scope["append_probe"](grants, receipt(scope["thread_hash"](owner), call, output))
    result, failure, stop = {}, None, None
    with ThreadingHTTPServer(("127.0.0.1", 0), handler(pilot)) as server:
        server.bodies = []
        threading.Thread(target=server.serve_forever, daemon=True).start()
        engine, staged = scope["prepare_engine"](core, usage, pilot, supervisor, run, server.server_port, preservation="fixture")
        files = (ENTRY, scope["ENTRY"], compare["PROGRAM"], ENTRY.with_name("targeted_query_client.py"),
                 usage_path, pilot_path, runner, Path(sys.executable), grants, *staged)
        before = {str(path): core.sha(path) for path in files}
        try:
            engine.start()
            cases = (("mixed", owner, call, output), ("other_thread", other, call, output),
                     ("changed_call", owner, {**call, "id": "fc_other", "call_id": "call_other"}, {**output, "call_id": "call_other"}))
            for name, thread, candidate, value in cases:
                body = request_body(candidate, value, name)
                headers = {"Content-Type": "application/json", "Authorization": "Bearer scope-loopback", "User-Agent": "codex-cli",
                           scope["TURN_HEADER"]: json.dumps({"thread_id": thread, "session_id": thread}), "X-Headroom-Session-Id": name}
                status, _, wire = usage["probe"]["exchange"](engine.port, "POST", "/v1/responses", headers, core.encoded(body))
                assert status == 200 and SUMMARY.encode("ascii") in wire, "fixture_response_incomplete"
                forwarded = server.bodies[-1]
                actual = forwarded_output(forwarded, candidate["call_id"])
                if name == "mixed":
                    assert actual == value, "owned_read_bytes_changed"
                    other_output = forwarded_output(forwarded, ORDINARY)
                    marker = ccr_hash(other_output)
                    core.retrieval(engine.port, marker, REPORT.decode("utf-8"), 10)
                    result[name] = {"owned_exact": True, "other_compacted": True, "other_retrieved_exact": True}
                else:
                    assert actual != value and ccr_hash(actual), "unexpected_read_protection"
                    result[name] = {"compacted": True}
            assert len(usage["probe"]["request_logs"](engine)) == len(usage["records"](engine)) == len(cases), "usage_mismatch"
        except Exception as error:
            failure = str(error) if isinstance(error, AssertionError) and re.fullmatch(r"[a-z0-9_]+", str(error)) else type(error).__name__.lower()
        finally:
            try:
                engine.close()
                stop = engine.stopped
            finally:
                server.shutdown()
    after = {str(path): core.sha(path) for path in files}
    passed = failure is None and len(result) == len(server.bodies) == 3 and before == after
    data = {"passed": passed, "failure": failure, "engine": "0.44.0", "report_bytes": len(REPORT), "cases": result,
            "local_requests": len(server.bodies), "external_model_requests": 0, "stop": stop, "before": before, "after": after}
    core.write_new(run / "result.json", data)
    print(json.dumps({**{name: data[name] for name in ("passed", "failure", "local_requests", "cases")},
                      "proof": (run / "result.json").relative_to(ROOT).as_posix()}))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
