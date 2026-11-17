"""A query capability belongs to one CLI-owned MCP server, never to tool arguments."""
from dataclasses import replace
from pathlib import Path
import runpy
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
