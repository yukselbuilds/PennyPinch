"""The local upstream must reject invalid client-tool / CCR continuations."""
import importlib.util
import json
from pathlib import Path
import unittest

SPEC = importlib.util.spec_from_file_location("claude_tool_fixture", Path(__file__).with_name("tool_fixture.py"))
fixture = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(fixture)


def body(block=None):
    return {
        "model": fixture.MODEL,
        "stream": True,
        "output_config": {"effort": fixture.EFFORT},
        "tools": [{"name": fixture.CLIENT_TOOL, "input_schema": {"type": "object"}}],
        "messages": [{"role": "user", "content": "fixture only"}] if block is None else [
            {"role": "assistant", "content": [{"type": "tool_use", "id": fixture.CLIENT_ID,
                                                 "name": fixture.CLIENT_TOOL, "input": {}}]},
            {"role": "user", "content": [block]},
        ],
    }


class ToolFixtureTests(unittest.TestCase):
    def started(self):
        state = fixture.Conversation()
        reply = state.advance(body())
        self.assertEqual(reply["content"][0]["id"], fixture.CLIENT_ID)
        return state

    def test_wrong_call_id_is_rejected_before_retrieval(self):
        state = self.started()
        with self.assertRaisesRegex(AssertionError, "client_tool_result_missing"):
            state.advance(body({"type": "tool_result", "tool_use_id": "wrong",
                                "content": "[compressed hash=abc123def456]"}))
        self.assertFalse(state.complete)

    def test_success_requires_same_hash_original_content_and_second_tool_witness(self):
        state = self.started()
        second = body({"type": "tool_result", "tool_use_id": fixture.CLIENT_ID,
                       "content": "[compressed Retrieve more: hash=abc123def456]"})
        second["tools"].append({"name": "headroom_retrieve", "input_schema": {"type": "object"}})
        reply = state.advance(second)
        self.assertEqual(reply["content"][0]["input"], {"hash": "abc123def456"})
        continuation = {**second, "messages": second["messages"] + [
            {"role": "assistant", "content": reply["content"]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": fixture.RETRIEVE_ID,
                                             "content": json.dumps({"hash": "abc123def456", "original_content": fixture.LOGS,
                                                                    "original_item_count": 120})}]},
        ]}
        final = state.advance(continuation)
        self.assertEqual(final["content"][0]["id"], fixture.SECOND_ID)
        self.assertFalse(state.complete)
        continuation["messages"].extend([
            {"role": "assistant", "content": final["content"]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": fixture.SECOND_ID,
                                             "content": fixture.WITNESS}]},
        ])
        self.assertEqual(state.advance(continuation)["content"][0]["text"], fixture.FINAL)
        self.assertTrue(state.complete)

    def test_wrong_restored_content_cannot_be_accepted(self):
        state = self.started()
        second = body({"type": "tool_result", "tool_use_id": fixture.CLIENT_ID,
                       "content": "[compressed Retrieve more: hash=abc123def456]"})
        second["tools"].append({"name": "headroom_retrieve", "input_schema": {"type": "object"}})
        reply = state.advance(second)
        second["messages"].extend([
            {"role": "assistant", "content": reply["content"]},
            {"role": "user", "content": [{"type": "tool_result", "tool_use_id": fixture.RETRIEVE_ID,
                                             "content": json.dumps({"hash": "abc123def456", "original_content": fixture.LOGS[:-1],
                                                                    "original_item_count": 120})}]},
        ])
        with self.assertRaisesRegex(AssertionError, "retrieved_content_changed"):
            state.advance(second)
        self.assertFalse(state.complete)

    def test_missing_compression_or_changed_model_never_finishes(self):
        state = self.started()
        with self.assertRaisesRegex(AssertionError, "compression_marker_missing"):
            state.advance(body({"type": "tool_result", "tool_use_id": fixture.CLIENT_ID,
                                "content": fixture.LOGS}))
        changed = body()
        changed["model"] = "changed-fixture-model"
        with self.assertRaisesRegex(AssertionError, "model_changed"):
            fixture.Conversation().advance(changed)


if __name__ == "__main__":
    unittest.main()
