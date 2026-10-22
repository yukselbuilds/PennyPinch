"""Loopback-only Anthropic SSE fixture with a client-visible stream gate."""

from __future__ import annotations

import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any


TEXT_MARKER = "HEADROOM_CLAUDE_FIXTURE_STREAM_OK"
MAX_REQUEST_BYTES = 8_000_000


class FixtureState:
    def __init__(self) -> None:
        self.lock = threading.Lock()
        self.request: dict[str, Any] | None = None
        self.request_count = 0
        self.partial_sent = threading.Event()
        self.partial_observed = threading.Event()
        self.terminal_sent = threading.Event()
        self.gate_timed_out = False
        self.handler_error = False

    def capture(self, metadata: dict[str, Any]) -> None:
        with self.lock:
            self.request = metadata
            self.request_count += 1


class _LoopbackServer(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = False


class _Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    @property
    def state(self) -> FixtureState:
        return self.server.state  # type: ignore[attr-defined]

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def do_HEAD(self) -> None:
        self.send_response(200)
        self.send_header("Content-Length", "0")
        self.send_header("Connection", "close")
        self.end_headers()

    def do_POST(self) -> None:
        if not self.path.split("?", 1)[0].endswith("/messages"):
            self._json_error(404, "not-found")
            return
        try:
            length = int(self.headers.get("content-length", "0"))
            if length < 0 or length > MAX_REQUEST_BYTES:
                self._json_error(413, "request-too-large")
                return
            body = json.loads(self.rfile.read(length))
        except (ValueError, json.JSONDecodeError, UnicodeDecodeError):
            self._json_error(400, "invalid-json")
            return
        if not isinstance(body, dict):
            self._json_error(400, "invalid-json")
            return

        sentinel = self.server.sentinel  # type: ignore[attr-defined]
        x_api_key = self.headers.get("x-api-key")
        authorization = self.headers.get("authorization", "")
        thinking = body.get("thinking")
        output_config = body.get("output_config")
        tools = body.get("tools", [])
        self.state.capture(
            {
                "path": self.path.split("?", 1)[0],
                "model": body.get("model"),
                "stream": body.get("stream") is True,
                "thinking": thinking if isinstance(thinking, dict) else None,
                "effort": output_config.get("effort") if isinstance(output_config, dict) else None,
                "tools_count": len(tools) if isinstance(tools, list) else -1,
                "api_key_matches_sentinel": x_api_key == sentinel
                or authorization == f"Bearer {sentinel}",
                "anthropic_version_present": bool(self.headers.get("anthropic-version")),
                "body_keys": sorted(str(key) for key in body.keys())[:80],
            }
        )
        self._stream_response()

    def _json_error(self, status: int, code: str) -> None:
        body = json.dumps({"type": "error", "error": {"type": code, "message": code}}).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(body)

    @staticmethod
    def _frame(event_name: str, payload: dict[str, Any]) -> bytes:
        encoded = json.dumps(payload, ensure_ascii=False, separators=(",", ":"))
        return f"event: {event_name}\ndata: {encoded}\n\n".encode("utf-8")

    def _stream_response(self) -> None:
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Transfer-Encoding", "chunked")
        self.send_header("Connection", "close")
        self.end_headers()

        initial_events = (
            (
                "message_start",
                {
                    "type": "message_start",
                    "message": {
                        "id": "msg-headroom-fixture",
                        "type": "message",
                        "role": "assistant",
                        "model": "claude-opus-4-6",
                        "content": [],
                        "stop_reason": None,
                        "stop_sequence": None,
                        "usage": {"input_tokens": 12, "output_tokens": 0},
                    },
                },
            ),
            (
                "content_block_start",
                {
                    "type": "content_block_start",
                    "index": 0,
                    "content_block": {"type": "text", "text": ""},
                },
            ),
            (
                "content_block_delta",
                {
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": {"type": "text_delta", "text": TEXT_MARKER},
                },
            ),
        )
        try:
            for name, payload in initial_events:
                self._write_frame(name, payload)
            self.state.partial_sent.set()
            if not self.state.partial_observed.wait(timeout=25):
                self.state.gate_timed_out = True
            terminal_events = (
                (
                    "content_block_stop",
                    {"type": "content_block_stop", "index": 0},
                ),
                (
                    "message_delta",
                    {
                        "type": "message_delta",
                        "delta": {"stop_reason": "end_turn", "stop_sequence": None},
                        "usage": {"output_tokens": 4},
                    },
                ),
                ("message_stop", {"type": "message_stop"}),
            )
            for name, payload in terminal_events:
                self._write_frame(name, payload)
            self._write_chunk(b"")
            self.state.terminal_sent.set()
        except (BrokenPipeError, ConnectionResetError, OSError):
            self.state.handler_error = True

    def _write_frame(self, event_name: str, payload: dict[str, Any]) -> None:
        self._write_chunk(self._frame(event_name, payload))

    def _write_chunk(self, data: bytes) -> None:
        if not data:
            self.wfile.write(b"0\r\n\r\n")
            self.wfile.flush()
            return
        self.wfile.write(f"{len(data):X}\r\n".encode("ascii"))
        self.wfile.write(data + b"\r\n")
        self.wfile.flush()


class LocalAnthropicFixture:
    """Captures request metadata only and serves one gated synthetic stream."""

    def __init__(self, sentinel: str) -> None:
        self.state = FixtureState()
        self.server = _LoopbackServer(("127.0.0.1", 0), _Handler)
        self.server.state = self.state  # type: ignore[attr-defined]
        self.server.sentinel = sentinel  # type: ignore[attr-defined]
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)

    @property
    def port(self) -> int:
        return int(self.server.server_address[1])

    def start(self) -> None:
        self.thread.start()

    def release_terminal(self) -> None:
        self.state.partial_observed.set()

    def close(self) -> None:
        self.state.partial_observed.set()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)
        if self.thread.is_alive():
            raise RuntimeError("fixture-thread-did-not-stop")
