"""Local representation prototype; passing this does not establish model quality."""
import json
from pathlib import Path
import runpy
import unittest

ROOT = Path(__file__).resolve().parents[2]


def snapshot(output=None, *, tool="exec_command", scope="owned-json-trial", call_id="json-call", arguments=None):
    if output is None:
        output = {"stdout": "INFO queue=alpha result=sent\r\n" * 30, "stderr": "", "exit_code": 0, "producer_id": "fixed"}
    return [{"type": "session_meta", "payload": {"id": scope}},
            {"type": "response_item", "payload": {"type": "function_call", "call_id": call_id, "name": tool,
                "arguments": json.dumps(arguments if arguments is not None else {"cmd": "fixed producer", "workdir": "."})}},
            {"type": "response_item", "payload": {"type": "function_call_output", "call_id": call_id, "output": output}}]


def encoded(records):
    return ("\n".join(json.dumps(item, separators=(",", ":")) for item in records) + "\n").encode()


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

    def test_guarded_capture_changes_only_owned_successful_stdout(self):
        records = snapshot()
        original = records[-1]["payload"]["output"]
        result = self.prototype["trial_capture"](encoded(records), "owned-json-trial", "json-call")
        self.assertEqual(result["status"], "candidate_ready")
        candidate = result["candidate"]
        self.assertEqual({**candidate, "stdout": self.prototype["decode_runs"](candidate["stdout"])}, original)
        self.assertEqual({key: value for key, value in candidate.items() if key != "stdout"},
                         {key: value for key, value in original.items() if key != "stdout"})
        self.assertLess(result["after_bytes"], result["before_bytes"])

    def test_error_source_numeric_and_unknown_channels_are_preserved(self):
        ordinary = snapshot()[-1]["payload"]["output"]
        variants = [snapshot({**ordinary, "stderr": "WARN exact=0.123456789012349\r\n"}),
                    snapshot({**ordinary, "exit_code": 7}), snapshot({**ordinary, "exit_code": False}),
                    snapshot(ordinary, tool="Read"), snapshot({**ordinary, "stdout": "9007199254741199\n" * 30}),
                    snapshot(json.dumps(ordinary)), snapshot({**ordinary, "isError": True})]
        errored = snapshot()
        errored[-1]["payload"]["error"] = "fixed_error"
        variants.append(errored)
        for records in variants:
            with self.subTest(tool=records[1]["payload"]["name"]):
                result = self.prototype["trial_capture"](encoded(records), "owned-json-trial", "json-call")
                self.assertEqual(result["status"], "protected")
                self.assertEqual(result["candidate"], records[-1]["payload"]["output"])

    def test_owner_call_conflicts_merged_history_and_bad_input_are_rejected(self):
        records = snapshot()
        for owner, identity in (("other-owner", "json-call"), ("owned-json-trial", "other-call")):
            self.assertEqual(self.prototype["trial_capture"](encoded(records), owner, identity)["status"], "ambiguous")
        conflict = snapshot()
        conflict.append({"type": "response_item", "payload": {**conflict[-1]["payload"], "output": {"stdout": "different"}}})
        for altered in (conflict, records + records[1:], [records[0], records[-1], records[1]],
                        snapshot(arguments={"cmd": "fixed producer"}), snapshot(scope="")):
            self.assertEqual(self.prototype["trial_capture"](encoded(altered), "owned-json-trial", "json-call")["status"], "ambiguous")
        self.assertEqual(self.prototype["trial_capture"](b'broken-json', "owned-json-trial", "json-call")["status"], "invalid_file")

    def test_small_nonreducing_stdout_is_returned_unchanged(self):
        records = snapshot({"stdout": "INFO once\n", "stderr": "", "exit_code": 0})
        result = self.prototype["trial_capture"](encoded(records), "owned-json-trial", "json-call")
        self.assertEqual(result["status"], "unchanged")
        self.assertEqual(result["candidate"], records[-1]["payload"]["output"])


if __name__ == "__main__":
    unittest.main()
