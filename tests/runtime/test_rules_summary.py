"""The acceptance summary cannot promote missing, duplicated or failed trials."""
import copy
from pathlib import Path
import runpy
import unittest

ROOT = Path(__file__).resolve().parents[2]
plan = runpy.run_path(str(ROOT / "bench/optimization/rules_pilot.py"))["HELDOUT_PLAN"]


def pairs():
    result = []
    for case in plan["cases"]:
        for cache in plan["required_cache_starts"]:
            for repeat in range(plan["repetitions_per_case_and_cache_start"]):
                rows = []
                for arm in plan["pair_order_by_repetition"][repeat]:
                    rows.append({"arm": arm, "turn_completed": True, "exact_answer": True, "fixed_tool_call": True,
                        "tool_checks": {"sequence_valid": True, "owned_server": True, "all_completed": True, "no_reported_errors": True},
                        "first_upstream_cache_read_input_tokens": 0 if cache == "cold" else 20,
                        "request_count": 2, "usage_record_count": 2, "forwarded_http_attempts": 2,
                        "total_usage": {"total_input_tokens": 90 if arm == "control" else 80, "output_tokens": 10,
                                        "total_tokens": 100 if arm == "control" else 90,
                                        "cache_read_input_tokens": 0 if cache == "cold" else 20, "cache_creation_input_tokens": 0},
                        "elapsed_ms": 1000 if arm == "control" else 1100,
                        "resource_error": None, "resources": {"local_cpu_ms": 40, "observed_engine_private_bytes": 1000}})
                result.append({"case": case["id"], "repeat_index": repeat, "arm_order": plan["pair_order_by_repetition"][repeat],
                    "heldout_observations": {"cache_start_requested": cache, "cache_start_observed": cache, "cache_start_matches": True},
                    "passed": True, "failure": None, "pair_attempts_total": 4, "prior_attempts": 0,
                    "pair_attempt_limit": 8, "rows": rows, "inputs_match": True,
                    "stops": [{"shutdown": "graceful", "engine_exit_code": 0, "owned_port_closed": True}] * 2})
    return result


class RulesSummaryTests(unittest.TestCase):
    def setUp(self):
        self.summary = runpy.run_path(str(ROOT / "bench/optimization/rules_summary.py"))["summarize"]

    def test_no_or_partial_results_do_not_pass(self):
        self.assertFalse(self.summary(plan, [])["passed"])
        result = self.summary(plan, pairs()[:1])
        self.assertFalse(result["passed"])
        self.assertEqual(result["required_pairs"], 18)
        self.assertTrue(result["missing_pairs"])

    def test_every_case_repeat_cache_group_and_named_arm_are_required(self):
        result = self.summary(plan, pairs())
        self.assertTrue(result["passed"])
        for group in result["cache_groups"].values():
            self.assertEqual(group["total_tokens_saved"], 90)
            self.assertEqual(group["percent_of_control"], 10)
            self.assertEqual(group["added_completed_task_elapsed_p95_ms"], 100)

    def test_failed_duplicated_or_changed_input_trials_cannot_be_hidden(self):
        for change in ({"passed": False}, {"inputs_match": False}, {"arm_order": ["candidate", "control"]}):
            records = pairs()
            records[0].update(change)
            self.assertFalse(self.summary(plan, records)["passed"])
        duplicate = pairs()
        duplicate.append(copy.deepcopy(duplicate[0]))
        self.assertFalse(self.summary(plan, duplicate)["passed"])

    def test_unknown_counters_wrong_answers_and_limits_reject_the_matrix(self):
        for mutation in ("unknown", "quality", "cache", "budget", "cpu", "elapsed", "memory", "gain"):
            records = pairs()
            for record in records:
                candidate = next(row for row in record["rows"] if row["arm"] == "candidate")
                if mutation == "unknown": candidate["total_usage"]["cache_creation_input_tokens"] = None
                if mutation == "quality": candidate["exact_answer"] = False
                if mutation == "cache": candidate["first_upstream_cache_read_input_tokens"] = None
                if mutation == "budget": record["pair_attempts_total"] = 9
                if mutation == "cpu": candidate["resources"]["local_cpu_ms"] = 2001
                if mutation == "elapsed": candidate["elapsed_ms"] = 2001
                if mutation == "memory": candidate["resources"]["observed_engine_private_bytes"] = 1610612737
                if mutation == "gain":
                    candidate["total_usage"].update(total_tokens=100, total_input_tokens=90)
            with self.subTest(mutation=mutation):
                self.assertFalse(self.summary(plan, records)["passed"])


if __name__ == "__main__":
    unittest.main()
