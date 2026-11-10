"""Prepare, sign and locally verify an update directory; never upload or create keys."""
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
import argparse
import base64
import json
import os
import re
import shutil
import struct
import subprocess
import sys
import uuid

ROOT = Path(__file__).resolve().parents[1]
spec = spec_from_file_location('windows_staging', Path(__file__).with_name('package-windows.py'))
staging = module_from_spec(spec)
spec.loader.exec_module(staging)


def execute(command, environment, project):
    timeout = 8 if '--headroom-update-probe' in command else 600
    try:
        result = subprocess.run(command, cwd=project, env=environment, capture_output=True,
                                timeout=timeout, creationflags=subprocess.CREATE_NO_WINDOW)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ValueError('release_tool_unavailable_or_timed_out') from error
    if result.returncode:
        # Signer output may contain sensitive material; report only a fixed error code.
        raise ValueError('release_tool_failed')


def regular(path, project, limit):
    path = staging.inside(path, project)
    info = staging.no_links(path)
    if not path.is_file() or not 0 < info.st_size <= limit:
        raise ValueError('invalid_release_input')
    return path


def fingerprints(project, paths):
    return {path.relative_to(project).as_posix(): staging.sha(path) for path in sorted(set(paths))}


def prepare(project, application, output, config, public_key, private_key, notes_file,
            channel, from_version):
    project = project.resolve(strict=True)
    staging.no_links(project)
    output = staging.inside(output, project / 'work')
    if output.exists():
        raise ValueError('output_already_exists')
    application = regular(application, project, 128 * 1024 * 1024)
    config = regular(config, project, 64 * 1024)
    public_key = regular(public_key, project, 1024)
    private_key = regular(private_key, project, 16 * 1024)
    notes_file = regular(notes_file, project, 4096)
    if private_key in (application, config, public_key, notes_file):
        raise ValueError('private_key_input_collision')
    app = json.loads(config.read_bytes())
    version = app['version']
    identifier = app['identifier']
    semver = r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-[0-9A-Za-z.-]+)?'
    if (not isinstance(version, str) or len(version) > 64
            or re.fullmatch(semver, version) is None
            or not isinstance(from_version, str) or len(from_version) > 64
            or re.fullmatch(semver, from_version) is None
            or channel not in ('stable', 'preview')
            or ('-' in version) != (channel == 'preview')
            or not isinstance(identifier, str)
            or re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9.-]{0,199}', identifier) is None):
        raise ValueError('invalid_release_version_or_channel')
    notes = notes_file.read_text(encoding='utf-8')
    if any(ord(char) < 32 and char not in '\n\r\t' for char in notes):
        raise ValueError('invalid_release_notes')
    key = public_key.read_bytes().strip()
    try:
        decoded_key = base64.b64decode(key, validate=True)
    except ValueError as error:
        raise ValueError('invalid_release_public_key') from error
    if not decoded_key.startswith(b'untrusted comment:'):
        raise ValueError('invalid_release_public_key')
    executable = application.read_bytes()
    if key not in executable:
        raise ValueError('application_trust_key_mismatch')
    if len(executable) < 64 or executable[:2] != b'MZ':
        raise ValueError('application_not_windows_x64')
    header = struct.unpack_from('<I', executable, 0x3C)[0]
    if (header + 26 > len(executable) or executable[header:header + 4] != b'PE\0\0'
            or struct.unpack_from('<H', executable, header + 4)[0] != 0x8664
            or struct.unpack_from('<H', executable, header + 24)[0] != 0x20B):
        raise ValueError('application_not_windows_x64')
    runtime = regular(project / 'runtime/manifest.windows.json', project, 64 * 1024)
    engine = json.loads(runtime.read_bytes())['engine']
    template = regular(project / 'src-tauri/windows-update.nsi', project, 64 * 1024)
    nsis = regular(project / 'work/caches/cargo-target/.tauri/NSIS/makensis.exe', project, 32 * 1024 * 1024)
    tauri = regular((project / 'node_modules/@tauri-apps/cli/tauri.js').resolve(strict=True), project, 1024 * 1024)
    inputs = [application, config, public_key, notes_file, runtime, template, nsis, tauri,
              Path(__file__).resolve(), Path(staging.__file__).resolve()]
    # Unit fixtures use a smaller project; actual receipts include the verifier and pinned tool inputs.
    inputs = [path for path in inputs if path.is_relative_to(project)]
    native_cli = (tauri.parent.parent / 'cli-win32-x64-msvc').resolve()
    for folder in (project / 'src-tauri/src', nsis.parent / 'Include', nsis.parent / 'Plugins',
                   nsis.parent / 'Stubs', tauri.parent, native_cli):
        if folder.exists():
            staging.inside(folder, project)
            staging.no_links(folder)
            for path in folder.rglob('*'):
                staging.no_links(path)
                if path.is_file():
                    inputs.append(path)
    for name in ('src-tauri/Cargo.toml', 'src-tauri/Cargo.lock', 'src-tauri/build.rs',
                 'package.json', 'pnpm-lock.yaml'):
        if (project / name).exists():
            inputs.append(regular(project / name, project, 16 * 1024 * 1024))
    inputs = [path for path in inputs if path != private_key]
    tools = {}
    for name in ('node', 'cargo', 'rustc'):
        location = shutil.which(name)
        if not location:
            raise ValueError('release_tool_missing')
        tools[name] = Path(location).resolve(strict=True)
    tools['python'] = Path(sys.executable).resolve(strict=True)
    tools_before = {name: staging.sha(path) for name, path in tools.items()}
    before = fingerprints(project, inputs)
    environment = {name: value for name, value in os.environ.items()
                   if not name.upper().startswith(('TAURI_SIGNING_', 'HEADROOM_DESKTOP_UPDATE_', 'HEADROOM_PREPARED_UPDATE_'))}
    environment.update(CARGO_HOME=str(project / 'work/caches/cargo-home'),
                       CARGO_TARGET_DIR=str(project / 'work/caches/cargo-target'))
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(output.name + '.partial-' + uuid.uuid4().hex)
    temporary.mkdir()
    try:
        public = temporary / channel
        public.mkdir()
        copied_application = temporary / 'input-application.exe'
        copied_application.write_bytes(executable)
        execute([str(copied_application), '--headroom-update-probe', identifier, version, '1'], environment, project)
        installer = public / f'Headroom_{version}_x64-update.exe'
        execute([str(nsis), '/V2', '/NOCD', f'/DAPPLICATION={copied_application}',
                 f'/DOUTPUT={installer}', str(template)], environment, project)
        regular(installer, project, 1024 * 1024 * 1024)
        manifest = public / 'manifest.json'
        manifest.write_bytes((json.dumps({
            'schema_version': 2, 'app_identifier': identifier, 'target': 'windows-x86_64',
            'channel': channel, 'version': version, 'notes': notes,
            'engine': {name: engine[name] for name in ('version', 'runtime_manifest_sha256')},
            'installer': {'size_bytes': installer.stat().st_size, 'sha256': staging.sha(installer)},
            'application': {'state_format': 1, 'size_bytes': len(executable), 'sha256': staging.sha(copied_application)},
        }, ensure_ascii=False, indent=2) + '\n').encode('utf-8'))
        signer_environment = {**environment, 'TAURI_SIGNING_PRIVATE_KEY_PASSWORD':
                              os.environ.get('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', '')}
        execute([str(tools['node']), str(tauri), 'signer', 'sign', '--private-key-path', str(private_key),
                 '--app-version', version, str(manifest)], signer_environment, project)
        signature = regular(public / 'manifest.json.sig', project, 4096)
        expected = {path.name: staging.sha(path) for path in (manifest, signature, installer, copied_application)}
        verification = temporary / 'verification.json'
        verifier_environment = {**environment,
            'HEADROOM_PREPARED_UPDATE_DIRECTORY': str(public),
            'HEADROOM_PREPARED_UPDATE_APPLICATION': str(copied_application),
            'HEADROOM_PREPARED_UPDATE_PUBLIC_KEY': key.decode('ascii'),
            'HEADROOM_PREPARED_UPDATE_IDENTIFIER': identifier,
            'HEADROOM_PREPARED_UPDATE_FROM_VERSION': from_version,
            'HEADROOM_PREPARED_UPDATE_CHANNEL': channel,
        }
        execute([str(tools['cargo']), 'test', '--offline', '--locked', '--manifest-path', 'src-tauri/Cargo.toml',
                 'desktop::update::tests::verifies_prepared_release_directory', '--', '--exact',
                 '--ignored', '--test-threads=1'], verifier_environment, project)
        if (not verification.is_file()
                or json.loads(verification.read_bytes()) != expected):
            raise ValueError('update_verification_failed')
        execute([str(installer), '/HEADROOM-EXTRACT'], environment, project)
        extracted = regular(public / 'application.exe', project, 128 * 1024 * 1024)
        if staging.sha(extracted) != expected[copied_application.name]:
            raise ValueError('extracted_application_mismatch')
        execute([str(extracted), '--headroom-update-probe', identifier, version, '1'], environment, project)
        extracted.unlink()
        copied_application.unlink()
        verification.unlink()
        public_files = {name for name in expected if name != 'input-application.exe'}
        if ({path.name for path in public.iterdir()} != public_files
                or {path.name for path in temporary.iterdir()} != {channel}):
            raise ValueError('unexpected_release_output')
        after = fingerprints(project, inputs)
        tools_after = {name: staging.sha(path) for name, path in tools.items()}
        if before != after or tools_before != tools_after or any(staging.sha(public / name) != value for name, value in expected.items()
                                  if name != 'input-application.exe'):
            raise ValueError('release_input_changed')
        receipt = {'schema_version': 1, 'version': version, 'channel': channel,
                   'from_version': from_version, 'app_identifier': identifier,
                   'files': {name: value for name, value in expected.items() if name != 'input-application.exe'},
                   'inputs_before': before, 'inputs_after': after,
                   'tools_before': tools_before, 'tools_after': tools_after,
                   'verification': 'signature_manifest_package_extraction_and_application_probe',
                   'published': False}
        (temporary / 'release-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')
        # Windows rename refuses an output directory created by another process.
        temporary.rename(output)
        return receipt
    finally:
        if temporary.exists():
            staging.inside(temporary, project)
            shutil.rmtree(temporary)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('application', 'output', 'public-key', 'private-key', 'notes'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--config', type=Path, default=ROOT / 'src-tauri/tauri.conf.json')
    parser.add_argument('--channel', choices=('stable', 'preview'), required=True)
    parser.add_argument('--from-version', required=True)
    args = parser.parse_args()
    try:
        receipt = prepare(ROOT, args.application, args.output, args.config, args.public_key,
                          args.private_key, args.notes, args.channel, args.from_version)
    except (ValueError, OSError, KeyError, TypeError) as error:
        code = str(error) if isinstance(error, ValueError) and re.fullmatch(r'[a-z_]+', str(error)) else 'release_preparation_failed'
        print(json.dumps({'status': 'failed', 'reason': code}))
        return 1
    print(json.dumps({'status': 'prepared_locally', 'version': receipt['version'], 'channel': receipt['channel']}))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
