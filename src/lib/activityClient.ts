import {invoke,isTauri} from "@tauri-apps/api/core";
import {decodeMetricCollection,decodeMetricTotals,isMetricHash,isMetricLabel} from "./metrics.ts";
import type {MetricPeriod,MetricsTransport} from "./metrics.ts";
import type {ActivityData,ActivityDetailSnapshot,ActivityFilter,ActivityIdentity,ActivityLayer,ActivityMeasurementDetail,ActivityReading,ActivityResult,ActivityRow,ActivitySnapshot,RequestMetadata} from "./activity.ts";

const record=(v:unknown):v is Record<string,unknown>=>typeof v==="object"&&v!==null&&!Array.isArray(v);
const member=<T extends string>(v:unknown,choices:readonly T[]):v is T=>typeof v==="string"&&choices.includes(v as T);
const count=(v:unknown):v is number=>typeof v==="number"&&Number.isSafeInteger(v)&&v>=0&&v<=100_000;
const token=(v:unknown):v is number=>typeof v==="number"&&Number.isSafeInteger(v)&&v>=0;
const signed=(v:unknown):v is number=>typeof v==="number"&&Number.isSafeInteger(v);
const uuid=(v:unknown):v is string=>typeof v==="string"&&/^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/.test(v)&&v!=="00000000-0000-0000-0000-000000000000";
const utc=(v:unknown):v is string=>typeof v==="string"&&/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?Z$/.test(v)&&Number.isFinite(Date.parse(v))&&new Date(v).toISOString().slice(0,19)===v.slice(0,19);
const periods=["session","today","seven_days"] as const;
const outcomes=["unknown","no_error_reported","error_reported"] as const;
const measurements=["measured","unmeasurable","conflict"] as const;
const layers=["headroom","rtk","cache","output"] as const;
const errors=["invalid_input","unsafe_path","busy","unsupported_schema","corrupt","capacity","unavailable","invalid_local_time","not_found"] as const;

