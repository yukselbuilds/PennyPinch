"""The controlled turn must not replace credentials or change its model contract."""

import importlib.util
import json
import os
from pathlib import Path
import tempfile
import sqlite3
import tomllib
from types import SimpleNamespace
import unittest


ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("mac_traffic_test", ROOT / "scripts/probe-macos-desktop-traffic.py")
TRAFFIC = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TRAFFIC)


class DesktopTrafficTests(unittest.TestCase):
    @unittest.skipIf(os.name == "nt", "POSIX ownership is part of the native collector guard")
    def test_native_collector_receipt_requires_the_expected_model_and_returns_only_counts(self):
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory).resolve()
            (state / "metrics").mkdir()
            with sqlite3.connect(state / "metrics/metrics.sqlite3") as connection:
                connection.execute("CREATE TABLE events(payload TEXT)")
                connection.execute("INSERT INTO events VALUES (?)", (json.dumps({"attribution": {"model": "other"}}),))
                connection.commit()
                self.assertFalse(TRAFFIC.metrics_observed(state, "expected")["model_matches"])
                connection.execute("INSERT INTO events VALUES (?)", (json.dumps({"attribution": {"model": "expected"}, "extra": "private-test-only"}),))
            observed = TRAFFIC.metrics_observed(state, "expected")
            self.assertEqual(observed["matched_rows"], 1)
            self.assertEqual(observed["stored_events"], 2)
            self.assertNotIn("private-test-only", json.dumps(observed))

    def test_fixture_profile_keeps_the_builtin_endpoint_auth_store_and_analytics_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory)
            (state / "codex").mkdir()
            config = state / "codex/config.toml"
            config.write_text('openai_base_url = "http://127.0.0.1:41301/v1"\nmodel_provider = "openai"\n'
                              'cli_auth_credentials_store = "file"\n[analytics]\nenabled = false\n')
            fixture = SimpleNamespace(model="different-model-under-test", reasoning_effort="high", api_key="synthetic-local-only")
            TRAFFIC.ControlledTurn(fixture, "http://127.0.0.1:41301/v1").prepare(state, config)
            parsed = tomllib.loads(config.read_text())
            self.assertEqual(parsed["openai_base_url"], "http://127.0.0.1:41301/v1")
            self.assertEqual(parsed["model_provider"], "openai")
            self.assertEqual(parsed["model"], fixture.model)
            self.assertEqual(parsed["model_reasoning_effort"], "high")
            self.assertEqual(parsed["cli_auth_credentials_store"], "file")
            self.assertFalse(parsed["analytics"]["enabled"])
            self.assertEqual(parsed["projects"][str(state / "workspace")]["trust_level"], "trusted")
            self.assertEqual(json.loads((state / "codex/auth.json").read_text()),
                             {"auth_mode": "apikey", "OPENAI_API_KEY": fixture.api_key})

    def test_existing_credential_is_refused_before_any_profile_change(self):
        with tempfile.TemporaryDirectory() as directory:
            state = Path(directory)
            (state / "codex").mkdir()
            auth = state / "codex/auth.json"
            auth.write_bytes(b'existing-test-credential')
            config = state / "codex/config.toml"
            config.write_bytes(b'[analytics]\nenabled = false\n')
            with self.assertRaisesRegex(TRAFFIC.core.ProbeError, "test_auth_already_exists"):
                TRAFFIC.ControlledTurn(None, None).prepare(state, config)
            self.assertEqual(auth.read_bytes(), b'existing-test-credential')
            self.assertEqual(config.read_bytes(), b'[analytics]\nenabled = false\n')
            self.assertFalse((state / "workspace").exists())


if __name__ == "__main__":
    unittest.main()
