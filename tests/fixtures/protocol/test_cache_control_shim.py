"""Fail-closed tests for the exact-version/source startup adaptation."""

from __future__ import annotations

import importlib.util
import sys
import types
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[3]
SHIM_PATH = ROOT / "work" / "proofs" / "protocol" / "cache_control_shim.py"


def load_shim():
    spec = importlib.util.spec_from_file_location("headroom_proof_cache_control_shim", SHIM_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("shim-module-unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class ShimGuardTests(unittest.TestCase):
    def _fake_headroom_modules(self, version: str):
        headroom = types.ModuleType("headroom")
        headroom.__path__ = []
        version_module = types.ModuleType("headroom._version")
        version_module.__version__ = version
        cache = types.ModuleType("headroom.cache")
        cache.__path__ = []
        prefix_tracker = types.ModuleType("headroom.cache.prefix_tracker")
        prefix_tracker.__file__ = str(Path(__file__).resolve())

        def original(*_args, **_kwargs):
            return []

        prefix_tracker.normalize_message_cache_control = original
        headroom._version = version_module
        headroom.cache = cache
        cache.prefix_tracker = prefix_tracker
        modules = {
            "headroom": headroom,
            "headroom._version": version_module,
            "headroom.cache": cache,
            "headroom.cache.prefix_tracker": prefix_tracker,
        }
        return modules, prefix_tracker, original

    def test_version_mismatch_does_not_patch_normalizer(self) -> None:
        shim = load_shim()
        modules, prefix_tracker, original = self._fake_headroom_modules("0.42.0")
        with patch.dict(sys.modules, modules):
            with self.assertRaisesRegex(RuntimeError, "headroom-version-mismatch"):
                shim.install()
        self.assertIs(prefix_tracker.normalize_message_cache_control, original)

    def test_source_mismatch_does_not_patch_normalizer(self) -> None:
        shim = load_shim()
        modules, prefix_tracker, original = self._fake_headroom_modules("0.41.0")
        with patch.dict(sys.modules, modules):
            with self.assertRaisesRegex(RuntimeError, "prefix-tracker-source-mismatch"):
                shim.install()
        self.assertIs(prefix_tracker.normalize_message_cache_control, original)


if __name__ == "__main__":
    unittest.main()
