"""OpenCode Chat fixture; fixed MCP data comes from the accepted Claude fixture."""
from __future__ import annotations

import hashlib
import http.client
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("opencode_shared_mcp_data", ROOT / "tests/fixtures/claude/tool_fixture.py")
data = importlib.util.module_from_spec(spec)
spec.loader.exec_module(data)
LOGS, WITNESS = data.LOGS, data.WITNESS
FINAL = "HEADROOM_OPENCODE_TOOL_CCR_OK"
MODEL = "headroom-opencode-tool-fixture"
RESPONSES_MODEL = "gpt-4.1"
CLIENT_TOOL = "headroom_fixture_inspect_logs"
CLIENT_ID, SECOND_ID, RETRIEVE_ID = "call_headroom_logs_01", "call_headroom_witness_02", "call_headroom_retrieve_01"


def wire_text(value):
    if isinstance(value, str):
        return value
    assert isinstance(value, list) and all(block.get("type") == "text" for block in value), "tool_content_shape_changed"
    return "".join(block["text"] for block in value)


def tool_text(value):
    value = wire_text(value)
    try:
        parsed = json.loads(value)
    except (ValueError, TypeError):
        return value
    if isinstance(parsed, dict) and isinstance(parsed.get("content"), list):
        assert not parsed.get("isError"), "mcp_error_result"
        return wire_text(parsed["content"])
    return value


def result(body, call_id):
    matches = [message for message in body.get("messages", [])
               if message.get("role") == "tool" and message.get("tool_call_id") == call_id]
    assert len(matches) == 1, "tool_result_missing"
    return wire_text(matches[0]["content"])


def use(call_id, name, arguments):
    return {"id": call_id, "type": "function", "function": {"name": name, "arguments": json.dumps(arguments)}}


class Conversation:
    def __init__(self, model=MODEL):
        self.step, self.complete, self.rows, self.error = 0, False, [], None
        self.model = model
        self.reasoning = None
        self.original_wire_result = LOGS

    def advance(self, body):
        assert body.get("model") == self.model, "wire_model_changed"
        reasoning = {key: body.get(key) for key in ("reasoning_effort", "reasoning") if key in body}
        if self.reasoning is None:
            self.reasoning = reasoning
        assert reasoning == self.reasoning, "reasoning_changed"
        assert any(tool.get("function", {}).get("name") == CLIENT_TOOL for tool in body.get("tools", [])), "client_tool_missing"
        row = {"step": self.step, "upstream_stream": body.get("stream", False),
               "wire_model_preserved": True, "reasoning_fields_preserved": True}
        if self.step == 0:
            reply = {"role": "assistant", "content": None, "tool_calls": [use(CLIENT_ID, CLIENT_TOOL, {"kind": "logs"})]}
        elif self.step == 1:
            compressed = result(body, CLIENT_ID)
            marker = re.search(r"(?:hash=|<<ccr:)([a-fA-F0-9]{12,24})(?=[^a-fA-F0-9]|$)", compressed)
            assert marker and compressed != self.original_wire_result, "compression_marker_missing"
            assert any(tool.get("function", {}).get("name") == "headroom_retrieve" for tool in body.get("tools", [])), "retrieval_tool_missing"
            self.content_hash = marker.group(1)
            row.update(original_wire_bytes=len(self.original_wire_result.encode()), compressed_wire_bytes=len(compressed.encode()),
                       original_wire_sha256=hashlib.sha256(self.original_wire_result.encode()).hexdigest(),
                       compression_marker=True, client_call_id_preserved=True)
            reply = {"role": "assistant", "content": None,
                     "tool_calls": [use(RETRIEVE_ID, "headroom_retrieve", {"hash": self.content_hash})]}
        elif self.step == 2:
            restored = json.loads(result(body, RETRIEVE_ID))
            assert restored.get("hash") == self.content_hash and restored.get("original_content") == self.original_wire_result, "retrieval_changed"
            assert tool_text(restored["original_content"]) == LOGS, "retrieved_log_changed"
            row.update(retrieved_wire_byte_exact=True, retrieved_log_byte_exact=True,
                       retrieved_wire_sha256=hashlib.sha256(restored["original_content"].encode()).hexdigest())
            reply = {"role": "assistant", "content": None,
                     "tool_calls": [use(SECOND_ID, CLIENT_TOOL, {"kind": "witness"})]}
        elif self.step == 3:
            assert tool_text(result(body, SECOND_ID)) == WITNESS, "second_tool_changed"
            row["second_call_id_preserved"] = True
            reply = {"role": "assistant", "content": FINAL}
            self.complete = True
        else:
            raise AssertionError("unexpected_extra_request")
        self.rows.append(row)
        self.step += 1
        return reply


