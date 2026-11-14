"""Synthetic MCP tool and loopback Anthropic continuation, never a real model."""
from __future__ import annotations

import argparse
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import http.client
import json
from pathlib import Path
import re
import sys

MODEL = "claude-opus-4-6"
EFFORT = "high"
CLIENT_TOOL = "mcp__headroom_fixture__inspect_logs"
CLIENT_ID = "toolu_headroom_logs_01"
SECOND_ID = "toolu_headroom_witness_02"
RETRIEVE_ID = "toolu_headroom_retrieve_01"
FINAL = "HEADROOM_CLAUDE_TOOL_CCR_OK"
WITNESS = "HEADROOM_SECOND_TOOL_OK"
LOGS = "\n".join(
    f"2026-01-01T00:00:{n % 60:02d}.{n % 1000:03d}Z INFO worker-{n % 8:03d} "
    f"request completed status=200 latency_ms=17 region=fixture trace_id={n:016d}"
    for n in range(120)
)


def text(content):
    if isinstance(content, str):
        return content
    assert isinstance(content, list), "invalid_content_shape"
    return "".join(block["text"] for block in content if block.get("type") == "text")


def result(body, call_id):
    matches = [block for message in body.get("messages", [])
               if isinstance(message.get("content"), list) for block in message["content"]
               if block.get("type") == "tool_result" and block.get("tool_use_id") == call_id]
    assert len(matches) == 1 and not matches[0].get("is_error", False), "client_tool_result_missing"
    return text(matches[0]["content"])


def use(call_id, name, arguments):
    return {"type": "tool_use", "id": call_id, "name": name, "input": arguments}


class Conversation:
    def __init__(self):
        self.step = 0
        self.complete = False
        self.rows = []
        self.error = None
        self.rejected_observation = None
        self.thinking_digest = None

    def advance(self, body):
        assert body.get("model") == MODEL, "model_changed"
        assert body.get("output_config", {}).get("effort") == EFFORT, "effort_changed"
        digest = hashlib.sha256(json.dumps(body.get("thinking"), sort_keys=True).encode()).hexdigest()
        if self.thinking_digest is None:
            self.thinking_digest = digest
        assert digest == self.thinking_digest, "thinking_changed"
        assert any(tool.get("name") == CLIENT_TOOL for tool in body.get("tools", [])), "client_tool_missing"
        row = {"step": self.step, "upstream_stream": body.get("stream", False),
               "model_preserved": True, "effort_preserved": True, "thinking_preserved": True}
        if self.step == 0:
            blocks = [use(CLIENT_ID, CLIENT_TOOL, {"kind": "logs"})]
        elif self.step == 1:
            compressed = result(body, CLIENT_ID)
            self.rejected_observation = {
                "text_bytes": len(compressed.encode()), "equals_original": compressed == LOGS,
                "original_bytes": len(LOGS.encode()), "text_sha256": hashlib.sha256(compressed.encode()).hexdigest(),
                "message_count": len(body.get("messages", [])),
                "cache_marker_count": sum(
                    "cache_control" in block for message in body.get("messages", [])
                    if isinstance(message.get("content"), list) for block in message["content"]),
                "retrieve_tool_present": any(tool.get("name") == "headroom_retrieve" for tool in body.get("tools", [])),
            }
            marker = re.search(r"(?:hash=|<<ccr:)([a-fA-F0-9]{12,24})(?=[^a-fA-F0-9]|$)", compressed)
            assert marker and compressed != LOGS, "compression_marker_missing"
            assert any(tool.get("name") == "headroom_retrieve" for tool in body.get("tools", [])), "retrieval_tool_missing"
            self.content_hash = marker.group(1)
            row.update(compressed_bytes=len(compressed.encode()), original_bytes=len(LOGS.encode()),
                       original_sha256=hashlib.sha256(LOGS.encode()).hexdigest(),
                       client_call_id_preserved=True, compression_marker=True)
            self.rejected_observation = None
            blocks = [use(RETRIEVE_ID, "headroom_retrieve", {"hash": self.content_hash})]
        elif self.step == 2:
            self.rejected_observation = {"stage": "retrieval_continuation", "tool_results": [
                {"id": block.get("tool_use_id") if block.get("tool_use_id") in {CLIENT_ID, SECOND_ID, RETRIEVE_ID} else "other",
                 "is_error": bool(block.get("is_error", False))}
                for message in body.get("messages", []) if isinstance(message.get("content"), list)
                for block in message["content"] if block.get("type") == "tool_result"]}
            retrieved = json.loads(result(body, RETRIEVE_ID))
            assert retrieved.get("hash") == self.content_hash and "error" not in retrieved, "retrieval_hash_or_status_changed"
            restored = retrieved.get("original_content")
            assert restored == LOGS, "retrieved_content_changed"
            self.rejected_observation = None
            row.update(retrieved_byte_exact=True, retrieved_sha256=hashlib.sha256(restored.encode()).hexdigest())
            blocks = [use(SECOND_ID, CLIENT_TOOL, {"kind": "witness"})]
        elif self.step == 3:
            assert result(body, SECOND_ID) == WITNESS, "second_tool_result_changed"
            row.update(second_client_call_id_preserved=True)
            blocks = [{"type": "text", "text": FINAL}]
            self.complete = True
        else:
            raise AssertionError("unexpected_extra_request")
        self.rows.append(row)
        self.step += 1
        return {"id": f"msg_headroom_fixture_{self.step}", "type": "message", "role": "assistant",
                "model": MODEL, "content": blocks, "stop_reason": "end_turn" if self.complete else "tool_use",
                "stop_sequence": None, "usage": {"input_tokens": 12, "output_tokens": 4}}


