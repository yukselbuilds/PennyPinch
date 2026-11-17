"""Local representation prototype; passing this does not establish model quality."""
import json
from pathlib import Path
import runpy
import unittest

ROOT = Path(__file__).resolve().parents[2]


class ExplicitRunsTests(unittest.TestCase):
    def setUp(self):
        self.prototype = runpy.run_path(str(ROOT / "bench/optimization/rules_runs_json.py"))

    def test_roundtrip_preserves_line_endings_empty_text_and_marker_like_input(self):
        for original in ("", "x\r\n" * 3 + "y\n" * 2 + "tail", "... (repeated 99 times)\n" * 4,
                         "first\rsecond\u2028last\n", "... (repeated 4 times)", "Türkçe 👋\r\n" * 3):
            with self.subTest(original=original):
                encoded = self.prototype["encode_runs"](original)
                self.assertEqual(self.prototype["decode_runs"](encoded), original)

    def test_independent_build_counts_are_available_as_explicit_numbers(self):
        plan = json.loads((ROOT / "bench/optimization/rules-heldout-plan.json").read_bytes())
        case, = [item for item in plan["cases"] if item["id"] == "build_manifest_v1"]
        stdout = "".join(run["text"] * run["occurrences"] for run in case["stdout_runs"])
        encoded = self.prototype["encode_runs"](stdout)
        frame = json.loads(encoded)
        self.assertEqual(frame["format"], "headroom.runs.json.v1")
        self.assertEqual(sum(run["occurrences"] for run in frame["runs"] if run["line"].startswith("INFO stage=compile result=ok")), case["expected"]["compile_ok"])
        self.assertEqual(sum(run["occurrences"] for run in frame["runs"] if run["line"].startswith("WARN ")), case["expected"]["warnings"])
        self.assertEqual(self.prototype["decode_runs"](encoded), stdout)
        self.assertLess(len(encoded.encode()), len(stdout.encode()))

    def test_invalid_frame_or_resource_expansion_is_rejected(self):
        valid = {"format": "headroom.runs.json.v1", "runs": [{"line": "x\n", "occurrences": 3}]}
        invalid = [None, [], {**valid, "format": "other"}, {**valid, "extra": True}, {**valid, "runs": None}]
        for count in (True, 0, -1, 1.0, "3", 10**9):
            invalid.append({**valid, "runs": [{"line": "x\n", "occurrences": count}]})
        invalid.append({**valid, "runs": [{"line": "a" * 1000, "occurrences": 64000}]})
        invalid.append({**valid, "runs": [{"line": 1, "occurrences": 1}]})
        invalid.append({**valid, "runs": [{"line": "first\nsecond\n", "occurrences": 1}]})
        for frame in invalid:
            with self.subTest(frame_type=type(frame).__name__):
                with self.assertRaises(ValueError):
                    self.prototype["decode_runs"](json.dumps(frame))
        with self.assertRaises(ValueError):
            self.prototype["encode_runs"]("x\n" * 64001)


if __name__ == "__main__":
    unittest.main()