def response(message, step, stream, model=MODEL):
    base = {"id": f"chatcmpl_headroom_{step}", "object": "chat.completion.chunk" if stream else "chat.completion",
            "created": 1, "model": model}
    finish = "tool_calls" if message.get("tool_calls") else "stop"
    usage = {"prompt_tokens": 12, "completion_tokens": 4, "total_tokens": 16}
    if not stream:
        return json.dumps({**base, "choices": [{"index": 0, "message": message, "finish_reason": finish}], "usage": usage}).encode()
    frames = [{**base, "choices": [{"index": 0, "delta": {"role": "assistant"}, "finish_reason": None}]}]
    if message.get("tool_calls"):
        call = message["tool_calls"][0]
        arguments = call["function"]["arguments"]
        middle = len(arguments) // 2
        frames.extend([{**base, "choices": [{"index": 0, "delta": {"tool_calls": [
            {"index": 0, **call, "function": {**call["function"], "arguments": arguments[:middle]}}]}, "finish_reason": None}]},
            {**base, "choices": [{"index": 0, "delta": {"tool_calls": [
                {"index": 0, "function": {"arguments": arguments[middle:]}}]}, "finish_reason": None}]}])
    else:
        middle = len(message["content"]) // 2
        frames.extend({**base, "choices": [{"index": 0, "delta": {"content": part}, "finish_reason": None}]}
                      for part in (message["content"][:middle], message["content"][middle:]))
    frames.extend([{**base, "choices": [{"index": 0, "delta": {}, "finish_reason": finish}]},
                   {**base, "choices": [], "usage": usage}])
    return b"".join(b"data: " + json.dumps(frame).encode() + b"\n\n" for frame in frames) + b"data: [DONE]\n\n"


def normalize_request(body, surface):
    if surface == "chat":
        return body
    assert isinstance(body.get("input"), list), "responses_input_missing"
    messages = []
    for item in body["input"]:
        if item.get("type") == "function_call_output":
            messages.append({"role": "tool", "tool_call_id": item["call_id"], "content": item["output"]})
        elif item.get("type") == "function_call":
            messages.append({"role": "assistant", "content": None, "tool_calls": [
                use(item["call_id"], item["name"], json.loads(item["arguments"]))]})
        elif item.get("role"):
            messages.append(item)
    tools = [{"type": "function", "function": {"name": tool["name"], "parameters": tool.get("parameters", {})}}
             for tool in body.get("tools", []) if tool.get("type") == "function"]
    return {**body, "messages": messages, "tools": tools}


def responses_response(message, step, stream):
    if message.get("tool_calls"):
        call = message["tool_calls"][0]
        item = {"type": "function_call", "id": f"fc_headroom_{step}", "call_id": call["id"],
                "name": call["function"]["name"], "arguments": call["function"]["arguments"], "status": "completed"}
    else:
        item = {"type": "message", "id": f"msg_headroom_{step}", "role": "assistant", "status": "completed",
                "content": [{"type": "output_text", "text": message["content"], "annotations": []}]}
    final = {"id": f"resp_headroom_{step}", "object": "response", "created_at": 1,
             "model": RESPONSES_MODEL, "status": "completed", "output": [item], "error": None,
             "incomplete_details": None, "usage": {"input_tokens": 12, "output_tokens": 4, "total_tokens": 16,
                 "input_tokens_details": {"cached_tokens": 0}, "output_tokens_details": {"reasoning_tokens": 0}}}
    if not stream:
        return json.dumps(final).encode()
    initial = {**final, "status": "in_progress", "output": [], "usage": None}
    events = [{"type": "response.created", "response": initial},
              {"type": "response.output_item.added", "output_index": 0,
               "item": {**item, "arguments": "", "status": "in_progress"} if item["type"] == "function_call"
               else {**item, "content": [], "status": "in_progress"}}]
    if item["type"] == "function_call":
        middle = len(item["arguments"]) // 2
        for part in (item["arguments"][:middle], item["arguments"][middle:]):
            events.append({"type": "response.function_call_arguments.delta", "item_id": item["id"],
                           "output_index": 0, "delta": part})
        events.append({"type": "response.function_call_arguments.done", "item_id": item["id"],
                       "output_index": 0, "arguments": item["arguments"]})
    else:
        events.append({"type": "response.content_part.added", "item_id": item["id"], "output_index": 0,
                       "content_index": 0, "part": {"type": "output_text", "text": "", "annotations": []}})
        middle = len(message["content"]) // 2
        for part in (message["content"][:middle], message["content"][middle:]):
            events.append({"type": "response.output_text.delta", "item_id": item["id"], "output_index": 0,
                           "content_index": 0, "delta": part})
        events.extend([{"type": "response.output_text.done", "item_id": item["id"], "output_index": 0,
                        "content_index": 0, "text": message["content"]},
                       {"type": "response.content_part.done", "item_id": item["id"], "output_index": 0,
                        "content_index": 0, "part": item["content"][0]}])
    events.extend([{"type": "response.output_item.done", "output_index": 0, "item": item},
                   {"type": "response.completed", "response": final}])
    return b"".join(("event: " + event["type"] + "\ndata: " + json.dumps({**event, "sequence_number": index})
                     + "\n\n").encode() for index, event in enumerate(events))


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):
        pass

    def do_CONNECT(self):
        self.server.blocked_connects += 1
        self.send_error(502, "local_fixture_only")

    def do_POST(self):
        try:
            expected_path = "/v1/responses" if self.server.surface == "responses" else "/v1/chat/completions"
            assert self.path.split("?", 1)[0] == expected_path, "unexpected_route"
            assert self.headers.get("authorization") == "Bearer " + self.server.sentinel, "synthetic_auth_changed"
            length = int(self.headers.get("content-length", "0"))
            assert 0 < length <= 4 * 1024 * 1024, "request_size_changed"
            body = json.loads(self.rfile.read(length))
            message = self.server.conversation.advance(normalize_request(body, self.server.surface))
            self.server.conversation.rows[-1]["synthetic_auth_preserved"] = True
        except (AssertionError, ValueError, TypeError, KeyError) as error:
            if self.server.conversation.error is None:
                self.server.conversation.error = str(error) if isinstance(error, AssertionError) else "fixture_invalid_request"
            self.send_error(400, "fixture_rejected")
            return
        stream = body.get("stream", False)
        raw = responses_response(message, self.server.conversation.step, stream) if self.server.surface == "responses" else response(
            message, self.server.conversation.step, stream, self.server.conversation.model)
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream" if stream else "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(raw)
        self.wfile.flush()


