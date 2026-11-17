"""The JSON candidate uses its own data and preserves raw recovery semantics."""
import json
from pathlib import Path
import runpy
import unittest

ROOT = Path(__file__).resolve().parents[2]
pilot = runpy.run_path(str(ROOT / "bench/optimization/rules_pilot.py"))
live = runpy.run_path(str(ROOT / "bench/optimization/rules_live.py"))
summary = runpy.run_path(str(ROOT / "bench/optimization/rules_summary.py"))


class JsonAdapterTests(unittest.TestCase):
    def test_fixture_renderer_is_prepared_without_a_model_request(self):
        setup = pilot["prepare_provider"]()
        self.assertEqual(setup["model_requests"], 0)
        self.assertGreaterEqual(setup["cpu_ms"], 0)
        self.assertGreaterEqual(setup["elapsed_ms"], 0)
        response = {"id": "resp_prepared", "object": "response", "status": "completed", "model": "local",
                    "output": [], "usage": {"input_tokens": 1, "output_tokens": 0, "total_tokens": 1}}
        frames = pilot["Provider"].render_sse(response)
        self.assertIn(b"response.completed", b"".join(frames))
        self.assertIn(b"[DONE]", frames[-1])

    def test_new_cases_have_their_own_plan_and_guarded_representation(self):
        from headroom.transforms.lossless_compaction import expand_runs

        representation = runpy.run_path(str(ROOT / "bench/optimization/rules_runs_json.py"))
        self.assertEqual(len(pilot["JSON_CASES"]), 3)
        for case, details in pilot["JSON_CASES"].items():
            with self.subTest(case=case):
                plan = pilot["case_plan"](case)
                self.assertEqual(plan["candidate_id"], "explicit_runs_json_v1")
                self.assertEqual(pilot["case_details"](case), details)
                original = pilot["envelope"](case, "control")
                candidate = pilot["envelope"](case, "candidate")
                if details["expected_guard_status"] == "candidate_ready":
                    self.assertEqual({**candidate, "stdout": representation["decode_runs"](candidate["stdout"])}, original)
                    self.assertNotEqual(candidate["stdout"], expand_runs(original["stdout"]))
                else:
                    self.assertEqual(candidate, original)
        self.assertIsNone(pilot["case_plan"]("counts"))
        self.assertIs(pilot["case_plan"]("build_manifest_v1"), pilot["HELDOUT_PLAN"])

    def test_recovery_returns_original_before_json_transformation(self):
        case = "json_queue_receipts_v1"
        original = pilot["envelope"](case, "control")
        candidate = pilot["envelope"](case, "candidate")
        self.assertNotEqual(original, candidate)
        self.assertEqual(pilot["recovery_capture"](case, "candidate"), original)
        self.assertEqual(pilot["recovery_capture"]("counts", "candidate"), pilot["envelope"]("counts", "candidate"))

    def test_recovery_diagnostics_compare_raw_capture_without_retaining_it(self):
        original = {"stdout": "RAW_LOCAL_ONLY", "stderr": "", "exit_code": 0}
        candidate = {**original, "stdout": "ENCODED_LOCAL_ONLY"}
        body = {"input": [
            {"type": "function_call", "call_id": "read", "namespace": pilot["NAMESPACE"], "name": "inspect_result"},
            {"type": "function_call_output", "call_id": "read", "output": pilot["encode"](candidate)},
            {"type": "function_call", "call_id": "restore", "namespace": pilot["NAMESPACE"], "name": "headroom_retrieve"},
            {"type": "function_call_output", "call_id": "restore", "output": pilot["encode"](original)}]}
        evidence = live["forwarded_tool_evidence"](body, candidate, original_capture=original)
        self.assertEqual([item["capture_exact"] for item in evidence], [True, True])
        self.assertNotIn("RAW_LOCAL_ONLY", json.dumps(evidence))
        expected = {"ok": True}
        events = [{"type": "item.completed", "item": {"type": "mcp_tool_call", "server": "headroom_rule_pilot", "status": "completed",
            "tool": tool, "result": {"content": [{"type": "text", "text": pilot["encode"](capture)}]}}}
            for tool, capture in (("inspect_result", candidate), ("headroom_retrieve", original))]
        events.extend([{"type": "item.completed", "item": {"type": "agent_message", "text": json.dumps(expected)}}, {"type": "turn.completed"}])
        result = live["check_turn"](0, b"\n".join(json.dumps(event).encode() for event in events), expected,
                                     expected_capture=candidate, original_capture=original)
        self.assertTrue(result["fixed_tool_call"])
        self.assertTrue(all(item["capture_exact"] for item in result["producer_delivery"]))

    def test_schema_prompt_model_and_effort_are_identical_across_arms(self):
        case = "json_queue_receipts_v1"
        prompt = live["task_prompt"](pilot, case)
        self.assertIn("occurrences total copies", prompt)
        commands = [pilot["client_args"](Path("codex.exe"), Path("owned-work"), case, arm, "selected-model",
                    {"model_reasoning_effort": "medium"}, prompt=prompt, output_schema=Path("same-schema.json"))
                    for arm in ("control", "candidate")]
        self.assertEqual(commands[0][-1], commands[1][-1])
        for command in commands:
            self.assertEqual(command[command.index("--model") + 1], "selected-model")
            self.assertIn('model_reasoning_effort="medium"', command)
            self.assertEqual(command[command.index("--output-schema") + 1], "same-schema.json")

    def test_summary_keeps_rejected_marker_trials_out_of_the_json_candidate(self):
        self.assertFalse(summary["matches_candidate"]({"candidate_id": "repeated_log_lines_v1"}, "explicit_runs_json_v1"))
        self.assertFalse(summary["matches_candidate"]({}, "explicit_runs_json_v1"))
        self.assertTrue(summary["matches_candidate"]({}, "repeated_log_lines_v1"))
        self.assertTrue(summary["matches_candidate"]({"candidate_id": "explicit_runs_json_v1"}, "explicit_runs_json_v1"))


if __name__ == "__main__":
    unittest.main()
