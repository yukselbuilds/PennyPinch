"""Small packaging fixtures exercise corruption, path and atomic publication boundaries."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('windows_package', ROOT / 'scripts/package-windows.py')
package = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package)


class WindowsPackageTests(unittest.TestCase):
    def setUp(self):
        parent = ROOT / 'work/proofs/windows-install/unit'
        parent.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix='Paket çalışma ', dir=parent)
        self.root = Path(self.temporary.name)
        (self.root / 'work/source').mkdir(parents=True)
        (self.root / 'runtime').mkdir()
        (self.root / 'src-tauri').mkdir()
        (self.root / 'src-tauri/windows-hooks.nsh').write_text('!define HEADROOM_MAX_INSTALL_DIR_CHARS 100\n')
        for name in ('python.exe', 'runner.py', 'prefix.py'):
            (self.root / 'work/source' / name).write_bytes(name.encode())
        self.lock = self.root / 'runtime/engine-deps/uv.lock'
        self.lock.parent.mkdir()
        self.lock.write_text('version = 1\n', encoding='utf-8')
        (self.root / 'work/source/runtime-manifest.json').write_text(json.dumps({
            'engine': {'lock_sha256': package.sha(self.lock)},
        }), encoding='utf-8')
        engine = {'payload_root': '../work/source'}
        for key, filename in [('python', 'python.exe'), ('runner', 'runner.py'),
                              ('runtime_manifest', 'runtime-manifest.json'), ('prefix_tracker', 'prefix.py')]:
            engine[key] = filename
            engine[key + '_sha256'] = package.sha(self.root / 'work/source' / filename)
        self.manifest = {'engine': engine}
        for name, value in [('manifest.json', self.manifest), ('manifest.windows.json',
                             {'engine': {**engine, 'payload_root': 'payload'}})]:
            (self.root / 'runtime' / name).write_text(json.dumps(value), encoding='utf-8')
        for name in package.OVERLAYS:
            (self.root / 'runtime' / name).write_text('fixture')
        (self.root / 'NOTICE.md').write_text('fixture notice')
        self.pin()
        self.output = self.root / 'work/kurulum boşluk/resources'

    def tearDown(self):
        # TemporaryDirectory owns this exact workspace subtree.
        assert Path(self.temporary.name).resolve().is_relative_to(ROOT / 'work/proofs/windows-install/unit')
        self.temporary.cleanup()

    def pin(self):
        (self.root / 'runtime/windows-payload.json').write_text(json.dumps({
            'schema_version': 1, 'engine': {'dependency_lock_sha256': package.sha(self.lock)},
            'inventory': package.inventory(self.root / 'work/source')[1]}))

    def test_atomic_unicode_stage_and_exact_reuse(self):
        self.assertFalse(package.stage(self.root, self.output)['reused'])
        self.assertTrue(package.stage(self.root, self.output)['reused'])
        self.assertEqual((self.output / 'runtime/payload/python.exe').read_bytes(), b'python.exe')
        (self.output / 'runtime/payload/python.exe').write_bytes(b'corrupt')
        receipt = json.loads((self.output / 'package-inputs.json').read_text())
        receipt['resources']['runtime/payload/python.exe'] = package.sha(self.output / 'runtime/payload/python.exe')
        (self.output / 'package-inputs.json').write_text(json.dumps(receipt))
        with self.assertRaises(ValueError):
            package.stage(self.root, self.output)

    def test_corrupt_payload_and_private_inputs_never_publish(self):
        (self.root / 'work/source/python.exe').write_bytes(b'corrupt')
        with self.assertRaises(ValueError):
            package.stage(self.root, self.output)
        self.assertFalse(self.output.exists())
        (self.root / 'work/source/auth.json').write_bytes(b'private canary')
        with self.assertRaisesRegex(ValueError, 'private_input'):
            package.inventory(self.root / 'work/source')

    def test_dependency_lock_drift_never_publishes_or_reuses_a_stage(self):
        package.stage(self.root, self.output)
        receipt = self.output / 'package-inputs.json'
        before = receipt.read_bytes()
        self.lock.write_text('version = 2\n', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'payload_dependency_lock_mismatch'):
            package.stage(self.root, self.output)
        self.assertEqual(receipt.read_bytes(), before)

        self.pin()
        new_output = self.output.with_name('new')
        with self.assertRaisesRegex(ValueError, 'payload_dependency_lock_mismatch'):
            package.stage(self.root, new_output)
        self.assertFalse(new_output.exists())

    def test_manifest_drift_and_output_escape_are_rejected(self):
        (self.root / 'runtime/manifest.windows.json').write_text('{}')
        with self.assertRaises(ValueError):
            package.stage(self.root, self.output)
        with self.assertRaises(ValueError):
            package.inside(self.root / 'work/../runtime/output', self.root / 'work')

    def test_copy_failure_has_no_published_package(self):
        with mock.patch.object(package.shutil, 'copyfile', side_effect=OSError('fixture')):
            with self.assertRaises(OSError):
                package.stage(self.root, self.output)
        self.assertFalse(self.output.exists())

    def test_reparse_attribute_is_rejected_before_reading(self):
        target = self.root / 'work/source/python.exe'
        with mock.patch.object(Path, 'lstat', return_value=type('Info', (), {
                'st_mode': 0o100644, 'st_file_attributes': 0x400})()):
            with self.assertRaisesRegex(ValueError, 'reparse'):
                package.no_links(target)

    def test_installer_budget_covers_files_directories_and_utf16(self):
        package.validate_path_budget(self.root, ['a' * 158])
        for name in ('a' * 159, 'a' * 147 + '/x.py', '\U0001f4a1' * 80):
            with self.subTest(name_units=len(name.encode('utf-16-le')) // 2):
                with self.assertRaisesRegex(ValueError, 'budget_exceeded'):
                    package.validate_path_budget(self.root, [name])


if __name__ == '__main__':
    unittest.main()
