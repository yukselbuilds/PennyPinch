import {invoke,isTauri} from "@tauri-apps/api/core";

export type EvidenceStatus="not_tested"|"unknown"|"environment_blocked"|"local_fixture_partial"|"local_fixture_pass"|"live_verified"|"unsupported";
export type CatalogClient="engine"|"codex"|"claude_code"|"opencode";
export type CatalogSurface="runtime"|"cli"|"desktop"|"cli_desktop"|"other";
export type CatalogAuth="none"|"synthetic"|"chatgpt"|"api_key"|"oauth"|"unknown";
export type CatalogPlatform="windows_10_x64"|"windows_11_x64"|"macos"|"linux"|"unknown";
export interface CompatibilityRecord {
 id:string;client_kind:CatalogClient;product:string;version:string;surface:CatalogSurface;tested_versions:string[];auth_method:CatalogAuth;platform:CatalogPlatform;
 engine_version:string;evidence_status:EvidenceStatus;evidence_date:string;evidence:string[];scope:string;
 capability_counts:{status:EvidenceStatus;count:number}[];
}
export interface CompatibilityInstallation {
 installation_id:string;client_kind:Exclude<CatalogClient,"engine">;surface:"cli"|"desktop";version:string|null;
 freshness:"current"|"stale"|"unknown";config_supported:boolean;config_state:"matches_owned_endpoint"|"conflict"|"not_owned"|"unavailable"|"busy"|"not_rechecked";
 historical_evidence_ids:string[];
}
export interface CompatibilitySnapshot {
 schema_version:1;request_id:string;status:"snapshot";observed_at_utc:string;catalog_revision:string;catalog_date:string;
 channel:"development";app_version:string;engine:{pinned_version:string;manifest_sha256:string;active_version:string|null;engine_status:"unavailable"|"stopped"|"starting"|"ready"|"stopping"|"failed"|"unknown"};
 automatic_updates:false;updater_available:false;adapters:{client_kind:Exclude<CatalogClient,"engine">;fingerprint:string}[];
 records:CompatibilityRecord[];scan_generation:number|null;context_fingerprint:string|null;installations:CompatibilityInstallation[];
}
export type CompatibilityResult={kind:"snapshot";snapshot:CompatibilitySnapshot}|{kind:"desktop_unavailable"|"client_busy"|"invalid_response"|"request_failed"}|{kind:"catalog_error";code:"invalid_input"|"busy"|"unavailable"};
type Obj=Record<string,unknown>;
const clients=["codex","claude_code","opencode"];
const statuses=["not_tested","unknown","environment_blocked","local_fixture_partial","local_fixture_pass","live_verified","unsupported"];
const uuid=(v:unknown):v is string=>typeof v==="string"&&/^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(v);
const hash=(v:unknown):v is string=>typeof v==="string"&&/^[a-f0-9]{64}$/.test(v);
const obj=(v:unknown):v is Obj=>typeof v==="object"&&v!==null&&!Array.isArray(v);
const keys=(v:Obj,names:string)=>Object.keys(v).sort().join(",")===names.split(" ").sort().join(",");
const text=(v:unknown,max:number):v is string=>typeof v==="string"&&v.length>0&&v.length<=max&&!/[\u0000-\u001f\u007f]/.test(v);
const one=(v:unknown,values:string[]):v is string=>typeof v==="string"&&values.includes(v);
const date=(v:unknown):v is string=>typeof v==="string"&&/^\d{4}-\d{2}-\d{2}$/.test(v)&&Number.isFinite(Date.parse(v))&&new Date(v).toISOString().slice(0,10)===v;
const timestamp=(v:unknown)=>typeof v==="string"&&/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(v)&&Number.isFinite(Date.parse(v))&&new Date(v).toISOString().slice(0,19)===v.slice(0,19);
const evidence=(v:unknown):v is string=>typeof v==="string"&&v.length<=160&&/^docs\/(proofs|reviews)\/[a-zA-Z0-9/_.-]+\.md$/.test(v)&&!v.includes("..");

