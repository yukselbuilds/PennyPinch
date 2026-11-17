"""The paired benchmark must ask for the same types its answer oracle requires."""
import json
from pathlib import Path
import runpy
import unittest

ROOT = Path(__file__).resolve().parents[2]
pilot = runpy.run_path(str(ROOT / "bench/optimization/rules_pilot.py"))
live = runpy.run_path(str(ROOT / "bench/optimization/rules_live.py"))
EXPECTED = pilot["CASES"]["counts"][4]


def check(answer, *, progress=False):
    messages = ["Working", json.dumps(answer)] if progress else [json.dumps(answer)]
    events = [{"type": "item.completed", "item": {"type": "agent_message", "text": text}} for text in messages]
    events.append({"type": "turn.completed"})
    return live["check_turn"](0, b"\n".join(json.dumps(event).encode() for event in events), EXPECTED)


class RulesPilotContractTests(unittest.TestCase):
    def test_requested_schema_matches_exact_answer_types_without_giving_the_answer(self):
        schema = live["ANSWER_SCHEMA"]
        types = {int: "integer", str: "string"}
        self.assertEqual(schema["properties"], {key: {"type": types[type(value)]} for key, value in EXPECTED.items()})
        self.assertEqual(set(schema["required"]), set(EXPECTED))
        self.assertFalse(schema["additionalProperties"])
        self.assertFalse(any(character.isdigit() for character in json.dumps(schema)))
        prompt = pilot["CASES"]["counts"][3]
        self.assertIn("passed and held as JSON integers", prompt)
        self.assertIn("exact_limit and signed_factor as strings", prompt)

    def test_both_arms_use_one_schema_and_the_selected_model_and_effort(self):
        schema_path = Path("owned-proof/answer-schema.json")
        prompts = []
        for arm in ("control", "candidate"):
            command = pilot["client_args"](Path("codex.exe"), Path("owned-work"), "counts", arm, "selected-model",
                                             {"model_reasoning_effort": "medium"}, output_schema=schema_path)
            self.assertEqual(command[command.index("--output-schema") + 1], str(schema_path))
            self.assertEqual(command[command.index("--model") + 1], "selected-model")
            self.assertIn('model_reasoning_effort="medium"', command)
            prompts.append(command[-1])
        self.assertEqual(prompts[0], prompts[1])

    def test_string_counts_are_diagnosed_and_remain_rejected(self):
        row = check({**EXPECTED, "passed": str(EXPECTED["passed"]), "held": str(EXPECTED["held"])})
        self.assertFalse(row["exact_answer"])
        self.assertFalse(row["answer_checks"]["type_matches"]["passed"])
        self.assertFalse(row["answer_checks"]["type_matches"]["held"])
        self.assertEqual(row["answer_checks"]["integer_string_matches"], {"passed": True, "held": True})

    def test_wrong_integer_is_distinguished_from_wrong_type(self):
        row = check({**EXPECTED, "passed": EXPECTED["passed"] + 1})
        self.assertFalse(row["exact_answer"])
        self.assertTrue(all(row["answer_checks"]["type_matches"].values()))
        self.assertFalse(row["answer_checks"]["exact_fields"]["passed"])
        self.assertFalse(row["answer_checks"]["integer_string_matches"]["passed"])

    def test_exact_oracle_stays_strict(self):
        self.assertTrue(check(EXPECTED)["exact_answer"])
        for invalid in ({**EXPECTED, "passed": float(EXPECTED["passed"])},
                        {**EXPECTED, "passed": True}, {**EXPECTED, "extra": "field"}):
            self.assertFalse(check(invalid)["exact_answer"])

    def test_progress_is_ignored_and_the_final_answer_must_still_be_exact(self):
        self.assertTrue(check(EXPECTED, progress=True)["exact_answer"])
        self.assertFalse(check({**EXPECTED, "held": EXPECTED["held"] + 1}, progress=True)["exact_answer"])
        events = [
            {"type": "item.completed", "item": {"type": "agent_message", "text": json.dumps(EXPECTED)}},
            {"type": "item.completed", "item": {"type": "agent_message", "text": "Incomplete result"}},
            {"type": "turn.completed"},
        ]
        self.assertFalse(live["check_turn"](0, b"\n".join(json.dumps(event).encode() for event in events), EXPECTED)["exact_answer"])

    def test_producer_capture_is_matched_without_retaining_content(self):
        capture = {"stdout": "PRIVATE_DELIVERY_CANARY", "stderr": "", "exit_code": 0}
        call = {"type": "mcp_tool_call", "server": "headroom_rule_pilot", "tool": "inspect_result",
                "status": "completed", "result": {"content": [{"type": "text", "text": pilot["encode"](capture)}]}}
        events = [{"type": "item.completed", "item": call},
                  {"type": "item.completed", "item": {"type": "agent_message", "text": json.dumps(EXPECTED)}},
                  {"type": "turn.completed"}]
        row = live["check_turn"](0, b"\n".join(json.dumps(event).encode() for event in events), EXPECTED, expected_capture=capture)
        self.assertTrue(row["fixed_tool_call"])
        self.assertTrue(row["producer_delivery"][0]["capture_exact"])
        self.assertNotIn("PRIVATE_DELIVERY_CANARY", json.dumps(row))
        call["result"] = {"content": None, "isError": True}
        row = live["check_turn"](0, b"\n".join(json.dumps(event).encode() for event in events), EXPECTED, expected_capture=capture)
        self.assertFalse(row["fixed_tool_call"])
        self.assertTrue(row["producer_delivery"][0]["error_reported"])
        self.assertFalse(row["producer_delivery"][0]["capture_exact"])

    def test_forwarded_capture_matches_its_owned_call(self):
        capture = {"stdout": "PRIVATE_FORWARDED_CANARY", "stderr": "", "exit_code": 0}
        body = {"input": [
            {"type": "function_call", "call_id": "owned", "namespace": "mcp__headroom_rule_pilot", "name": "recover_result"},
            {"type": "function_call_output", "call_id": "owned", "output": [{"type": "input_text", "text": pilot["encode"](capture)}]},
            {"type": "function_call", "call_id": "other", "namespace": "other_server", "name": "recover_result"},
            {"type": "function_call_output", "call_id": "other", "output": pilot["encode"](capture)},
            {"type": "function_call", "call_id": [], "name": "recover_result"},
            {"type": "function_call_output", "call_id": [], "output": None},
        ]}
        result = live["forwarded_tool_evidence"](body, capture)
        self.assertEqual(len(result), 1)
        self.assertTrue(result[0]["capture_exact"])
        self.assertNotIn("PRIVATE_FORWARDED_CANARY", json.dumps(result))
        body["input"][1]["output"] = [{"type": "input_text", "text": "<<ccr:owned,string,test>>"}]
        result = live["forwarded_tool_evidence"](body, capture)
        self.assertFalse(result[0]["capture_exact"])
        self.assertTrue(result[0]["ccr_marker_present"])
        self.assertEqual(live["forwarded_tool_evidence"]({"input": None}, capture), [])


if __name__ == "__main__":
    unittest.main()
