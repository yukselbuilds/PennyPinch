import base64
import hashlib
import http.client
import importlib.util
import json
import socket
import struct
import time
import unittest
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import Request, urlopen
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[2]
FIXTURE_SCRIPT = ROOT / "scripts" / "macos-desktop-fixture.py"
spec = importlib.util.spec_from_file_location("headroom_macos_desktop_fixture", FIXTURE_SCRIPT)
if spec is None or spec.loader is None:
    raise RuntimeError("fixture_helper_unavailable")
fixture_module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fixture_module)
DesktopResponsesFixture = fixture_module.DesktopResponsesFixture


API_KEY = "synthetic-desktop-fixture-key"
MODEL = "synthetic-desktop-fixture-model"
EFFORT = "medium"
NONCE = "fixture-run-nonce-7c2a9d"


def valid_request(**overrides):
    request = {
        "model": MODEL,
        "reasoning": {"effort": EFFORT},
        "input": [{"role": "user", "content": [{"type": "input_text", "text": f"Correlation token: {NONCE}. Reply briefly to this local fixture prompt."}]}],
        "stream": True,
    }
    request.update(overrides)
    return request


def make_fixture():
    return DesktopResponsesFixture(API_KEY, MODEL, EFFORT, NONCE)


def read_line(response):
    line = response.readline(64 * 1024 + 1)
    if len(line) > 64 * 1024:
        raise AssertionError("SSE line exceeds test bound")
    return line


def read_ws_frame(sock):
    first, second = _recv_exact(sock, 2)
    if second & 0x80:
        raise AssertionError("server WebSocket frame must be unmasked")
    length = second & 0x7F
    if length == 126:
        length = struct.unpack("!H", _recv_exact(sock, 2))[0]
    elif length == 127:
        length = struct.unpack("!Q", _recv_exact(sock, 8))[0]
    if length > 1024 * 1024:
        raise AssertionError("WebSocket frame exceeds test bound")
    return first & 0x0F, json.loads(_recv_exact(sock, length))


def _recv_exact(sock, size):
    output = bytearray()
    while len(output) < size:
        part = sock.recv(size - len(output))
        if not part:
            raise EOFError("socket closed during bounded frame")
        output.extend(part)
    return bytes(output)


def masked_ws_frame(payload):
    mask = b"\x19\x27\x35\x43"
    length = len(payload)
    if length < 126:
        header = bytes([0x81, 0x80 | length])
    elif length <= 0xFFFF:
        header = bytes([0x81, 0x80 | 126]) + struct.pack("!H", length)
    else:
        header = bytes([0x81, 0x80 | 127]) + struct.pack("!Q", length)
    return header + mask + bytes(value ^ mask[index % 4] for index, value in enumerate(payload))


def wait_for_partial(fixture):
    deadline = time.monotonic() + 1.0
    while time.monotonic() < deadline:
        if fixture.report()["partial_sent"]:
            return True
        time.sleep(0.01)
    return fixture.report()["partial_sent"]


