export const runtimeRequestId='7132bced-3ce0-4c1b-8f48-d93d7b96f7ce';
export const runtimeGeneration='7132bced3ce04c1b8f48d93d7b96f7ce';
export function runtimeSnapshot(engine='ready',compression=engine==='ready'?'enabled':'unavailable',revision=compression==='enabled'||compression==='paused'?0:null){
 const owned=engine==='ready'||engine==='stopping';
 return {schema_version:1,request_id:runtimeRequestId,snapshot_id:'d0d48df0-ff19-423d-935b-3b555356e9a1',observed_at_utc:'2026-11-05T12:00:00Z',
  runtime_generation:owned?runtimeGeneration:null,engine_version:owned?'0.45.0':null,engine:{status:engine,error_code:null},compression:{status:compression,active_stream_count:null,revision},
  discovery:{status:'not_scanned',clients:[]},metrics:{status:'waiting',before_total:null,after_total:null,saved_total:null,saved_percent:null,request_count:null,covered_count:null},
  capabilities:{start:engine==='stopped'||engine==='failed',stop:owned,pause:engine==='ready'&&(compression==='enabled'||compression==='paused'),connect:false}};
}
