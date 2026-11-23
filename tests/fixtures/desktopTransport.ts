// Isolated UI evidence only. No native API, filesystem, auth, or provider access.
import {runtimeSnapshot} from './runtime.mjs';
import {activityList,activityDetail} from './activity.mjs';
import {compatibilitySnapshot} from './compatibility.mjs';
export type Scenario = 'data'|'empty'|'error'|'recommendations'|'settings-guidance';
let language='tr',theme='dark',scenario:Scenario='data',revision=1,exit='idle',generation=1;
let showRecommendations=false,recommendationsResolved=false;
let experimentalRules=false,ruleCapture=false;
const ruleCaptureId='63b3217e-b756-4e55-8fc8-6adb85ce7df1';
export function resolveRecommendations(){recommendationsResolved=true;}
const callbacks=new Set<()=>void>();
let compatibilityRevision=0;
let compatibilityGate:Promise<void>|null=null,releaseGate:(()=>void)|null=null;
let scanGate:Promise<void>|null=null,releaseScanGate:(()=>void)|null=null;
type SessionMode='data'|'empty'|'error';
type SessionCall={schema_version:number;request_id:string;operation:'read'|'open_codex_center';snapshot_id:string|null;confirm_open:boolean};
const sessionSnapshotId='ae0df834-8df0-48c2-a174-c0f652bc2ad1';
let sessionMode:SessionMode='data',sessionCalls:SessionCall[]=[],sessionOpenCount=0,activeSessionReads=0,maxSessionReads=0;
let failSessionLaunch=false;
let sessionGate:Promise<void>|null=null,releaseSessionGate:(()=>void)|null=null;
const uuidPattern=/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
function sessionSnapshot(){
 const scopes=sessionMode==='empty'
  ?[{client_kind:'codex',state:'observed',reason:null,server_version:'0.167.0-alpha.4.1',loaded_count:0,partial:false},
    {client_kind:'claude_code',state:'unknown',reason:'unsupported_version',server_version:null,loaded_count:null,partial:false},
    {client_kind:'opencode',state:'unknown',reason:'endpoint_unavailable',server_version:null,loaded_count:null,partial:false}]
  :[{client_kind:'codex',state:'observed',reason:null,server_version:'0.167.0-alpha.4.1',loaded_count:2,partial:false},
    {client_kind:'claude_code',state:'unknown',reason:'unsupported_version',server_version:null,loaded_count:null,partial:false},
    {client_kind:'opencode',state:'unknown',reason:'endpoint_unavailable',server_version:null,loaded_count:null,partial:false}];
 const rows=sessionMode==='empty'?[]:[
  {key:'a'.repeat(64),id_hint:'aaaaaaaa',client_kind:'codex',project:'fixture-project',model:'fixture-model',provider:'openai',reasoning:'medium',created_at:'2026-11-13T09:00:00Z',updated_at:'2026-11-13T09:30:00Z',state:'open',role:'main',parent_key:null,traffic_assignment:'unverified'},
  {key:'b'.repeat(64),id_hint:'bbbbbbbb',client_kind:'codex',project:'fixture-project',model:'fixture-model',provider:'openai',reasoning:'medium',created_at:'2026-11-13T09:05:00Z',updated_at:'2026-11-13T09:25:00Z',state:'open',role:'subagent',parent_key:'a'.repeat(64),traffic_assignment:'unverified'},
  {key:'c'.repeat(64),id_hint:'cccccccc',client_kind:'codex',project:'fixture-project',model:'fixture-model',provider:'openai',reasoning:'medium',created_at:'2026-11-12T10:00:00Z',updated_at:'2026-11-12T10:30:00Z',state:'stored',role:'main',parent_key:null,traffic_assignment:'unverified'},
 ];
 return {snapshot_id:sessionSnapshotId,observed_at_utc:'2026-11-13T09:31:00Z',scope:'current_cli_environment',clients:scopes,rows,can_open_codex:true,traffic_assignment:'unverified'};
}
export function setSessionMode(mode:SessionMode){sessionMode=mode;sessionOpenCount=0;}
export function failNextSessionLaunch(){failSessionLaunch=true;}
export function blockSessionReads(){if(sessionGate)throw Error('fixture_session_read_already_blocked');sessionGate=new Promise(resolve=>{releaseSessionGate=resolve;});}
export function releaseSessionReads(){releaseSessionGate?.();releaseSessionGate=null;sessionGate=null;}
export function sessionHistory(){return sessionCalls.map(call=>({...call}));}
export function sessionReadStats(){return {reads:sessionCalls.filter(call=>call.operation==='read').length,max_active: maxSessionReads};}
if(typeof window!=='undefined')(window as any).__HeadroomSessionFixture={setMode:setSessionMode,failLaunch:failNextSessionLaunch,blockReads:blockSessionReads,releaseReads:releaseSessionReads,history:sessionHistory,readStats:sessionReadStats};
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
const reference={engine_version:'0.45.0',manifest_sha256:'ef1ed3803ebfd7c819eefd2b5662f3cc72d5d688cfcd2b8eb4b5d2221ac80c29',mode:'cache',savings_profile:'coding',ccr_ttl_seconds:7200};
const operating={close_to_tray:true,retention_days:30,protected_tools:[]};
const at='2026-11-08T12:00:00Z';
function preferences(id:string){return {schema_version:1,request_id:id,status:'ok',revision,state_fingerprint:'ab'.repeat(32),preferences:{language,theme,operating,...(showRecommendations?{show_recommendations:true}:{}),...(experimentalRules?{experimental_rules:true}:{})},runtime_manifest:reference};}
function desktop(id:string){return {schema_version:1,request_id:id,status:'snapshot',tray_available:true,close_to_tray:true,exit_state:exit,exit_token:exit==='confirmation_required'?'4aa54830-c0af-4b4c-8b28-6343270eb55c':null};}
function overview(period:string,id:string,includeRecommendations:boolean){
 const hasAdvice=scenario==='recommendations'&&!recommendationsResolved;
 const events=scenario==='recommendations'?40:scenario==='data'?1:0;
 const after=hasAdvice?1250:500;
 const totals={status:events?'measured':'no_comparable_events',comparable_events:events,excluded_events:0,
 before_tokens:events?1000:null,after_tokens:events?after:null,saved_tokens:events?1000-after:null,saved_percent:events?(1000-after)/10:null};
 const coverage={events,distinct_origins:events,unplaced_events:0,rolling_window:false,aggregate_only:false,unidentified_rows_observed:false,complete_traffic:false};
 const counts={requests:events,events,model_observed:hasAdvice?events:0,client_verified:0,agent_verified:0,conflicts:0};
 const groups=events?[{group_id:'b'.repeat(64),model:hasAdvice?'fixture-model':null,provider:hasAdvice?'fixture-provider':null,
  client:null,upstream_id:null,agent:null,conflicted:false,is_other:false,coverage:counts,totals}]:[];
 const attribution={scope:'observed_requests',coverage:counts,client_status:'unknown',agent_status:'unknown',groups,omitted_groups:0};
 const recommendations=includeRecommendations&&hasAdvice?[
  {kind:'input_growth',basis:'local_estimate',component:'whole_input',before_tokens:1000,after_tokens:1250,events},
  {kind:'slow_optimization',p95_ms:1100,requests:20},
  {kind:'cache_share_drop',model:'fixture-model',provider:'fixture-provider',previous_hits:12,previous_requests:20,recent_hits:8,recent_requests:20},
  {kind:'installation_stale',client:'codex'},
 ]:[];
 return {schema_version:1,request_id:id,status:'ok',observed_at_utc:at,data:{monitoring_session_id:'7132bced-3ce0-4c1b-8f48-d93d7b96f7cf',period,
 from_utc:period==='session'?null:'2026-11-08T00:00:00Z',to_utc:at,retention_days:30,last_event_utc:events?at:null,totals,coverage,attribution,
 series:events?[{at_utc:at,totals}]:[]},collection:{status:'recording',interval_seconds:5,last_capture_utc:at,runtime_generation:'7132bced3ce04c1b8f48d93d7b96f7ce',last_error:null,sampling_interrupted_since_start:false},recommendations};
}
export function reset(nextLanguage:string,nextTheme:string,nextScenario:Scenario){releaseCompatibility();releaseDiscoveryScan();releaseSessionReads();language=nextLanguage;theme=nextTheme;scenario=nextScenario;revision=1;exit='idle';generation=1;compatibilityRevision=0;showRecommendations=false;recommendationsResolved=false;experimentalRules=false;ruleCapture=false;sessionMode=nextScenario==='empty'?'empty':nextScenario==='error'?'error':'data';sessionCalls=[];sessionOpenCount=0;failSessionLaunch=false;activeSessionReads=0;maxSessionReads=0;}
export function isTauri(){return true;}
export function getCurrentWebview(){return {setZoom:async()=>{}};}
export async function listen(_event:string,callback:()=>void){callbacks.add(callback);return()=>callbacks.delete(callback);}
export async function invoke(command:string,args:any){
 const id=args.requestId??args.request?.request_id;
 if(command==='manage_sessions'){
  const request=args?.request;
  const requestKeys=request&&typeof request==='object'&&!Array.isArray(request)?Object.keys(request).sort().join(','):'';
  if(!args||Object.keys(args).length!==1||requestKeys!=='confirm_open,operation,request_id,schema_version,snapshot_id'
   ||request.schema_version!==1||typeof request.request_id!=='string'||!uuidPattern.test(request.request_id))throw Error('fixture_invalid_session_request');
  const {schema_version,request_id,operation,snapshot_id,confirm_open}=request;
  if(operation==='read'&&snapshot_id===null&&confirm_open===false){
   const call={schema_version,request_id,operation,snapshot_id,confirm_open};sessionCalls.push(call);
   activeSessionReads++;maxSessionReads=Math.max(maxSessionReads,activeSessionReads);
   try{if(sessionGate)await sessionGate;}finally{activeSessionReads--;}
   if(sessionMode==='error')return {schema_version:1,request_id,status:'error',snapshot:null,error:'unavailable'};
   return {schema_version:1,request_id,status:'snapshot',snapshot:sessionSnapshot(),error:null};
  }
  if(operation==='open_codex_center'&&snapshot_id===sessionSnapshotId&&confirm_open===true){
   sessionCalls.push({schema_version,request_id,operation,snapshot_id,confirm_open});
   if(sessionOpenCount++>0)return {schema_version:1,request_id,status:'error',snapshot:null,error:'busy'};
   if(failSessionLaunch)return {schema_version:1,request_id,status:'error',snapshot:null,error:'launch_failed'};
   return {schema_version:1,request_id,status:'launched',snapshot:null,error:null};
  }
  throw Error('fixture_invalid_session_operation');
 }
 if(command==='scan_discovery'&&scanGate)await scanGate;
 if(command==='get_compatibility_snapshot'){
  if(compatibilityGate)await compatibilityGate;
  if(scenario==='error')throw Error('isolated_catalog_read_failure');
  const result=compatibilitySnapshot(id);result.scan_generation=generation;result.context_fingerprint=(compatibilityRevision?'e':'d').repeat(64);
  result.installations=scenario==='empty'?[]:installations.map(i=>({installation_id:i.installation_id,client_kind:i.client_kind,surface:i.surface,version:i.version,
   freshness:'current',config_supported:true,config_state:'not_owned',historical_evidence_ids:result.records.filter(r=>
    r.client_kind===i.client_kind&&(r.surface===i.surface||r.surface==='cli_desktop')
    &&r.engine_version===result.engine.pinned_version&&r.tested_versions.includes(i.version)).map(r=>r.id)}));
  return result;
 }
 if(command==='get_runtime_snapshot')return {...runtimeSnapshot(),request_id:id};
 if(command==='get_preferences')return preferences(id);
 if(command==='save_preferences'){
  language=args.language;theme=args.theme;
  if(typeof args.showRecommendations==='boolean')showRecommendations=args.showRecommendations;
  if(typeof args.experimentalRules==='boolean')experimentalRules=args.experimentalRules;
  revision++;for(const cb of callbacks)cb();return preferences(id);
 }
 if(scenario==='settings-guidance'&&command==='preview_learn'){
  if(!args.experimentalRules)return {status:'ready',source:'codex',suggestions:[{kind:'path',failures:3,recoveries:0,first_line:12}]};
  if(!experimentalRules)return {status:'unavailable'};
  ruleCapture=true;
  return {status:'ready',source:'codex',suggestions:[],replay_id:ruleCaptureId};
 }
 if(scenario==='settings-guidance'&&command==='replay_rules'){
  if(!experimentalRules||!ruleCapture||args.replayId!==ruleCaptureId)return {status:'expired'};
  return {status:'trial_ready',report:{rule:'repeated_log_lines_v1',checked:1,eligible:1,protected:0,ambiguous:0,
    compacted:1,before_bytes:300,after_bytes:120,round_trip_exact:true}};
 }
 if(scenario==='settings-guidance'&&command==='release_rule_preview'&&args.replayId===ruleCaptureId){
  ruleCapture=false;
  return;
 }
 if(command==='get_desktop_state')return desktop(id);
 if(command==='plugin:window|is_visible')return args.label==='main';
 if(command==='request_desktop_exit'||command==='cancel_desktop_exit'){exit=command==='request_desktop_exit'?'confirmation_required':'idle';return desktop(id);}
 if(command==='get_operating_snapshot')return {schema_version:1,request_id:id,status:'snapshot',autostart:{status:'disabled',fingerprint:null},runtime:{status:'inactive',runtime_generation:null,protected_tools:null,local_stats_enabled:null,beacon_enabled:null,mode:null,savings_profile:null,ccr_ttl_seconds:null}};
 if(command==='get_discovery_snapshot'||command==='scan_discovery')return {schema_version:1,request_id:id,status:'partial',scan_generation:command==='scan_discovery'?++generation:generation,installations:scenario==='empty'?[]:installations,warnings:['scope_partial','absence_is_not_unsupported']};
 if(command==='get_connection_ownership')return {schema_version:1,request_id:id,status:'snapshot',client_kind:args.request.client_kind,ownership:null};
 if(command==='get_saved_installation_choice')return {schema_version:1,request_id:id,status:'not_selected',client_kind:args.request.client_kind,scan_generation:generation,installation_id:null};
 if(command==='get_overview_snapshot'){if(scenario==='error')throw Error('isolated_read_failure');return overview(args.request.period,id,args.includeRecommendations===true);}
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
