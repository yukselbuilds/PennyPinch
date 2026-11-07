import {invoke,isTauri} from '@tauri-apps/api/core';
import {decodeOperatingPreferences} from './preferences.ts';
export type StartupStatus='registered'|'disabled'|'conflict'|'unavailable';
export interface EffectiveSettings {
  status:'verified'|'inactive'|'unknown';runtime_generation:string|null;protected_tools:string[]|null;
  local_stats_enabled:true|null;beacon_enabled:false|null;mode:'cache'|null;savings_profile:'coding'|null;ccr_ttl_seconds:7200|null;
}
export interface OperatingSnapshot {schema_version:1;request_id:string;status:'snapshot';autostart:{status:StartupStatus;fingerprint:string|null};runtime:EffectiveSettings}
export type OperatingError='validation_failed'|'environment_blocked'|'busy'|'config_conflict';
export type OperatingResult={kind:'snapshot';snapshot:OperatingSnapshot}|{kind:'error';code:OperatingError}|{kind:'unavailable'|'invalid_response'|'request_failed'|'client_busy'};
const uuid=(s:unknown):s is string=>typeof s==='string'&&/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(s);
const record=(v:unknown):v is Record<string,unknown>=>typeof v==='object'&&v!==null&&!Array.isArray(v);
const keys=(v:Record<string,unknown>,k:string[])=>Object.keys(v).sort().join()===k.sort().join();
const fingerprint=(v:unknown):v is string|null=>v===null||typeof v==='string'&&/^[0-9a-f]{64}$/.test(v);
export function decodeOperatingSnapshot(value:unknown,id:string):OperatingSnapshot|null {
 if(!uuid(id)||!record(value)||!keys(value,['schema_version','request_id','status','autostart','runtime'])||value.schema_version!==1||value.request_id!==id||value.status!=='snapshot')return null;
 const a=value.autostart,r=value.runtime;
 if(!record(a)||!keys(a,['status','fingerprint'])||!['registered','disabled','conflict','unavailable'].includes(a.status as string)||!fingerprint(a.fingerprint)
  ||((a.status==='registered'||a.status==='conflict')&&a.fingerprint===null)||(a.status==='disabled'||a.status==='unavailable')&&a.fingerprint!==null)return null;
 if(!record(r)||!keys(r,['status','runtime_generation','protected_tools','local_stats_enabled','beacon_enabled','mode','savings_profile','ccr_ttl_seconds'])||!['verified','inactive','unknown'].includes(r.status as string))return null;
 if(r.status==='verified') {
  if(typeof r.runtime_generation!=='string'||!/^[0-9a-f]{32}$/.test(r.runtime_generation)||!decodeOperatingPreferences({close_to_tray:true,retention_days:30,protected_tools:r.protected_tools})
   ||r.local_stats_enabled!==true||r.beacon_enabled!==false||r.mode!=='cache'||r.savings_profile!=='coding'||r.ccr_ttl_seconds!==7200)return null;
 } else if(Object.entries(r).some(([key,v])=>key!=='status'&&v!==null))return null;
 return {schema_version:1,request_id:id,status:'snapshot',autostart:{status:a.status as StartupStatus,fingerprint:a.fingerprint},runtime:{...r} as unknown as EffectiveSettings};
}
export function createOperatingClient(options:{isNative:()=>boolean;createRequestId:()=>string|null;invoke:(command:string,args:Record<string,unknown>)=>Promise<unknown>}) {
 let pending=false;
 async function request(command:string,args:Record<string,unknown>={},mutation=false):Promise<OperatingResult> {
  if(!options.isNative())return {kind:'unavailable'};
  if(mutation&&pending)return {kind:'client_busy'};
  if(mutation&&(!fingerprint(args.expectedFingerprint)||typeof args.enabled!=='boolean'))return {kind:'error',code:'validation_failed'};
  if(mutation)pending=true;
  try {
   const id=options.createRequestId();if(!uuid(id))return {kind:'request_failed'};
   const raw=await options.invoke(command,{requestId:id,...args});const snapshot=decodeOperatingSnapshot(raw,id);if(snapshot)return {kind:'snapshot',snapshot};
   if(record(raw)&&keys(raw,['schema_version','request_id','status','error'])&&raw.request_id===id&&raw.schema_version===1&&raw.status==='error'&&record(raw.error)&&keys(raw.error,['code','retryable','message_key','diagnostic_id'])) {
    const code=raw.error.code as OperatingError;
    if(['validation_failed','environment_blocked','busy','config_conflict'].includes(code)&&raw.error.retryable===(code==='busy'||code==='config_conflict')&&raw.error.message_key==='operating.'+code&&raw.error.diagnostic_id===null)return {kind:'error',code};
   }return {kind:'invalid_response'};
  } catch{return {kind:'request_failed'}}finally{if(mutation)pending=false;}
 }
 return {available:options.isNative,read:()=>request('get_operating_snapshot'),setAutostart:(enabled:boolean,expectedFingerprint:string|null)=>request('set_autostart',{enabled,expectedFingerprint},true)};
}
export const operatingClient=createOperatingClient({isNative:isTauri,createRequestId:()=>globalThis.crypto?.randomUUID?.()??null,invoke:(command,args)=>invoke(command,args)});
