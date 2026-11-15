"""Additional local protocol comparisons using the existing engine and resource harness."""
from copy import deepcopy
from pathlib import Path
import argparse
import base64
import ctypes
import hashlib
from http.server import ThreadingHTTPServer
import http.client
import importlib.util
import json
import os
import site
import struct
import sys
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("surface_replay", HERE / "replay.py")
replay = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = replay
spec.loader.exec_module(replay)


def conversation(surface, case, model, turns):
    if surface == "responses_websocket":
        rows = replay.fixtures.conversation(case, model, "high", turns)
        for row in rows:
            row.pop("stream")
            row["type"] = "response.create"
        return rows
    tool = "Read" if case in replay.fixtures.PROTECTED else "inspect_workspace"
    if surface == "chat_http_sse":
        body = {"model": model, "stream": True, "reasoning_effort": "high",
                "tools": [{"type": "function", "function": {"name": tool, "description": "Read fixture",
                           "parameters": {"type": "object", "properties": {}}}}],
                "messages": [{"role": "user", "content": "Inspect the synthetic fixture."}]}
    else:
        body = {"model": model, "stream": True, "max_tokens": 2048,
                "thinking": {"type": "enabled", "budget_tokens": 1024},
                "tools": [{"name": tool, "description": "Read fixture",
                           "input_schema": {"type": "object", "properties": {}}}],
                "messages": [{"role": "user", "content": [{"type": "text", "text": "Inspect fixture."}]}]}
    rows = []
    for index in range(1, 4 + turns):
        call_id = f"call-{case}-{index}"
        text = replay.fixtures.output(case, index)
        if surface == "chat_http_sse":
            body["messages"].extend([
                {"role": "assistant", "content": None,
                 "tool_calls": [{"id": call_id, "type": "function",
                                 "function": {"name": tool, "arguments": "{}"}}]},
                {"role": "tool", "tool_call_id": call_id, "content": text},
            ])
        else:
            body["messages"].extend([
                {"role": "assistant", "content": [{"type": "tool_use", "id": call_id, "name": tool, "input": {}}]},
                {"role": "user", "content": [{"type": "tool_result", "tool_use_id": call_id, "content": text}]},
            ])
            if index == 4:
                body["messages"][-1]["content"][0]["cache_control"] = {"type": "ephemeral", "ttl": "1h"}
        if index >= 4:
            rows.append(deepcopy(body))
    return rows


def outputs(body, surface):
    if surface == "responses_websocket":
        return replay.fixtures.outputs(body)
    if surface == "chat_http_sse":
        return {item["tool_call_id"]: item["content"] for item in body["messages"] if item["role"] == "tool"}
    return {block["tool_use_id"]: block["content"] for item in body["messages"]
            for block in item["content"] if block["type"] == "tool_result"}


def calls(body, surface):
    if surface == "chat_http_sse":
        return [(call["id"], call["function"]["name"], call["function"]["arguments"])
                for item in body["messages"] for call in item.get("tool_calls", [])]
    return [(block["id"], block["name"], block["input"]) for item in body["messages"]
            for block in item["content"] if block["type"] == "tool_use"]


def validate(original, forwarded, previous, surface, protected):
    if surface == "responses_websocket":
        replay.validate_body(original, forwarded, previous, protected)
        return
    replay.require(forwarded["model"] == original["model"], "surface_model_changed")
    setting = "reasoning_effort" if surface == "chat_http_sse" else "thinking"
    replay.require(forwarded[setting] == original[setting], "surface_reasoning_changed")
    replay.require(calls(original, surface) == calls(forwarded, surface), "surface_call_identity_changed")
    old, new = outputs(original, surface), outputs(forwarded, surface)
    replay.require(set(old) == set(new), "surface_output_identity_changed")
    if protected:
        replay.require(old == new, "surface_protected_content_changed")
    if previous is not None:
        replay.require(forwarded["messages"][:len(previous["messages"])] == previous["messages"],
                       "surface_prefix_changed")
        replay.require(forwarded["tools"] == previous["tools"], "surface_tools_prefix_changed")