def sse(message):
    def frame(name, payload):
        return f"event: {name}\ndata: {json.dumps(payload, separators=(',', ':'))}\n\n".encode()
    start = {**message, "content": [], "stop_reason": None, "usage": {"input_tokens": 12, "output_tokens": 0}}
    yield frame("message_start", {"type": "message_start", "message": start})
    for index, block in enumerate(message["content"]):
        tool = block["type"] == "tool_use"
        initial = {**block, "input": {}} if tool else {"type": "text", "text": ""}
        yield frame("content_block_start", {"type": "content_block_start", "index": index, "content_block": initial})
        value = json.dumps(block["input"], separators=(',', ':')) if tool else block["text"]
        middle = len(value) // 2
        for part in (value[:middle], value[middle:]):
            delta = {"type": "input_json_delta", "partial_json": part} if tool else {"type": "text_delta", "text": part}
            yield frame("content_block_delta", {"type": "content_block_delta", "index": index, "delta": delta})
        yield frame("content_block_stop", {"type": "content_block_stop", "index": index})
    yield frame("message_delta", {"type": "message_delta", "delta": {"stop_reason": message["stop_reason"],
                "stop_sequence": None}, "usage": {"output_tokens": 4}})
    yield frame("message_stop", {"type": "message_stop"})


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):
        pass

    def do_CONNECT(self):
        self.server.blocked_connects += 1
        self.send_error(502, "local_fixture_only")

    def do_POST(self):
        try:
            assert self.path.split("?", 1)[0] == "/v1/messages", "unexpected_route"
            size = int(self.headers.get("content-length", "0"))
            assert 0 < size <= 4 * 1024 * 1024, "request_size_invalid"
            assert (self.headers.get("x-api-key") == self.server.sentinel or
                    self.headers.get("authorization") == "Bearer " + self.server.sentinel), "sentinel_changed"
            assert self.headers.get("anthropic-version"), "anthropic_version_missing"
            body = json.loads(self.rfile.read(size))
            message = self.server.conversation.advance(body)
            self.server.conversation.rows[-1]["synthetic_auth_preserved"] = True
        except (AssertionError, ValueError, KeyError, TypeError) as error:
            if self.server.conversation.error is None:
                self.server.conversation.error = str(error) if isinstance(error, AssertionError) else "fixture_invalid_request"
            self.send_error(400, "fixture_rejected")
            return
        stream = body.get("stream", False)
        data = b"".join(sse(message)) if stream else json.dumps(message).encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream" if stream else "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(data)
        self.wfile.flush()


