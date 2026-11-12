"""Isolated loopback Responses/SSE and WebSocket fixture for macOS Desktop QA."""

from __future__ import annotations

import base64
import hashlib
import json
import socket
import struct
import threading
import time
from collections import Counter
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit


MAX_BODY_BYTES = 1024 * 1024
MAX_HEADER_BYTES = 16 * 1024
MAX_JSON_DEPTH = 32
MAX_JSON_NODES = 16_384
REQUEST_TIMEOUT_SECONDS = 5.0
TERMINAL_RELEASE_TIMEOUT_SECONDS = 120.0
ERROR_CODES = frozenset(
    {
        "auth_mismatch",
        "model_mismatch",
        "reasoning_mismatch",
        "nonce_missing",
        "invalid_json",
        "invalid_input",
        "body_too_large",
        "stream_required",
        "duplicate_response",
        "invalid_websocket",
        "release_timeout",
        "client_disconnected",
    }
)


def _no_duplicate_keys(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate_json_key")
        result[key] = value
    return result


def _parse_json(data: bytes):
    def bounded(value, depth=0, count=None):
        if count is None:
            count = [0]
        count[0] += 1
        if depth > MAX_JSON_DEPTH or count[0] > MAX_JSON_NODES:
            raise ValueError("json_limit")
        if isinstance(value, dict):
            for child in value.values():
                bounded(child, depth + 1, count)
        elif isinstance(value, list):
            for child in value:
                bounded(child, depth + 1, count)

    value = json.loads(data.decode("utf-8", "strict"), object_pairs_hook=_no_duplicate_keys)
    bounded(value)
    return value


def _input_text_values(value, depth=0, count=None):
    if count is None:
        count = [0]
    count[0] += 1
    if depth > MAX_JSON_DEPTH or count[0] > MAX_JSON_NODES:
        raise ValueError("input_limit")
    if isinstance(value, str):
        return [value]
    if isinstance(value, list):
        values = []
        for item in value:
            values.extend(_input_text_values(item, depth + 1, count))
        return values
    if isinstance(value, dict):
        values = []
        for key in ("text", "content", "input_text"):
            if key in value:
                values.extend(_input_text_values(value[key], depth + 1, count))
        return values
    return []


def _is_bounded_token(value: str, maximum: int) -> bool:
    return (
        isinstance(value, str)
        and 1 <= len(value) <= maximum
        and value.isascii()
        and all(32 <= ord(char) < 127 for char in value)
    )


class _FixtureState:
    def __init__(self, api_key, model, reasoning_effort, nonce, release_event):
        self.api_key = api_key
        self.model = model
        self.reasoning_effort = reasoning_effort
        self.nonce = nonce
        self.final_marker = f"HEADROOM_FIXTURE_RESULT:{nonce}"
        self.partial_marker = f"HEADROOM_FIXTURE_PARTIAL:{nonce}"
        self.release_event = release_event
        self.closed_event = threading.Event()
        self.lock = threading.Lock()
        self.active_sockets: set[socket.socket] = set()
        self.transport_counts = Counter()
        self.check_counts = Counter()
        self.error_counts = Counter()
        self.request_count = 0
        self.accepted_response_count = 0
        self.rejected_request_count = 0
        self.partial_sent = False
        self.terminal_sent = False
        self.closed = False

    def register_socket(self, connection):
        with self.lock:
            self.active_sockets.add(connection)

    def unregister_socket(self, connection):
        with self.lock:
            self.active_sockets.discard(connection)

    def record_transport(self, name):
        with self.lock:
            self.transport_counts[name] += 1

    def record_checks(self, **checks):
        with self.lock:
            for name, matched in checks.items():
                self.check_counts[f"{name}_{'match' if matched else 'mismatch'}"] += 1

    def error(self, code):
        if code not in ERROR_CODES:
            code = "invalid_input"
        with self.lock:
            self.error_counts[code] += 1
            self.rejected_request_count += 1
            return code

    def reserve_response(self):
        with self.lock:
            if self.accepted_response_count:
                return False
            self.accepted_response_count = 1
            return True

    def mark_partial(self):
        with self.lock:
            self.partial_sent = True

    def mark_terminal(self):
        with self.lock:
            self.terminal_sent = True

    def report(self):
        with self.lock:
            return {
                "transport_counts": dict(sorted(self.transport_counts.items())),
                "auth_match_count": self.check_counts["auth_match"],
                "auth_mismatch_count": self.check_counts["auth_mismatch"],
                "model_match_count": self.check_counts["model_match"],
                "model_mismatch_count": self.check_counts["model_mismatch"],
                "reasoning_match_count": self.check_counts["reasoning_match"],
                "reasoning_mismatch_count": self.check_counts["reasoning_mismatch"],
                "input_nonce_match_count": self.check_counts["input_nonce_match"],
                "input_nonce_mismatch_count": self.check_counts["input_nonce_mismatch"],
                "request_count": self.request_count,
                "accepted_response_count": self.accepted_response_count,
                "rejected_request_count": self.rejected_request_count,
                "partial_sent": self.partial_sent,
                "terminal_sent": self.terminal_sent,
                "error_counts": dict(sorted(self.error_counts.items())),
                "closed": self.closed,
            }


class _FixtureHTTPServer(ThreadingHTTPServer):
    allow_reuse_address = False
    daemon_threads = False
    block_on_close = True

    def handle_error(self, _request, _client_address):
        return


class _FixtureHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    @property
    def state(self) -> _FixtureState:
        return self.server.fixture_state  # type: ignore[attr-defined]

    def setup(self):
        super().setup()
        self.connection.settimeout(REQUEST_TIMEOUT_SECONDS)
        self.state.register_socket(self.connection)

    def finish(self):
        try:
            super().finish()
        finally:
            self.state.unregister_socket(self.connection)

    def log_message(self, _format, *_args):
        return

    def parse_request(self):
        if not super().parse_request():
            return False
        header_bytes = len(self.requestline.encode("iso-8859-1", "replace"))
        header_bytes += sum(len(name) + len(value) + 4 for name, value in self.headers.items())
        if len(self.path) > 2048 or header_bytes > MAX_HEADER_BYTES:
            self._send_error(400, "invalid_input")
            return False
        return True

    def do_GET(self):
        route = urlsplit(self.path).path
        if route in ("/models", "/v1/models"):
            self.state.record_transport("model_list")
            if not self._check_auth():
                self._send_error(401, "auth_mismatch")
                return
            self._send_json(
                200,
                {"object": "list", "data": [{"id": self.state.model, "object": "model", "owned_by": "local-fixture"}]},
            )
            return
        if route in ("/responses", "/v1/responses") and self.headers.get("Upgrade", "").lower() == "websocket":
            self.state.record_transport("websocket")
            self._serve_websocket()
            return
        self._send_error(404, "invalid_input")

    def do_POST(self):
        route = urlsplit(self.path).path
        if route not in ("/responses", "/v1/responses"):
            self._send_error(404, "invalid_input")
            return
        self.state.record_transport("http_sse")
        if not self._check_auth():
            self._send_error(401, "auth_mismatch")
            return
        body = self._read_json_body()
        if body is None:
            return
        if body.get("stream") is not True:
            self._send_error(400, "stream_required")
            return
        validation_error = self._validate_response_request(body)
        if validation_error:
            self._send_error(self._error_status(validation_error), validation_error)
            return
        self._serve_response("http_sse")

    def _check_auth(self):
        auth_values = self.headers.get_all("Authorization", [])
        matched = len(auth_values) == 1 and auth_values[0] == f"Bearer {self.state.api_key}"
        self.state.record_checks(auth=matched)
        return matched

    def _read_json_body(self):
        content_lengths = self.headers.get_all("Content-Length", [])
        if len(content_lengths) != 1 or self.headers.get("Transfer-Encoding") is not None:
            self._send_error(400, "invalid_input")
            return None
        raw_length = content_lengths[0]
        if not raw_length.isascii() or not raw_length.isdigit():
            self._send_error(400, "invalid_input")
            return None
        length = int(raw_length)
        if length <= 0:
            self._send_error(400, "invalid_input")
            return None
        if length > MAX_BODY_BYTES:
            self._send_error(413, "body_too_large")
            return None
        try:
            data = self.rfile.read(length)
            if len(data) != length:
                self._send_error(400, "invalid_json")
                return None
            body = _parse_json(data)
        except (UnicodeDecodeError, json.JSONDecodeError, RecursionError, ValueError):
            self._send_error(400, "invalid_json")
            return None
        if not isinstance(body, dict):
            self._send_error(400, "invalid_input")
            return None
        return body

    @staticmethod
    def _error_status(code):
        return 413 if code == "body_too_large" else 401 if code == "auth_mismatch" else 409 if code == "duplicate_response" else 400

    def _validate_response_request(self, body):
        with self.state.lock:
            self.state.request_count += 1
        model_matches = body.get("model") == self.state.model
        reasoning = body.get("reasoning")
        effort_matches = isinstance(reasoning, dict) and reasoning.get("effort") == self.state.reasoning_effort
        try:
            if "input" not in body:
                raise ValueError("input_missing")
            texts = _input_text_values(body["input"])
            total_text = sum(len(text.encode("utf-8")) for text in texts)
            if total_text > MAX_BODY_BYTES:
                raise ValueError("input_limit")
            nonce_present = any(self.state.nonce in text for text in texts)
        except (UnicodeEncodeError, ValueError):
            self.state.record_checks(model=model_matches, reasoning=effort_matches)
            return "invalid_input"
        self.state.record_checks(model=model_matches, reasoning=effort_matches, input_nonce=nonce_present)
        if not model_matches:
            return "model_mismatch"
        if not effort_matches:
            return "reasoning_mismatch"
        if not nonce_present:
            return "nonce_missing"
        return None

    def _serve_websocket(self):
        if not self._check_auth():
            self._send_error(401, "auth_mismatch")
            return
        key_values = self.headers.get_all("Sec-WebSocket-Key", [])
        version_values = self.headers.get_all("Sec-WebSocket-Version", [])
        connection_tokens = {token.strip().lower() for token in self.headers.get("Connection", "").split(",")}
        if (
            len(key_values) != 1
            or len(version_values) != 1
            or version_values[0] != "13"
            or "upgrade" not in connection_tokens
            or not self._valid_ws_key(key_values[0])
        ):
            self._send_error(400, "invalid_websocket")
            return
        self.close_connection = True
        self.send_response(101)
        self.send_header("Upgrade", "websocket")
        self.send_header("Connection", "Upgrade")
        accept = base64.b64encode(
            hashlib.sha1((key_values[0] + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode("ascii")).digest()
        ).decode("ascii")
        self.send_header("Sec-WebSocket-Accept", accept)
        self.end_headers()
        try:
            request = _parse_json(self._read_ws_text_frame())
        except (OSError, EOFError, ValueError, UnicodeDecodeError, json.JSONDecodeError, RecursionError, struct.error):
            self.state.error("invalid_websocket")
            return
        if not isinstance(request, dict) or request.get("type") != "response.create":
            self.state.error("invalid_websocket")
            return
        validation_error = self._validate_response_request(request)
        if validation_error:
            self.state.error(validation_error)
            try:
                self.connection.sendall(b"\x88\x02\x03\xf0")
            except OSError:
                pass
            return
        self._serve_response("websocket")

    @staticmethod
    def _valid_ws_key(value):
        try:
            return len(base64.b64decode(value, validate=True)) == 16
        except (ValueError, TypeError):
            return False

    def _read_ws_text_frame(self):
        first, second = self._read_exact(self.rfile, 2)
        if (first & 0xF0) != 0x80 or (first & 0x0F) != 1 or not (second & 0x80):
            raise ValueError("websocket_frame_shape")
        length = second & 0x7F
        if length == 126:
            length = struct.unpack("!H", self._read_exact(self.rfile, 2))[0]
        elif length == 127:
            length = struct.unpack("!Q", self._read_exact(self.rfile, 8))[0]
            if length >> 63:
                raise ValueError("websocket_length_invalid")
        if length <= 0 or length > MAX_BODY_BYTES:
            raise ValueError("websocket_frame_limit")
        mask = self._read_exact(self.rfile, 4)
        payload = self._read_exact(self.rfile, length)
        return bytes(value ^ mask[index % 4] for index, value in enumerate(payload))

    @staticmethod
    def _read_exact(stream, size):
        output = bytearray()
        while len(output) < size:
            part = stream.read(size - len(output))
            if not part:
                raise EOFError("websocket_frame_short")
            output.extend(part)
        return bytes(output)

    def _serve_response(self, transport):
        if not self.state.reserve_response():
            self._send_error(409, "duplicate_response")
            return
        self.close_connection = True
        response_id = "resp-local-fixture"
        item_id = "msg-local-fixture"
        response = {
            "id": response_id,
            "object": "response",
            "status": "in_progress",
            "model": self.state.model,
            "output": [],
        }
        item = {"id": item_id, "type": "message", "role": "assistant", "status": "in_progress", "content": []}
        partial_events = [
            {"type": "response.created", "response": response},
            {"type": "response.in_progress", "response": response},
            {"type": "response.output_item.added", "output_index": 0, "item": item},
            {
                "type": "response.content_part.added",
                "item_id": item_id,
                "output_index": 0,
                "content_index": 0,
                "part": {"type": "output_text", "text": "", "annotations": []},
            },
            {"type": "response.output_text.delta", "item_id": item_id, "output_index": 0, "content_index": 0, "delta": self.state.partial_marker},
        ]
        try:
            if transport == "http_sse":
                self.send_response(200)
                self.send_header("Content-Type", "text/event-stream; charset=utf-8")
                self.send_header("Cache-Control", "no-cache")
                self.send_header("Connection", "close")
                self.end_headers()
            for event in partial_events:
                self._send_event(transport, event)
            self.state.mark_partial()
            if not self._wait_for_release():
                self.state.error("release_timeout")
                return
            marker = self.state.partial_marker + "\n" + self.state.final_marker
            completed_part = {"type": "output_text", "text": marker, "annotations": []}
            completed_item = {**item, "status": "completed", "content": [completed_part]}
            terminal_events = [
                {"type": "response.output_text.delta", "item_id": item_id, "output_index": 0,
                 "content_index": 0, "delta": "\n" + self.state.final_marker},
                {"type": "response.output_text.done", "item_id": item_id, "output_index": 0, "content_index": 0, "text": marker},
                {"type": "response.content_part.done", "item_id": item_id, "output_index": 0, "content_index": 0, "part": completed_part},
                {"type": "response.output_item.done", "output_index": 0, "item": completed_item},
                {
                    "type": "response.completed",
                    "response": {**response, "status": "completed", "output": [completed_item], "usage": {"input_tokens": 11, "output_tokens": 7, "total_tokens": 18}},
                },
            ]
            for event in terminal_events:
                self._send_event(transport, event)
                if event["type"] == "response.completed":
                    self.state.mark_terminal()
            if transport == "websocket":
                self.connection.sendall(b"\x88\x02\x03\xe8")
        except (BrokenPipeError, ConnectionResetError, OSError, socket.timeout):
            self.state.error("client_disconnected")

    def _wait_for_release(self):
        deadline = time.monotonic() + TERMINAL_RELEASE_TIMEOUT_SECONDS
        while time.monotonic() < deadline:
            if self.state.release_event.wait(0.05):
                return True
            if self.state.closed_event.is_set():
                return False
        return False

    def _send_event(self, transport, event):
        data = json.dumps(event, separators=(",", ":"), ensure_ascii=True).encode("utf-8")
        if len(data) > MAX_BODY_BYTES:
            raise OSError("event_limit")
        if transport == "http_sse":
            packet = b"event: " + event["type"].encode("ascii") + b"\ndata: " + data + b"\n\n"
            self.wfile.write(packet)
            self.wfile.flush()
        else:
            self.connection.sendall(self._ws_server_frame(data))

    @staticmethod
    def _ws_server_frame(payload):
        length = len(payload)
        if length < 126:
            header = bytes((0x81, length))
        elif length <= 0xFFFF:
            header = bytes((0x81, 126)) + struct.pack("!H", length)
        else:
            header = bytes((0x81, 127)) + struct.pack("!Q", length)
        return header + payload

    def _send_json(self, status, payload):
        data = json.dumps(payload, separators=(",", ":"), ensure_ascii=True).encode("utf-8")
        self.close_connection = True
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(data)

    def _send_error(self, status, code):
        safe_code = code if code in ERROR_CODES else "invalid_input"
        self.state.error(safe_code)
        self._send_json(status, {"error": {"code": safe_code}})


class DesktopResponsesFixture:
    """Starts an owned loopback Responses fixture; no external traffic is made."""

    def __init__(self, api_key, model, reasoning_effort, nonce, *, release_event=None):
        if not _is_bounded_token(api_key, 512):
            raise ValueError("invalid_api_key")
        if not _is_bounded_token(model, 256):
            raise ValueError("invalid_model")
        if not _is_bounded_token(reasoning_effort, 32):
            raise ValueError("invalid_reasoning_effort")
        if not _is_bounded_token(nonce, 256):
            raise ValueError("invalid_nonce")
        self.api_key = api_key
        self.model = model
        self.reasoning_effort = reasoning_effort
        self.nonce = nonce
        self.partial_marker = f"HEADROOM_FIXTURE_PARTIAL:{nonce}"
        self.final_marker = f"HEADROOM_FIXTURE_RESULT:{nonce}"
        self.release_event = release_event if release_event is not None else threading.Event()
        if not isinstance(self.release_event, threading.Event):
            raise ValueError("invalid_release_event")
        self._state = _FixtureState(api_key, model, reasoning_effort, nonce, self.release_event)
        self._server = _FixtureHTTPServer(("127.0.0.1", 0), _FixtureHandler)
        self._server.fixture_state = self._state  # type: ignore[attr-defined]
        host, port = self._server.server_address[:2]
        self.origin = f"http://{host}:{port}"
        self.base_url = self.origin + "/v1"
        self._closed = False
        self._server_thread = threading.Thread(target=self._server.serve_forever, name="desktop-fixture-http", daemon=False)
        self._server_thread.start()

    def release_terminal(self):
        self.release_event.set()

    def report(self):
        return self._state.report()

    def close(self):
        if self._closed:
            return not self._server_thread.is_alive()
        self._closed = True
        with self._state.lock:
            self._state.closed = True
        self._state.closed_event.set()
        self._server.shutdown()
        with self._state.lock:
            sockets = list(self._state.active_sockets)
        for connection in sockets:
            try:
                connection.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            try:
                connection.close()
            except OSError:
                pass
        self._server.server_close()
        self._server_thread.join(timeout=5)
        return not self._server_thread.is_alive()

    def __enter__(self):
        return self

    def __exit__(self, _exc_type, _exc_value, _traceback):
        self.close()


__all__ = ["DesktopResponsesFixture"]
