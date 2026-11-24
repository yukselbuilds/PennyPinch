"""Small packaging fixtures exercise corruption, path and atomic publication boundaries."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock
import uuid

ROOT = Path(__file__).resolve().parents[2]
MAKENSIS = ROOT / 'work/caches/cargo-target/.tauri/NSIS/makensis.exe'
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

    @unittest.skipUnless(os.name == 'nt' and MAKENSIS.is_file(), 'requires the cached Windows NSIS compiler')
    def test_nsis_retires_known_runtime_metadata_only_from_the_registered_install(self):
        import winreg

        destination = self.root / 'install'
        packages = destination / 'runtime/payload/Lib/site-packages'
        old = packages / 'opentelemetry_api-1.45.1.dist-info'
        patched = packages / 'opentelemetry_api-1.45.2.dist-info'
        retired = destination / 'opentelemetry-1.45.1.retired'
        old_engine = packages / 'headroom_ai-0.44.0.dist-info'
        patched_engine = packages / 'headroom_ai-0.45.0.dist-info'
        retired_engine = destination / 'headroom-ai-0.44.0.retired'
        registry = 'Software\\PennyPinchInstallerTest_' + uuid.uuid4().hex
        hooks = ROOT / 'src-tauri/windows-hooks.nsh'
        installer = self.root / 'sample.exe'
        source = self.root / 'sample.nsi'
        source.write_text(f'''Unicode true
!include LogicLib.nsh
!define MANUPRODUCTKEY "{registry}"
!include "{hooks}"
Var PassiveMode
RequestExecutionLevel user
SilentInstall silent
InstallDir "{destination}"
OutFile "{installer}"
Section
  SetShellVarContext current
  !insertmacro HeadroomPrepareLegacyRuntimeRetirement
  FileOpen $4 "$INSTDIR\\write-marker.txt" w
  FileWrite $4 "ready"
  FileClose $4
  !insertmacro HeadroomRetireLegacyRuntime
  SetErrorLevel 0
SectionEnd
''', encoding='utf-8-sig')
        built = subprocess.run([str(MAKENSIS), '/V2', str(source)], capture_output=True, timeout=20)
        self.assertEqual(built.returncode, 0, built.stdout.decode(errors='replace')[-1200:])

        def run():
            return subprocess.run([str(installer)], capture_output=True, timeout=20).returncode

        patched.mkdir(parents=True)
        patched_engine.mkdir()
        marker = destination / 'write-marker.txt'
        (patched / 'METADATA').write_bytes(b'patched metadata')
        (patched_engine / 'METADATA').write_bytes(b'current engine metadata')
        self.assertEqual(run(), 0)
        marker.unlink()

        (old_engine / 'licenses').mkdir(parents=True)
        (old_engine / 'sboms').mkdir()
        (old_engine / 'METADATA').write_bytes(b'old engine metadata')
        (old_engine / 'licenses/NOTICE').write_bytes(b'old engine notice')
        (old_engine / 'sboms/headroom-py.cyclonedx.json').write_bytes(b'old engine SBOM')
        (old_engine / 'notes.txt').write_bytes(b'keep engine notes')
        self.assertEqual(run(), 2)
        self.assertFalse(marker.exists())
        self.assertEqual((old_engine / 'METADATA').read_bytes(), b'old engine metadata')

        old.mkdir()
        (old / 'METADATA').write_bytes(b'old metadata')
        (old / 'notes.txt').write_bytes(b'preserve this file')
        self.assertEqual(run(), 2)
        self.assertFalse(marker.exists())
        self.assertEqual((old / 'METADATA').read_bytes(), b'old metadata')
        with winreg.CreateKey(winreg.HKEY_CURRENT_USER, registry) as key:
            winreg.SetValueEx(key, '', 0, winreg.REG_SZ, str(destination))
        try:
            self.assertEqual(run(), 0)
            self.assertFalse(old.exists())
            self.assertFalse((retired / 'METADATA').exists())
            self.assertEqual((retired / 'notes.txt').read_bytes(), b'preserve this file')
            self.assertEqual((patched / 'METADATA').read_bytes(), b'patched metadata')
            self.assertFalse(old_engine.exists())
            self.assertEqual(sorted(path.name for path in retired_engine.iterdir()), ['notes.txt'])
            self.assertEqual((retired_engine / 'notes.txt').read_bytes(), b'keep engine notes')
            self.assertEqual((patched_engine / 'METADATA').read_bytes(), b'current engine metadata')
            self.assertEqual(run(), 0)
            marker.unlink()
            old_engine.mkdir()
            (old_engine / 'METADATA').write_bytes(b'old engine metadata')
            self.assertEqual(run(), 2)
            self.assertFalse(marker.exists())
            self.assertEqual((old_engine / 'METADATA').read_bytes(), b'old engine metadata')
            self.assertEqual((retired_engine / 'notes.txt').read_bytes(), b'keep engine notes')
            old.mkdir()
            (old / 'METADATA').write_bytes(b'old metadata')
            self.assertEqual(run(), 2)
            self.assertFalse(marker.exists())
            self.assertEqual((old / 'METADATA').read_bytes(), b'old metadata')
            self.assertEqual((retired / 'notes.txt').read_bytes(), b'preserve this file')
        finally:
            winreg.DeleteKey(winreg.HKEY_CURRENT_USER, registry)


if __name__ == '__main__':
    unittest.main()
