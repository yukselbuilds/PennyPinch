"""A query capability belongs to one CLI-owned MCP server, never to tool arguments."""
from dataclasses import replace
import json
from pathlib import Path
import runpy
from types import SimpleNamespace
import unittest


ROOT = Path(__file__).resolve().parents[2]
client = runpy.run_path(str(ROOT / "bench/context_tools/targeted_query_client.py"))
query = client["query"]
RAW = b'{"id":"a","value":18446744073709551617}\r\n{"id":"b","decimal":0.1234567890123456789012345}'


class TargetedQueryClientTests(unittest.TestCase):
    def setUp(self):
        self.output = query.ToolOutput(query.REPORT_TOOL, "not-a-model-call-id", RAW, b"", 0)
        self.session = client["ReportSession"](self.output)

    def test_complete_scan_returns_exact_bytes_count_and_source_identity(self):
        reference = self.session.capture("mcp-request-7")
        self.assertNotIn("stdout", reference)
        result = self.session.select({"archive_id": reference["archive_id"], "record_id": "b"})
        self.assertEqual(result["stdout"].encode(), RAW.splitlines(keepends=True)[1])
        self.assertEqual((result["record_count"], result["matched_count"], result["truncated"]), (2, 1, False))
        original = self.session.read_original(reference["archive_id"])
        self.assertEqual(original["stdout"].encode(), RAW)
        self.assertEqual((original["stderr"], original["exit_code"]), ("", 0))
        self.assertEqual(original["source_call_id"], "mcp-request-7")
        self.assertEqual(result["source_call_id"], original["source_call_id"])
        self.assertEqual(original["source_tool"], query.REPORT_TOOL)

    def test_other_server_and_restart_cannot_reuse_capability(self):
        handle = self.session.capture("mcp-request-1")["archive_id"]
        for other in (client["ReportSession"](self.output), client["ReportSession"](self.output)):
            with self.assertRaisesRegex(LookupError, "^archive_unavailable$"):
                other.select({"archive_id": handle, "record_id": "a"})
            with self.assertRaisesRegex(LookupError, "^archive_unavailable$"):
                other.read_original(handle)

    def test_arguments_cannot_supply_owner_path_or_command(self):
        args = {"archive_id": self.session.capture("mcp-1")["archive_id"], "record_id": "a"}
        for extra in ({"owner": "another-session"}, {"path": "../private"}, {"command": "read private"}, {"record_id": "../private"}):
            with self.assertRaisesRegex(ValueError, "^invalid_query$"):
                self.session.select({**args, **extra})
        with self.assertRaisesRegex(ValueError, "^invalid_source_request$"):
            self.session.capture(7)

    def test_partial_or_ambiguous_scan_never_claims_complete_count(self):
        for raw, expected in ((RAW + b'\n{"id":"a"}', "ambiguous_jsonl"), (RAW + b'\nBAD', "invalid_jsonl")):
            session = client["ReportSession"](replace(self.output, stdout=raw))
            handle = session.capture("mcp-1")["archive_id"]
            result = session.select({"archive_id": handle, "record_id": "a"})
            self.assertEqual(result["status"], expected)
            self.assertFalse({"record_count", "matched_count", "truncated", "stdout"} & result.keys())
            self.assertEqual(session.read_original(handle)["stdout"].encode(), raw)

    def test_error_and_stderr_require_full_output_with_original_envelope(self):
        for output in (replace(self.output, stderr=b"ERROR\r\n"), replace(self.output, exit_code=9)):
            session = client["ReportSession"](output)
            handle = session.capture("mcp-error")["archive_id"]
            self.assertEqual(session.select({"archive_id": handle, "record_id": "a"})["status"], "full_output_required")
            original = session.read_original(handle)
            self.assertEqual((original["stdout"].encode(), original["stderr"].encode(), original["exit_code"]),
                             (output.stdout, output.stderr, output.exit_code))

    def test_read_parts_preserve_utf8_ranges_and_bound_escaped_output(self):
        # Put a multibyte character across the nominal chunk boundary.
        raw = b"x" * 6143 + "λ\r\n".encode() + b"\x00" * 7200 + b"last"
        error = "Uyarı — λ\r\n".encode()
        session = client["ReportSession"](replace(self.output, stdout=raw, stderr=error, exit_code=9))
        ref = session.capture("mcp-part")
        first = session.read_part(ref["archive_id"], "stdout", 0)
        self.assertEqual(first["next_offset"], 6143)
        for stream, expected in (("stdout", raw), ("stderr", error)):
            offset, parts = 0, []
            while True:
                part = session.read_part(ref["archive_id"], stream, offset)
                self.assertEqual({key: part[key] for key in ref}, ref)
                self.assertEqual((part["stream"], part["offset"], part["exit_code"]), (stream, offset, 9))
                self.assertEqual(part["total_bytes"], len(expected))
                self.assertLessEqual(len(client["encode"](part).encode()), 8192)
                data = part["text"].encode()
                self.assertTrue(data)
                parts.append(data)
                offset += len(data)
                self.assertEqual(part["next_offset"], offset)
                if offset == len(expected):
                    break
            self.assertEqual(b"".join(parts), expected)
            if stream == "stdout":
                self.assertLess(len(parts[-1]), 6144)

    def test_read_parts_validate_bounds_and_deny_a_foreign_archive(self):
        session = client["ReportSession"](replace(self.output, stdout="AλZ".encode()))
        handle = session.capture("mcp-part")["archive_id"]
        for stream, offset in (("file", 0), ("stdout", -1), ("stdout", 5), ("stdout", 2), ("stdout", True), ("stdout", "1")):
            with self.subTest(stream=stream, offset=offset), self.assertRaisesRegex(ValueError, "^invalid_read_range$"):
                session.read_part(handle, stream, offset)
        other = client["ReportSession"](self.output)
        with self.assertRaisesRegex(LookupError, "^archive_unavailable$"):
            other.read_part(handle, "stdout", 0)
        empty = session.read_part(handle, "stderr", 0)
        self.assertEqual((empty["text"], empty["next_offset"], empty["total_bytes"]), ("", 0, 0))

    def test_readback_rejects_an_incomplete_duplicate_or_out_of_order_range(self):
        replay = runpy.run_path(str(ROOT / "bench/context_tools/targeted_query_client_replay.py"))
        ref = self.session.capture("mcp-readback")
        expected = b"abcd"
        reader = replay["Readback"](ref, expected, b"", 0)
        page = {**ref, "stream": "stdout", "offset": 0, "next_offset": 2, "total_bytes": 4, "text": "ab", "exit_code": 0}
        with self.assertRaisesRegex(AssertionError, "readback_incomplete"):
            reader.finish()
        with self.assertRaisesRegex(AssertionError, "read_part_range_changed"):
            reader.accept({**page, "offset": 2, "next_offset": 4, "text": "cd"})
        reader.accept(page)
        with self.assertRaisesRegex(AssertionError, "read_part_range_changed"):
            reader.accept(page)
        with self.assertRaisesRegex(AssertionError, "readback_incomplete"):
            reader.finish()
        reader.accept({**page, "offset": 2, "next_offset": 4, "text": "cd"})
        reader.accept({**page, "stream": "stderr", "next_offset": 0, "total_bytes": 0, "text": ""})
        self.assertEqual(reader.finish(), {"stdout": b"abcd", "stderr": b""})

    def test_scope_uses_the_dashboard_window_to_cover_all_22_requests(self):
        replay = runpy.run_path(str(ROOT / "bench/context_tools/targeted_query_client_replay.py"))
        rows = [{"request_id": f"hr_100_{i:06}"} for i in range(22)]
        data = json.dumps({"recent_requests": rows, "request_logs": rows[-10:]}).encode()
        engine = SimpleNamespace(port=1, control_token="fixture")
        def exchange(port, method, path, headers):
            self.assertEqual((port, method, path), (1, "GET", "/stats?cached=1"))
            self.assertEqual(headers, {"X-Headroom-Desktop-Control": "fixture"})
            return 200, "application/json", data
        self.assertEqual(replay["dashboard_request_ids"](engine, exchange), {row["request_id"] for row in rows})

    def test_responses_text_parts_are_read_without_accepting_nontext_content(self):
        replay = runpy.run_path(str(ROOT / "bench/context_tools/targeted_query_client_replay.py"))
        for kind in ("text", "input_text"):
            body = {"input": [{"type": "function_call_output", "call_id": "call_query_1",
                               "output": [{"type": kind, "text": "exact text"}]}]}
            self.assertEqual(replay["tool_text"](body, 1), "exact text")
        body["input"][0]["output"] = [{"type": "input_text", "text": "Wall time: 0.125 seconds\nOutput:\n"},
                                      {"type": "input_text", "text": '{"archive_id":"fixed-test-reference"}'}]
        self.assertEqual(replay["tool_text"](body, 1), '{"archive_id":"fixed-test-reference"}')
        body["input"][0]["output"] = [{"type": "input_image", "text": "not a text part"}]
        with self.assertRaisesRegex(AssertionError, "tool_output_shape_changed"):
            replay["tool_text"](body, 1)


if __name__ == "__main__":
    unittest.main()
