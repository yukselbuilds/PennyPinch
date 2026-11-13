// Synthetic UI transport only: no filesystem, native processes, accounts or network.
import type { InstallationSummary } from '../../src/lib/integrations';
import type { LoginFlow } from '../../src/lib/accounts';
export const installations: InstallationSummary[] = ['codex', 'claude_code', 'opencode'].map((client_kind, n) => ({
  installation_id: `7132bced-3ce0-4c1b-8f48-d93d7b96f7c${n}`,
  client_kind: client_kind as InstallationSummary['client_kind'], surface: 'cli',
  version: ['0.163.0-alpha.11.2', '2.1.158', '1.19.3'][n], source: 'user_local_native',
  location_label: 'selected_file', status: 'detected', connection_status: 'not_verified',
}));
installations.push({ ...installations[0], installation_id: '7132bced-3ce0-4c1b-8f48-d93d7b96f7c3', surface: 'desktop', version: '26.1106.51428' });
let scenario = 'data';
let flow: LoginFlow | null = null;
let calls: string[] = [];
let readGate: Promise<void> | null = null;
let release: (() => void) | null = null;
export function reset(value: string) { scenario = value; flow = null; calls = []; releaseRead(); }
export function history() { return [...calls]; }
export function blockRead() { readGate = new Promise(resolve => { release = resolve; }); }
export function releaseRead() { release?.(); release = null; readGate = null; }
export function setFlowState(state: LoginFlow['state']) { if (!flow) throw Error('no_fixture_flow'); flow = { ...flow, state }; }
export function isTauri() { return scenario !== 'preview'; }
export async function invoke(command: string, args: any) {
  if (command !== 'manage_account') throw Error('fixture_command_rejected');
  const r = args.request; calls.push(r.operation);
  const reply = { schema_version: 1, request_id: r.request_id, status: 'snapshot', account: null as any, flow: null as LoginFlow | null, error: null as string | null };
  if (scenario === 'error') throw Error('private-transport-sentinel');
  if (r.operation === 'read') {
    if (readGate) await readGate;
    const i = installations.find(value => value.installation_id === r.installation_id)!;
    const unsupported = i.surface === 'desktop', unknown = unsupported || i.client_kind === 'claude_code';
    const absent = scenario === 'absent';
    reply.account = { installation_id: i.installation_id, client_kind: i.client_kind, surface: i.surface,
      state: unknown ? 'unknown' : absent ? 'absent' : 'present', method: unknown ? 'unknown' : absent ? 'none' : i.client_kind === 'codex' ? 'chatgpt' : 'provider_credentials',
      source: unknown ? 'unavailable' : i.client_kind === 'codex' ? 'official_cli' : 'local_store',
      reason: unsupported ? 'unsupported' : unknown ? 'read_not_passive' : 'none', login_supported: !unsupported, remote_verified: false,
      providers: !unknown && !absent && i.client_kind === 'opencode' ? [{ provider: 'openai', method: 'api_key' }] : [] };
  } else if (r.operation === 'start_login') {
    if (!r.confirm_profile_change) throw Error('fixture_consent_required');
    if (scenario === 'launch_failure') { reply.status = 'error'; reply.error = 'launch_failed'; }
    else {
      flow = { flow_id: crypto.randomUUID(), installation_id: r.installation_id,
        client_kind: installations.find(i => i.installation_id === r.installation_id)!.client_kind, state: 'pending' };
      reply.flow = { ...flow };
      if (scenario === 'lost_start_reply') throw Error('fixture_reply_lost_after_launch');
    }
  } else if (r.operation === 'active_login' || r.operation === 'poll_login') reply.flow = flow && { ...flow };
  else if (r.operation === 'cancel_login') {
    if (!flow || r.flow_id !== flow.flow_id) throw Error('fixture_wrong_flow');
    flow = { ...flow, state: 'cancelled' }; reply.flow = { ...flow };
  } else throw Error('fixture_operation_rejected');
  return reply;
}
