"""Exercise the embedded preview against the Windows engine's Learn package."""

import hashlib
import json
import os
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "runtime/manifest.json"
PAYLOAD = (MANIFEST.parent / json.loads(MANIFEST.read_text(encoding="utf-8"))["engine"]["payload_root"]).resolve()
PYTHON = PAYLOAD / "python.exe"
SCRIPT = (ROOT / "runtime/learn_preview.py").read_text(encoding="utf-8")
PRIVATE = "SYNTHETIC_PRIVATE_COMMAND_OR_PATH_67a0"
CODEX = {"type": "session_meta", "payload": {"id": "fixture-codex"}}


def codex_pair(index, status, *, cwd="D:/fixture", name="exec_command"):
    call = {
        "type": "function_call", "call_id": str(index), "name": name,
        "arguments": json.dumps({"cmd": f"test {PRIVATE}", "workdir": cwd}),
    }
    result = {
        "type": "function_call_output", "call_id": str(index),
        "output": f"Chunk ID: fixture\nWall time: 0.1 seconds\nProcess exited with code {status}\n"
                  f"Output:\nNo such file or directory: {PRIVATE}\nProcess exited with code 99",
    }
    return [{"type": "response_item", "payload": item} for item in (call, result)]


def claude_pair(index, failed, *, session="fixture-claude"):
    shared = {"sessionId": session, "cwd": "D:/fixture"}
    return [
        {**shared, "type": "assistant", "message": {"content": [
            {"type": "tool_use", "id": str(index), "name": "Bash",
             "input": {"command": f"check {PRIVATE}"}}
        ]}},
        {**shared, "type": "user", "message": {"content": [
            {"type": "tool_result", "tool_use_id": str(index), "is_error": failed,
             "content": [{"type": "text", "text": f"command not found: {PRIVATE}"}]}
        ]}},
    ]


def encode(records):
    return ("\n".join(json.dumps(record) for record in records) + "\n").encode("utf-8")