export function decodeActivityFilter(v:unknown):ActivityFilter|null {
  if(!record(v)||!member(v.client,["any","unknown","codex","claude_code","opencode"] as const)||!record(v.model)
    ||!member(v.outcome,["any",...outcomes])||!member(v.measurement,["any",...measurements]))return null;
  const model=v.model.kind==="any"?{kind:"any" as const}:v.model.kind==="unknown"?{kind:"unknown" as const}
    :v.model.kind==="exact"&&isMetricLabel(v.model.value)?{kind:"exact" as const,value:v.model.value}:null;
  return model?{client:v.client,model,outcome:v.outcome,measurement:v.measurement}:null;
}
function metadata(v:unknown):RequestMetadata|null {
  const ms=(x:unknown)=>x===null||typeof x==="number"&&Number.isFinite(x)&&x>=0&&x<=86_400_000;
  if(!record(v)||![v.total_ms,v.optimization_ms,v.first_token_ms].every(ms)||!member(v.outcome,outcomes)
    ||!(v.cache_reported_hit===null||typeof v.cache_reported_hit==="boolean")||!(v.transforms_count===null||count(v.transforms_count)&&v.transforms_count<=128)||typeof v.conflicted!=="boolean")return null;
  if(v.conflicted&&(v.outcome!=="unknown"||[v.total_ms,v.optimization_ms,v.first_token_ms,v.cache_reported_hit,v.transforms_count].some(x=>x!==null)))return null;
  return {total_ms:v.total_ms as number|null,optimization_ms:v.optimization_ms as number|null,first_token_ms:v.first_token_ms as number|null,
    outcome:v.outcome,cache_reported_hit:v.cache_reported_hit,transforms_count:v.transforms_count as number|null,conflicted:v.conflicted};
}
function identity(v:unknown):ActivityIdentity|null {
  if(!record(v)||!(v.model===null||isMetricLabel(v.model))||!(v.provider===null||isMetricLabel(v.provider))
    ||!(v.client===null||member(v.client,["codex","claude_code","opencode"] as const))||!(v.upstream_id===null||isMetricHash(v.upstream_id))
    ||(v.client===null)!==(v.upstream_id===null)||typeof v.conflicted!=="boolean")return null;
  let agent:ActivityIdentity["agent"]=null;
  if(v.agent!==null){
    const a=v.agent;
    if(!record(a)||!isMetricHash(a.id)||!(a.parent_id===null||isMetricHash(a.parent_id))||a.parent_id===a.id
      ||!member(a.role,["unknown","main","subagent"] as const)||a.role==="main"&&a.parent_id!==null||v.client===null)return null;
    agent={id:a.id,parent_id:a.parent_id,role:a.role};
  }
  if(v.conflicted&&[v.model,v.provider,v.client,v.upstream_id,agent].some(x=>x!==null))return null;
  return {model:v.model,provider:v.provider,client:v.client,upstream_id:v.upstream_id,agent,conflicted:v.conflicted};
}
function row(v:unknown,start:number,end:number):ActivityRow|null {
  if(!record(v)||!isMetricHash(v.origin_id)||!(v.timestamp_utc===null||utc(v.timestamp_utc))||!count(v.events)||v.events===0
    ||!member(v.measurement,measurements)||!Array.isArray(v.layers)||v.layers.length<1||v.layers.length>4
    ||v.layers.some(x=>!member(x,layers))||new Set(v.layers).size!==v.layers.length)return null;
  const at=v.timestamp_utc===null?null:Date.parse(v.timestamp_utc);
  if(at!==null&&(at<start||at>end))return null;
  const id=identity(v.identity),m=metadata(v.metadata),totals=decodeMetricTotals(v.totals);
  if(!id||!m||!totals||totals.comparable_events+totals.excluded_events!==v.events
    ||v.measurement==="measured"&&totals.comparable_events===0||v.measurement==="unmeasurable"&&totals.comparable_events!==0)return null;
  return {origin_id:v.origin_id,timestamp_utc:v.timestamp_utc,identity:id,metadata:m,totals,measurement:v.measurement,events:v.events,layers:[...v.layers] as ActivityLayer[]};
}
function matches(r:ActivityRow,f:ActivityFilter):boolean {
  return (f.client==="any"||(f.client==="unknown"?r.identity.client===null:r.identity.client===f.client))
    &&(f.model.kind==="any"||(f.model.kind==="unknown"?r.identity.model===null:r.identity.model===f.model.value))
    &&(f.outcome==="any"||r.metadata.outcome===f.outcome)&&(f.measurement==="any"||r.measurement===f.measurement);
}
function list(v:Record<string,unknown>,period:MetricPeriod,filter:ActivityFilter):ActivitySnapshot|null {
  const d=v.data,c=decodeMetricCollection(v.collection);
  if(!utc(v.observed_at_utc)||!record(d)||!c||!uuid(d.monitoring_session_id)||d.period!==period||!(d.from_utc===null||utc(d.from_utc))||!utc(d.to_utc)
    ||![7,30,90].includes(d.retention_days as number)||!record(d.coverage)||!count(d.matched_requests)||!count(d.shown_requests)||d.limit!==100
    ||d.shown_requests!==Math.min(d.matched_requests,100)||d.has_more!==(d.matched_requests>100)||!count(d.unplaced_requests)
    ||d.unplaced_requests>d.matched_requests||!Array.isArray(d.rows)||d.rows.length!==d.shown_requests)return null;
  const start=d.from_utc===null?-Infinity:Date.parse(d.from_utc),end=Date.parse(d.to_utc),cov=d.coverage;
  if(start>end||end>Date.parse(v.observed_at_utc)||!count(cov.events)||!count(cov.distinct_origins)||!count(cov.unplaced_events)
    ||cov.distinct_origins>cov.events||cov.unplaced_events>cov.events||d.matched_requests>cov.distinct_origins||d.unplaced_requests>cov.unplaced_events
    ||cov.complete_traffic!==false||[cov.rolling_window,cov.aggregate_only,cov.unidentified_rows_observed].some(x=>typeof x!=="boolean"))return null;
  const rows:ActivityRow[]=[],ids=new Set();let previous=Infinity,unplaced=0;
  for(const raw of d.rows){
    const r=row(raw,start,end);if(!r||ids.has(r.origin_id)||!matches(r,filter))return null;
    const time=r.timestamp_utc===null?-Infinity:Date.parse(r.timestamp_utc);if(time>previous)return null;previous=time;
    if(r.timestamp_utc===null)unplaced+=1;ids.add(r.origin_id);rows.push(r);
  }
  if(unplaced>d.unplaced_requests)return null;
  const data:ActivityData={monitoring_session_id:d.monitoring_session_id,period,from_utc:d.from_utc,to_utc:d.to_utc,retention_days:d.retention_days as number,
    coverage:{events:cov.events,distinct_origins:cov.distinct_origins,unplaced_events:cov.unplaced_events,complete_traffic:false,
      rolling_window:cov.rolling_window as boolean,aggregate_only:cov.aggregate_only as boolean,unidentified_rows_observed:cov.unidentified_rows_observed as boolean},
    matched_requests:d.matched_requests,shown_requests:d.shown_requests,limit:100,has_more:d.has_more as boolean,unplaced_requests:d.unplaced_requests,rows};
  return {schema_version:1,request_id:v.request_id as string,status:"ok",observed_at_utc:v.observed_at_utc,data,collection:c};
}
function reading(v:unknown):ActivityReading|null {
  if(!record(v)||!(v.tokens===null||token(v.tokens))||!member(v.basis_kind,["local_estimate","provider_tokenizer","unknown"] as const)
    ||!(v.basis_id===null||isMetricHash(v.basis_id))||v.basis_kind==="unknown"&&v.basis_id!==null
    ||!member(v.component,["message_input","whole_input","tool_schema","novel_input","cache_input","output","unknown"] as const))return null;
  return {tokens:v.tokens,basis_kind:v.basis_kind,basis_id:v.basis_id,component:v.component};
}
function measurement(v:unknown,end:number):ActivityMeasurementDetail|null {
  if(!record(v)||!isMetricHash(v.event_id)||!(v.timestamp_utc===null||utc(v.timestamp_utc))||v.timestamp_utc!==null&&Date.parse(v.timestamp_utc)>end
    ||!Array.isArray(v.observed_via)||v.observed_via.length<1||v.observed_via.length>2||v.observed_via.some(x=>!member(x,["proxy","mcp"] as const))||new Set(v.observed_via).size!==v.observed_via.length
    ||!member(v.optimizer,layers)||!member(v.status,["measured","missing_counter","unknown_basis","incomparable","uncorrelated","missing_timestamp","conflict"] as const)
    ||typeof v.main_metric_eligible!=="boolean"||!record(v.provider_usage)||[v.provider_usage.input,v.provider_usage.output,v.provider_usage.cached_input].some(x=>x!==null&&!token(x)))return null;
  const cacheWrite=v.provider_usage.cache_write_input??null,attempts=v.provider_usage.upstream_attempts??null;
  if((cacheWrite!==null&&!token(cacheWrite))||(attempts!==null&&(!token(attempts)||attempts<1)))return null;
  const before=reading(v.before),after=reading(v.after),m=metadata(v.metadata);
  if(!before||!after||!m)return null;
  if(v.status==="measured"){
    if(before.tokens===null||after.tokens===null||v.timestamp_utc===null||before.basis_kind==="unknown"||before.basis_id===null
      ||before.basis_id!==after.basis_id||before.basis_kind!==after.basis_kind||before.component==="unknown"||before.component!==after.component
      ||!signed(v.saved_tokens)||v.saved_tokens!==before.tokens-after.tokens)return null;
    const percent=before.tokens===0?null:100*v.saved_tokens/before.tokens;
    if(percent===null?v.saved_percent!==null:typeof v.saved_percent!=="number"||!Number.isFinite(v.saved_percent)||Math.abs(v.saved_percent-percent)>1e-9*Math.max(1,Math.abs(percent)))return null;
    if(v.main_metric_eligible!==(v.optimizer==="headroom"&&["message_input","whole_input"].includes(before.component)))return null;
  }else if(v.saved_tokens!==null||v.saved_percent!==null||v.main_metric_eligible)return null;
  return {event_id:v.event_id,timestamp_utc:v.timestamp_utc,observed_via:[...v.observed_via] as ("proxy"|"mcp")[],optimizer:v.optimizer,status:v.status,
    before,after,saved_tokens:v.saved_tokens as number|null,saved_percent:v.saved_percent as number|null,main_metric_eligible:v.main_metric_eligible,
    provider_usage:{input:v.provider_usage.input as number|null,output:v.provider_usage.output as number|null,
      cached_input:v.provider_usage.cached_input as number|null,cache_write_input:cacheWrite,upstream_attempts:attempts},metadata:m};
}
function detail(v:Record<string,unknown>,period:MetricPeriod,origin:string):ActivityDetailSnapshot|null {
  const d=v.data;if(!utc(v.observed_at_utc)||!record(d)||d.period!==period||!Array.isArray(d.measurements)||!count(d.omitted_measurements))return null;
  const r=row(d.row,-Infinity,Date.parse(v.observed_at_utc));if(!r||r.origin_id!==origin||d.measurements.length!==Math.min(r.events,32)||d.omitted_measurements!==r.events-d.measurements.length)return null;
  const items:ActivityMeasurementDetail[]=[],ids=new Set();
  for(const raw of d.measurements){const m=measurement(raw,Date.parse(v.observed_at_utc));if(!m||ids.has(m.event_id)||!r.layers.includes(m.optimizer))return null;ids.add(m.event_id);items.push(m);}
  if(d.omitted_measurements===0&&items.filter(m=>m.main_metric_eligible).length!==r.totals.comparable_events)return null;
  return {schema_version:1,request_id:v.request_id as string,status:"ok",observed_at_utc:v.observed_at_utc,data:{period,row:r,measurements:items,omitted_measurements:d.omitted_measurements}};
}

