"""Build a verified delivery ZIP around the existing Windows preview installer."""
import argparse
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path, PurePosixPath
import hashlib
import json
import os
import re
import shutil
import stat
import sys
import uuid
import zipfile


ROOT = Path(__file__).resolve().parents[1]
STAGER = Path(__file__).with_name('package-windows.py')
spec = spec_from_file_location('windows_staging', STAGER)
staging = module_from_spec(spec)
spec.loader.exec_module(staging)


def fingerprint(path):
    staging.no_links(path)
    return staging.sha(path)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def check_notice_name(entry):
    name = entry.filename
    parts = PurePosixPath(name).parts
    devices = {'CON', 'PRN', 'AUX', 'NUL'} | {f'{stem}{number}' for stem in ('COM', 'LPT') for number in range(1, 10)}
    if (not name or name.startswith('/') or name != '/'.join(parts) or entry.is_dir()
            or stat.S_ISLNK(entry.external_attr >> 16)
            or any(part in ('', '.', '..') or part.endswith((' ', '.'))
                   or part.split('.', 1)[0].upper() in devices
                   or any(ord(char) < 32 or char in '<>:"\\|?*' for char in part) for part in parts)):
        raise ValueError('unsafe_notice_path')


def read_notices(path, expected_count, installer_sha):
    with zipfile.ZipFile(path) as archive:
        entries = archive.infolist()
        if len(entries) != expected_count:
            raise ValueError('notice_count_mismatch')
        for entry in entries:
            check_notice_name(entry)
        names = {entry.filename.casefold() for entry in entries}
        if len(names) != len(entries) or any(str(parent) in names for name in names for parent in PurePosixPath(name).parents):
            raise ValueError('notice_path_collision')
        if sum(entry.file_size for entry in entries) > 32 * 1024 * 1024:
            raise ValueError('notice_archive_too_large')
        index = json.loads(archive.read('NOTICE-INDEX.json'))
        if index['installer']['sha256'] != installer_sha:
            raise ValueError('notice_installer_mismatch')
        if set(archive.namelist()) != set(index['files']) | {'NOTICE-INDEX.json'}:
            raise ValueError('notice_index_mismatch')
        files = {name: archive.read(name) for name in archive.namelist()}
    if any(digest(files[name]) != row['sha256'] for name, row in index['files'].items()):
        raise ValueError('notice_entry_mismatch')
    return {f'LICENSES/{name}': data for name, data in files.items()}


def verify_delivery(path, expected):
    with zipfile.ZipFile(path) as archive:
        if sorted(archive.namelist()) != sorted(expected):
            raise ValueError('delivery_file_list_mismatch')
        for name, fingerprint_expected in expected.items():
            with archive.open(name) as file:
                if hashlib.file_digest(file, 'sha256').hexdigest() != fingerprint_expected:
                    raise ValueError('delivery_entry_mismatch')


