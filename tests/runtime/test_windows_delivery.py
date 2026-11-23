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
        (self.project / 'LICENSE').write_bytes(b'isolated project license')
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

    def test_explicit_receipt_builds_separate_verified_delivery(self):
        self.write_notices()
        candidate = json.loads((self.proofs / 'windows-installer.json').read_bytes())
        notices = json.loads((self.proofs / 'windows-installer-notices.json').read_bytes())
        receipt = self.proofs / 'verified-candidate.json'
        self.write_json(receipt.name, {**candidate, **notices})
        self.write_json('windows-installer-notices.json', {})
        self.output.parent.mkdir(parents=True)
        self.output.write_bytes(b'previous delivery')

        result = delivery.package(self.project, receipt.relative_to(self.project))

        output = self.project / result['output']
        self.assertEqual(output.parent, self.output.parent)
        self.assertEqual(output.name, f'Headroom_0.1.0_Windows_x64_preview_{self.installer_sha[:12]}.zip')
        self.assertEqual(result['before'], result['after'])
        self.assertEqual(self.output.read_bytes(), b'previous delivery')
        with zipfile.ZipFile(output) as archive:
            self.assertEqual(archive.read('Headroom_0.1.0_x64-setup.exe'), b'isolated installer fixture')
            self.assertEqual(archive.read('LICENSES/LICENSE.txt'), b'isolated notice fixture')

    def test_prerelease_delivery_preserves_the_version_and_project_license(self):
        self.write_notices()
        candidate = json.loads((self.proofs / 'windows-installer.json').read_bytes())
        notices = json.loads((self.proofs / 'windows-installer-notices.json').read_bytes())
        version = '0.1.1-preview.3'
        self.write_json('pennypinch.json', {**candidate, **notices, 'app_name': 'PennyPinch', 'app_version': version})
        name = f'PennyPinch_{version}_x64-setup.exe'
        installer = self.project / 'work/caches/cargo-target/release/bundle/nsis'
        (installer / name).write_bytes((installer / 'Headroom_0.1.0_x64-setup.exe').read_bytes())

        result = delivery.package(self.project, Path('docs/proofs/pennypinch.json'))

        output = self.project / result['output']
        self.assertEqual(output.name, f'PennyPinch_{version}_Windows_x64_preview_{self.installer_sha[:12]}.zip')
        with zipfile.ZipFile(output) as archive:
            self.assertEqual(archive.read(name), b'isolated installer fixture')
            self.assertEqual(archive.read('LICENSE'), b'isolated project license')
            self.assertIn('  LICENSE\n', archive.read('SHA256SUMS.txt').decode('utf-8'))
            self.assertIn(f'PennyPinch {version}', archive.read('README.txt').decode('utf-8-sig'))

    def extracted_preview(self, installed='not_tested', evidence_sha=None):
        self.write_notices()
        candidate = json.loads((self.proofs / 'windows-installer.json').read_bytes())
        notices = json.loads((self.proofs / 'windows-installer-notices.json').read_bytes())
        name = 'PennyPinch_0.1.0_x64-setup.exe'
        installer = self.project / 'work/verified-preview' / name
        installer.parent.mkdir()
        installer.write_bytes(b'isolated installer fixture')
        evidence = self.proofs / 'extraction.json'
        self.write_json(evidence.name, {
            'status': 'pass', 'authenticode': {'setup': 'NotSigned'},
            'nsis_extraction': {'resource_files': 1},
            'executable_probe': {'version': '0.1.0', 'correct_version': 'pass'},
            'artifacts': {name: {'path': installer.relative_to(self.project).as_posix(),
                                 'sha256': evidence_sha or self.installer_sha, 'bytes': installer.stat().st_size}},
        })
        receipt = self.proofs / 'preview.json'
        self.write_json(receipt.name, {**candidate, **notices, 'app_name': 'PennyPinch',
                                       'local_installed_slice': installed,
                                       'preview_evidence': {'path': evidence.relative_to(self.project).as_posix(),
                                                            'sha256': delivery.fingerprint(evidence)}})
        return receipt.relative_to(self.project), evidence, installer

    def test_extracted_preview_uses_the_archived_installer(self):
        receipt, evidence, installer = self.extracted_preview()

        result = delivery.package(self.project, receipt)

        self.assertIn(str(evidence), result['before'])
        self.assertEqual(result['before'], result['after'])
        with zipfile.ZipFile(self.project / result['output']) as archive:
            self.assertEqual(archive.read(installer.name), installer.read_bytes())

    def test_extracted_preview_rejects_a_changed_evidence_file(self):
        receipt, evidence, _ = self.extracted_preview()
        evidence.write_bytes(evidence.read_bytes() + b' ')

        with self.assertRaisesRegex(ValueError, 'preview_evidence_mismatch'):
            delivery.package(self.project, receipt)
        self.assertFalse(self.output.parent.exists())

    def test_extracted_preview_rejects_evidence_for_another_installer(self):
        receipt, _, _ = self.extracted_preview(evidence_sha='a' * 64)

        with self.assertRaisesRegex(ValueError, 'preview_evidence_mismatch'):
            delivery.package(self.project, receipt)
        self.assertFalse(self.output.parent.exists())

    def test_extracted_preview_does_not_override_failed_installation(self):
        receipt, _, _ = self.extracted_preview(installed='fail')

        with self.assertRaisesRegex(ValueError, 'unverified_installer_candidate'):
            delivery.package(self.project, receipt)
        self.assertFalse(self.output.parent.exists())
