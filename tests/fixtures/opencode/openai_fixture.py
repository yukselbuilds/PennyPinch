"""Loopback OpenAI Chat Completions fixture for the pinned OpenCode V1 CLI."""

from __future__ import annotations

import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any


OUTPUT_MARKER = "HEADROOM_OPENCODE_FIXTURE_TEXT_OK"
MAX_REQUEST_BYTES = 8_000_000


class FixtureState:
    def __init__(self) -> None:
        self.lock = threading.Lock()
        self.requests: list[dict[str, Any]] = []
        self.chat_request_count = 0
        self.sse_chunks_sent = 0
        self.terminal_sent = threading.Event()
        self.handler_error = False

    @property
    def request_count(self) -> int:
        with self.lock:
            return len(self.requests)

    def capture(self, metadata: dict[str, Any]) -> None:
        with self.lock:
            self.requests.append(metadata)
            if metadata.get("method") == "POST" and str(metadata.get("path", "")).endswith("/chat/completions"):
                self.chat_request_count += 1

    def snapshot(self) -> list[dict[str, Any]]:
        with self.lock:
            return list(self.requests)


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
        self._capture_nonbody("HEAD")
        self.send_response(200)
        self.send_header("Content-Length", "0")
        self.send_header("Connection", "close")
        self.end_headers()

    def do_GET(self) -> None:
        path = self.path.split("?", 1)[0]
        self._capture_nonbody("GET")
        if path.endswith("/models"):
            payload = {
                "object": "list",
                "data": [{"id": "headroom-opencode-fixture", "object": "model", "created": 0, "owned_by": "fixture"}],
            }
            self._json_response(200, payload)
        else:
            self._json_response(404, {"error": {"type": "not_found", "message": "not-found"}})

    def do_POST(self) -> None:
        path = self.path.split("?", 1)[0]
        try:
            length = int(self.headers.get("content-length", "0"))
            if length < 0 or length > MAX_REQUEST_BYTES:
                self._json_response(413, {"error": {"type": "request_too_large", "message": "request-too-large"}})
                return
            body = json.loads(self.rfile.read(length))
        except (ValueError, json.JSONDecodeError, UnicodeDecodeError):
            self._json_response(400, {"error": {"type": "invalid_request", "message": "invalid-json"}})
            return
        if not isinstance(body, dict):
            self._json_response(400, {"error": {"type": "invalid_request", "message": "invalid-json"}})
            return

        auth = self.headers.get("authorization", "")
        messages = body.get("messages")
        tools = body.get("tools")
        stream_options = body.get("stream_options")
        self.state.capture(
            {
                "method": "POST",
                "path": path,
                "model": body.get("model") if isinstance(body.get("model"), str) else None,
                "stream": body.get("stream") is True,
                "authorization_matches_sentinel": auth == f"Bearer {self.server.sentinel}",  # type: ignore[attr-defined]
                "message_count": len(messages) if isinstance(messages, list) else -1,
                "message_roles": [item.get("role") for item in messages if isinstance(item, dict) and isinstance(item.get("role"), str)][:16] if isinstance(messages, list) else [],
                "tools_count": len(tools) if isinstance(tools, list) else 0,
                "tool_choice_present": "tool_choice" in body,
                "stream_usage_requested": isinstance(stream_options, dict) and stream_options.get("include_usage") is True,
                "body_keys": sorted(str(key) for key in body)[:64],
            }
        )
        if not path.endswith("/chat/completions"):
            self._json_response(404, {"error": {"type": "not_found", "message": "unsupported-fixture-route"}})
            return
        if body.get("stream") is not True:
            self._json_response(400, {"error": {"type": "invalid_request", "message": "stream-required"}})
            return
        self._stream_response(body.get("model") if isinstance(body.get("model"), str) else "unknown-model")

    def _capture_nonbody(self, method: str) -> None:
        self.state.capture(
            {
                "method": method,
                "path": self.path.split("?", 1)[0],
                "authorization_matches_sentinel": self.headers.get("authorization", "")
                == f"Bearer {self.server.sentinel}",  # type: ignore[attr-defined]
            }
        )

    def _json_response(self, status: int, payload: dict[str, Any]) -> None:
        data = json.dumps(payload, separators=(",", ":")).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(data)

    @staticmethod
    def _event(model: str, choices: list[dict[str, Any]], usage: dict[str, int] | None = None) -> bytes:
        payload: dict[str, Any] = {
            "id": "chatcmpl-headroom-opencode-fixture",
            "object": "chat.completion.chunk",
            "created": int(time.time()),
            "model": model,
            "choices": choices,
        }
        if usage is not None:
            payload["usage"] = usage
        return b"data: " + json.dumps(payload, separators=(",", ":")).encode("utf-8") + b"\n\n"

    def _stream_response(self, model: str) -> None:
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream; charset=utf-8")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Transfer-Encoding", "chunked")
        self.send_header("Connection", "close")
        self.end_headers()
        frames = (
            self._event(
                model,
                [{"index": 0, "delta": {"role": "assistant"}, "finish_reason": None}],
            ),
            self._event(
                model,
                [{"index": 0, "delta": {"content": OUTPUT_MARKER}, "finish_reason": None}],
            ),
            self._event(
                model,
                [{"index": 0, "delta": {}, "finish_reason": "stop"}],
            ),
            self._event(
                model,
                [],
                {"prompt_tokens": 12, "completion_tokens": 4, "total_tokens": 16},
            ),
            b"data: [DONE]\n\n",
        )
        try:
            for frame in frames:
                self._write_chunk(frame)
                with self.state.lock:
                    self.state.sse_chunks_sent += 1
            self._write_chunk(b"")
            self.state.terminal_sent.set()
        except (BrokenPipeError, ConnectionResetError, OSError):
            self.state.handler_error = True

    def _write_chunk(self, data: bytes) -> None:
        if not data:
            self.wfile.write(b"0\r\n\r\n")
            self.wfile.flush()
            return
        self.wfile.write(f"{len(data):X}\r\n".encode("ascii"))
        self.wfile.write(data + b"\r\n")
        self.wfile.flush()


class LocalOpenAICompatFixture:
    """Captures request metadata only and returns a synthetic streamed reply."""

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

    def close(self) -> None:
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)
        if self.thread.is_alive():
            raise RuntimeError("fixture-thread-did-not-stop")
