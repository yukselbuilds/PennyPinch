"""Focused tests for Claude result parsing and the partial-stream gate."""

from __future__ import annotations

import importlib.util
import io
import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
RUNNER_PATH = ROOT / "work" / "proofs" / "claude" / "run_claude_proof.py"
SPEC = importlib.util.spec_from_file_location("claude_proof_runner_test", RUNNER_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("runner-module-unavailable")
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)


class FakeFixture:
    def __init__(self) -> None:
        self.released = False

    def release_terminal(self) -> None:
        self.released = True


def encoded(record: dict[str, object]) -> bytes:
    return (json.dumps(record, separators=(",", ":")) + "\n").encode("utf-8")


class ClaudeProofHelperTests(unittest.TestCase):
    def test_duplicate_direct_and_model_usage_is_not_double_counted(self) -> None:
        totals, source, _fields = RUNNER.CliStreamObserver._usage_from_result(
            {
                "usage": {"input_tokens": 12, "output_tokens": 4},
                "modelUsage": {"claude-opus-4-6": {"inputTokens": 12, "outputTokens": 4}},
            }
        )
        self.assertEqual(totals, {"input_tokens": 12, "output_tokens": 4})
        self.assertEqual(source, "usage")

    def test_model_usage_breakdown_is_labeled_when_direct_usage_is_absent(self) -> None:
        totals, source, _fields = RUNNER.CliStreamObserver._usage_from_result(
            {"modelUsage": {"claude-opus-4-6": {"inputTokens": 12, "outputTokens": 4}}}
        )
        self.assertEqual(totals, {"input_tokens": 12, "output_tokens": 4})
        self.assertEqual(source, "modelUsage-breakdown")

    def test_missing_usage_stays_unknown(self) -> None:
        totals, source, _fields = RUNNER.CliStreamObserver._usage_from_result({"result": "done"})
        self.assertEqual(totals, {})
        self.assertEqual(source, "partial")

    def test_final_result_marker_alone_does_not_release_partial_gate(self) -> None:
        fixture = FakeFixture()
        observer = RUNNER.CliStreamObserver(
            io.BytesIO(
                encoded(
                    {
                        "type": "result",
                        "subtype": "success",
                        "is_error": False,
                        "result": RUNNER.OUTPUT_MARKER,
                    }
                )
            ),
            fixture,
        )
        self.assertTrue(observer.finish())
        self.assertTrue(observer.final_marker_seen)
        self.assertFalse(fixture.released)

    def test_text_delta_releases_partial_gate(self) -> None:
        fixture = FakeFixture()
        observer = RUNNER.CliStreamObserver(
            io.BytesIO(
                encoded(
                    {
                        "type": "stream_event",
                        "event": {
                            "type": "content_block_delta",
                            "delta": {"type": "text_delta", "text": RUNNER.OUTPUT_MARKER},
                        },
                    }
                )
            ),
            fixture,
        )
        self.assertTrue(observer.finish())
        self.assertTrue(observer.partial_marker_seen)
        self.assertTrue(fixture.released)


if __name__ == "__main__":
    unittest.main()