class Upstream(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = False

    def __init__(self, sentinel, surface="chat"):
        super().__init__(("127.0.0.1", 0), Handler)
        self.surface = surface
        self.sentinel, self.conversation, self.blocked_connects = sentinel, Conversation(RESPONSES_MODEL if surface == "responses" else MODEL), 0


def serve_retrieval(receipt, port):
    """Fixture bridge to the exact test-owned engine; no caller URL/path/credentials."""
    calls = 0
    for raw in sys.stdin.buffer:
        assert len(raw) <= 65536, "retrieval_request_too_large"
        message = json.loads(raw)
        if "id" not in message:
            continue
        method = message.get("method")
        if method == "initialize":
            value = {"protocolVersion": message.get("params", {}).get("protocolVersion", "2024-11-05"),
                     "capabilities": {"tools": {}}, "serverInfo": {"name": "headroom-fixture-retrieval", "version": "1"}}
        elif method == "tools/list":
            value = {"tools": [{"name": "retrieve", "description": "Retrieve original fixture content from its local Headroom runtime.",
                "inputSchema": {"type": "object", "properties": {"hash": {"type": "string"}}, "required": ["hash"],
                                "additionalProperties": False}}]}
        elif method == "tools/call":
            params = message.get("params", {})
            arguments = params.get("arguments", {})
            assert params.get("name") == "retrieve" and set(arguments) == {"hash"}, "retrieval_arguments_invalid"
            key = arguments["hash"]
            assert isinstance(key, str) and re.fullmatch(r"[a-fA-F0-9]{12,24}", key), "retrieval_hash_invalid"
            connection = http.client.HTTPConnection("127.0.0.1", port, timeout=10)
            try:
                connection.request("POST", "/v1/retrieve", json.dumps({"hash": key}), {"Content-Type": "application/json"})
                response_value = connection.getresponse()
                assert response_value.status == 200, "local_retrieval_failed"
                raw_value = response_value.read(1024 * 1024 + 1)
                assert len(raw_value) <= 1024 * 1024, "local_retrieval_too_large"
                restored = json.loads(raw_value)
            finally:
                connection.close()
            assert restored.get("hash") == key and isinstance(restored.get("original_content"), str), "local_retrieval_shape_changed"
            calls += 1
            assert calls == 1, "unexpected_repeat_retrieval"
            temporary = receipt.with_suffix(".tmp")
            temporary.write_text(json.dumps({"calls": calls,
                "retrieved_wire_sha256": hashlib.sha256(restored["original_content"].encode()).hexdigest()}) + "\n")
            temporary.replace(receipt)
            value = {"content": [{"type": "text", "text": json.dumps(restored)}], "isError": False}
        elif method == "ping":
            value = {}
        else:
            sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": message["id"],
                                        "error": {"code": -32601, "message": "unsupported_fixture_method"}}) + "\n")
            sys.stdout.flush()
            continue
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": message["id"], "result": value}) + "\n")
        sys.stdout.flush()
