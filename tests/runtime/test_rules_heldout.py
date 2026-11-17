"""Held-out task boundaries; local fixtures are never live quality evidence."""
import copy
import hashlib
import json
from pathlib import Path
import runpy
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
pilot = runpy.run_path(str(ROOT / "bench/optimization/rules_pilot.py"))
live = runpy.run_path(str(ROOT / "bench/optimization/rules_live.py"))


def row(arm, cached):
    return {"arm": arm, "first_upstream_cache_read_input_tokens": cached, "elapsed_ms": 10 if arm == "control" else 20,
            "total_usage": {"total_input_tokens": 30, "output_tokens": 2, "total_tokens": 32,
                            "cache_read_input_tokens": cached, "cache_creation_input_tokens": 0},
            "resources": {"local_cpu_ms": 4.5, "resource_samples": 2, "observed_engine_private_bytes": 512},
            "resource_error": None}


class RulesHeldoutTests(unittest.TestCase):
    def test_frozen_cases_have_strict_schemas_and_preserved_channels(self):
        plan = pilot["HELDOUT_PLAN"]
        self.assertEqual(len(pilot["HELDOUT_CASES"]), 3)
        self.assertTrue(set(pilot["HELDOUT_CASES"]).isdisjoint(plan["excluded_design_cases"]))
        for case, details in pilot["HELDOUT_CASES"].items():
            with self.subTest(case=case):
                stdout, stderr, code, instruction, expected = pilot["case_data"](case)
                original = {"stdout": stdout, "stderr": stderr, "exit_code": code}
                self.assertEqual(hashlib.sha256(pilot["encode"](original).encode()).hexdigest(), details["capture_sha256"])
                schema = details["answer_schema"]
                self.assertEqual(schema["properties"], {key: {"type": "integer" if type(value) is int else "string"} for key, value in expected.items()})
                self.assertEqual(set(schema["required"]), set(expected))
                self.assertFalse(schema["additionalProperties"])
                self.assertFalse(any(char.isdigit() for char in instruction + json.dumps(schema)))
                self.assertEqual(pilot["envelope"](case, "control"), original)
                candidate = pilot["envelope"](case, "candidate")
                self.assertEqual((candidate["stderr"], candidate["exit_code"]), (stderr, code))
                if details["rule_expected_eligible"]:
                    from headroom.transforms.lossless_compaction import expand_runs
                    self.assertEqual(expand_runs(candidate["stdout"]), stdout)
                    self.assertLess(len(candidate["stdout"]), len(stdout))
                else:
                    self.assertEqual(candidate, original)

    def test_changed_capture_is_rejected(self):
        case = "build_manifest_v1"
        changed = copy.deepcopy(pilot["HELDOUT_CASES"][case])
        changed["stdout_runs"][0]["occurrences"] += 1
        with mock.patch.dict(pilot["HELDOUT_CASES"], {case: changed}):
            with self.assertRaisesRegex(AssertionError, "heldout_capture_changed"):
                pilot["case_data"](case)

    def test_first_cache_keeps_zero_and_does_not_use_retry_totals(self):
        first = {"truncated": False, "attempts": [{"status": 200, "usage": {"cache_read_input_tokens": 0}},
                                                {"status": 200, "usage": {"cache_read_input_tokens": 512}}]}
        later = {"truncated": False, "attempts": [{"status": 200, "usage": {"cache_read_input_tokens": 128}}]}
        records = {"hr_9_000002": later, "hr_10_000001": first}
        self.assertEqual(live["first_cache_read"](records), 0)
        first["attempts"][0]["usage"] = None
        self.assertIsNone(live["first_cache_read"](records))
        first["attempts"][0] = {"status": 400, "usage": None}
        self.assertIsNone(live["first_cache_read"](records))
        first["attempts"][0] = {"status": 200, "usage": {"cache_read_input_tokens": False}}
        self.assertIsNone(live["first_cache_read"](records))

    def test_single_request_usage_is_retained_for_incomplete_tasks(self):
        identity = "hr_10_000001"
        usage_row = {"surface": "openai_responses", "client_status": 200, "truncated": False,
                     "attempts": [{"status": 200, "usage": {"cache_read_input_tokens": 0}}], "usage": row("control", 0)["total_usage"]}
        usage = {"probe": {"request_logs": lambda engine: [identity]}, "records": lambda engine: {identity: usage_row}}
        result = live["measure"](None, usage, 1)
        self.assertEqual(result["request_count"], 1)
        self.assertEqual(result["total_usage"]["total_tokens"], 32)
        self.assertEqual(result["first_upstream_cache_read_input_tokens"], 0)

    def test_cache_conditions_and_missing_measurements_remain_explicit(self):
        for cached, expected in ((0, "cold"), (2, "warm")):
            observed = live["heldout_observations"]([row("candidate", cached), row("control", cached)], expected)
            self.assertTrue(observed["cache_start_matches"])
            self.assertTrue(observed["all_provider_counters_known"])
            self.assertTrue(observed["resource_data_complete"])
            self.assertEqual(observed["pair_metrics"]["added_completed_task_elapsed_ms"], 10)
            self.assertEqual(observed["matrix_acceptance"], "not_tested")
        for value in (None, False, 1):
            self.assertEqual(live["heldout_observations"]([row("control", 0), row("candidate", value)], "cold")["cache_start_observed"], "unconfirmed")
        incomplete = row("candidate", 0)
        incomplete["total_usage"]["cache_creation_input_tokens"] = None
        incomplete["resource_error"] = "sampling_failed"
        observed = live["heldout_observations"]([row("control", 0), incomplete], "cold")
        self.assertFalse(observed["all_provider_counters_known"])
        self.assertFalse(observed["resource_data_complete"])

    def test_budget_reserves_second_arm_independent_of_its_name(self):
        self.assertEqual(live["arm_budget"](0, 0), 6)
        self.assertEqual(live["arm_budget"](2, 1), 6)
        self.assertEqual(live["arm_budget"](6, 1), 2)
        for sent, index in ((6, 0), (7, 1), (-1, 0), (0, 2)):
            with self.assertRaises(AssertionError):
                live["arm_budget"](sent, index)
        self.assertEqual(pilot["HELDOUT_PLAN"]["pair_order_by_repetition"][1], ["candidate", "control"])

    def test_rule_cpu_is_sum_of_known_preparations_and_invalid_values_fail(self):
        temporary_root = ROOT / "work/proofs/optimization/heldout-unit"
        temporary_root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=temporary_root) as directory:
            path = Path(directory) / "rule-resources.jsonl"
            path.write_text('{"rule_prepare_cpu_ms":1.25}\n{"rule_prepare_cpu_ms":2.75}\n', encoding="ascii")
            self.assertEqual(live["read_rule_cpu"](path), 4)
            for invalid in ('{"rule_prepare_cpu_ms":-1}\n', '{"rule_prepare_cpu_ms":NaN}\n', '{"rule_prepare_cpu_ms":false}\n', '{}\n',
                            '{"rule_prepare_cpu_ms":1e308}\n{"rule_prepare_cpu_ms":1e308}\n'):
                path.write_text(invalid, encoding="ascii")
                with self.assertRaises(AssertionError):
                    live["read_rule_cpu"](path)

    def test_timeout_retains_usage_and_finishes_resources(self):
        temporary_root = ROOT / "work/proofs/optimization/heldout-unit"
        temporary_root.mkdir(parents=True, exist_ok=True)
        sampler = mock.Mock()
        sampler.finish.return_value = {"harness_cpu_ms": 1, "engine_cpu_ms": 2, "resource_samples": 2, "observed_engine_private_bytes": 512}
        fake_pilot = {**pilot, "client_args": mock.Mock(return_value=[]), "client_environment": lambda: {},
                      "run_client": mock.Mock(side_effect=subprocess.TimeoutExpired("fixed fixture", 120))}
        measured = {"request_count": 1, "usage_record_count": 1, "total_usage": None, "first_upstream_cache_read_input_tokens": None}
        with tempfile.TemporaryDirectory(dir=temporary_root) as directory:
            arm = Path(directory) / "control"
            arm.mkdir()
            counter = arm / "http-attempts.txt"
            counter.write_bytes(b"1\n")
            (arm / "rule-resources.jsonl").write_text('{"rule_prepare_cpu_ms":1}\n', encoding="ascii")
            usage = {"replay": SimpleNamespace(Resources=mock.Mock(return_value=sampler))}
            with mock.patch.dict(live["run_arm"].__globals__, {"measure": mock.Mock(return_value=measured)}):
                result = live["run_arm"](fake_pilot, Path("codex.exe"), None, SimpleNamespace(port=12345, pids=[]), arm,
                    "selected-model", "medium", None, Path(directory) / "catalog.json", counter, usage, None, 6, case="failed_release_v1")
            self.assertFalse((arm / "client").exists())
        sampler.finish.assert_called_once()
        self.assertEqual(result["run_failure"], "arm_timeout")
        self.assertFalse(result["turn_completed"])
        self.assertEqual(result["request_count"], 1)
        self.assertEqual(result["resources"]["local_cpu_ms"], 4)


if __name__ == "__main__":
    unittest.main()
