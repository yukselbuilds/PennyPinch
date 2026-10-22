"""In-memory local upstream for the pinned Headroom protocol proof."""

from __future__ import annotations

import json
import hashlib
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any


RESPONSES_EVENTS = [
    ("response.created", {"type": "response.created", "response": {"id": "resp-proof", "status": "in_progress", "output": []}}),
    ("response.output_item.added", {"type": "response.output_item.added", "output_index": 0, "item": {"id": "fc-proof", "type": "function_call", "call_id": "call-proof", "name": "inspect", "arguments": ""}}),
    ("response.function_call_arguments.delta", {"type": "response.function_call_arguments.delta", "item_id": "fc-proof", "output_index": 0, "delta": '{"target":'}),
    ("response.function_call_arguments.delta", {"type": "response.function_call_arguments.delta", "item_id": "fc-proof", "output_index": 0, "delta": '"proof","count":3}'}),
    ("response.function_call_arguments.done", {"type": "response.function_call_arguments.done", "item_id": "fc-proof", "output_index": 0, "arguments": '{"target":"proof","count":3}'}),
    ("response.output_item.done", {"type": "response.output_item.done", "output_index": 0, "item": {"id": "fc-proof", "type": "function_call", "call_id": "call-proof", "name": "inspect", "arguments": '{"target":"proof","count":3}'}}),
    ("response.future_extension", {"type": "response.future_extension", "opaque": {"version": 3, "bytes": "AQID"}}),
    ("response.completed", {"type": "response.completed", "response": {"id": "resp-proof", "status": "completed", "output": [{"id": "fc-proof", "type": "function_call", "call_id": "call-proof", "name": "inspect", "arguments": '{"target":"proof","count":3}'}], "usage": {"input_tokens": 12, "output_tokens": 4}}}),
]

ANTHROPIC_EVENTS = [
    ("message_start", {"type": "message_start", "message": {"id": "msg-proof", "type": "message", "role": "assistant", "model": "proof-anthropic-model", "content": [], "stop_reason": None, "stop_sequence": None, "usage": {"input_tokens": 9, "output_tokens": 0}}}),
    ("content_block_start", {"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "tool-proof", "name": "inspect", "input": {}}}),
    ("content_block_delta", {"type": "content_block_delta", "index": 0, "delta": {"type": "input_json_delta", "partial_json": '{"target":'}}),
    ("content_block_delta", {"type": "content_block_delta", "index": 0, "delta": {"type": "input_json_delta", "partial_json": '"proof","count":3}'}}),
    ("content_block_stop", {"type": "content_block_stop", "index": 0}),
    ("content_block_start", {"type": "content_block_start", "index": 1, "content_block": {"type": "future_block", "opaque": {"encoding": "base64", "payload": "AQID"}}}),
    ("content_block_stop", {"type": "content_block_stop", "index": 1}),
    ("future_event", {"type": "future_event", "opaque": {"version": 3, "bytes": "BAUG"}}),
    ("message_delta", {"type": "message_delta", "delta": {"stop_reason": "tool_use", "stop_sequence": None}, "usage": {"output_tokens": 4}}),
    ("message_stop", {"type": "message_stop"}),
]


def _sse_frame(event_name: str, payload: dict[str, Any]) -> bytes:
    data = json.dumps(payload, ensure_ascii=False, separators=(",", ":"))
    return f"event: {event_name}\ndata: {data}\n\n".encode("utf-8")


class _FixtureState:
    def __init__(self) -> None:
        self.lock = threading.Lock()
        self.requests: list[dict[str, Any]] = []

    def add(self, request: dict[str, Any]) -> None:
        with self.lock:
            self.requests.append(request)

    def find(self, case: str) -> dict[str, Any] | None:
        with self.lock:
            return next((r for r in reversed(self.requests) if r["case"] == case), None)

    def count(self, case: str) -> int:
        with self.lock:
            return sum(r["case"] == case for r in self.requests)


class _Server(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = False


class _Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    @property
    def state(self) -> _FixtureState:
        return self.server.state  # type: ignore[attr-defined]

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def do_POST(self) -> None:
        size = int(self.headers.get("content-length", "0"))
        if size < 0 or size > 8_000_000:
            self.send_error(413)
            return
        raw = self.rfile.read(size)
        try:
            body = json.loads(raw)
        except (json.JSONDecodeError, UnicodeDecodeError):
            self.send_error(400)
            return
        if not isinstance(body, dict):
            self.send_error(400)
            return

        case = self.headers.get("x-proof-case", "")
        self.state.add(
            {
                "path": self.path,
                "case": case,
                "body": body,
                "raw_sha256": hashlib.sha256(raw).hexdigest(),
                "authorization": self.headers.get("authorization"),
                "x_api_key": self.headers.get("x-api-key"),
                "anthropic_version": self.headers.get("anthropic-version"),
            }
        )

        if self.path.endswith("/responses"):
            if case == "responses-rate-limit":
                self._json_response(
                    429,
                    {"error": {"type": "rate_limit_error", "message": "controlled fixture throttle"}},
                    {"Retry-After": "1", "x-request-id": "fixture-rate-limit"},
                )
            elif body.get("stream") is True:
                self._sse_response(RESPONSES_EVENTS)
            else:
                self._json_response(200, {"id": "resp-fixture", "status": "completed", "output": []})
            return

        if self.path.endswith("/messages"):
            self._sse_response(ANTHROPIC_EVENTS)
            return

        self._json_response(404, {"error": "unknown fixture route"})

    def _json_response(self, status: int, payload: dict[str, Any], extra: dict[str, str] | None = None) -> None:
        data = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Connection", "close")
        for key, value in (extra or {}).items():
            self.send_header(key, value)
        self.end_headers()
        self.wfile.write(data)

    def _sse_response(self, events: list[tuple[str, dict[str, Any]]]) -> None:
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Transfer-Encoding", "chunked")
        self.send_header("Connection", "close")
        self.end_headers()
        for event_name, payload in events:
            frame = _sse_frame(event_name, payload)
            # Fragment at transport level as well as across tool-argument events.
            for offset in range(0, len(frame), 13):
                chunk = frame[offset : offset + 13]
                self.wfile.write(f"{len(chunk):X}\r\n".encode("ascii"))
                self.wfile.write(chunk + b"\r\n")
                self.wfile.flush()
        self.wfile.write(b"0\r\n\r\n")
        self.wfile.flush()


class LocalUpstreamFixture:
    """Loopback-only upstream that keeps captured requests in memory."""

    def __init__(self) -> None:
        self.state = _FixtureState()
        self.server = _Server(("127.0.0.1", 0), _Handler)
        self.server.state = self.state  # type: ignore[attr-defined]
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    @property
    def port(self) -> int:
        return int(self.server.server_address[1])

    def start(self) -> None:
        self.thread.start()

    def close(self) -> None:
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)
        if self.thread.is_alive():
            raise RuntimeError("fixture-thread-did-not-stop")

    def request(self, case: str) -> dict[str, Any] | None:
        return self.state.find(case)

    def request_count(self, case: str) -> int:
        return self.state.count(case)
