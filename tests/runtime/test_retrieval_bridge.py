"""Real Windows socket ownership plus bounded MCP/error behavior; no client accounts or inference."""
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import importlib.util
import io
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import threading
import unittest
from unittest import mock
import uuid

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("retrieval_bridge", ROOT / "runtime/retrieval_bridge.py")
bridge = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bridge)
KEY = "a123456789bcdef0"
CONTENT = "özgün araç sonucu\n" * 120


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_POST(self):
        self.server.calls.append((self.path, self.rfile.read(int(self.headers["Content-Length"])), dict(self.headers)))
        raw = self.server.body
        self.send_response(self.server.status)
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)


class RetrievalBridgeTests(unittest.TestCase):
    def setUp(self):
        parent = ROOT / "work/proofs/opencode-retrieval/unit"
        parent.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="köprü ", dir=parent)
        self.root = Path(self.temporary.name)
        self.endpoint_id = str(uuid.uuid4())
        self.record = self.root / "connection-endpoint.json"
        self.server = None
        self.thread = None

    def tearDown(self):
        if self.server:
            self.server.shutdown()
            self.server.server_close()
            self.thread.join(timeout=3)
            self.assertFalse(self.thread.is_alive())
        assert self.root.resolve().is_relative_to(ROOT / "work/proofs/opencode-retrieval/unit")
        self.temporary.cleanup()

    def start_server(self):
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.server.daemon_threads = True
        self.server.calls = []
        self.server.body = json.dumps({"hash": KEY, "original_content": CONTENT, "irrelevant": "not forwarded"}).encode()
        self.server.status = 200
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.write_record()
        return bridge.Retrieval(self.record, self.endpoint_id, sys.executable)

    def write_record(self, **overrides):
        value = {"schema_version": 1, "bind_address": "127.0.0.1", "port": self.server.server_port if self.server else 12345,
                 "endpoint_id": self.endpoint_id, "revision": 1}
        self.record.write_text(json.dumps({**value, **overrides}), encoding="utf-8")

    @unittest.skipUnless(sys.platform == "win32", "Windows peer owner API")
    def test_real_peer_roundtrip_ignores_proxy_and_restores_exact_bytes(self):
        retrieval = self.start_server()
        with mock.patch.dict(os.environ, {"HTTP_PROXY": "http://127.0.0.1:1", "HTTPS_PROXY": "http://127.0.0.1:1"}):
            value = retrieval.retrieve({"hash": KEY.upper()})
        self.assertEqual(value, {"hash": KEY, "original_content": CONTENT})
        self.assertEqual(hashlib.sha256(value["original_content"].encode()).digest(), hashlib.sha256(CONTENT.encode()).digest())
        self.assertEqual(len(self.server.calls), 1)
        route, body, headers = self.server.calls[0]
        self.assertEqual(route, "/v1/retrieve")
        self.assertEqual(json.loads(body), {"hash": KEY})
        self.assertFalse(any(key.lower() in ("authorization", "cookie", "proxy-authorization") for key in headers))

    @unittest.skipUnless(sys.platform == "win32", "Windows peer owner API")
    def test_foreign_image_is_rejected_before_application_bytes(self):
        self.start_server()
        foreign = Path(os.environ["SystemRoot"]) / "System32/cmd.exe"
        retrieval = bridge.Retrieval(self.record, self.endpoint_id, foreign)
        with self.assertRaisesRegex(bridge.BridgeError, "headroom_owner_unverified"):
            retrieval.retrieve({"hash": KEY})
        self.assertEqual(self.server.calls, [])

    def test_hash_path_endpoint_and_extra_arguments_do_not_reach_network(self):
        self.write_record()
        retrieval = bridge.Retrieval(self.record, self.endpoint_id, sys.executable)
        with mock.patch.object(bridge.http.client.HTTPConnection, "connect", side_effect=AssertionError("must_not_connect")):
            for arguments in ({}, {"hash": "../secrets"}, {"hash": KEY, "url": "https://invalid.example"}, {"hash": True}, None):
                with self.assertRaisesRegex(bridge.BridgeError, "retrieval_hash_invalid"):
                    retrieval.retrieve(arguments)
            for overrides in ({"bind_address": "0.0.0.0"}, {"endpoint_id": str(uuid.uuid4())}, {"port": True},
                              {"port": 65536}, {"revision": 0}, {"secret": "not logged"}):
                self.write_record(**overrides)
                with self.assertRaisesRegex(bridge.BridgeError, "headroom_endpoint_unavailable"):
                    retrieval.retrieve({"hash": KEY})
        for path in (Path("relative.json"), self.record.parent, self.record.parent / ".." / self.record.parent.name / self.record.name):
            with self.assertRaises(bridge.BridgeError):
                bridge.Retrieval(path, self.endpoint_id, sys.executable)

    @unittest.skipUnless(sys.platform == "win32", "Windows peer owner API")
    def test_errors_and_size_limits_never_return_partial_content(self):
        retrieval = self.start_server()
        for status, body, code in ((404, b"secret upstream detail", "retrieval_missing_or_expired"),
                                   (302, b"redirect", "headroom_retrieval_unavailable"),
                                   (200, b'{"hash":"wrong","original_content":"secret"}', "headroom_retrieval_invalid"),
                                   (200, b'{"hash":NaN}', "headroom_retrieval_unavailable")):
            self.server.status, self.server.body = status, body
            with self.assertRaisesRegex(bridge.BridgeError, "^" + code + "$"):
                retrieval.retrieve({"hash": KEY})
        self.server.body = json.dumps({"hash": KEY, "original_content": CONTENT}).encode()
        with mock.patch.object(bridge, "MAX_CONTENT", 128):
            with self.assertRaisesRegex(bridge.BridgeError, "retrieval_content_too_large"):
                retrieval.retrieve({"hash": KEY})

    def test_stdio_protocol_tool_errors_and_eof_are_bounded(self):
        class Retrieval:
            def retrieve(self, arguments):
                if arguments == {"hash": KEY}:
                    return {"hash": KEY, "original_content": CONTENT + "\ud800"}
                raise bridge.BridgeError("retrieval_hash_invalid")
        messages = [
            {"id": 1, "method": "initialize", "params": {"protocolVersion": "2099-01-01"}},
            {"method": "notifications/initialized"},
            {"id": 2, "method": "tools/list"},
            {"id": 3, "method": "tools/call", "params": {"name": "retrieve", "arguments": {"hash": KEY}}},
            {"id": 4, "method": "tools/call", "params": {"name": "retrieve", "arguments": {"path": "SECRET_CANARY"}}},
            {"id": 5, "method": "resources/read", "params": {"uri": "SECRET_CANARY"}},
            {"id": 6, "method": "ping"},
        ]
        source = io.BytesIO(b"".join(json.dumps({"jsonrpc": "2.0", **m}).encode() + b"\n" for m in messages))
        sink = io.BytesIO()
        bridge.serve(source, sink, Retrieval())
        replies = [json.loads(line) for line in sink.getvalue().splitlines()]
        self.assertEqual([r["id"] for r in replies], [1, 2, 3, 4, 5, 6])
        self.assertEqual(replies[0]["result"]["protocolVersion"], bridge.PROTOCOLS[-1])
        self.assertEqual([tool["name"] for tool in replies[1]["result"]["tools"]], ["retrieve"])
        self.assertEqual(json.loads(replies[2]["result"]["content"][0]["text"])["original_content"], CONTENT + "\ud800")
        self.assertTrue(replies[3]["result"]["isError"])
        self.assertEqual(replies[4]["error"]["code"], -32601)
        self.assertNotIn(b"SECRET_CANARY", sink.getvalue())
        for invalid in (b'{"jsonrpc":"2.0","id":true,"method":"ping"}\n', b'{"id":1,"id":2}\n', b"x" * (bridge.MAX_MESSAGE + 1)):
            sink = io.BytesIO()
            bridge.serve(io.BytesIO(invalid), sink, Retrieval())
            reply = json.loads(sink.getvalue())
            self.assertIsNone(reply["id"])
            self.assertEqual(reply["error"]["code"], -32600)

    @unittest.skipUnless(sys.platform == "win32", "Windows product bridge")
    def test_real_stdio_process_initializes_and_exits_without_stderr(self):
        self.write_record()
        messages = b'{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}\n' + b'{"jsonrpc":"2.0","id":2,"method":"tools/list"}\n'
        result = subprocess.run([sys.executable, "-I", "-B", str(ROOT / "runtime/retrieval_bridge.py"),
                                 str(self.record), self.endpoint_id, sys.executable, ""], input=messages, capture_output=True, timeout=5)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stderr, b"")
        self.assertEqual(len(result.stdout.splitlines()), 2)

    @unittest.skipUnless(sys.platform == "win32", "Windows desktop-use gate")
    def test_stdio_child_holds_package_gate_until_stdin_closes(self):
        self.write_record()
        gate = self.root / "fixture.desktop-use"
        gate.touch()
        kernel = bridge.ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.CreateFileW.argtypes = [bridge.wintypes.LPCWSTR, bridge.wintypes.DWORD, bridge.wintypes.DWORD,
                                      bridge.ctypes.c_void_p, bridge.wintypes.DWORD, bridge.wintypes.DWORD, bridge.wintypes.HANDLE]
        kernel.CreateFileW.restype = bridge.wintypes.HANDLE
        kernel.CloseHandle.argtypes = [bridge.wintypes.HANDLE]
        invalid = bridge.ctypes.c_void_p(-1).value

        def exclusive():
            handle = kernel.CreateFileW(str(gate), 0x80000000, 0, None, 3, 0x00200000, None)
            if handle != invalid:
                kernel.CloseHandle(handle)
                return 0
            return bridge.ctypes.get_last_error()

        with subprocess.Popen([sys.executable, "-I", "-B", str(ROOT / "runtime/retrieval_bridge.py"),
                               str(self.record), self.endpoint_id, sys.executable, str(gate)],
                              stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE) as child:
            children = []
            def initialize():
                try:
                    child.stdin.write(b'{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}\n')
                    child.stdin.flush()
                    children.append(child.stdout.readline())
                except OSError:
                    children.append(b"")
            reader = threading.Thread(target=initialize, daemon=True)
            reader.start()
            try:
                reader.join(timeout=5)
                self.assertFalse(reader.is_alive())
                self.assertIn("result", json.loads(children[0]))
                self.assertEqual(exclusive(), 32)
                child.stdin.close()
                self.assertEqual(child.wait(timeout=5), 0)
                self.assertEqual(child.stderr.read(), b"")
                self.assertEqual(exclusive(), 0)
            finally:
                if child.poll() is None:
                    child.kill()
                reader.join(timeout=5)


if __name__ == "__main__":
    unittest.main()