def ws_read(stream):
    def exact(size):
        value = stream.read(size)
        if len(value) != size:
            raise EOFError()
        return value
    first, length = exact(2)
    replay.require(first & 128, "fragmented_fixture_frame")
    masked = bool(length & 128)
    length &= 127
    if length == 126:
        length = struct.unpack("!H", exact(2))[0]
    elif length == 127:
        length = struct.unpack("!Q", exact(8))[0]
    replay.require(length <= 2 * 1024 * 1024, "fixture_frame_limit")
    mask = exact(4) if masked else None
    data = exact(length)
    if mask:
        data = bytes(value ^ mask[index % 4] for index, value in enumerate(data))
    return first & 15, data


def ws_write(stream, opcode, payload):
    data = replay.encoded(payload) if opcode == 1 else payload
    length = len(data)
    if length < 126:
        header = bytes([128 | opcode, length])
    elif length < 65536:
        header = bytes([128 | opcode, 126]) + struct.pack("!H", length)
    else:
        header = bytes([128 | opcode, 127]) + struct.pack("!Q", length)
    stream.write(header + data)
    stream.flush()


class SurfaceHandler(replay.FixtureHandler):
    def capture(self, key, body, surface):
        auth = self.headers.get("x-api-key") == replay.SENTINEL if surface == "anthropic_http_sse" else (
            self.headers.get("Authorization") == "Bearer " + replay.SENTINEL)
        with self.server.capture_lock:
            self.server.captured.setdefault(key, []).append({"body": body, "auth_preserved": auth})

    def do_GET(self):
        replay.require(self.path == "/v1/responses" and self.headers.get("Upgrade", "").lower() == "websocket",
                       "fixture_websocket_route")
        accept = base64.b64encode(hashlib.sha1((self.headers["Sec-WebSocket-Key"]
                                  + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest()).decode()
        self.send_response(101)
        self.send_header("Upgrade", "websocket")
        self.send_header("Connection", "Upgrade")
        self.send_header("Sec-WebSocket-Accept", accept)
        self.end_headers()
        self.connection.settimeout(30)
        self.close_connection = True
        turn = 0
        try:
            while True:
                opcode, data = ws_read(self.rfile)
                if opcode in (8, 9):
                    ws_write(self.wfile, 8 if opcode == 8 else 10, data)
                    if opcode == 8:
                        return
                    continue
                replay.require(opcode == 1, "fixture_text_frame")
                body = json.loads(data)
                replay.require(body["type"] == "response.create", "fixture_create_frame")
                self.capture(self.headers["X-Proof-Case"] + f"-t{turn}", body, "responses_websocket")
                turn += 1
                response = {"id": f"resp-ws-{turn}", "model": body["model"], "status": "in_progress", "output": []}
                for event in [
                    {"type": "response.created", "response": response},
                    {"type": "response.output_text.delta", "item_id": "m", "output_index": 0,
                     "content_index": 0, "delta": "FIXTURE_OK"},
                    {"type": "response.completed", "response": {**response, "status": "completed"}},
                ]:
                    ws_write(self.wfile, 1, event)
        except (OSError, EOFError):
            return

    def do_POST(self):
        size = int(self.headers.get("Content-Length", "0"))
        replay.require(0 < size <= 2 * 1024 * 1024, "surface_request_limit")
        body = json.loads(self.rfile.read(size))
        surface = "anthropic_http_sse" if self.path == "/v1/messages" else "chat_http_sse"
        replay.require(self.path in ("/v1/messages", "/v1/chat/completions"), "surface_http_route")
        self.capture(self.headers["X-Proof-Case"], body, surface)
        self.respond(body, surface)

    def respond(self, body, surface):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Connection", "close")
        self.end_headers()
        if surface == "chat_http_sse":
            events = [
                {"id": "chat-fixture", "object": "chat.completion.chunk", "model": body["model"],
                 "choices": [{"index": 0, "delta": {"content": "FIXTURE_OK"}, "finish_reason": None}]},
                {"id": "chat-fixture", "object": "chat.completion.chunk", "model": body["model"],
                 "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]},
            ]
            for event in events:
                self.wfile.write(b"data: " + replay.encoded(event) + b"\n\n")
                self.wfile.flush()
            self.wfile.write(b"data: [DONE]\n\n")
            self.wfile.flush()
        else:
            events = [
                ("message_start", {"type": "message_start", "message": {"id": "msg-fixture", "type": "message",
                  "role": "assistant", "model": body["model"], "content": [], "usage": {"input_tokens": 1, "output_tokens": 1}}}),
                ("content_block_start", {"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
                ("content_block_delta", {"type": "content_block_delta", "index": 0,
                                         "delta": {"type": "text_delta", "text": "FIXTURE_OK"}}),
                ("content_block_stop", {"type": "content_block_stop", "index": 0}),
                ("message_delta", {"type": "message_delta", "delta": {"stop_reason": "end_turn", "stop_sequence": None},
                                   "usage": {"output_tokens": 1}}),
                ("message_stop", {"type": "message_stop"}),
            ]
            for name, event in events:
                self.wfile.write(b"event: " + name.encode() + b"\ndata: " + replay.encoded(event) + b"\n\n")
                self.wfile.flush()
        self.close_connection = True


def http_request(port, body, key, surface, timeout):
    path = "/v1/messages" if surface == "anthropic_http_sse" else "/v1/chat/completions"
    headers = {"Content-Type": "application/json", "Authorization": "Bearer " + replay.SENTINEL,
               "x-api-key": replay.SENTINEL, "anthropic-version": "2023-06-01",
               "X-Proof-Case": key, "x-headroom-session-id": key.rsplit("-t", 1)[0]}
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=timeout)
    started, first_delta = time.perf_counter(), None
    try:
        connection.request("POST", path, replay.encoded(body), headers)
        response = connection.getresponse()
        first_response = (time.perf_counter() - started) * 1000
        replay.require(response.status == 200, "surface_http_status")
        text, complete = [], False
        while True:
            line = response.readline(65536)
            if not line:
                break
            if not line.startswith(b"data: "):
                continue
            if line.strip() == b"data: [DONE]":
                complete = True
                continue
            event = json.loads(line[6:])
            delta = (event.get("choices", [{}])[0].get("delta", {}).get("content")
                     if surface == "chat_http_sse" else event.get("delta", {}).get("text"))
            if delta:
                first_delta = first_delta if first_delta is not None else (time.perf_counter() - started) * 1000
                text.append(delta)
            if event.get("type") == "message_stop":
                complete = True
        replay.require("".join(text) == "FIXTURE_OK" and complete, "surface_stream_changed")
        return {"status": response.status, "first_response_ms": first_response,
                "first_fixture_delta_ms": first_delta, "total_ms": (time.perf_counter() - started) * 1000,
                "model_ttft_ms": None}
    finally:
        connection.close()


def websocket_request(connection, body, timeout):
    started, first_delta = time.perf_counter(), None
    connection.send(replay.encoded(body).decode())
    text = []
    while True:
        remaining = timeout - (time.perf_counter() - started)
        replay.require(remaining > 0, "websocket_request_timeout")
        event = json.loads(connection.recv(timeout=remaining))
        if event["type"] == "response.output_text.delta":
            first_delta = first_delta if first_delta is not None else (time.perf_counter() - started) * 1000
            text.append(event["delta"])
        if event["type"] == "response.completed":
            break
    replay.require("".join(text) == "FIXTURE_OK", "websocket_stream_changed")
    return {"status": 101, "first_response_ms": None, "first_fixture_delta_ms": first_delta,
            "total_ms": (time.perf_counter() - started) * 1000, "model_ttft_ms": None}


def execute(plan, run, controller, supervisor, payload, tokenizer):
    from websockets.sync.client import connect
    server = ThreadingHTTPServer(("127.0.0.1", 0), SurfaceHandler)
    server.daemon_threads = True
    server.capture_lock, server.captured = threading.Lock(), {}
    threading.Thread(target=server.serve_forever, daemon=True).start()
    rows, engines = [], []
    try:
        for iteration in range(plan["process_runs"]):
            engine = replay.Engine(controller, supervisor, payload, run / f"engine-{iteration}", server.server_port, plan)
            engines.append({})
            try:
                engines[-1].update(engine.start())
                for surface in plan["surfaces"]:
                    for case in plan["cases"]:
                        previous, sockets = None, {}
                        scopes = {arm: f"r{iteration}-{surface}-{case}-{arm}" for arm in ("control", "headroom")}
                        try:
                            if surface == "responses_websocket":
                                for arm in scopes:
                                    port = engine.port if arm == "headroom" else server.server_port
                                    sockets[arm] = connect(
                                        f"ws://127.0.0.1:{port}/v1/responses",
                                        additional_headers={"Authorization": "Bearer " + replay.SENTINEL,
                                                            "X-Proof-Case": scopes[arm],
                                                            "x-headroom-session-id": scopes[arm]},
                                        compression=None, proxy=None, max_size=2 * 1024 * 1024,
                                        open_timeout=30, close_timeout=5, legacy=True)
                            bodies = conversation(surface, case, plan["models"][surface], plan["conversation_turns"])
                            for turn, body in enumerate(bodies):
                                arms = ("control", "headroom") if (iteration + turn) % 2 == 0 else ("headroom", "control")
                                for arm in arms:
                                    key = scopes[arm] + f"-t{turn}"
                                    sampler = replay.Resources([os.getpid()] + (engine.pids if arm == "headroom" else []), 50)
                                    try:
                                        outcome = (websocket_request(sockets[arm], body, 30) if sockets else
                                                   http_request(engine.port if arm == "headroom" else server.server_port,
                                                                body, key, surface, 30))
                                    finally:
                                        resources = sampler.finish()
                                    with server.capture_lock:
                                        captured = server.captured.pop(key, [])
                                    replay.require(len(captured) == 1 and captured[0]["auth_preserved"],
                                                   "surface_upstream_identity")
                                    forwarded = captured[0]["body"]
                                    if arm == "control":
                                        replay.require(forwarded == body, "surface_control_changed")
                                    else:
                                        validate(body, forwarded, previous, surface, case in replay.fixtures.PROTECTED)
                                        previous = forwarded
                                    retrieved = 0
                                    for call_id, content in outputs(forwarded, surface).items():
                                        for content_hash in set(replay.re.findall(
                                                r"(?:Retrieve more: hash=|<<ccr:)([a-fA-F0-9]{12,24})(?=[^a-fA-F0-9]|$)", content)):
                                            replay.require(arm == "headroom", "surface_control_ccr")
                                            replay.retrieval(engine.port, content_hash, outputs(body, surface)[call_id], 30)
                                            retrieved += 1
                                    before_tokens = len(tokenizer.encode(replay.encoded(body).decode(), disallowed_special=()))
                                    after_tokens = len(tokenizer.encode(replay.encoded(forwarded).decode(), disallowed_special=()))
                                    rows.append({
                                        "process_run": iteration, "surface": surface, "case": case, "turn": turn, "arm": arm,
                                        "input_sha256": hashlib.sha256(replay.encoded(body)).hexdigest(),
                                        "forwarded_sha256": hashlib.sha256(replay.encoded(forwarded)).hexdigest(),
                                        "original_tokens": before_tokens, "forwarded_tokens": after_tokens,
                                        "saved_tokens": before_tokens - after_tokens,
                                        "saved_percent": 100 * (before_tokens - after_tokens) / before_tokens,
                                        "ccr_retrievals_verified": retrieved, "prefix_and_integrity": "pass",
                                        "provider_cached_input_tokens": None, "invoice_savings": None,
                                        "response_quality": None, **outcome, **resources,
                                    })
                                    replay.write_new(run / f"row-{len(rows):03}.json", rows[-1])
                        finally:
                            for connection in sockets.values():
                                connection.close()
            finally:
                engine.close()
                engines[-1]["stop"] = engine.stopped
    finally:
        server.shutdown()
        server.server_close()
    checks, statistics = {}, {}
    for surface in plan["surfaces"]:
        selected = [row for row in rows if row["surface"] == surface]
        pairs = {}
        for row in selected:
            pairs.setdefault((row["process_run"], row["case"], row["turn"]), {})[row["arm"]] = row
        extra = [pair["headroom"]["total_ms"] - pair["control"]["total_ms"] for key, pair in pairs.items() if key[2] > 0]
        headroom = [row for row in selected if row["arm"] == "headroom"]
        cpu = [row["engine_cpu_ms"] + row["harness_cpu_ms"] for row in headroom if row["turn"] > 0]
        statistics[surface] = {"warm_added_p95_ms": replay.percentile(extra), "warm_cpu_p95_ms": replay.percentile(cpu)}
        checks[surface] = (
            len(pairs) == plan["process_runs"] * len(plan["cases"]) * plan["conversation_turns"]
            and replay.percentile(extra) <= plan["thresholds"]["warm_added_p95_ms"]
            and replay.percentile(cpu) <= plan["thresholds"]["warm_cpu_p95_ms"]
            and max(row["observed_engine_private_bytes"] for row in headroom) <= plan["thresholds"]["engine_observed_private_bytes"]
            and all(row["saved_percent"] >= 100 * plan["thresholds"]["repetitive_logs_min_saved_fraction"]
                    for row in headroom if row["case"] == "repetitive_logs"))
    return {"passed": all(checks.values()), "checks": checks, "statistics": statistics, "rows": rows,
            "engines": engines, "external_provider_calls": 0, "token_encoding": plan["token_encoding"],
            "token_basis": plan["token_basis"], "stderr_status": "controller_redirected_to_null_contents_unknown"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", action="store_true")
    parser.add_argument("--self-check", action="store_true")
    args = parser.parse_args()
    plan = json.loads((HERE / "plan.json").read_text())
    plan.update(json.loads((HERE / "surfaces-plan.json").read_text()))
    for surface in plan["surfaces"]:
        for case in plan["cases"]:
            rows = conversation(surface, case, plan["models"][surface], plan["conversation_turns"])
            replay.require(len(rows) == plan["conversation_turns"], "surface_fixture_turns")
            previous = None
            for body in rows:
                validate(body, body, previous, surface, case in replay.fixtures.PROTECTED)
                previous = body
    if args.self_check and not args.run:
        print(json.dumps({"passed": True, "surfaces": len(plan["surfaces"]), "fixture_cases": len(plan["cases"]), "network_calls": 0}))
        return 0
    replay.require(args.run and os.name == "nt" and Path.cwd().resolve() == ROOT, "explicit_windows_run_required")
    controller = replay.load_module("surfaces_controller", ROOT / "runtime/desktop_controller.py")
    manifest, payload, _ = controller.load_manifest(ROOT / "runtime/manifest.json")
    replay.require(replay.sha(Path(sys.executable)) == manifest["engine"]["python_sha256"], "surface_pinned_python")
    supervisor_path = payload / manifest["engine"]["runner"]
    supervisor = replay.load_module("surfaces_supervisor", supervisor_path)
    cache = controller.verify_tokenizer_cache(payload)
    os.environ["TIKTOKEN_CACHE_DIR"] = str(cache)
    site.addsitedir(str(payload / "Lib/site-packages"))
    import tiktoken
    tokenizer = tiktoken.get_encoding(plan["token_encoding"])
    run = ROOT / "work/proofs/optimization/surfaces" / str(uuid.uuid4())
    run.mkdir(parents=True)
    sources = [Path(__file__), HERE / "fixtures.py", HERE / "replay.py", HERE / "plan.json",
               HERE / "surfaces-plan.json", ROOT / "runtime/manifest.json", supervisor_path, Path(sys.executable)]
    sources.extend((ROOT / "runtime").glob("*.py"))
    sources.extend((ROOT / "runtime").glob("*.json"))
    sources.extend((payload / "Lib/site-packages/headroom").rglob("*.py"))
    sources.extend((payload / "Lib/site-packages/websockets").rglob("*.py"))
    sources.extend(path for path in cache.iterdir() if path.is_file())
    sources.extend(replay.prepare_fixture_runtime(run, payload))
    before = replay.fingerprints(sources)
    replay.write_new(run / "plan.json", plan)
    result = {"passed": False}
    try:
        result = execute(plan, run, controller, supervisor, payload, tokenizer)
    except BaseException as error:
        result.update(passed=False, error_class=type(error).__name__,
                      error_code=str(error) if isinstance(error, AssertionError) else "bounded_surface_replay_failed")
    finally:
        result.update(before=before, after=replay.fingerprints(sources), fixture_network=replay.fixture_network_evidence(run),
                      environment={"windows_build": list(sys.getwindowsversion()[:3]), "python": sys.version,
                                   "logical_cpus": os.cpu_count()}, run=run.relative_to(ROOT).as_posix())
        result["inputs_unchanged"] = result["before"] == result["after"]
        result["passed"] = (result["passed"] and result["inputs_unchanged"]
                            and result["fixture_network"]["huggingface_hub_payload_bytes"] == 0)
        replay.write_new(run / "result.json", result)
    print(json.dumps({key: value for key, value in result.items() if key not in ("before", "after", "rows", "engines")}))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

