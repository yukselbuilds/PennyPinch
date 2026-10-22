"""Focused tests for content-free OpenCode CLI event parsing."""

from __future__ import annotations

import importlib.util
import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
RUNNER_PATH = ROOT / "work" / "proofs" / "opencode" / "run_opencode_proof.py"
SPEC = importlib.util.spec_from_file_location("opencode_proof_runner_test", RUNNER_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("runner-module-unavailable")
RUNNER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(RUNNER)


def jsonl(*records: dict[str, object]) -> bytes:
    return b"".join(
        (json.dumps(record, separators=(",", ":")) + "\n").encode("utf-8")
        for record in records
    )


class OpenCodeProofParserTests(unittest.TestCase):
    def test_completed_text_and_step_usage_are_summarized_without_text_retention(self) -> None:
        summary = RUNNER.summarize_cli_json_lines(
            jsonl(
                {
                    "type": "text",
                    "sessionID": "synthetic-session-id",
                    "part": {
                        "type": "text",
                        "text": RUNNER.OUTPUT_MARKER,
                        "time": {"start": 1, "end": 2},
                    },
                },
                {
                    "type": "step_finish",
                    "part": {
                        "type": "step-finish",
                        "reason": "stop",
                        "tokens": {"input": 12, "output": 4},
                    },
                },
            )
        )
        self.assertTrue(summary["final_marker_seen"])
        self.assertEqual(summary["usage"], {"input_tokens": 12, "output_tokens": 4})
        self.assertEqual(summary["usage_source"], "step-finish")
        self.assertNotIn("text", summary)
        self.assertNotIn("sessionID", summary)

    def test_missing_terminal_usage_is_unknown(self) -> None:
        summary = RUNNER.summarize_cli_json_lines(
            jsonl({"type": "text", "part": {"type": "text", "text": RUNNER.OUTPUT_MARKER}})
        )
        self.assertTrue(summary["final_marker_seen"])
        self.assertEqual(summary["usage"], {})
        self.assertEqual(summary["usage_source"], "missing")
        self.assertFalse(summary["step_finish_seen"])


class EngineConfigProofTests(unittest.TestCase):
    class FakeHelper:
        def __init__(self, ttl_payload: dict[str, object]) -> None:
            self.responses = [
                (
                    200,
                    {
                        "version": "0.41.0",
                        "config": {
                            "openai_api_url": "http://127.0.0.1:43210/v1",
                            "mode": "cache",
                            "savings_profile": "coding",
                            "cache": True,
                            "optimize": True,
                            "runtime_env": {"HEADROOM_OUTPUT_SHAPER": "0"},
                        },
                    },
                ),
                (200, ttl_payload),
            ]

        def request_json(self, _port: int, _path: str) -> tuple[int, object]:
            return self.responses.pop(0)

    def test_nested_ccr_ttl_must_be_exactly_7200(self) -> None:
        result = RUNNER.verify_engine_config(
            self.FakeHelper({"store": {"default_ttl_seconds": 7200}}),
            8787,
            43210,
        )
        self.assertEqual(result["ccr_default_ttl_seconds"], 7200)

    def test_missing_or_wrong_nested_ccr_ttl_is_rejected(self) -> None:
        for ttl_payload in (
            {"store": {}},
            {"store": {"default_ttl_seconds": 1800}},
            {},
        ):
            with self.subTest(ttl_payload=ttl_payload):
                with self.assertRaises(RUNNER.ProofFailure) as raised:
                    RUNNER.verify_engine_config(self.FakeHelper(ttl_payload), 8787, 43210)
                self.assertEqual(raised.exception.code, "engine-ccr-ttl-mismatch")


if __name__ == "__main__":
    unittest.main()