@unittest.skipUnless(os.name == "nt" and PYTHON.is_file(), "requires pinned Windows engine payload")
class LearnPreviewTests(unittest.TestCase):
    def run_preview(self, data, operation=None):
        completed = subprocess.run(
            [PYTHON, "-I", "-S", "-B", "-c", SCRIPT, *([operation] if operation else [])],
            cwd=PAYLOAD, env={"SystemRoot": os.environ["SystemRoot"]},
            input=data, capture_output=True, timeout=15, check=True,
        )
        self.assertNotIn(PRIVATE.encode(), completed.stdout + completed.stderr)
        self.assertEqual(completed.stderr, b"")
        return json.loads(completed.stdout)

    def test_local_trial_preserves_archive_bytes_channels_and_exact_repeats(self):
        stdout = "INFO ready λ=18446744073709551617 ratio=0.1234567890123456789012345\r\n" * 250

        def pair(index, *, status=0, stderr="", name="exec_command", value=stdout):
            records = codex_pair(index, status, name=name)
            records[1]["payload"]["output"] = {"exit_code": status, "stdout": value, "stderr": stderr,
                                                "opaque": {"tool_revision": 2}}
            return records

        normal = pair(1)
        raw = b"\xef\xbb\xbf" + encode([CODEX, *normal, *normal])
        response = self.run_preview(raw, "replay")
        self.assertEqual(response["status"], "trial_ready")
        report = response["report"]
        self.assertEqual((report["checked"], report["eligible"], report["compacted"], report["ambiguous"]), (1, 1, 1, 0))
        self.assertEqual(report["original_sha256"], hashlib.sha256(raw).hexdigest())
        self.assertEqual(report["before_bytes"], len(stdout.encode()))
        self.assertLess(report["after_bytes"], report["before_bytes"])
        self.assertTrue(report["round_trip_exact"])
        for case in (pair(2, status=1), pair(3, stderr="warning on successful command"),
                     pair(4, name="Read"), pair(5, value='[{"integer":18446744073709551617}]'),
                     pair(6, value="diff --git a/a b/a\n" + stdout),
                     pair(7, value="INFO first\n... (repeated 999999999 times)\n"),
                     pair(8, value="INFO " + "x" * (1024 * 1024))):
            with self.subTest(case=case[0]["payload"]["call_id"]):
                protected = self.run_preview(encode([CODEX, *case]), "replay")["report"]
                self.assertEqual((protected["protected"], protected["eligible"], protected["before_bytes"]), (1, 0, 0))

        conflicting = pair(1, value="INFO other\n" * 4)
        ambiguous = self.run_preview(encode([CODEX, *normal, *conflicting]), "replay")["report"]
        self.assertEqual((ambiguous["checked"], ambiguous["ambiguous"], ambiguous["eligible"]), (1, 1, 0))
        missing = self.run_preview(encode([CODEX, normal[1]]), "replay")["report"]
        self.assertEqual((missing["checked"], missing["ambiguous"]), (1, 1))

    def test_trial_keeps_distinct_calls_and_scopes_without_trusting_printed_channels(self):
        pairs = [codex_pair(index, 0) for index in (1, 2)]
        for _, result in pairs:
            result["payload"]["output"] = {"exit_code": 0, "stdout": "INFO same\n" * 50, "stderr": ""}
        report = self.run_preview(encode([CODEX, *pairs[0], *pairs[1], *pairs[0]]), "replay")["report"]
        self.assertEqual((report["checked"], report["eligible"], report["compacted"]), (2, 2, 2))

        printed = pairs[0][1]["payload"]["output"]
        pairs[0][1]["payload"]["output"] = json.dumps(printed)
        report = self.run_preview(encode([CODEX, *pairs[0]]), "replay")["report"]
        self.assertEqual((report["protected"], report["eligible"]), (1, 0))
        pairs[0][1]["payload"]["output"] = printed
        pairs[0][0]["payload"]["arguments"] = '{"cmd":"test"}'
        report = self.run_preview(encode([CODEX, *pairs[0]]), "replay")["report"]
        self.assertEqual((report["ambiguous"], report["eligible"]), (1, 0))

        claude = [*claude_pair(1, False), *claude_pair(1, True, session="other-agent")]
        report = self.run_preview(encode(claude), "replay")["report"]
        self.assertEqual((report["checked"], report["protected"], report["ambiguous"]), (2, 2, 0))
        claude[1].pop("sessionId")
        report = self.run_preview(encode(claude), "replay")["report"]
        self.assertEqual((report["checked"], report["protected"], report["ambiguous"]), (2, 1, 1))

    def test_codex_matches_exact_recovery_and_returns_line_without_content(self):
        data = b"\xef\xbb\xbf\n" + encode([CODEX, *codex_pair(1, 1), *codex_pair(2, 1),
                                        *codex_pair(3, 1), *codex_pair(4, 0)])
        self.assertEqual(self.run_preview(data), {
            "status": "ready", "source": "codex",
            "suggestions": [{"kind": "path", "failures": 3, "recoveries": 1, "first_line": 4}],
        })

    def test_claude_uses_explicit_error_and_does_not_match_another_session(self):
        data = encode([*claude_pair(1, True), *claude_pair(2, True), *claude_pair(3, True),
                       *claude_pair(4, False, session="fixture-other")])
        self.assertEqual(self.run_preview(data), {
            "status": "ready", "source": "claude_code",
            "suggestions": [{"kind": "dependency", "failures": 3, "recoveries": 0, "first_line": 2}],
        })

    def test_normal_retests_changes_parallel_tools_and_replays_do_not_count(self):
        parallel = [codex_pair(index, 1) for index in range(20, 23)]
        cases = (
            [item for index in range(1, 11) for item in
             (*codex_pair(index, 0), *codex_pair(index + 40, 0, name="apply_patch"))],
            [*codex_pair(1, 1), *codex_pair(2, 0, name="apply_patch"), *codex_pair(3, 1),
             *codex_pair(4, 1)],
            [*codex_pair(1, 1), *codex_pair(2, 1, cwd="D:/another"), *codex_pair(3, 1)],
            [*codex_pair(1, 1), *codex_pair(1, 1), *codex_pair(1, 1)],
            [*[call for call, _ in parallel], *[result for _, result in parallel]],
        )
        for case in cases:
            with self.subTest():
                self.assertEqual(self.run_preview(encode([CODEX, *case])),
                                 {"status": "ready", "source": "codex", "suggestions": []})

    def test_bad_format_and_limits_return_fixed_results(self):
        for data in (b"", b"\xff\n", b'{"type":', b"42\n", encode([{"model": "synthetic"}])):
            with self.subTest():
                self.assertEqual(self.run_preview(data), {"status": "invalid_file"})
        self.assertEqual(self.run_preview(b" " * (16 * 1024 * 1024 + 1)), {"status": "too_large"})
        self.assertEqual(self.run_preview(b"\n" * 64_001), {"status": "too_large"})
        self.assertEqual(self.run_preview(b"\n" * 64_000 + b"{}"), {"status": "too_large"})
        self.assertEqual(self.run_preview(b"\xff\n", "replay"), {"status": "invalid_file"})
        self.assertEqual(self.run_preview(b"\n" * 64_001, "replay"), {"status": "too_large"})


if __name__ == "__main__":
    unittest.main()