class Upstream(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = False

    def __init__(self, sentinel):
        super().__init__(("127.0.0.1", 0), Handler)
        self.sentinel = sentinel
        self.conversation = Conversation()
        self.blocked_connects = 0


class IngressHandler(BaseHTTPRequestHandler):
    """Observe native client bytes before the motor; forward SSE without buffering a turn."""
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):
        pass

    def do_POST(self):
        connection = None
        try:
            assert self.path.split("?", 1)[0] == "/v1/messages", "unexpected_client_route"
            size = int(self.headers.get("content-length", "0"))
            assert 0 < size <= 4 * 1024 * 1024, "client_request_too_large"
            raw = self.rfile.read(size)
            body = json.loads(raw)
            original_result = None
            for message in body.get("messages", []):
                if isinstance(message.get("content"), list):
                    for block in message["content"]:
                        if block.get("type") == "tool_result" and block.get("tool_use_id") == CLIENT_ID:
                            original_result = text(block["content"])
            if original_result is not None:
                assert original_result == LOGS, "client_result_offloaded_or_changed"
            self.server.rows.append({"model_preserved": body.get("model") == MODEL,
                                     "effort_preserved": body.get("output_config", {}).get("effort") == EFFORT,
                                     "native_stream": body.get("stream") is True,
                                     "original_log_present_byte_exact": original_result == LOGS,
                                     "original_log_bytes": len(original_result.encode()) if original_result is not None else None})
            connection = http.client.HTTPConnection("127.0.0.1", self.server.engine_port, timeout=40)
            headers = {key: value for key, value in self.headers.items()
                       if key.lower() not in {"host", "connection", "transfer-encoding", "content-length"}}
            connection.request("POST", self.path, raw, headers)
            response = connection.getresponse()
            self.send_response(response.status)
            for key, value in response.getheaders():
                if key.lower() not in {"connection", "transfer-encoding", "content-length"}:
                    self.send_header(key, value)
            self.send_header("Transfer-Encoding", "chunked")
            self.send_header("Connection", "close")
            self.end_headers()
            total = 0
            while chunk := response.read1(16384):
                total += len(chunk)
                assert total <= 4 * 1024 * 1024, "client_response_too_large"
                self.wfile.write(f"{len(chunk):x}\r\n".encode() + chunk + b"\r\n")
                self.wfile.flush()
            self.wfile.write(b"0\r\n\r\n")
            self.wfile.flush()
        except (AssertionError, OSError, ValueError) as error:
            self.server.error = str(error) if isinstance(error, AssertionError) else "local_relay_io_failed"
            if connection is None:
                self.send_error(400, "fixture_client_rejected")
            self.close_connection = True
        finally:
            if connection is not None:
                connection.close()


class Ingress(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = False

    def __init__(self, engine_port):
        super().__init__(("127.0.0.1", 0), IngressHandler)
        self.engine_port, self.rows, self.error = engine_port, [], None


def serve_mcp(receipt_path):
    """A single fixed tool; no shell, network, user file access or free-form output."""
    calls = []
    for raw in sys.stdin.buffer:
        assert len(raw) <= 64 * 1024, "mcp_request_too_large"
        message = json.loads(raw)
        method = message.get("method")
        if "id" not in message:
            continue
        if method == "initialize":
            result_value = {"protocolVersion": message.get("params", {}).get("protocolVersion", "2024-11-05"),
                            "capabilities": {"tools": {}}, "serverInfo": {"name": "headroom-fixture", "version": "1"}}
        elif method == "tools/list":
            result_value = {"tools": [{"name": "inspect_logs", "description": "Return synthetic fixture data only.",
                "inputSchema": {"type": "object", "properties": {"kind": {"type": "string", "enum": ["logs", "witness"]}},
                                "required": ["kind"], "additionalProperties": False}}]}
        elif method == "tools/call":
            params = message.get("params", {})
            assert params.get("name") == "inspect_logs", "unexpected_mcp_tool"
            arguments = params.get("arguments")
            assert arguments in ({"kind": "logs"}, {"kind": "witness"}), "unexpected_mcp_arguments"
            kind = arguments["kind"]
            calls.append(kind)
            assert calls in (["logs"], ["logs", "witness"]), "unexpected_mcp_sequence"
            temporary = receipt_path.with_suffix(".tmp")
            temporary.write_text(json.dumps({"calls": calls}) + "\n", encoding="utf-8")
            temporary.replace(receipt_path)
            result_value = {"content": [{"type": "text", "text": LOGS if kind == "logs" else WITNESS}], "isError": False}
        elif method == "ping":
            result_value = {}
        else:
            sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": message["id"],
                                        "error": {"code": -32601, "message": "unsupported_fixture_method"}}) + "\n")
            sys.stdout.flush()
            continue
        sys.stdout.write(json.dumps({"jsonrpc": "2.0", "id": message["id"], "result": result_value}) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--mcp-receipt", required=True, type=Path)
    options = parser.parse_args()
    target = options.mcp_receipt.resolve()
    allowed = Path(__file__).resolve().parents[3] / "work/proofs/claude-tools/runs"
    assert target.is_relative_to(allowed) and target.parent.is_dir(), "receipt_outside_test_run"
    serve_mcp(target)
