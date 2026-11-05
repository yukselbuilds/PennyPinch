import test from 'node:test';
import assert from 'node:assert/strict';
import { equivalentCodexInstallation } from '../../src/lib/equivalentInstallations.ts';
const candidates = [
  { installation_id:'019a0000-0000-7000-8000-000000000001', client_kind:'codex',surface:'cli' },
  { installation_id:'019a0000-0000-7000-8000-000000000002', client_kind:'codex',surface:'desktop' },
];
const make = item => ({schema_version:1,request_id:'request',plan_id:'plan',expires_at_unix_seconds:2000,
  installation_id:item.installation_id,client_kind:'codex',scope:'user_config',scan_generation:7,
  config_fingerprint:'a'.repeat(64),config_presence:'present',config_source:'codex_toml',endpoint_id:'endpoint',
  endpoint_host:'127.0.0.1',endpoint_port:41731,endpoint_field:{field:'codex_openai_base_url',before:'missing',proposed_value:'http://127.0.0.1:41731/v1',after_headroom:true},
  status:'preview',model:'present',reasoning:'present',model_unchanged:true,provider_unchanged:true,
  reasoning_unchanged:true,auth_unchanged:true,agent_count_unchanged:true,runtime_effective:'unknown',auth:'unknown',
  hook_shell:'absent',restart_required:true,blockers:[]});
test('equal native plans give one temporary target across surfaces, ignoring only per-preview identity', () => {
  const plans=candidates.map(make); Object.assign(plans[1],{plan_id:'different',request_id:'different',expires_at_unix_seconds:2100});
  assert.equal(equivalentCodexInstallation(candidates,plans,7,1_000_000),candidates[0].installation_id);
  assert.equal(plans[0].installation_id,candidates[0].installation_id);
});
test('different scope, fingerprint, endpoint, effect, unsupported, stale or expired plans never group', () => {
  const differences=[{scope:'project_local_config'},{config_fingerprint:'b'.repeat(64)},{endpoint_port:41732},
    {endpoint_id:'other'},{auth:'secret_configured'},{model:'absent'},{config_source:'claude_json'},
    {endpoint_field:{...make(candidates[1]).endpoint_field,before:'other_configured_redacted'}},
    {status:'blocked',blockers:['unsupported_client_surface']},{scan_generation:8},{client_kind:'claude_code'},
    {installation_id:'missing'},{expires_at_unix_seconds:1000},{auth_unchanged:false}];
  for(const change of differences){const plans=candidates.map(make);Object.assign(plans[1],change);
    assert.equal(equivalentCodexInstallation(candidates,plans,7,1_000_000),null,JSON.stringify(change));}
});
test('partial results, duplicate targets and unbounded discovery never group', () => {
  assert.equal(equivalentCodexInstallation(candidates,[make(candidates[0])],7,1_000_000),null);
  const repeated=[candidates[0],candidates[0]];
  assert.equal(equivalentCodexInstallation(repeated,repeated.map(make),7,1_000_000),null);
  const many=Array.from({length:5},(_,i)=>({...candidates[0],installation_id:String(i)}));
  assert.equal(equivalentCodexInstallation(many,many.map(make),7,1_000_000),null);
});
