"""Stage the pinned Windows resources; Python is a build tool, never an app prerequisite."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import uuid

ROOT = Path(__file__).resolve().parents[1]
OVERLAYS = ('desktop_controller.py', 'desktop_helper.py', 'cache_control_shim.py',
            'optimization_control.py', 'optimization-sources.json')


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def no_links(path):
    info = path.lstat()
    if stat.S_ISLNK(info.st_mode) or getattr(info, 'st_file_attributes', 0) & 0x400:
        raise ValueError('reparse_point_rejected')
    return info


def inventory(root):
    no_links(root)
    entries = {}
    names = set()
    total = 0
    pending = [root]
    while pending:
        directory = pending.pop()
        for path in directory.iterdir():
            info = no_links(path)
            relative = path.relative_to(root).as_posix()
            if relative.casefold() in names:
                raise ValueError('case_collision')
            names.add(relative.casefold())
            if stat.S_ISDIR(info.st_mode):
                pending.append(path)
            elif stat.S_ISREG(info.st_mode):
                if path.name.lower() in ('auth.json', 'credentials.json', '.env'):
                    raise ValueError('private_input_rejected')
                entries[relative] = sha(path)
                total += info.st_size
            else:
                raise ValueError('non_regular_input')
    digest = hashlib.sha256()
    for name, fingerprint in sorted(entries.items()):
        digest.update(name.encode('utf-8') + b'\0' + fingerprint.encode('ascii') + b'\n')
    return entries, {'tree_sha256': digest.hexdigest(), 'files': len(entries), 'bytes': total}


def inside(path, boundary):
    boundary = boundary.resolve(strict=True)
    absolute = Path(os.path.abspath(path))
    if absolute == boundary or not absolute.is_relative_to(boundary):
        raise ValueError('output_outside_workspace')
    current = boundary
    for part in absolute.relative_to(boundary).parts:
        current /= part
        if current.exists() or current.is_symlink():
            no_links(current)
    return absolute


def windows_manifest(runtime):
    for path in (runtime, runtime / 'manifest.json', runtime / 'manifest.windows.json'):
        no_links(path)
    development = json.loads((runtime / 'manifest.json').read_text(encoding='utf-8'))
    packaged = json.loads((runtime / 'manifest.windows.json').read_text(encoding='utf-8'))
    expected = {**development, 'engine': {**development['engine'], 'payload_root': 'payload'}}
    if packaged != expected:
        raise ValueError('windows_manifest_drift')
    return development, packaged


def validate_path_budget(project, resources):
    hook = project / 'src-tauri/windows-hooks.nsh'
    no_links(hook)
    limits = re.findall(r'^!define HEADROOM_MAX_INSTALL_DIR_CHARS (\d+)$', hook.read_text(encoding='utf-8'), re.M)
    if len(limits) != 1 or not 1 <= int(limits[0]) <= 200:
        raise ValueError('installer_path_budget_invalid')
    limit = int(limits[0])
    for name in resources:
        file_units = len(name.encode('utf-16-le')) // 2
        directory_units = len(str(Path(name).parent).encode('utf-16-le')) // 2
        if limit + 1 + file_units >= 260 or limit + 1 + directory_units >= 248:
            raise ValueError('installer_path_budget_exceeded')


def stage(project, output):
    project = project.resolve(strict=True)
    output = inside(output, project / 'work')
    runtime = project / 'runtime'
    development, packaged = windows_manifest(runtime)
    source = inside(runtime / development['engine']['payload_root'], project)
    paths, measured = inventory(source)
    pins = json.loads((runtime / 'windows-payload.json').read_text(encoding='utf-8'))
    if pins['schema_version'] != 1 or measured != pins['inventory']:
        raise ValueError('payload_inventory_mismatch')
    for key, expected in [('python', 'python_sha256'), ('runner', 'runner_sha256'),
                          ('runtime_manifest', 'runtime_manifest_sha256'), ('prefix_tracker', 'prefix_tracker_sha256')]:
        if paths[packaged['engine'][key]] != packaged['engine'][expected]:
            raise ValueError('payload_pin_mismatch')
    source_files = [runtime / name for name in OVERLAYS] + [runtime / 'manifest.windows.json',
                    runtime / 'windows-payload.json', project / 'NOTICE.md', project / 'src-tauri/windows-hooks.nsh']
    for path in source_files:
        no_links(path)
    before = {str(p.relative_to(project)): sha(p) for p in source_files}
    expected_resources = {'runtime/payload/' + name: value for name, value in paths.items()}
    expected_resources.update({'runtime/' + name: sha(runtime / name) for name in OVERLAYS})
    expected_resources.update({'runtime/manifest.json': sha(runtime / 'manifest.windows.json'),
                               'NOTICE.md': sha(project / 'NOTICE.md')})
    validate_path_budget(project, expected_resources)
    if output.exists():
        receipt = json.loads((output / 'package-inputs.json').read_text(encoding='utf-8'))
        actual = inventory(output)[0]
        expected_files = expected_resources | {'package-inputs.json': sha(output / 'package-inputs.json')}
        if (receipt['inputs'] != before or receipt['payload'] != measured
                or receipt['resources'] != expected_resources or actual != expected_files):
            raise ValueError('existing_stage_conflict')
        return {'reused': True, 'payload': measured, 'resources': len(actual)}
    output.parent.mkdir(parents=True, exist_ok=True)
    inside(output, project / 'work')
    temporary = output.with_name(output.name + '.partial-' + uuid.uuid4().hex)
    temporary.mkdir()
    bundle_runtime = temporary / 'runtime'
    payload = bundle_runtime / 'payload'
    payload.mkdir(parents=True)
    for name in paths:
        target = payload / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source / name, target)
    for name in OVERLAYS:
        shutil.copyfile(runtime / name, bundle_runtime / name)
    shutil.copyfile(runtime / 'manifest.windows.json', bundle_runtime / 'manifest.json')
    shutil.copyfile(project / 'NOTICE.md', temporary / 'NOTICE.md')
    if inventory(payload)[0] != paths or inventory(source)[1] != measured:
        raise ValueError('payload_changed_while_staging')
    if {str(p.relative_to(project)): sha(p) for p in source_files} != before:
        raise ValueError('inputs_changed_while_staging')
    if inventory(temporary)[0] != expected_resources:
        raise ValueError('staged_resources_mismatch')
    receipt = {'schema_version': 1, 'inputs': before, 'payload': measured,
               'resources': expected_resources}
    (temporary / 'package-inputs.json').write_text(json.dumps(receipt, indent=2) + '\n', encoding='utf-8')
    if output.exists():
        raise ValueError('concurrent_stage_conflict')
    os.rename(temporary, output)
    return {'reused': False, 'payload': measured, 'resources': len(receipt['resources']) + 1}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, default=ROOT / 'work/build/windows/resources')
    args = parser.parse_args()
    try:
        result = stage(ROOT, args.output)
    except (OSError, ValueError, KeyError):
        print(json.dumps({'passed': False, 'code': 'windows_package_inputs_invalid'}))
        return 1
    print(json.dumps({'passed': True, **result}))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
