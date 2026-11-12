"""Guard the Desktop preflight's config boundary without running a client."""

import importlib.util
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("desktop_preflight", ROOT / "scripts/probe-macos-desktop.py")
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)


class DesktopPreflightTests(unittest.TestCase):
    def test_config_change_diagnosis_reports_names_and_matches_without_values(self):
        before = b'model="model-before"\nopenai_base_url="http://127.0.0.1:41731/v1"\n'
        after = b'model="PRIVATE_MODEL_VALUE"\nopenai_base_url="http://127.0.0.1:41731/v1"\npersonality="PRIVATE_VALUE"\nPRIVATE_UNKNOWN_KEY="PRIVATE_CONTENT"\n'
        observed = PROBE.config_change_observation(before, after)
        self.assertFalse(observed["protected_fields_match"]["model"])
        self.assertTrue(observed["protected_fields_match"]["openai_base_url"])
        self.assertEqual(observed["changed_known_fields"], ["model", "personality"])
        self.assertEqual(observed["changed_other_fields"], 1)
        self.assertNotIn("PRIVATE", repr(observed))

    def test_config_observation_requires_the_exact_isolated_endpoint(self):
        endpoint = "http://127.0.0.1:41731/v1"
        reply = {"result": {"config": {"openai_base_url": endpoint, "model_provider": "openai", "private": "DO_NOT_EXPORT"}}}
        observed = PROBE.config_observation(reply, endpoint)
        self.assertEqual(observed, {"endpoint_matches": True, "built_in_provider": True})
        self.assertNotIn("DO_NOT_EXPORT", repr(observed))
        for invalid in [None, [], {"error": {"message": "DO_NOT_EXPORT"}}, {"result": {}}, {"result": None},
                        {"result": {"config": {"openai_base_url": endpoint + "/other", "model_provider": "openai"}}}]:
            with self.subTest(invalid=bool(invalid)):
                with self.assertRaises(PROBE.core.ProbeError):
                    PROBE.config_observation(invalid, endpoint)

    def test_child_environment_never_inherits_account_or_github_secrets(self):
        state = Path("/private/tmp/hr-c7-test")
        env = PROBE.isolated_environment(state)
        self.assertEqual(env["CODEX_HOME"], str(state / "codex"))
        self.assertEqual(env["CODEX_ELECTRON_USER_DATA_PATH"], str(state / "electron"))
        self.assertEqual(set(env), {"PATH", "HOME", "TMPDIR", "CODEX_HOME", "CODEX_ELECTRON_USER_DATA_PATH"})

    def test_profile_diagnosis_never_exports_unknown_filenames_or_file_contents(self):
        parent = ROOT / "work/proofs/macos/h048-c7/profile-unit"
        parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=parent) as value:
            directory = Path(value)
            self.assertEqual(directory.resolve().parent, parent.resolve())
            (directory / "Crashpad").mkdir()
            (directory / "Local State").write_text("PRIVATE_CONTENT", encoding="utf-8")
            (directory / "PRIVATE_ACCOUNT_CANARY").touch()
            observed = PROBE.profile_metadata(directory)
            self.assertEqual(observed["kind"], "directory")
            self.assertEqual(observed["known_entries"], ["Crashpad", "Local State"])
            self.assertEqual(observed["other_entries"], 1)
            self.assertNotIn("PRIVATE", repr(observed))


if __name__ == "__main__":
    unittest.main()