export function decodeCompatibilitySnapshot(value:unknown,id:string):CompatibilitySnapshot|null {
 if(!uuid(id)||!obj(value)||!keys(value,"schema_version request_id status observed_at_utc catalog_revision catalog_date channel app_version engine automatic_updates updater_available adapters records scan_generation context_fingerprint installations")
  ||value.schema_version!==1||value.request_id!==id||value.status!=="snapshot"||!timestamp(value.observed_at_utc)||!hash(value.catalog_revision)||!date(value.catalog_date)
  ||value.channel!=="development"||!text(value.app_version,64)||value.automatic_updates!==false||value.updater_available!==false
  ||!obj(value.engine)||!keys(value.engine,"pinned_version manifest_sha256 active_version engine_status")||!text(value.engine.pinned_version,64)||!hash(value.engine.manifest_sha256)
  ||!(value.engine.active_version===null||text(value.engine.active_version,64))||!one(value.engine.engine_status,["unavailable","stopped","starting","ready","stopping","failed","unknown"])
  ||!(value.scan_generation===null||(Number.isSafeInteger(value.scan_generation)&&Number(value.scan_generation)>=0))
  ||!(value.context_fingerprint===null||hash(value.context_fingerprint))||!Array.isArray(value.adapters)||value.adapters.length!==3||!Array.isArray(value.records)||!value.records.length||value.records.length>32
  ||!Array.isArray(value.installations)||value.installations.length>512)return null;
 if(value.context_fingerprint!==null&&value.scan_generation===null)return null;
 if(value.engine.engine_status==="ready"&&value.engine.active_version!==value.engine.pinned_version)return null;
 const adapterKinds=new Set();for(const a of value.adapters){if(!obj(a)||!keys(a,"client_kind fingerprint")||!one(a.client_kind,clients)||!hash(a.fingerprint)||adapterKinds.has(a.client_kind))return null;adapterKinds.add(a.client_kind);}
 const records=new Map<string,CompatibilityRecord>();
 for(const r of value.records){
  if(!obj(r)||!keys(r,"id client_kind product version surface tested_versions capability_counts auth_method platform engine_version evidence_status evidence_date evidence scope")
   ||!text(r.id,96)||!/^[a-z0-9-]+$/.test(r.id)||records.has(r.id)||!one(r.client_kind,["engine",...clients])||!text(r.product,120)||!text(r.version,160)
   ||!one(r.surface,["runtime","cli","desktop","cli_desktop","other"])||!Array.isArray(r.tested_versions)||r.tested_versions.length>8||!r.tested_versions.every(v=>text(v,64))||new Set(r.tested_versions).size!==r.tested_versions.length||!one(r.auth_method,["none","synthetic","chatgpt","api_key","oauth","unknown"])
   ||!one(r.platform,["windows_10_x64","windows_11_x64","macos","linux","unknown"])||!text(r.engine_version,64)||!one(r.evidence_status,statuses)||!date(r.evidence_date)
   ||r.evidence_date>value.catalog_date||!Array.isArray(r.evidence)||!r.evidence.length||r.evidence.length>8||!r.evidence.every(evidence)||!text(r.scope,320)
   ||!Array.isArray(r.capability_counts)||!r.capability_counts.length||r.capability_counts.length>7)return null;
  const scopes=new Set();let count=0;
  for(const c of r.capability_counts){if(!obj(c)||!keys(c,"status count")||!one(c.status,statuses)||scopes.has(c.status)||!Number.isSafeInteger(c.count)||Number(c.count)<1||Number(c.count)>32)return null;scopes.add(c.status);count+=Number(c.count);}
  if(count>32)return null;
  if(r.evidence_status==="live_verified"&&(!r.tested_versions.length||r.platform==="unknown"||!["chatgpt","api_key","oauth"].includes(r.auth_method)))return null;
  records.set(r.id,r as unknown as CompatibilityRecord);
 }
 const ids=new Set();for(const i of value.installations){
  if(!obj(i)||!keys(i,"installation_id client_kind surface version freshness config_supported config_state historical_evidence_ids")||!uuid(i.installation_id)||ids.has(i.installation_id)
   ||!one(i.client_kind,clients)||!one(i.surface,["cli","desktop"])||!(i.version===null||text(i.version,64))||!one(i.freshness,["current","stale","unknown"])
   ||typeof i.config_supported!=="boolean"||!one(i.config_state,["matches_owned_endpoint","conflict","not_owned","unavailable","busy","not_rechecked"])
   ||!Array.isArray(i.historical_evidence_ids)||i.historical_evidence_ids.length>32||new Set(i.historical_evidence_ids).size!==i.historical_evidence_ids.length)return null;
  if((i.freshness!=="current"&&(i.config_supported||i.historical_evidence_ids.length))||(i.config_supported&&i.version===null)
   ||(i.config_state==="matches_owned_endpoint"&&!i.config_supported))return null;
  for(const ref of i.historical_evidence_ids){const r=typeof ref==="string"?records.get(ref):null;if(!r||r.client_kind!==i.client_kind||![i.surface,"cli_desktop"].includes(r.surface)||r.engine_version!==value.engine.pinned_version||typeof i.version!=="string"||!r.tested_versions.includes(i.version))return null;}
  ids.add(i.installation_id);
 }
 return value as unknown as CompatibilitySnapshot;
}

export function createCompatibilityClient(transport:{isAvailable:()=>boolean;requestId:()=>string|null;invoke:(command:string,args:Record<string,unknown>)=>Promise<unknown>}){
 let pending=false;
 return {async getSnapshot():Promise<CompatibilityResult>{
  if(pending)return {kind:"client_busy"};
  try {if(!transport.isAvailable())return {kind:"desktop_unavailable"};}catch{return {kind:"request_failed"};}
  let id:string|null;try{id=transport.requestId();}catch{return {kind:"request_failed"};}if(!uuid(id))return {kind:"request_failed"};
  pending=true;try{
   const value=await transport.invoke("get_compatibility_snapshot",{request:{schema_version:1,request_id:id}});const snapshot=decodeCompatibilitySnapshot(value,id);
   if(snapshot)return {kind:"snapshot",snapshot};
   if(obj(value)&&keys(value,"schema_version request_id status error")&&value.schema_version===1&&value.request_id===id&&value.status==="error"&&obj(value.error)
    &&keys(value.error,"code retryable message_key diagnostic_id")&&one(value.error.code,["invalid_input","busy","unavailable"])&&value.error.retryable===(value.error.code==="busy")
    &&value.error.message_key===`compatibility.${value.error.code}`&&value.error.diagnostic_id===null)return {kind:"catalog_error",code:value.error.code as "invalid_input"|"busy"|"unavailable"};
   return {kind:"invalid_response"};
  }catch{return {kind:"request_failed"};}finally{pending=false;}
 }};
}
export const compatibilityClient=createCompatibilityClient({isAvailable:()=>isTauri(),requestId:()=>globalThis.crypto?.randomUUID?.()??null,invoke:(command,args)=>invoke(command,args)});