export function createActivityClient(transport:MetricsTransport){
  async function call<T>(command:string,period:MetricPeriod,args:Record<string,unknown>,decode:(v:Record<string,unknown>)=>T|null):Promise<ActivityResult<T>>{
    try{
      if(!transport.isNative())return {kind:"unavailable"};
      const id=transport.createRequestId();if(!uuid(id))return {kind:"error",error:"request_failed"};
      if(!member(period,periods))return {kind:"error",error:"invalid_input"};
      const raw=await transport.invoke(command,{request:{schema_version:1,request_id:id,period,...args}});
      if(!record(raw)||raw.schema_version!==1||raw.request_id!==id)return {kind:"error",error:"invalid_response"};
      if(raw.status==="error")return member(raw.code,errors)&&typeof raw.retryable==="boolean"?{kind:"error",error:raw.code}:{kind:"error",error:"invalid_response"};
      if(raw.status!=="ok")return {kind:"error",error:"invalid_response"};
      const snapshot=decode(raw);return snapshot?{kind:"snapshot",snapshot}:{kind:"error",error:"invalid_response"};
    }catch{return {kind:"error",error:"request_failed"};}
  }
  return {
    read:async(period:MetricPeriod,filter:ActivityFilter):Promise<ActivityResult<ActivitySnapshot>>=>{
      const f=decodeActivityFilter(filter);return f?call("get_activity_snapshot",period,{filter:f},v=>list(v,period,f)):{kind:"error",error:"invalid_input"};
    },
    detail:async(period:MetricPeriod,origin:string):Promise<ActivityResult<ActivityDetailSnapshot>>=>isMetricHash(origin)
      ?call("get_activity_detail",period,{origin_id:origin},v=>detail(v,period,origin)):{kind:"error",error:"invalid_input"},
  };
}
export function createNativeActivityClient(){return createActivityClient({isNative:()=>isTauri(),createRequestId:()=>globalThis.crypto?.randomUUID?.()??null,invoke:(command,args)=>invoke(command,args)});}
