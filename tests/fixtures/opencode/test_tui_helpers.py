import copy
import importlib.util
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("opencode_tui_helpers", ROOT / "bench/opencode/tui.py")
tui = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = tui
spec.loader.exec_module(tui)


def exports(case="mixed"):
    expected = tui.fixture.CASES[case]
    parent_model = expected["parent"][1].split("/", 1)
    child_model = expected["child"][1].split("/", 1)
    parent_id, child_id = "ses_parent", "ses_child"
    metadata = {"sessionId": child_id, "parentSessionId": parent_id,
                "model": {"providerID": child_model[0], "modelID": child_model[1]}}
    def tools(role, with_task):
        names = [(tui.fixture.shared.CLIENT_TOOL, "logs"), ("headroom_retrieve", "retrieve"), (tui.fixture.shared.CLIENT_TOOL, "witness")]
        if role == "parent" and expected["parent"][0] == "responses":
            names.pop(1)
        if with_task:
            names.append(("task", "task"))
        return [{"type": "tool", "tool": name, "callID": f"call_{case}_{role}_{kind}",
                 "state": {"status": "completed", "metadata": metadata if kind == "task" else {}}} for name, kind in names]
    parent = {"info": {"id": parent_id}, "messages": [
        {"info": {"role": "user"}, "parts": [{"type": "text", "text": tui.fixture.marker(case, "parent")}]},
        {"info": {"role": "assistant", "providerID": parent_model[0], "modelID": parent_model[1], "agent": "headroom_primary"},
         "parts": [*tools("parent", True), {"type": "text", "text": tui.fixture.marker(case, "parent") + "_OK"}]}]}
    child = {"info": {"id": child_id, "parentID": parent_id}, "messages": [
        {"info": {"role": "assistant", "providerID": child_model[0], "modelID": child_model[1], "agent": expected["agent"]},
         "parts": [*tools("child", False), {"type": "text", "text": tui.fixture.marker(case, "child") + "_OK"}]}]}
    return parent, child


class TuiHelperTests(unittest.TestCase):
    def test_both_protocol_exports_keep_parent_child_identity(self):
        for case in tui.fixture.CASES:
            row = tui.validate_exports(case, *exports(case))
            self.assertTrue(row["parent_final_exact"] and row["child_final_exact"])
            self.assertNotEqual(row["parent_session_id"], row["child_session_id"])

    def test_missing_terminal_prompt_cannot_pass_with_a_final_marker(self):
        parent, child = exports()
        parent["messages"][0]["parts"][0]["text"] = "HEADROOM_PARENT_same"
        with self.assertRaisesRegex(AssertionError, "terminal_prompt_missing"):
            tui.validate_exports("mixed", parent, child)

    def test_parent_task_and_child_export_must_agree(self):
        parent, child = exports()
        child["info"]["parentID"] = "ses_unrelated"
        with self.assertRaisesRegex(AssertionError, "child_parent_changed"):
            tui.validate_exports("mixed", parent, child)

    def test_partial_parent_tool_and_wrong_final_are_rejected(self):
        parent, child = exports()
        wrong = copy.deepcopy(parent)
        wrong["messages"][-1]["parts"][0]["state"]["status"] = "running"
        with self.assertRaisesRegex(AssertionError, "tui_parent_tool_incomplete"):
            tui.validate_exports("mixed", wrong, child)
        parent["messages"][-1]["parts"][-1]["text"] = "partial"
        with self.assertRaisesRegex(AssertionError, "tui_parent_final_changed"):
            tui.validate_exports("mixed", parent, child)

    def test_title_disable_is_local_to_tui_fixture(self):
        config = tui.configuration()
        self.assertEqual(config["agent"]["title"], {"disable": True})
        self.assertNotIn("title", tui.native.configuration()["agent"])
        self.assertEqual(config["permission"]["*"], "deny")
        self.assertEqual(config["subagent_depth"], 1)
        self.assertEqual(config["provider"]["lab"]["models"]["other"]["name"], "Headroom switch fixture")
        self.assertEqual(len(set(tui.MODEL_NAMES.values())), len(tui.fixture.CASES))


if __name__ == "__main__":
    unittest.main()
