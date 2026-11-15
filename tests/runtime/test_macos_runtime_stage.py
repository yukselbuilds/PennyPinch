"""Bounded contract tests for staging a private macOS engine runtime."""
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock
from contextlib import redirect_stdout

ROOT = Path(__file__).resolve().parents[2]


def _load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise ImportError(f"{name} module is unavailable")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


macos_payload = _load("macos_payload", ROOT / "scripts/macos-payload.py")
stage_module = _load("stage_macos_runtime", ROOT / "scripts/stage-macos-runtime.py")
OVERLAY_NAMES = (
    "macos_engine.py",
    "cache_control_shim.py",
    "optimization_control.py",
    "optimization-sources.json",
)


def _entries_from_rows(rows):
    entries = {}
    for row in rows:
        name, kind, *fields = row
        if kind == "file":
            mode, size, digest = fields
            entries[name] = {"kind": "file", "mode": mode, "size": size, "sha256": digest}
        elif kind == "directory":
            (mode,) = fields
            entries[name] = {"kind": "directory", "mode": mode}
        elif kind == "symlink":
            (target,) = fields
            entries[name] = {"kind": "symlink", "target": target}
        else:
            raise AssertionError("unexpected inventory entry kind")
    return entries


class MacOSRuntimeStageTests(unittest.TestCase):
    def setUp(self):
        parent = ROOT / "work/proofs/macos-stage/unit"
        parent.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="stage-", dir=parent)
        self.root = Path(self.temporary.name)
        self.payload = self.root / "payload-source"
        self.payload.mkdir()
        (self.payload / "site-packages").mkdir()
        (self.payload / "site-packages/demo.py").write_bytes(b"fixture-runtime\n")
        (self.payload / "python").mkdir()
        (self.payload / "python/python3.13").write_bytes(b"fixture-interpreter\n")

    def tearDown(self):
        self.assertTrue(self.root.resolve().is_relative_to(ROOT / "work/proofs/macos-stage/unit"))
        self.temporary.cleanup()

    def test_publishes_payload_overlays_and_exact_manifest(self):
        output = self.root / "published"
        before_summary, before_rows = macos_payload.inventory(self.payload)
        source_hashes = {
            name: hashlib.sha256((ROOT / "runtime" / name).read_bytes()).hexdigest()
            for name in OVERLAY_NAMES
        }
        lock_hash = hashlib.sha256((ROOT / "runtime/engine-deps/uv.lock").read_bytes()).hexdigest()

        stdout = io.StringIO()
        argv = [
            "stage-macos-runtime.py",
            "--payload",
            str(self.payload),
            "--output",
            str(output),
        ]
        with mock.patch.object(sys, "argv", argv), redirect_stdout(stdout):
            self.assertEqual(stage_module.main(), 0)
        result = json.loads(stdout.getvalue())

        self.assertEqual(
            set(result), {"manifest_sha256", "payload_files", "payload_bytes", "source_payload_unchanged"}
        )
        self.assertEqual(result["payload_files"], before_summary["files"])
        self.assertEqual(result["payload_bytes"], before_summary["bytes"])
        self.assertIs(result["source_payload_unchanged"], True)
        self.assertRegex(result["manifest_sha256"], r"^[0-9a-f]{64}$")
        manifest_bytes = (output / "manifest.json").read_bytes()
        self.assertEqual(hashlib.sha256(manifest_bytes).hexdigest(), result["manifest_sha256"])
        manifest = json.loads(manifest_bytes)
        self.assertEqual(
            set(manifest),
            {
                "schema_version", "platform", "engine_version", "python_version",
                "dependency_lock_sha256", "python", "site_packages", "root_mode",
                "entries", "overlays",
            },
        )
        self.assertEqual(manifest["schema_version"], 1)
        self.assertEqual(manifest["platform"], "darwin-arm64")
        self.assertEqual(manifest["engine_version"], "0.44.0")
        self.assertEqual(manifest["python_version"], "3.13.13")
        self.assertEqual(manifest["dependency_lock_sha256"], lock_hash)
        self.assertEqual(manifest["python"], "python/bin/python3.13")
        self.assertEqual(manifest["site_packages"], "site-packages")
        self.assertEqual(
            manifest["entries"],
            _entries_from_rows(before_rows),
        )
        self.assertEqual(manifest["overlays"], source_hashes)
        self.assertEqual(
            manifest_bytes,
            json.dumps(manifest, sort_keys=True, indent=2).encode("utf-8") + b"\n",
        )
        after_summary, after_rows = macos_payload.inventory(self.payload)
        self.assertEqual((after_summary, after_rows), (before_summary, before_rows))
        for name, digest in source_hashes.items():
            copied = output / name
            self.assertEqual(hashlib.sha256(copied.read_bytes()).hexdigest(), digest)

    def test_rejects_existing_output_and_paths_outside_work_without_replacing(self):
        existing = self.root / "existing"
        existing.mkdir()
        marker = existing / "keep.txt"
        marker.write_bytes(b"preserve")
        with self.assertRaises((ValueError, FileExistsError)):
            stage_module.stage(self.payload, existing)
        self.assertEqual(marker.read_bytes(), b"preserve")

        outside = ROOT / "scripts" / "stage-rejection-must-not-create"
        with self.assertRaises(ValueError):
            stage_module.stage(self.payload, outside)
        self.assertFalse(outside.exists())
        with self.assertRaises(ValueError):
            stage_module.stage(ROOT / "scripts", self.root / "outside-payload")
        self.assertFalse((self.root / "outside-payload").exists())

        with self.assertRaises(ValueError):
            stage_module.stage(self.payload, self.payload / "recursive-output")
        self.assertFalse((self.payload / "recursive-output").exists())

    def test_rejects_private_payload(self):
        private = self.payload / ".env.local"
        private.write_bytes(b"synthetic fixture only")
        with self.assertRaises(ValueError):
            stage_module.stage(self.payload, self.root / "private-output")
        private.unlink()

    def test_rejects_external_symlink(self):
        outside = self.root / "external.bin"
        outside.write_bytes(b"external fixture")
        link = self.payload / "external-link"
        try:
            link.symlink_to(outside)
        except (OSError, NotImplementedError) as error:
            if os.name == "nt":
                self.skipTest(f"file symlink privilege unavailable: {type(error).__name__}")
            raise
        with self.assertRaises(ValueError):
            stage_module.stage(self.payload, self.root / "link-output")
        self.assertFalse((self.root / "link-output").exists())

    def test_rejects_payload_changed_after_initial_inventory(self):
        source_file = self.payload / "python/python3.13"
        original_inventory = stage_module.macos_payload.inventory
        changed = False

        def inventory_then_change(path):
            nonlocal changed
            result = original_inventory(path)
            if Path(path) == self.payload and not changed:
                source_file.write_bytes(b"changed during stage")
                changed = True
            return result

        output = self.root / "changed-output"
        with mock.patch.object(
            stage_module.macos_payload, "inventory", side_effect=inventory_then_change
        ):
            with self.assertRaises(ValueError):
                stage_module.stage(self.payload, output)
        self.assertTrue(changed)
        self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
