from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
import base64
import json
import struct
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = spec_from_file_location('update_package', ROOT / 'scripts/prepare-windows-update.py')
package = module_from_spec(spec)
spec.loader.exec_module(package)


class UpdatePackageTests(unittest.TestCase):
    def setUp(self):
        folder = ROOT / 'work/proofs/update/release-package-tests'
        folder.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=folder)
        self.root = Path(self.temporary.name)
        (self.root / 'src-tauri').mkdir()
        (self.root / 'runtime').mkdir()
        for name in ('src-tauri/windows-update.nsi', 'work/caches/cargo-target/.tauri/NSIS/makensis.exe',
                     'node_modules/@tauri-apps/cli/tauri.js'):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b'local tool fixture')
        self.config = self.root / 'src-tauri/tauri.conf.json'
        self.config.write_text(json.dumps({'identifier': 'dev.headroom.desktop', 'version': '0.1.1'}))
        (self.root / 'runtime/manifest.windows.json').write_text(json.dumps({'engine': {
            'version': '0.41.0', 'runtime_manifest_sha256': 'a' * 64,
        }}))
        self.public = self.root / 'release.pub'
        self.public.write_bytes(base64.b64encode(b'untrusted comment: fixture public key\nRWR-fixture-key\n') + b'\n')
        self.private = self.root / 'release.key'
        self.private.write_text('private fixture value must never enter the output')
        self.notes = self.root / 'notes.md'
        self.notes.write_text('Yerel test notu.\n', encoding='utf-8')
        self.application = self.root / 'headroom.exe'
        data = bytearray(256)
        data[:2] = b'MZ'
        struct.pack_into('<I', data, 0x3C, 64)
        data[64:68] = b'PE\0\0'
        struct.pack_into('<H', data, 68, 0x8664)
        struct.pack_into('<H', data, 88, 0x20B)
        self.application.write_bytes(data + self.public.read_bytes().strip())
        self.output = self.root / 'work/published'
        self.calls = []

    def tearDown(self):
        assert self.root.resolve().is_relative_to(ROOT / 'work/proofs/update/release-package-tests')
        self.temporary.cleanup()

    def runner(self, command, environment, project):
        self.calls.append((command, environment))
        if Path(command[0]).name.lower() == 'makensis.exe':
            target = next(arg.split('=', 1)[1] for arg in command if arg.startswith('/DOUTPUT='))
            Path(target).write_bytes(b'NSIS package fixture')
        elif 'signer' in command:
            Path(str(command[-1]) + '.sig').write_bytes(b'public signature fixture')
        elif Path(command[0]).name.lower() in ('cargo', 'cargo.exe'):
            directory = Path(environment['HEADROOM_PREPARED_UPDATE_DIRECTORY'])
            application = Path(environment['HEADROOM_PREPARED_UPDATE_APPLICATION'])
            files = [*directory.iterdir(), application]
            (directory.parent / 'verification.json').write_text(json.dumps({
                path.name: package.staging.sha(path) for path in files}))
        elif command[-1] == '/HEADROOM-EXTRACT':
            (Path(command[0]).parent / 'application.exe').write_bytes(self.application.read_bytes())

    def prepare(self, **kwargs):
        values = dict(project=self.root, application=self.application, output=self.output,
                      config=self.config, public_key=self.public, private_key=self.private,
                      notes_file=self.notes, channel='stable', from_version='0.1.0')
        values.update(kwargs)
        with patch.object(package, 'execute', side_effect=self.runner):
            return package.prepare(**values)

    def test_prepares_only_the_three_channel_files_and_a_contentless_receipt(self):
        with patch.dict(package.os.environ, {'TAURI_SIGNING_PRIVATE_KEY_PASSWORD': 'fixture password'}):
            result = self.prepare()
        channel = self.output / 'stable'
        self.assertEqual(sorted(path.name for path in channel.iterdir()), [
            'Headroom_0.1.1_x64-update.exe', 'manifest.json', 'manifest.json.sig'])
        self.assertEqual(sorted(path.name for path in self.output.iterdir()), ['release-receipt.json', 'stable'])
        manifest = json.loads((channel / 'manifest.json').read_bytes())
        self.assertEqual(manifest['application']['sha256'], package.staging.sha(self.application))
        self.assertEqual(manifest['notes'], 'Yerel test notu.\n')
        self.assertEqual(result['version'], '0.1.1')
        self.assertNotIn(self.private.read_text(), json.dumps(json.loads((self.output / 'release-receipt.json').read_bytes())))
        for command, environment in self.calls:
            self.assertNotIn('fixture password', command)
            if 'signer' not in command:
                self.assertNotIn('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', environment)

    def test_rejects_wrong_channel_and_an_application_without_the_trusted_key(self):
        with self.assertRaises(ValueError):
            self.prepare(channel='preview')
        self.application.write_bytes(self.application.read_bytes().replace(self.public.read_bytes().strip(), b'x' * len(self.public.read_bytes().strip())))
        with self.assertRaisesRegex(ValueError, 'application_trust_key_mismatch'):
            self.prepare()
        self.assertFalse(self.output.exists())
        self.assertEqual(self.calls, [])

    def test_refuses_existing_output_and_preserves_original_inputs_on_tool_failure(self):
        self.output.mkdir()
        (self.output / 'user-file').write_bytes(b'preserve')
        with self.assertRaises(ValueError):
            self.prepare()
        self.assertEqual((self.output / 'user-file').read_bytes(), b'preserve')
        other = self.root / 'work/failed-output'
        original = self.application.read_bytes()
        with patch.object(package, 'execute', side_effect=ValueError('tool_failed')):
            with self.assertRaisesRegex(ValueError, 'tool_failed'):
                package.prepare(self.root, self.application, other, self.config, self.public,
                                self.private, self.notes, 'stable', '0.1.0')
        self.assertFalse(other.exists())
        self.assertEqual(self.application.read_bytes(), original)
        self.assertFalse(list((self.root / 'work').glob('*.partial-*')))

    def test_does_not_publish_a_package_that_the_rust_verifier_rejects(self):
        def reject(command, environment, project):
            self.runner(command, environment, project)
            if Path(command[0]).name.lower() in ('cargo', 'cargo.exe'):
                raise ValueError('update_verification_failed')
        with patch.object(package, 'execute', side_effect=reject):
            with self.assertRaisesRegex(ValueError, 'update_verification_failed'):
                package.prepare(self.root, self.application, self.output, self.config, self.public,
                                self.private, self.notes, 'stable', '0.1.0')
        self.assertFalse(self.output.exists())

    def test_rejects_changed_inputs_and_mismatched_extracted_bytes(self):
        def change_notes(command, environment, project):
            self.runner(command, environment, project)
            if Path(command[0]).name.lower() in ('cargo', 'cargo.exe'):
                self.notes.write_text('a concurrent change', encoding='utf-8')
        with patch.object(package, 'execute', side_effect=change_notes):
            with self.assertRaisesRegex(ValueError, 'release_input_changed'):
                package.prepare(self.root, self.application, self.output, self.config, self.public,
                                self.private, self.notes, 'stable', '0.1.0')
        self.assertFalse(self.output.exists())

        def wrong_extraction(command, environment, project):
            self.runner(command, environment, project)
            if command[-1] == '/HEADROOM-EXTRACT':
                (Path(command[0]).parent / 'application.exe').write_bytes(b'different executable')
        with patch.object(package, 'execute', side_effect=wrong_extraction):
            with self.assertRaisesRegex(ValueError, 'extracted_application_mismatch'):
                package.prepare(self.root, self.application, self.output, self.config, self.public,
                                self.private, self.notes, 'stable', '0.1.0')
        self.assertFalse(self.output.exists())

    def test_preserves_an_output_created_by_another_process_during_preparation(self):
        def concurrent_output(command, environment, project):
            self.runner(command, environment, project)
            if Path(command[0]).name.lower() in ('cargo', 'cargo.exe'):
                self.output.mkdir()
                (self.output / 'user-file').write_bytes(b'preserve')
        with patch.object(package, 'execute', side_effect=concurrent_output):
            with self.assertRaises(OSError):
                package.prepare(self.root, self.application, self.output, self.config, self.public,
                                self.private, self.notes, 'stable', '0.1.0')
        self.assertEqual((self.output / 'user-file').read_bytes(), b'preserve')
        self.assertFalse(list((self.root / 'work').glob('*.partial-*')))

    def test_refuses_unexpected_files_left_in_the_public_channel(self):
        def extra_file(command, environment, project):
            self.runner(command, environment, project)
            if Path(command[0]).name.lower() in ('cargo', 'cargo.exe'):
                directory = Path(environment['HEADROOM_PREPARED_UPDATE_DIRECTORY'])
                (directory / 'unexpected-file').write_bytes(b'do not publish')
        with patch.object(package, 'execute', side_effect=extra_file):
            with self.assertRaisesRegex(ValueError, 'unexpected_release_output'):
                package.prepare(self.root, self.application, self.output, self.config, self.public,
                                self.private, self.notes, 'stable', '0.1.0')
        self.assertFalse(self.output.exists())


if __name__ == '__main__':
    unittest.main()
