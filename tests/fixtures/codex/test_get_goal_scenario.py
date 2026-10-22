"""Focused safe-shape tests for the H008-a3 native get_goal query."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import unittest


FIXTURE_PATH = Path(__file__).with_name("local_gateway_fixture.py")
SPEC = importlib.util.spec_from_file_location("h008_goal_fixture_test", FIXTURE_PATH)
assert SPEC is not None and SPEC.loader is not None
FIXTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FIXTURE)
PROJECT_ROOT = Path(__file__).resolve().parents[3]
RUNNER_PATH = PROJECT_ROOT / "work" / "proofs" / "codex" / "run_codex_proof.py"
RUNNER_SPEC = importlib.util.spec_from_file_location("h008_goal_runner_test", RUNNER_PATH)
assert RUNNER_SPEC is not None and RUNNER_SPEC.loader is not None
RUNNER = importlib.util.module_from_spec(RUNNER_SPEC)
RUNNER_SPEC.loader.exec_module(RUNNER)


class GoalQueryShapeTests(unittest.TestCase):
    def test_only_advertised_zero_argument_get_goal_is_selected(self) -> None:
        schema = {
            "type": "function",
            "name": "get_goal",
            "parameters": {
                "type": "object",
                "properties": {},
                "required": [],
                "additionalProperties": False,
            },
        }
        selected = FIXTURE._select_get_goal_function([schema])
        self.assertEqual(selected, ("get_goal", "{}", "function_call"))
        self.assertIsNone(FIXTURE._select_get_goal_function([{"type": "function", "name": "exec_command"}]))
        self.assertIsNone(
            FIXTURE._select_get_goal_function(
                [{**schema, "parameters": {**schema["parameters"], "required": ["objective"]}}]
            )
        )


    def test_goal_output_summary_proves_null_without_retaining_content(self) -> None:
        direct = FIXTURE._summarize_goal_output("null")
        wrapped = FIXTURE._summarize_goal_output({"goal": None})
        active = FIXTURE._summarize_goal_output('{"goal":{"objective":"synthetic secret"}}')

        self.assertIs(direct["goal_is_null"], True)
        self.assertIs(wrapped["goal_is_null"], True)
        self.assertIs(active["goal_is_null"], False)
        self.assertTrue(direct["sha256"])
        self.assertNotIn("synthetic secret", repr(active))
        self.assertEqual(set(active), {"output_type", "parsed_json", "goal_field_present", "goal_is_null", "sha256"})

    def test_sanitizer_redacts_plain_and_json_escaped_paths_and_sentinel(self) -> None:
        sentinel = "sk-h008-local-fixture-only"
        path = r"D:\Projeler\Headroom\work\proofs\codex\runs\h008-synthetic\goal.json"
        samples = [f"{sentinel} {path}", json.dumps({"detail": f"{sentinel} {path}"})]
        for sample in samples:
            with self.subTest(escaped=sample.startswith("{")):
                safe = RUNNER._redact_excerpt(sample, [sentinel, path])
                self.assertNotIn(sentinel, safe)
                self.assertNotIn(path, safe)
                self.assertNotIn(path.replace("\\", "\\\\"), safe)


if __name__ == "__main__":
    unittest.main()
