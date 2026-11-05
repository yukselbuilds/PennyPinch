import type {MetricPeriod} from "./metrics.ts";
import {defaultActivityFilter} from "./activity.ts";
import type {ActivityDetailSnapshot,ActivityFilter,ActivityResult,ActivitySnapshot,ActivityState} from "./activity.ts";

/** One in-flight query per channel, with the latest requested scope winning. */
function channel<Q,T>(read:(q:Q)=>Promise<ActivityResult<T>>,publish:(state:ActivityState<T>)=>void){
  let query:Q|null=null,key:string|null=null,epoch=0,pending=false,dirty=false;
  let state:ActivityState<T>={kind:"loading"};
  async function pump():Promise<void>{
    if(query===null||pending||!dirty)return;
    const q=query,stamp=epoch;pending=true;dirty=false;
    if(state.kind==="ready"){state={...state,refreshing:true};publish(state);}
    let result:ActivityResult<T>;
    try{result=await read(q);}catch{result={kind:"error",error:"request_failed"};}
    if(query!==null&&stamp===epoch){
      state=result.kind==="snapshot"?{kind:"ready",snapshot:result.snapshot,stale:false,refreshing:false,issue:null}
        :result.kind==="unavailable"?{kind:"unavailable"}:state.kind==="ready"?{...state,stale:true,refreshing:false,issue:result.error}:result;
      publish(state);
    }
    pending=false;if(dirty)void pump();
  }
  return {
    target(next:Q|null){
      const nextKey=next===null?null:JSON.stringify(next);
      if(key!==nextKey){epoch+=1;key=nextKey;state={kind:"loading"};publish(state);}
      query=next;dirty=next!==null;if(dirty)void pump();
    },
    refresh(){if(query!==null){dirty=true;void pump();}},
  };
}
export interface ActivityReadView {
  period:MetricPeriod;filter:ActivityFilter;selectedId:string|null;
  state:ActivityState<ActivitySnapshot>;detail:ActivityState<ActivityDetailSnapshot>;
}
export function createActivitySession(
  client:{read:(p:MetricPeriod,f:ActivityFilter)=>Promise<ActivityResult<ActivitySnapshot>>;detail:(p:MetricPeriod,id:string)=>Promise<ActivityResult<ActivityDetailSnapshot>>},
  publish:(view:ActivityReadView)=>void,
){
  let view:ActivityReadView={period:"session",filter:defaultActivityFilter,selectedId:null,state:{kind:"loading"},detail:{kind:"loading"}};
  let active=false;
  const emit=()=>publish({...view});
  const list=channel((q:{period:MetricPeriod;filter:ActivityFilter})=>client.read(q.period,q.filter),state=>{view={...view,state};emit();});
  const detail=channel((q:{period:MetricPeriod;id:string})=>client.detail(q.period,q.id),state=>{view={...view,detail:state};emit();});
  return {
    target(period:MetricPeriod,filter:ActivityFilter,enabled:boolean){
      const changed=period!==view.period||JSON.stringify(filter)!==JSON.stringify(view.filter)||enabled!==active;
      active=enabled;
      if(changed){view={...view,period,filter,selectedId:null,detail:{kind:"loading"}};detail.target(null);emit();}
      list.target(enabled?{period,filter}:null);
    },
    select(id:string|null){
      if(!active||id===view.selectedId)return;
      if(id!==null&&(view.state.kind!=="ready"||!view.state.snapshot.data.rows.some(r=>r.origin_id===id)))return;
      view={...view,selectedId:id,detail:{kind:"loading"}};emit();detail.target(id===null?null:{period:view.period,id});
    },
    refresh(){list.refresh();if(view.selectedId!==null)detail.refresh();},
  };
}
