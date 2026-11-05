export const activityRequestId='7132bced-3ce0-4c1b-8f48-d93d7b96f7ce';
export const activityNow='2026-11-05T12:00:00Z';
export const hash=c=>c.repeat(64);
export const activityMetadata=()=>({total_ms:null,optimization_ms:null,first_token_ms:null,outcome:'unknown',cache_reported_hit:null,transforms_count:null,conflicted:false});
const totals=(saved=50,excluded=0)=>({status:'measured',comparable_events:1,excluded_events:excluded,before_tokens:100,after_tokens:100-saved,saved_tokens:saved,saved_percent:saved});
export function activityRows(){return [
 {origin_id:hash('a'),timestamp_utc:activityNow,identity:{model:'test-model-a',provider:'test-provider',client:'codex',upstream_id:hash('d'),agent:{id:hash('a'),parent_id:null,role:'main'},conflicted:false},
  metadata:{...activityMetadata(),total_ms:0,optimization_ms:0,outcome:'no_error_reported',cache_reported_hit:true,transforms_count:2},totals:totals(50,1),measurement:'measured',events:2,layers:['headroom','rtk']},
 {origin_id:hash('b'),timestamp_utc:'2026-11-05T11:59:00Z',identity:{model:'test-model-b',provider:'test-provider',client:'codex',upstream_id:hash('d'),agent:{id:hash('b'),parent_id:hash('a'),role:'subagent'},conflicted:false},
  metadata:{...activityMetadata(),total_ms:12.5,outcome:'error_reported',cache_reported_hit:false,transforms_count:0},totals:totals(-25),measurement:'measured',events:1,layers:['headroom']},
 {origin_id:hash('c'),timestamp_utc:null,identity:{model:null,provider:null,client:null,upstream_id:null,agent:null,conflicted:false},metadata:activityMetadata(),
  totals:{status:'no_comparable_events',comparable_events:0,excluded_events:1,before_tokens:null,after_tokens:null,saved_tokens:null,saved_percent:null},measurement:'unmeasurable',events:1,layers:['headroom']},
];}
export function activityMatches(r,f){return (f.client==='any'||(f.client==='unknown'?r.identity.client===null:r.identity.client===f.client))
 &&(f.model.kind==='any'||(f.model.kind==='unknown'?r.identity.model===null:r.identity.model===f.model.value))
 &&(f.outcome==='any'||r.metadata.outcome===f.outcome)&&(f.measurement==='any'||r.measurement===f.measurement);}
export function activityList(period='today',filter={client:'any',model:{kind:'any'},outcome:'any',measurement:'any'}){
 const rows=activityRows().filter(r=>activityMatches(r,filter));
 return {schema_version:1,request_id:activityRequestId,status:'ok',observed_at_utc:activityNow,
  data:{monitoring_session_id:'7132bced-3ce0-4c1b-8f48-d93d7b96f7cf',period,from_utc:period==='session'?null:period==='today'?'2026-11-05T00:00:00Z':'2026-10-30T00:00:00Z',to_utc:activityNow,retention_days:30,
   coverage:{events:4,distinct_origins:3,unplaced_events:1,rolling_window:true,aggregate_only:false,unidentified_rows_observed:false,complete_traffic:false},
   matched_requests:rows.length,shown_requests:rows.length,limit:100,has_more:false,unplaced_requests:rows.filter(r=>r.timestamp_utc===null).length,rows},
  collection:{status:'recording',interval_seconds:5,last_capture_utc:activityNow,runtime_generation:'7132bced3ce04c1b8f48d93d7b96f7d0',last_error:null,sampling_interrupted_since_start:false}};
}
export function activityDetail(period='today',origin=hash('a')){
 const row=activityRows().find(r=>r.origin_id===origin);if(!row)return {schema_version:1,request_id:activityRequestId,status:'error',code:'not_found',retryable:false};
 const reading=tokens=>({tokens,basis_kind:row.measurement==='unmeasurable'?'unknown':'local_estimate',basis_id:row.measurement==='unmeasurable'?null:hash('e'),component:'message_input'});
 const item={event_id:row.origin_id,timestamp_utc:row.timestamp_utc,observed_via:['proxy'],optimizer:'headroom',status:row.measurement==='unmeasurable'?'unknown_basis':'measured',
  before:reading(100),after:reading(row.totals.after_tokens??20),saved_tokens:row.totals.saved_tokens,saved_percent:row.totals.saved_percent,main_metric_eligible:row.measurement==='measured',
  provider_usage:{input:null,output:null,cached_input:null},metadata:row.metadata};
 const measurements=[item];if(origin===hash('a'))measurements.push({...item,event_id:hash('f'),optimizer:'rtk',after:reading(10),saved_tokens:90,saved_percent:90,main_metric_eligible:false,metadata:activityMetadata()});
 return {schema_version:1,request_id:activityRequestId,status:'ok',observed_at_utc:activityNow,data:{period,row,measurements,omitted_measurements:0}};
}
