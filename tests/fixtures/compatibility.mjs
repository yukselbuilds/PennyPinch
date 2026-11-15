// Public catalogue + synthetic IPC metadata only; no real desktop/account state.
import catalog from '../../compatibility/clients.json' with {type:'json'};
export function compatibilitySnapshot(requestId){
 const records=catalog.records.map(r=>({id:r.id,client_kind:r.match.client_kind,product:r.product,version:r.version,surface:r.match.surface,tested_versions:r.match.versions,
  auth_method:r.match.auth_method,platform:r.match.platform,engine_version:r.match.engine_version,evidence_status:r.status,evidence_date:r.evidence_date,evidence:r.evidence,scope:r.surface,
  capability_counts:[...new Set(Object.values(r.capabilities).map(c=>c.status))].map(status=>({status,count:Object.values(r.capabilities).filter(c=>c.status===status).length}))}));
 return {schema_version:1,request_id:requestId,status:'snapshot',observed_at_utc:'2026-11-08T12:00:00Z',catalog_revision:'a'.repeat(64),catalog_date:catalog.last_updated,
  channel:'development',app_version:catalog.release.app_version,engine:{pinned_version:catalog.release.engine_version,manifest_sha256:'b'.repeat(64),active_version:catalog.release.engine_version,engine_status:'ready'},
  automatic_updates:false,updater_available:false,adapters:['codex','claude_code','opencode'].map(client_kind=>({client_kind,fingerprint:'c'.repeat(64)})),
  records,scan_generation:1,context_fingerprint:'d'.repeat(64),installations:[{installation_id:'7132bced-3ce0-4c1b-8f48-d93d7b96f7c0',client_kind:'codex',surface:'cli',version:'0.166.0',freshness:'current',config_supported:true,config_state:'not_owned',historical_evidence_ids:records.filter(r=>r.client_kind==='codex'&&r.surface==='cli'&&r.tested_versions.includes('0.166.0')&&r.engine_version===catalog.release.engine_version).map(r=>r.id)}]};
}