class DesktopFixtureTests(unittest.TestCase):
    def test_models_and_http_sse_waits_for_explicit_terminal_release(self):
        fixture = make_fixture()
        self.addCleanup(fixture.close)
        self.assertEqual(fixture.base_url, fixture.origin + "/v1")

        model_request = Request(
            fixture.base_url + "/models",
            headers={"Authorization": f"Bearer {API_KEY}"},
        )
        with urlopen(model_request, timeout=3) as models_response:
            models = json.loads(models_response.read(64 * 1024 + 1))
        self.assertEqual([row["id"] for row in models["data"]], [MODEL])

        request = Request(
            fixture.base_url + "/responses",
            data=json.dumps(valid_request()).encode("utf-8"),
            headers={"Authorization": f"Bearer {API_KEY}", "Content-Type": "application/json"},
            method="POST",
        )
        response = urlopen(request, timeout=4)
        self.addCleanup(response.close)
        self.assertEqual(response.status, 200)
        self.assertIn("text/event-stream", response.headers.get("Content-Type", ""))
        observed_delta = False
        while True:
            line = read_line(response)
            if not line:
                break
            if line.strip() == b"event: response.output_text.delta":
                data_line = read_line(response)
                self.assertTrue(data_line.startswith(b"data: "))
                self.assertIn(fixture.partial_marker.encode(), data_line)
                observed_delta = True
                break
        self.assertTrue(observed_delta)
        self.assertTrue(wait_for_partial(fixture))
        before_release = fixture.report()
        self.assertTrue(before_release["partial_sent"])
        self.assertFalse(before_release["terminal_sent"])
        self.assertEqual(before_release["accepted_response_count"], 1)

        fixture.release_terminal()
        tail = response.read(1024 * 1024 + 1)
        self.assertLessEqual(len(tail), 1024 * 1024)
        self.assertIn(fixture.final_marker.encode(), tail)
        self.assertIn(b"event: response.completed", tail)
        report = fixture.report()
        self.assertEqual(report["transport_counts"]["http_sse"], 1)
        self.assertEqual(report["transport_counts"]["model_list"], 1)
        self.assertEqual(report["auth_match_count"], 2)
        self.assertEqual(report["model_match_count"], 1)
        self.assertEqual(report["reasoning_match_count"], 1)
        self.assertEqual(report["input_nonce_match_count"], 1)
        self.assertEqual(report["input_nonce_mismatch_count"], 0)
        self.assertTrue(report["terminal_sent"])
        serialized_report = json.dumps(report, sort_keys=True)
        for private_value in (API_KEY, NONCE, fixture.final_marker, fixture.base_url):
            self.assertNotIn(private_value, serialized_report)

    def test_websocket_response_create_is_validated_and_terminal_is_gated(self):
        fixture = make_fixture()
        self.addCleanup(fixture.close)
        parts = urlsplit(fixture.origin)
        sock = socket.create_connection((parts.hostname, parts.port), timeout=3)
        self.addCleanup(sock.close)
        sock.settimeout(3)
        key = base64.b64encode(b"0123456789abcdef").decode("ascii")
        handshake = (
            f"GET /v1/responses HTTP/1.1\r\nHost: 127.0.0.1:{parts.port}\r\n"
            "Upgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\n"
            f"Sec-WebSocket-Key: {key}\r\nAuthorization: Bearer {API_KEY}\r\n\r\n"
        ).encode("ascii")
        sock.sendall(handshake)
        handshake_reply = bytearray()
        while not handshake_reply.endswith(b"\r\n\r\n"):
            if len(handshake_reply) >= 8192:
                self.fail("WebSocket handshake exceeded bound")
            chunk = sock.recv(1)
            if not chunk:
                self.fail("WebSocket handshake closed early")
            handshake_reply.extend(chunk)
        self.assertIn(b" 101 ", bytes(handshake_reply).split(b"\r\n", 1)[0])
        expected_accept = base64.b64encode(hashlib.sha1((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").encode()).digest())
        self.assertIn(b"Sec-WebSocket-Accept: " + expected_accept, bytes(handshake_reply))

        request = valid_request()
        request["type"] = "response.create"
        sock.sendall(masked_ws_frame(json.dumps(request, separators=(",", ":")).encode()))
        observed_delta = False
        for _ in range(8):
            opcode, event = read_ws_frame(sock)
            self.assertEqual(opcode, 1)
            if event.get("type") == "response.output_text.delta":
                self.assertEqual(event.get("delta"), fixture.partial_marker)
                observed_delta = True
                break
        self.assertTrue(observed_delta)
        self.assertTrue(wait_for_partial(fixture))
        report = fixture.report()
        self.assertTrue(report["partial_sent"])
        self.assertFalse(report["terminal_sent"])
        fixture.release_event.set()
        terminal = None
        streamed_text = fixture.partial_marker
        done_text = None
        for _ in range(8):
            opcode, event = read_ws_frame(sock)
            self.assertEqual(opcode, 1)
            if event.get("type") == "response.output_text.delta":
                streamed_text += event["delta"]
            if event.get("type") == "response.output_text.done":
                done_text = event["text"]
            if event.get("type") == "response.completed":
                terminal = event
                break
        self.assertIsNotNone(terminal)
        self.assertEqual(streamed_text, done_text)
        self.assertEqual(streamed_text, terminal["response"]["output"][0]["content"][0]["text"])
        self.assertIn(fixture.final_marker, json.dumps(terminal))
        # The server records delivery before it sends the normal close frame.
        self.assertEqual(_recv_exact(sock, 4), b"\x88\x02\x03\xe8")
        report = fixture.report()
        self.assertEqual(report["transport_counts"]["websocket"], 1)
        self.assertEqual(report["accepted_response_count"], 1)
        self.assertEqual(report["input_nonce_match_count"], 1)
        self.assertTrue(report["terminal_sent"])

    def test_rejects_wrong_auth_model_effort_and_missing_nonce(self):
        cases = (
            ("auth_mismatch", {"auth": "wrong"}, 401),
            ("model_mismatch", {"model": "other-model"}, 400),
            ("reasoning_mismatch", {"reasoning": {"effort": "high"}}, 400),
            ("nonce_missing", {"input": "No correlation token is included."}, 400),
        )
        for code, override, status in cases:
            with self.subTest(code=code):
                fixture = make_fixture()
                try:
                    headers = {"Content-Type": "application/json", "Authorization": f"Bearer {API_KEY}"}
                    if override.get("auth") == "wrong":
                        headers["Authorization"] = "Bearer wrong"
                    body_override = {key: value for key, value in override.items() if key != "auth"}
                    request = Request(
                        fixture.base_url + "/responses",
                        data=json.dumps(valid_request(**body_override)).encode("utf-8"),
                        headers=headers,
                        method="POST",
                    )
                    with self.assertRaises(HTTPError) as raised:
                        urlopen(request, timeout=3)
                    self.assertEqual(raised.exception.code, status)
                    report = fixture.report()
                    self.assertEqual(report["error_counts"].get(code), 1)
                    self.assertEqual(report["accepted_response_count"], 0)
                    if code == "nonce_missing":
                        self.assertEqual(report["input_nonce_mismatch_count"], 1)
                    self.assertFalse(report["partial_sent"])
                    self.assertFalse(report["terminal_sent"])
                    self.assertNotIn(API_KEY, json.dumps(report))
                finally:
                    fixture.close()

    def test_only_one_valid_model_response_is_accepted(self):
        fixture = make_fixture()
        self.addCleanup(fixture.close)
        first = Request(
            fixture.base_url + "/responses",
            data=json.dumps(valid_request()).encode(),
            headers={"Authorization": f"Bearer {API_KEY}", "Content-Type": "application/json"},
            method="POST",
        )
        first_response = urlopen(first, timeout=3)
        self.addCleanup(first_response.close)
        self.assertEqual(first_response.status, 200)
        self.assertTrue(wait_for_partial(fixture))
        second = Request(
            fixture.base_url + "/responses",
            data=json.dumps(valid_request()).encode(),
            headers={"Authorization": f"Bearer {API_KEY}", "Content-Type": "application/json"},
            method="POST",
        )
        with self.assertRaises(HTTPError) as raised:
            urlopen(second, timeout=3)
        self.assertEqual(raised.exception.code, 409)
        fixture.release_terminal()
        first_response.read(1024 * 1024 + 1)
        report = fixture.report()
        self.assertEqual(report["accepted_response_count"], 1)
        self.assertEqual(report["error_counts"].get("duplicate_response"), 1)
        self.assertTrue(report["terminal_sent"])
        self.assertTrue(fixture.close())
        self.assertTrue(fixture.report()["closed"])

    def test_oversized_and_duplicate_json_requests_fail_closed(self):
        fixture = make_fixture()
        self.addCleanup(fixture.close)
        parts = urlsplit(fixture.origin)
        connection = http.client.HTTPConnection(parts.hostname, parts.port, timeout=3)
        self.addCleanup(connection.close)
        connection.putrequest("POST", "/v1/responses")
        connection.putheader("Authorization", f"Bearer {API_KEY}")
        connection.putheader("Content-Type", "application/json")
        connection.putheader("Content-Length", str(1024 * 1024 + 1))
        connection.endheaders()
        oversized = connection.getresponse()
        self.assertEqual(oversized.status, 413)
        oversized.read(64 * 1024)
        connection.close()

        duplicate = (
            b'{"model":"synthetic-desktop-fixture-model","model":"other",'
            b'"reasoning":{"effort":"medium"},"input":"fixture prompt","stream":true}'
        )
        request = Request(
            fixture.base_url + "/responses",
            data=duplicate,
            headers={"Authorization": f"Bearer {API_KEY}", "Content-Type": "application/json"},
            method="POST",
        )
        with self.assertRaises(HTTPError) as raised:
            urlopen(request, timeout=3)
        self.assertEqual(raised.exception.code, 400)
        report = fixture.report()
        self.assertEqual(report["error_counts"].get("body_too_large"), 1)
        self.assertEqual(report["error_counts"].get("invalid_json"), 1)
        self.assertEqual(report["accepted_response_count"], 0)
        self.assertFalse(report["terminal_sent"])


if __name__ == "__main__":
    unittest.main()
