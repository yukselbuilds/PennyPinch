"""Contract tests for read-only verification of a packaged macOS runtime."""

import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import stat
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from unittest import mock


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
bundle_module = _load("verify_macos_bundle", ROOT / "scripts/verify-macos-bundle.py")
OVERLAYS = (
    "macos_engine.py",
    "cache_control_shim.py",
    "optimization_control.py",
    "optimization-sources.json",
)


def _manifest_entries(rows):
    entries = {}
    for name, kind, *fields in rows:
        if kind == "file":
            mode, size, digest = fields
            entries[name] = {"kind": kind, "mode": mode, "size": size, "sha256": digest}
        elif kind == "directory":
            (mode,) = fields
            entries[name] = {"kind": kind, "mode": mode}
        else:
            (target,) = fields
            entries[name] = {"kind": kind, "target": target}
    return entries


class MacOSBundleTests(unittest.TestCase):
    def setUp(self):
        parent = ROOT / "work/proofs/macos-bundle/unit"
        parent.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="bundle-", dir=parent)
        self.root = Path(self.temporary.name)
        self.runtime = self.root / "staged-runtime"
        self.payload = self.runtime / "payload"
        self.payload.mkdir(parents=True)
        (self.payload / "site-packages").mkdir()
        (self.payload / "site-packages/demo.py").write_bytes(b"synthetic runtime\n")
        for name in OVERLAYS:
            (self.runtime / name).write_bytes((name + "\n").encode("ascii"))
        summary, rows = macos_payload.inventory(self.payload)
        manifest = {
            "schema_version": 1,
            "platform": "darwin-arm64",
            "engine_version": "0.41.0",
            "python_version": "3.13.13",
            "dependency_lock_sha256": "a" * 64,
            "python": "python/bin/python3.13",
            "site_packages": "site-packages",
            "root_mode": stat.S_IMODE(self.payload.stat().st_mode),
            "entries": _manifest_entries(rows),
            "overlays": {
                name: hashlib.sha256((self.runtime / name).read_bytes()).hexdigest()
                for name in OVERLAYS
            },
        }
        self.manifest_bytes = json.dumps(
            manifest, ensure_ascii=True, indent=2, sort_keys=True
        ).encode("utf-8") + b"\n"
        (self.runtime / "manifest.json").write_bytes(self.manifest_bytes)
        if os.name == "posix":
            self.runtime.chmod(0o700)
            (self.runtime / "manifest.json").chmod(0o600)
        self.manifest_sha = hashlib.sha256(self.manifest_bytes).hexdigest()
        self.bundle = self.root / "Headroom.app"
        self.bundle_runtime = self.bundle / "Contents/Resources/runtime"
        self.bundle_runtime.parent.mkdir(parents=True)
        shutil.copytree(self.runtime, self.bundle_runtime, symlinks=True, copy_function=shutil.copy2)
        if os.name == "posix":
            self.bundle_runtime.chmod(0o700)

    def tearDown(self):
        self.assertTrue(self.root.resolve().is_relative_to(ROOT / "work/proofs/macos-bundle/unit"))
        self.temporary.cleanup()

    def test_verifies_identical_runtime_and_returns_only_bounded_metadata(self):
        result = bundle_module.verify(self.bundle, self.runtime, self.manifest_sha)
        self.assertEqual(
            set(result),
            {
                "schema_version", "status", "manifest_sha256", "payload_files",
                "payload_bytes", "payload_inventory_sha256", "source_unchanged",
                "bundle_unchanged",
            },
        )
        self.assertEqual(result["schema_version"], 1)
        self.assertEqual(result["status"], "verified")
        self.assertEqual(result["manifest_sha256"], self.manifest_sha)
        self.assertEqual(result["payload_inventory_sha256"], macos_payload.measure(self.payload)["tree_sha256"])
        self.assertIs(result["source_unchanged"], True)
        self.assertIs(result["bundle_unchanged"], True)
        self.assertNotIn(str(self.root), json.dumps(result))
        if os.name == "posix":
            self.bundle_runtime.chmod(0o755)
            self.assertEqual(
                bundle_module.verify(self.bundle, self.runtime, self.manifest_sha)["status"],
                "verified",
            )

    def test_cli_emits_metadata_on_success_and_fixed_error_on_failure(self):
        stdout, stderr = io.StringIO(), io.StringIO()
        argv = [
            "verify-macos-bundle.py", "--bundle", str(self.bundle), "--runtime",
            str(self.runtime), "--expected-manifest", self.manifest_sha,
        ]
        with mock.patch.object(sys, "argv", argv), redirect_stdout(stdout), redirect_stderr(stderr):
            self.assertEqual(bundle_module.main(), 0)
        self.assertEqual(json.loads(stdout.getvalue())["status"], "verified")
        self.assertEqual(stderr.getvalue(), "")

        stdout, stderr = io.StringIO(), io.StringIO()
        argv[-1] = "0" * 64
        with mock.patch.object(sys, "argv", argv), redirect_stdout(stdout), redirect_stderr(stderr):
            self.assertEqual(bundle_module.main(), 2)
        self.assertEqual(stdout.getvalue(), "")
        error = json.loads(stderr.getvalue())
        self.assertEqual(set(error), {"error"})
        self.assertIsInstance(error["error"], str)
        self.assertNotIn(str(self.root), stderr.getvalue())

    def test_rejects_wrong_manifest_digest_or_changed_manifest_copy(self):
        with self.assertRaises(ValueError):
            bundle_module.verify(self.bundle, self.runtime, "0" * 64)
        (self.bundle_runtime / "manifest.json").write_bytes(self.manifest_bytes + b" ")
        with self.assertRaises(ValueError):
            bundle_module.verify(self.bundle, self.runtime, self.manifest_sha)

        manifest = json.loads(self.manifest_bytes)
        manifest["schema_version"] = 2
        malformed = json.dumps(manifest, sort_keys=True, indent=2).encode("utf-8") + b"\n"
        (self.runtime / "manifest.json").write_bytes(malformed)
        (self.bundle_runtime / "manifest.json").write_bytes(malformed)
        digest = hashlib.sha256(malformed).hexdigest()
        with self.assertRaises(ValueError):
            bundle_module.verify(self.bundle, self.runtime, digest)

    def test_rejects_payload_overlay_and_extra_runtime_changes(self):
        source_file = self.bundle_runtime / "payload/site-packages/demo.py"
        source_file.write_bytes(b"changed bundle payload\n")
        with self.assertRaises(ValueError):
            bundle_module.verify(self.bundle, self.runtime, self.manifest_sha)
        source_file.write_bytes(b"synthetic runtime\n")

        overlay = self.bundle_runtime / OVERLAYS[0]
        overlay.write_bytes(b"changed overlay\n")
        with self.assertRaises(ValueError):
            bundle_module.verify(self.bundle, self.runtime, self.manifest_sha)
        overlay.write_bytes((OVERLAYS[0] + "\n").encode("ascii"))

        overlay.unlink()
        overlay.mkdir()
        with self.assertRaises(ValueError):
            bundle_module.verify(self.bundle, self.runtime, self.manifest_sha)
        overlay.rmdir()
        overlay.write_bytes((OVERLAYS[0] + "\n").encode("ascii"))

        (self.bundle_runtime / "unexpected.bin").write_bytes(b"extra")
        with self.assertRaises(ValueError):
            bundle_module.verify(self.bundle, self.runtime, self.manifest_sha)

    @unittest.skipUnless(os.name == "posix", "POSIX permission bits are required")
    def test_rejects_permission_drift(self):
        target = self.bundle_runtime / "payload/site-packages/demo.py"
        original_file_mode = stat.S_IMODE(target.stat().st_mode)
        target.chmod(original_file_mode ^ 0o100)
        with self.assertRaises(ValueError):
            bundle_module.verify(self.bundle, self.runtime, self.manifest_sha)
        target.chmod(original_file_mode)
        payload = self.bundle_runtime / "payload"
        payload.chmod(stat.S_IMODE(payload.stat().st_mode) ^ 0o010)
        with self.assertRaises(ValueError):
            bundle_module.verify(self.bundle, self.runtime, self.manifest_sha)

    def test_rejects_outside_and_overlapping_roots(self):
        with self.assertRaises(ValueError):
            bundle_module.verify(self.bundle, ROOT / "scripts", self.manifest_sha)
        with self.assertRaises(ValueError):
            bundle_module.verify(self.bundle, self.bundle_runtime, self.manifest_sha)

    def test_rejects_noncanonical_parent_component_in_runtime_root(self):
        (self.root / "unused").mkdir()
        alias = Path(os.fspath(self.root / "unused" / ".." / "staged-runtime"))
        self.assertIn("..", os.fspath(alias))
        with self.assertRaises(ValueError):
            bundle_module.verify(self.bundle, alias, self.manifest_sha)

    def test_rejects_aliased_roots(self):
        alias = self.root / "runtime-alias"
        try:
            alias.symlink_to(self.runtime, target_is_directory=True)
        except (OSError, NotImplementedError) as error:
            if os.name == "nt":
                self.skipTest(f"directory symlink privilege unavailable: {type(error).__name__}")
            raise
        with self.assertRaises(ValueError):
            bundle_module.verify(self.bundle, alias, self.manifest_sha)

    def test_rejects_changed_symlink_target(self):
        first = self.bundle_runtime / "payload/site-packages/alias.py"
        source = self.runtime / "payload/site-packages/alias.py"
        try:
            source.symlink_to("demo.py")
            first.symlink_to("demo.py")
        except (OSError, NotImplementedError) as error:
            if os.name == "nt":
                self.skipTest(f"file symlink privilege unavailable: {type(error).__name__}")
            raise
        _, rows = macos_payload.inventory(self.payload)
        manifest = json.loads(self.manifest_bytes)
        manifest["entries"] = _manifest_entries(rows)
        self.manifest_bytes = json.dumps(
            manifest, ensure_ascii=True, indent=2, sort_keys=True
        ).encode("utf-8") + b"\n"
        self.manifest_sha = hashlib.sha256(self.manifest_bytes).hexdigest()
        (self.runtime / "manifest.json").write_bytes(self.manifest_bytes)
        (self.bundle_runtime / "manifest.json").write_bytes(self.manifest_bytes)
        self.assertEqual(bundle_module.verify(self.bundle, self.runtime, self.manifest_sha)["status"], "verified")
        first.unlink()
        first.symlink_to("./demo.py")
        with self.assertRaises(ValueError):
            bundle_module.verify(self.bundle, self.runtime, self.manifest_sha)

    def test_rejects_inode_replacement_after_initial_snapshots(self):
        original_snapshot = bundle_module._runtime_snapshot
        calls = 0

        def snapshot_then_replace(path):
            nonlocal calls
            result = original_snapshot(path)
            calls += 1
            if calls == 2:
                overlay = self.bundle_runtime / OVERLAYS[0]
                replacement = self.bundle_runtime / ".replacement"
                replacement.write_bytes(overlay.read_bytes())
                if os.name == "posix":
                    replacement.chmod(stat.S_IMODE(overlay.stat().st_mode))
                replacement.replace(overlay)
            return result

        with mock.patch.object(bundle_module, "_runtime_snapshot", side_effect=snapshot_then_replace):
            with self.assertRaisesRegex(ValueError, "unsafe_or_changed_input"):
                bundle_module.verify(self.bundle, self.runtime, self.manifest_sha)
        self.assertEqual(calls, 4)


if __name__ == "__main__":
    unittest.main()
