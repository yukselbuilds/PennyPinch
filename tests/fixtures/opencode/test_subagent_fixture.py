import copy
import importlib.util
import json
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("subagent_acceptance_runner", ROOT / "bench/opencode/subagents.py")
runner = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = runner
spec.loader.exec_module(runner)
fixture = runner.fixture


def body(conversation, results=()):
    return {"model": conversation.model, "messages": [
        {"role": "user", "content": fixture.marker(conversation.case, conversation.role)},
        *[{"role": "tool", "tool_call_id": conversation.call_id(kind), "content": value} for kind, value in results]],
        "tools": [{"function": {"name": name}} for name in (fixture.shared.CLIENT_TOOL, "headroom_retrieve", "task")]}


class SubagentFixtureTests(unittest.TestCase):
    def test_child_completes_only_after_exact_ccr_and_witness(self):
        child = fixture.Conversation("same", "child")
        child.advance(body(child))
        child.advance(body(child, [("logs", "<<ccr:123456abcdef>>")]))
        child.advance(body(child, [("retrieve", json.dumps({"hash": "123456abcdef", "original_content": fixture.shared.LOGS}))]))
        final = child.advance(body(child, [("witness", fixture.shared.WITNESS)]))
        self.assertEqual(final["content"], "HEADROOM_CHILD_same_OK")
        self.assertTrue(child.complete)

    def test_wrong_content_is_not_a_successful_retrieval(self):
        child = fixture.Conversation("mixed", "child")
        child.advance(body(child))
        child.advance(body(child, [("logs", "<<ccr:123456abcdef>>")]))
        with self.assertRaisesRegex(AssertionError, "retrieval_changed"):
            child.advance(body(child, [("retrieve", json.dumps({"hash": "123456abcdef", "original_content": "shortened"}))]))
        self.assertFalse(child.complete)

    def test_responses_retrieval_metadata_preserves_exact_content_check(self):
        parent = fixture.Conversation("mixed", "parent")
        parent.step, parent.content_hash = 2, "123456abcdef"
        restored = {"hash": parent.content_hash, "original_content": fixture.shared.LOGS, "original_item_count": 120}
        with self.assertRaisesRegex(AssertionError, "retrieval_changed"):
            parent.advance(body(parent, [("retrieve", json.dumps({**restored, "original_content": "partial"}))]))
        parent.advance(body(parent, [("retrieve", json.dumps(restored))]))
        self.assertTrue(parent.rows[-1]["byte_exact_retrieval"])

    def test_parent_does_not_accept_uncompleted_task(self):
        parent = fixture.Conversation("same", "parent")
        parent.step = 4
        with self.assertRaisesRegex(AssertionError, "child_completion_missing"):
            parent.advance(body(parent, [("task", 'HEADROOM_CHILD_same_OK <task id="ses_child" state="running">')]))

    def test_identity_uses_latest_user_not_tool_prompt(self):
        request = body(fixture.Conversation("switch", "parent"))
        request["messages"].append({"role": "assistant", "content": "HEADROOM_CHILD_mixed"})
        self.assertEqual(fixture.identity(request), ("switch", "parent"))

    def test_responses_user_text_blocks_are_not_tool_blocks(self):
        request = {"messages": [{"role": "user", "content": [{"type": "input_text", "text": "HEADROOM_PARENT_mixed"}]}]}
        self.assertEqual(fixture.identity(request), ("mixed", "parent"))
        request["messages"][0]["content"][0]["type"] = "input_image"
        with self.assertRaisesRegex(AssertionError, "user_content_shape_changed"):
            fixture.identity(request)

    def test_wire_model_and_reasoning_cannot_silently_change(self):
        child = fixture.Conversation("mixed", "child")
        changed = body(child)
        changed["model"] = fixture.shared.MODEL
        with self.assertRaisesRegex(AssertionError, "wire_model_changed"):
            child.advance(changed)
        changed = body(child)
        changed["reasoning_effort"] = "low"
        with self.assertRaisesRegex(AssertionError, "unexpected_reasoning"):
            child.advance(changed)

    def test_task_and_child_export_must_agree_on_parent_and_model(self):
        parent_id, child_id = "ses_parent", "ses_child"
        task = {"state": {"metadata": {"sessionId": child_id, "parentSessionId": parent_id,
                                        "model": {"providerID": "lab", "modelID": "child"}}}}
        parent = {"info": {"id": parent_id}, "messages": [{"info": {
            "role": "assistant", "providerID": "openai", "modelID": "slot", "agent": "headroom_primary"}}]}
        child = {"info": {"id": child_id, "parentID": parent_id}, "messages": [{"info": {
            "role": "assistant", "providerID": "lab", "modelID": "child", "agent": "headroom_other"}, "parts": [
            *[{"type": "tool", "tool": name, "callID": "call_mixed_child_" + kind, "state": {"status": "completed"}}
              for name, kind in ((fixture.shared.CLIENT_TOOL, "logs"), ("headroom_retrieve", "retrieve"), (fixture.shared.CLIENT_TOOL, "witness"))],
            {"type": "text", "text": "HEADROOM_CHILD_mixed_OK"}]}]}
        self.assertTrue(runner.validate_sessions("mixed", parent, child, task, parent_id)["child_final_exact"])
        wrong_parent = copy.deepcopy(child)
        wrong_parent["info"]["parentID"] = "ses_different"
        with self.assertRaisesRegex(AssertionError, "child_parent_changed"):
            runner.validate_sessions("mixed", parent, wrong_parent, task, parent_id)
        wrong_model = copy.deepcopy(child)
        wrong_model["messages"][0]["info"]["modelID"] = "slot"
        with self.assertRaisesRegex(AssertionError, "child_model_agent_changed"):
            runner.validate_sessions("mixed", parent, wrong_model, task, parent_id)

    def test_configuration_limits_task_to_two_fixed_child_types(self):
        config = runner.configuration()
        self.assertEqual(config["subagent_depth"], 1)
        self.assertEqual(config["permission"]["task"], {"*": "deny", "headroom_inherit": "allow", "headroom_other": "allow"})
        self.assertNotIn("task", config["tools"])
        for name in ("headroom_inherit", "headroom_other"):
            self.assertEqual(config["agent"][name]["permission"]["task"], "deny")
            self.assertEqual(config["agent"][name]["permission"]["*"], "deny")


if __name__ == "__main__":
    unittest.main()
