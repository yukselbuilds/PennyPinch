"""Synchronize app-owned overlay pins; never updates the packaged engine or vendor source map."""
import argparse,hashlib,json,os,re,uuid
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
assert Path.cwd().resolve()==ROOT and (ROOT/'HEADROOM_DESKTOP_SPEC.md').is_file()
parser=argparse.ArgumentParser();parser.add_argument('--write',action='store_true');args=parser.parse_args()
manifest_path=ROOT/'runtime/manifest.json';controller_path=ROOT/'runtime/desktop_controller.py';manager_path=ROOT/'src-tauri/src/runtime/manager.rs'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source_path=ROOT/'runtime/optimization-sources.json';control_path=ROOT/'runtime/optimization_control.py'
manifest=json.loads(manifest_path.read_text(encoding='utf-8'));assert manifest['engine']['version']=='0.41.0'
package=(manifest_path.parent/manifest['engine']['payload_root']/'Lib/site-packages/headroom').resolve();assert package.is_relative_to(ROOT)
expected=json.loads(source_path.read_text(encoding='utf-8'))
assert expected and all((package/p).is_file() and sha(package/p)==h for p,h in expected.items())
control_hash=sha(control_path);source_hash=sha(source_path)
target_manifest={**manifest,'optimization_control':{'file':'optimization_control.py','sha256':control_hash,'sources':'optimization-sources.json','sources_sha256':source_hash}}
def replace(text,pattern,replacement):
 out,count=re.subn(pattern,lambda m:replacement(m),text);assert count==1;return out
controller=controller_path.read_text(encoding='utf-8')
controller=replace(controller,r'("optimization_sha256":\s*")[^"]+("\s*,)',lambda m:m[1]+control_hash+m[2])
controller=replace(controller,r'("optimization_sources_sha256":\s*")[^"]+("\s*,)',lambda m:m[1]+source_hash+m[2])
updates={manifest_path:(json.dumps(target_manifest,indent=2)+'\n').encode(),controller_path:controller.encode()}
pins={'EXPECTED_MANIFEST_SHA256':hashlib.sha256(updates[manifest_path]).hexdigest(),
 'EXPECTED_CONTROLLER_SHA256':hashlib.sha256(updates[controller_path]).hexdigest(),
 'EXPECTED_HELPER_SHA256':sha(ROOT/'runtime/desktop_helper.py'),
 'EXPECTED_OPTIMIZATION_SHA256':control_hash,'EXPECTED_OPTIMIZATION_SOURCES_SHA256':source_hash}
manager=manager_path.read_text(encoding='utf-8')
for name,value in pins.items():manager=replace(manager,rf'(const {name}:\s*&str\s*=\s*")[^"]+(";)',lambda m:m[1]+value+m[2])
original_manager=manager_path.read_bytes();updates[manager_path]=manager.replace('\r\n','\n').replace('\n','\r\n').encode() if b'\r\n' in original_manager else manager.encode()
changed={p:data for p,data in updates.items() if p.read_bytes()!=data}
if args.write:
 backup=ROOT/'work/proofs/pause/pin-backups'/str(uuid.uuid4());backup.mkdir(parents=True)
 before={str(p.relative_to(ROOT)):sha(p) for p in updates}
 for p,data in changed.items():
  assert p.resolve().is_relative_to(ROOT) and p.is_file() and p.stat().st_size>0
  previous=p.read_bytes();(backup/p.name).write_bytes(previous)
  temp=p.with_name(p.name+'.'+uuid.uuid4().hex+'.tmp');temp.write_bytes(data);assert temp.read_bytes()==data;os.replace(temp,p)
 after={str(p.relative_to(ROOT)):sha(p) for p in updates}
 (backup/'result.json').write_text(json.dumps({'before':before,'after':after,'changed':len(changed),'vendor_sources_unchanged':True},indent=2)+'\n',encoding='utf-8')
 print(json.dumps({'updated_files':len(changed),'vendor_sources_unchanged':True}))
else:
 print(json.dumps({'pins_current':not changed,'files_needing_update':len(changed),'vendor_sources_unchanged':True}))
 raise SystemExit(bool(changed))
