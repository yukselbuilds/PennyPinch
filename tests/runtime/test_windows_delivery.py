from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
import json
import os
import tempfile
import unittest
import zipfile


ROOT = Path(__file__).resolve().parents[2]
spec = spec_from_file_location('windows_delivery', ROOT / 'scripts/package-windows-delivery.py')
delivery = module_from_spec(spec)
spec.loader.exec_module(delivery)


@unittest.skipUnless(os.name == 'nt', 'Windows delivery output')
class WindowsDeliveryTests(unittest.TestCase):
    def setUp(self):
        parent = delivery.staging.inside(ROOT / 'work/proofs/windows-delivery/unit', ROOT)
        parent.mkdir(parents=True, exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(dir=parent)
        self.project = Path(self.temp.name).resolve()
        self.assertTrue(self.project.is_relative_to(parent.resolve()))
        self.addCleanup(self.temp.cleanup)
        self.proofs = self.project / 'docs/proofs'
        self.proofs.mkdir(parents=True)
        hooks = self.project / 'src-tauri/windows-hooks.nsh'
        hooks.parent.mkdir()
        hooks.write_text('!define HEADROOM_MAX_INSTALL_DIR_CHARS 100\n', encoding='utf-8')
        installer = self.project / 'work/caches/cargo-target/release/bundle/nsis/Headroom_0.1.0_x64-setup.exe'
        installer.parent.mkdir(parents=True)
        installer.write_bytes(b'isolated installer fixture')
        self.installer_sha = delivery.fingerprint(installer)
        self.archive = self.project / 'work/notices.zip'
        self.output = self.project / 'work/build/windows/delivery/Headroom_0.1.0_Windows_x64_preview.zip'
        self.write_json('windows-installer.json', {
            'app_version': '0.1.0', 'local_installed_slice': 'pass',
            'artifacts': {'installer_sha256': self.installer_sha, 'installer_bytes': installer.stat().st_size},
        })

    def write_json(self, name, record):
        (self.proofs / name).write_text(json.dumps(record), encoding='utf-8')

    def write_notices(self, member='LICENSE.txt', installer_sha=None):
        content = b'isolated notice fixture'
        index = {'installer': {'sha256': installer_sha or self.installer_sha},
                 'files': {member: {'sha256': delivery.digest(content)}}}
        with zipfile.ZipFile(self.archive, 'x') as archive:
            archive.writestr(member, content)
            archive.writestr('NOTICE-INDEX.json', json.dumps(index))
        self.write_json('windows-installer-notices.json', {
            'installer_sha256': self.installer_sha, 'companion_archive': {'path': 'work/notices.zip',
                'sha256': delivery.fingerprint(self.archive), 'bytes': self.archive.stat().st_size, 'verified_entries': 2},
        })

    def test_rejects_changed_notice_archive_before_output(self):
        self.write_notices()
        with self.archive.open('ab') as file:
            file.write(b'tampered')
        with self.assertRaisesRegex(ValueError, 'notice_archive_mismatch'):
            delivery.package(self.project)
        self.assertFalse(self.output.parent.exists())

    def test_rejects_notices_from_another_installer(self):
        self.write_notices(installer_sha='a' * 64)
        with self.assertRaisesRegex(ValueError, 'notice_installer_mismatch'):
            delivery.package(self.project)
        self.assertFalse(self.output.parent.exists())

    def test_rejects_traversal_even_with_matching_archive_hash(self):
        self.write_notices(member='../escape.txt')
        with self.assertRaisesRegex(ValueError, 'unsafe_notice_path'):
            delivery.package(self.project)
        self.assertFalse(self.output.parent.exists())

    def test_does_not_replace_existing_delivery(self):
        self.write_notices()
        self.output.parent.mkdir(parents=True)
        self.output.write_bytes(b'previous delivery')
        with self.assertRaises(FileExistsError):
            delivery.package(self.project)
        self.assertEqual(self.output.read_bytes(), b'previous delivery')
