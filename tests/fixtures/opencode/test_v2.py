"""Guard V2 fixture isolation, output parsing and route/auth acceptance."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("v2_fixture_tests", ROOT / "bench/opencode/v2.py")
v2 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(v2)


def events(*records):
    return b"\n".join(json.dumps(row).encode() for row in records)


class V2Tests(unittest.TestCase):
    def test_failed_job_assignment_stops_the_owned_cli_handle(self):
        process = Mock()
        process.poll.return_value = None
        job = Mock()
        job.assign.side_effect = OSError("synthetic_assignment_failure")
        supervisor = Mock()
        supervisor.KillOnCloseJob.return_value = job
        with patch.object(v2.subprocess, "Popen", return_value=process), self.assertRaises(OSError):
            v2.cli_call(["--version"], ROOT, {}, supervisor)
        job.close.assert_called_once()
        process.kill.assert_called_once()
        process.wait.assert_called_once_with(timeout=10)

    def test_split_text_requires_exact_final_and_one_terminal_step(self):
        final = [{"type": "text", "part": {"text": value}} for value in (v2.FINAL[:8], v2.FINAL[8:])]
        self.assertTrue(v2.summarize(events(*final, {"type": "step_finish"}))["final_marker_exact"])
        for tail in ([], [{"type": "step_finish"}, {"type": "error"}], [{"type": "step_finish"}] * 2):
            with self.assertRaises(AssertionError):
                v2.summarize(events(*final, *tail))
        with self.assertRaises(AssertionError):
            v2.summarize(events({"type": "text", "part": {"text": "unexpected " + v2.FINAL}}, {"type": "step_finish"}))

    def test_route_auth_model_and_stream_are_independent_guards(self):
        for surface, path, model in (("chat", "/v1/chat/completions", v2.fixture.MODEL),
                                     ("responses", "/v1/responses", v2.fixture.RESPONSES_MODEL)):
            headers = {"Authorization": "Bearer " + v2.SENTINEL}
            body = {"model": model, "stream": True, "input": v2.PROMPT}
            self.assertTrue(v2.validate_request(path, headers, body, surface)["wire_model_preserved"])
            for altered_path, altered_headers, altered_body in (
                (path + "/wrong", headers, body), (path, {}, body),
                (path, headers, {**body, "model": "other"}), (path, headers, {**body, "stream": False}),
                (path, headers, {**body, "input": "other"}),
            ):
                with self.assertRaises(AssertionError):
                    v2.validate_request(altered_path, altered_headers, altered_body, surface)

    def test_config_uses_v2_schema_and_deny_by_default(self):
        for surface in ("chat", "responses"):
            config = v2.configuration(12345, surface)
            provider = config["model"].split("/")[0]
            self.assertEqual(list(config["providers"]), [provider])
            self.assertNotIn("provider", config)
            self.assertEqual(config["policies"][-2:], [
                {"action": action, "resource": provider, "effect": "allow"} for action in ("provider.use", "integration.use")])
            self.assertTrue(all(row["effect"] == "deny" for row in config["policies"][:-2]))
            self.assertFalse(config["providers"][provider]["models"]["slot"]["capabilities"]["tools"])
            self.assertEqual(config["providers"][provider]["settings"]["baseURL"], "http://127.0.0.1:12345/v1")

    def test_global_paths_cannot_escape_private_profile(self):
        with tempfile.TemporaryDirectory(dir=ROOT / "work") as temporary:
            root = Path(temporary)
            names = ("home", "config", "state", "data", "cache", "db")
            raw = "\n".join(name + "  " + str(root / name) for name in names).encode()
            self.assertTrue(v2.validate_paths(raw, root)["all_paths_inside_fixture"])
            with self.assertRaises(AssertionError):
                v2.validate_paths(raw + ("\nextra  " + str(root.parent)).encode(), root)
            with self.assertRaises(AssertionError):
                v2.validate_paths(b"home", root)

    def test_environment_ignores_caller_auth_and_config_overrides(self):
        with tempfile.TemporaryDirectory(dir=ROOT / "work") as temporary, patch.dict(v2.os.environ, {
            "OPENAI_API_KEY": "caller-secret", "OPENCODE_CONFIG": "caller-file", "OPENCODE_CONFIG_DIR": "caller-directory",
            "OPENCODE_PASSWORD": "caller-password", "ANTHROPIC_API_KEY": "caller-other-secret"}):
            root = Path(temporary)
            env = v2.environment(root, 9)
            self.assertEqual(env["OPENAI_API_KEY"], v2.SENTINEL)
            for key in ("OPENCODE_CONFIG", "OPENCODE_CONFIG_DIR", "OPENCODE_CONFIG_CONTENT", "OPENCODE_PASSWORD", "ANTHROPIC_API_KEY"):
                self.assertNotIn(key, env)
            for key in ("HOME", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "ProgramData", "TEMP", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_STATE_HOME", "XDG_CACHE_HOME"):
                self.assertTrue(Path(env[key]).is_relative_to(root))


if __name__ == "__main__":
    unittest.main()
