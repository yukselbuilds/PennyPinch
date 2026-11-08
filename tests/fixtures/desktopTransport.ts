// Isolated UI evidence only. No native API, filesystem, auth, or provider access.
import {runtimeSnapshot} from './runtime.mjs';
import {activityList,activityDetail} from './activity.mjs';
import {compatibilitySnapshot} from './compatibility.mjs';
export type Scenario = 'data'|'empty'|'error';
let language='tr',theme='dark',scenario:Scenario='data',revision=1,exit='idle',generation=1;
const callbacks=new Set<()=>void>();
let compatibilityRevision=0;
let compatibilityGate:Promise<void>|null=null,releaseGate:(()=>void)|null=null;
let scanGate:Promise<void>|null=null,releaseScanGate:(()=>void)|null=null;
export function bumpCompatibilityContext(){compatibilityRevision++;}
export function blockCompatibility(){if(compatibilityGate)throw Error('fixture_already_blocked');compatibilityGate=new Promise(resolve=>{releaseGate=resolve;});}
export function releaseCompatibility(){releaseGate?.();releaseGate=null;compatibilityGate=null;}
export function blockDiscoveryScan(){if(scanGate)throw Error('fixture_scan_already_blocked');scanGate=new Promise(resolve=>{releaseScanGate=resolve;});}
export function releaseDiscoveryScan(){releaseScanGate?.();releaseScanGate=null;scanGate=null;}
export const installations=['codex','claude_code','opencode'].map((client_kind,index)=>({
 installation_id:`7132bced-3ce0-4c1b-8f48-d93d7b96f7c${index}`,
 client_kind,surface:'cli',version:['0.163.0-alpha.10.2','2.1.158','1.19.3'][index],
 source:'user_local_npm',location_label:'User-local npm',status:'detected',connection_status:'not_verified',
}));
const reference={engine_version:'0.41.0',manifest_sha256:'8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908',mode:'cache',savings_profile:'coding',ccr_ttl_seconds:7200};
const operating={close_to_tray:true,retention_days:30,protected_tools:[]};
const at='2026-11-08T12:00:00Z';
function preferences(id:string){return {schema_version:1,request_id:id,status:'ok',revision,state_fingerprint:'ab'.repeat(32),preferences:{language,theme,operating},runtime_manifest:reference};}
function desktop(id:string){return {schema_version:1,request_id:id,status:'snapshot',tray_available:true,close_to_tray:true,exit_state:exit,exit_token:exit==='confirmation_required'?'4aa54830-c0af-4b4c-8b28-6343270eb55c':null};}
function overview(period:string,id:string){
 const populated=scenario==='data';const totals={status:populated?'measured':'no_comparable_events',comparable_events:populated?1:0,excluded_events:0,
 before_tokens:populated?1000:null,after_tokens:populated?500:null,saved_tokens:populated?500:null,saved_percent:populated?50:null};
 const coverage={events:populated?1:0,distinct_origins:populated?1:0,unplaced_events:0,rolling_window:false,aggregate_only:false,unidentified_rows_observed:false,complete_traffic:false};
 const attribution={scope:'observed_requests',coverage:{requests:populated?1:0,events:populated?1:0,model_observed:0,client_verified:0,agent_verified:0,conflicts:0},client_status:'unknown',agent_status:'unknown',groups:[],omitted_groups:0};
 return {schema_version:1,request_id:id,status:'ok',observed_at_utc:at,data:{monitoring_session_id:'7132bced-3ce0-4c1b-8f48-d93d7b96f7cf',period,
 from_utc:period==='session'?null:'2026-11-08T00:00:00Z',to_utc:at,retention_days:30,last_event_utc:populated?at:null,totals,coverage,attribution,
 series:populated?[{at_utc:at,totals}]:[]},collection:{status:'recording',interval_seconds:5,last_capture_utc:at,runtime_generation:'7132bced3ce04c1b8f48d93d7b96f7ce',last_error:null,sampling_interrupted_since_start:false}};
}
export function reset(nextLanguage:string,nextTheme:string,nextScenario:Scenario){releaseCompatibility();releaseDiscoveryScan();language=nextLanguage;theme=nextTheme;scenario=nextScenario;revision=1;exit='idle';generation=1;compatibilityRevision=0;}
export function isTauri(){return true;}
export function getCurrentWebview(){return {setZoom:async()=>{}};}
export async function listen(_event:string,callback:()=>void){callbacks.add(callback);return()=>callbacks.delete(callback);}
export async function invoke(command:string,args:any){
 const id=args.requestId??args.request?.request_id;
 if(command==='scan_discovery'&&scanGate)await scanGate;
 if(command==='get_compatibility_snapshot'){
  if(compatibilityGate)await compatibilityGate;
  if(scenario==='error')throw Error('isolated_catalog_read_failure');
  const result=compatibilitySnapshot(id);result.scan_generation=generation;result.context_fingerprint=(compatibilityRevision?'e':'d').repeat(64);
  result.installations=scenario==='empty'?[]:installations.map(i=>({installation_id:i.installation_id,client_kind:i.client_kind,surface:i.surface,version:i.version,
   freshness:'current',config_supported:true,config_state:'not_owned',historical_evidence_ids:result.records.filter(r=>r.client_kind===i.client_kind&&r.surface===i.surface&&r.tested_versions.includes(i.version)).map(r=>r.id)}));
  return result;
 }
 if(command==='get_runtime_snapshot')return {...runtimeSnapshot(),request_id:id};
 if(command==='get_preferences')return preferences(id);
 if(command==='save_preferences'){language=args.language;theme=args.theme;revision++;for(const cb of callbacks)cb();return preferences(id);}
 if(command==='get_desktop_state')return desktop(id);
 if(command==='request_desktop_exit'||command==='cancel_desktop_exit'){exit=command==='request_desktop_exit'?'confirmation_required':'idle';return desktop(id);}
 if(command==='get_operating_snapshot')return {schema_version:1,request_id:id,status:'snapshot',autostart:{status:'disabled',fingerprint:null},runtime:{status:'inactive',runtime_generation:null,protected_tools:null,local_stats_enabled:null,beacon_enabled:null,mode:null,savings_profile:null,ccr_ttl_seconds:null}};
 if(command==='get_discovery_snapshot'||command==='scan_discovery')return {schema_version:1,request_id:id,status:'partial',scan_generation:command==='scan_discovery'?++generation:generation,installations:scenario==='empty'?[]:installations,warnings:['scope_partial','absence_is_not_unsupported']};
 if(command==='get_connection_ownership')return {schema_version:1,request_id:id,status:'snapshot',client_kind:args.request.client_kind,ownership:null};
 if(command==='get_saved_installation_choice')return {schema_version:1,request_id:id,status:'not_selected',client_kind:args.request.client_kind,scan_generation:generation,installation_id:null};
 if(command==='get_overview_snapshot'){if(scenario==='error')throw Error('isolated_read_failure');return overview(args.request.period,id);}
 if(command==='get_activity_snapshot'){
  if(scenario==='error')throw Error('isolated_read_failure');const result=activityList(args.request.period,args.request.filter);result.request_id=id;
  if(scenario==='empty'){result.data.rows=[];result.data.shown_requests=0;result.data.matched_requests=0;result.data.unplaced_requests=0;result.data.coverage={...result.data.coverage,events:0,distinct_origins:0,unplaced_events:0};}return result;
 }
 if(command==='get_activity_detail'){const result=activityDetail(args.request.period,args.request.origin_id);result.request_id=id;return result;}
 if(command==='export_report')return {schema_version:1,request_id:id,status:'cancelled',kind:args.request.kind,bytes_written:0};
 if(command==='get_recovery_snapshot')return {schema_version:1,request_id:id,status:'snapshot',observed_at_utc:at,runtime:{engine_status:'ready',ingress_status:'ready',error_code:null},traffic:{status:'none',period:'seven_days',last_observed_utc:null,outcome:'unknown',error_layer:'unknown'},clients:installations.map(i=>({client_kind:i.client_kind,detected_count:1,installation_status:'current',compatibility:'config_supported',config_state:'not_owned',config_scope:'unknown',endpoint_binding:'not_owned'}))};
 // Unsupported fixture actions fail visibly, rather than simulating an Apply or real verification.
 throw Error('isolated_unsupported_command');
}