def package(project, receipt=None):
    if os.name != 'nt':
        raise ValueError('windows_delivery_requires_windows')
    project = project.resolve(strict=True)
    work = project / 'work'
    candidate_path = staging.inside(project / (receipt or 'docs/proofs/windows-installer.json'), project)
    notices_path = candidate_path if receipt is not None else staging.inside(
        project / 'docs/proofs/windows-installer-notices.json', project)
    records = {path: path.read_bytes() for path in dict.fromkeys((candidate_path, notices_path))}
    candidate = json.loads(records[candidate_path])
    notices = candidate if receipt is not None else json.loads(records[notices_path])
    version = candidate['app_version']
    app_name = candidate.get('app_name', 'Headroom')
    installer_sha = candidate['artifacts']['installer_sha256']
    installer_size = candidate['artifacts']['installer_bytes']
    if (not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?', version)
            or app_name not in ('Headroom', 'PennyPinch')
            or not re.fullmatch(r'[0-9a-f]{64}', installer_sha)):
        raise ValueError('unverified_installer_candidate')
    installer_name = f'{app_name}_{version}_x64-setup.exe'
    if candidate['local_installed_slice'] == 'pass':
        installer = staging.inside(work / 'caches/cargo-target/release/bundle/nsis' / installer_name, work)
    elif receipt is not None and candidate['local_installed_slice'] == 'not_tested':
        proof = candidate['preview_evidence']
        proof_path = staging.inside(project / proof['path'], project)
        proof_bytes = proof_path.read_bytes()
        if digest(proof_bytes) != proof['sha256']:
            raise ValueError('preview_evidence_mismatch')
        evidence = json.loads(proof_bytes)
        artifact = evidence['artifacts'][installer_name]
        if (evidence['status'] != 'pass' or evidence['authenticode']['setup'] != 'NotSigned'
                or evidence['executable_probe']['version'] != version
                or evidence['executable_probe']['correct_version'] != 'pass'
                or evidence['nsis_extraction']['resource_files'] <= 0
                or artifact['sha256'] != installer_sha or artifact['bytes'] != installer_size):
            raise ValueError('preview_evidence_mismatch')
        installer = staging.inside(project / artifact['path'], work)
        if installer.name != installer_name:
            raise ValueError('preview_evidence_mismatch')
        records[proof_path] = proof_bytes
    else:
        raise ValueError('unverified_installer_candidate')
    companion = notices['companion_archive']
    archive = staging.inside(project / companion['path'], work)
    license_path = staging.inside(project / 'LICENSE', project)
    records[license_path] = license_path.read_bytes()
    suffix = f'_{installer_sha[:12]}' if receipt is not None else ''
    output = staging.inside(work / f'build/windows/delivery/{app_name}_{version}_Windows_x64_preview{suffix}.zip', work)
    if output.exists():
        raise FileExistsError('delivery_exists')

    before = {str(path): digest(data) for path, data in records.items()}
    for path in (installer, archive, project / 'src-tauri/windows-hooks.nsh', Path(__file__), STAGER):
        before[str(path)] = fingerprint(path)
    if (before[str(installer)] != installer_sha or installer.stat().st_size != installer_size
            or notices['installer_sha256'] != installer_sha):
        raise ValueError('installer_candidate_mismatch')
    if before[str(archive)] != companion['sha256'] or archive.stat().st_size != companion['bytes']:
        raise ValueError('notice_archive_mismatch')

    files = read_notices(archive, companion['verified_entries'], installer_sha)
    files['LICENSE'] = records[license_path]
    title = 'Headroom Desktop' if app_name == 'Headroom' else app_name
    files['README.txt'] = (
        f'{title} {version} · Windows x64\n\n'
        'İmzasız geliştirme önizlemesi\n'
        'ZIP dosyasının tamamını bir klasöre çıkarın.\n'
        f'{installer_name} dosyasını açın.\n'
        'WebView2 eksikse kurucu internet bağlantısı gerektirir.\n'
        'Üçüncü taraf bildirimleri LICENSES klasöründedir.\n\n'
        'Unsigned development preview\n'
        'Extract the entire ZIP into a folder.\n'
        f'Open {installer_name}.\n'
        'The installer requires an internet connection if WebView2 is missing.\n'
        'Third-party notices are in the LICENSES folder.\n'
    ).encode('utf-8-sig')
    expected = {installer_name: installer_sha} | {name: digest(data) for name, data in files.items()}
    files['SHA256SUMS.txt'] = ''.join(f'{fingerprint_value}  {name}\n'
                                     for name, fingerprint_value in sorted(expected.items())).encode('utf-8')
    expected['SHA256SUMS.txt'] = digest(files['SHA256SUMS.txt'])
    staging.validate_path_budget(project, expected)

    output.parent.mkdir(parents=True, exist_ok=True)
    staging.inside(output, work)
    temporary = output.with_name(output.name + '.partial-' + uuid.uuid4().hex)
    with zipfile.ZipFile(temporary, 'x') as delivery:
        with installer.open('rb') as source, delivery.open(zipfile.ZipInfo(installer_name), 'w') as target:
            shutil.copyfileobj(source, target, length=1024 * 1024)
        for name, data in sorted(files.items()):
            entry = zipfile.ZipInfo(name)
            entry.compress_type = zipfile.ZIP_DEFLATED
            delivery.writestr(entry, data)
    verify_delivery(temporary, expected)
    after = {name: fingerprint(Path(name)) for name in before}
    if before != after:
        raise ValueError('delivery_inputs_changed')
    checksum, size = fingerprint(temporary), temporary.stat().st_size
    os.rename(temporary, output)
    return {'passed': True, 'output': output.relative_to(project).as_posix(), 'sha256': checksum,
            'bytes': size, 'entries': len(expected), 'installer_sha256': installer_sha,
            'before': before, 'after': after, 'python': sys.version, 'windows_build': list(sys.getwindowsversion()[:3])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--receipt', type=Path, help='project-relative verified installer and notices receipt')
    args = parser.parse_args()
    try:
        result = package(ROOT, args.receipt)
    except (OSError, ValueError, KeyError, TypeError, zipfile.BadZipFile) as error:
        print(json.dumps({'passed': False, 'code': error.args[0] if isinstance(error, ValueError) else type(error).__name__}))
        return 1
    print(json.dumps(result))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
