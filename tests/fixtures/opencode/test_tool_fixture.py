"""Reject corrupt OpenCode Chat tool/CCR continuations before final success."""
import importlib.util
import json
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location("opencode_tool_fixture", Path(__file__).with_name("tool_fixture.py"))
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)


def request(call_id=None, content=None):
    return {"model": fixture.MODEL, "stream": True,
            "tools": [{"type": "function", "function": {"name": fixture.CLIENT_TOOL, "parameters": {"type": "object"}}},
                      {"type": "function", "function": {"name": "headroom_retrieve", "parameters": {"type": "object"}}}],
            "messages": [{"role": "user", "content": "synthetic test only"}]
            + ([{"role": "tool", "tool_call_id": call_id, "content": content}] if call_id else [])}


class ToolFixtureTests(unittest.TestCase):
    def retrieval_ready(self):
        state = fixture.Conversation()
        state.advance(request())
        value = request(fixture.CLIENT_ID, "[compressed Retrieve more: hash=abc123def456]")
        reply = state.advance(value)
        self.assertEqual(json.loads(reply["tool_calls"][0]["function"]["arguments"]), {"hash": "abc123def456"})
        return state

    def test_wrong_call_id_or_missing_marker_never_finishes(self):
        for call_id, content, code in [("wrong", fixture.LOGS, "tool_result_missing"),
                                      (fixture.CLIENT_ID, fixture.LOGS, "compression_marker_missing")]:
            with self.subTest(code=code):
                state = fixture.Conversation()
                state.advance(request())
                with self.assertRaisesRegex(AssertionError, code):
                    state.advance(request(call_id, content))
                self.assertFalse(state.complete)

    def test_wrong_retrieval_hash_and_content_are_rejected(self):
        for content in [{"hash": "bad", "original_content": fixture.LOGS},
                        {"hash": "abc123def456", "original_content": fixture.LOGS[:-1]}]:
            with self.subTest(hash=content["hash"]):
                state = self.retrieval_ready()
                with self.assertRaisesRegex(AssertionError, "retrieval_changed"):
                    state.advance(request(fixture.RETRIEVE_ID, json.dumps(content)))
                self.assertFalse(state.complete)

    def test_second_native_tool_witness_is_required_for_success(self):
        state = self.retrieval_ready()
        restored = json.dumps({"hash": "abc123def456", "original_content": fixture.LOGS})
        reply = state.advance(request(fixture.RETRIEVE_ID, restored))
        self.assertEqual(reply["tool_calls"][0]["id"], fixture.SECOND_ID)
        self.assertFalse(state.complete)
        self.assertEqual(state.advance(request(fixture.SECOND_ID, fixture.WITNESS))["content"], fixture.FINAL)
        self.assertTrue(state.complete)

    def test_wire_model_or_reasoning_drift_is_rejected(self):
        changed = request()
        changed["model"] = "other-wire-model"
        with self.assertRaisesRegex(AssertionError, "wire_model_changed"):
            fixture.Conversation().advance(changed)
        state = fixture.Conversation()
        state.advance(request())
        changed = request(fixture.CLIENT_ID, "[compressed hash=abc123def456]")
        changed["reasoning_effort"] = "high"
        with self.assertRaisesRegex(AssertionError, "reasoning_changed"):
            state.advance(changed)

    def test_responses_conversion_preserves_function_output_identity_and_bytes(self):
        value = {"model": fixture.RESPONSES_MODEL, "stream": True,
                 "tools": [{"type": "function", "name": fixture.CLIENT_TOOL, "parameters": {"type": "object"}}],
                 "input": [{"type": "function_call_output", "call_id": fixture.CLIENT_ID, "output": fixture.LOGS}]}
        normalized = fixture.normalize_request(value, "responses")
        self.assertEqual(normalized["model"], fixture.RESPONSES_MODEL)
        self.assertEqual(fixture.result(normalized, fixture.CLIENT_ID), fixture.LOGS)
        reply = fixture.Conversation(fixture.RESPONSES_MODEL).advance(normalized)
        serialized = json.loads(fixture.responses_response(reply, 1, False))
        self.assertEqual(serialized["output"][0]["call_id"], fixture.CLIENT_ID)
        self.assertEqual(serialized["output"][0]["name"], fixture.CLIENT_TOOL)


if __name__ == "__main__":
    unittest.main()
