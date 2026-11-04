"""Exercise the metadata-only MCP gate through its real stdio protocol."""
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

SCRIPT = Path(__file__).with_name("native_wait_gate.py")
MARKER = "HEADROOM_WAIT_GATE_TEST_0123456789ABCDEF"


class GateProtocolTests(unittest.TestCase):
    def run_gate(self, messages, gate_response=b'{"released":true}', status=200):
        calls = []

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def do_GET(self):
                calls.append((self.path, self.headers.get("X-Headroom-Probe")))
                self.send_response(status)
                self.end_headers()
                self.wfile.write(gate_response)

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            completed = subprocess.run(
                [sys.executable, "-I", "-u", str(SCRIPT), "--port", str(server.server_port), "--marker", MARKER],
                input="\n".join(json.dumps(x) for x in messages) + "\n",
                capture_output=True, text=True, timeout=8,
                creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
            )
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)
        self.assertEqual(completed.returncode, 0)
        self.assertEqual(completed.stderr, "")
        return [json.loads(x) for x in completed.stdout.splitlines()], calls

    def test_roundtrip_and_gate_identity(self):
        responses, calls = self.run_gate([
            {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}},
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
            {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "wait_for_probe", "arguments": {}}},
        ])
        self.assertEqual([x["id"] for x in responses], [1, 2, 3])
        self.assertEqual(responses[0]["result"]["protocolVersion"], "2025-06-18")
        self.assertEqual(responses[1]["result"]["tools"][0]["name"], "wait_for_probe")
        self.assertEqual(responses[2]["result"], {"content": [{"type": "text", "text": MARKER}], "isError": False})
        self.assertEqual(calls, [("/gate", MARKER)])

    def test_unexpected_arguments_never_contact_gate(self):
        responses, calls = self.run_gate([
            {"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "wait_for_probe", "arguments": {"path": "forbidden"}}},
        ])
        self.assertEqual(responses[0]["error"]["code"], -32602)
        self.assertEqual(calls, [])

    def test_unreleased_or_failed_gate_is_not_a_success(self):
        call = {"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "wait_for_probe", "arguments": {}}}
        for response, status in [(b'{"released":false}', 200), (b'{"released":1}', 200), (b'{"released":true}', 403)]:
            with self.subTest(status=status, response=response):
                responses, calls = self.run_gate([call], response, status)
                self.assertTrue(responses[0]["result"]["isError"])
                self.assertNotIn(MARKER, json.dumps(responses))
                self.assertEqual(len(calls), 1)


if __name__ == "__main__":
    unittest.main()
