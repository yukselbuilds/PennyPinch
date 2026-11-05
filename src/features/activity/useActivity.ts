import {useEffect,useRef,useState} from "react";
import {createNativeActivityClient} from "../../lib/activityClient";
import {createActivitySession,type ActivityReadView} from "../../lib/activitySession";
import {defaultActivityFilter,type ActivityFilter} from "../../lib/activity";
import type {MetricPeriod} from "../../lib/metrics";

export function useActivity(active:boolean){
  const [period,setPeriod]=useState<MetricPeriod>("session");
  const [filter,setFilter]=useState<ActivityFilter>(defaultActivityFilter);
  const [view,setView]=useState<ActivityReadView>({period,filter,selectedId:null,state:{kind:"loading"},detail:{kind:"loading"}});
  const session=useRef<ReturnType<typeof createActivitySession>|null>(null);
  if(!session.current)session.current=createActivitySession(createNativeActivityClient(),setView);
  useEffect(()=>{
    session.current!.target(period,filter,active);
    if(!active)return;
    const timer=window.setInterval(()=>{if(document.visibilityState!=="hidden")session.current!.refresh();},5000);
    return ()=>{window.clearInterval(timer);session.current!.target(period,filter,false);};
  },[period,filter,active]);
  const current=view.period===period&&JSON.stringify(view.filter)===JSON.stringify(filter);
  return {period,filter,state:current?view.state:{kind:"loading" as const},selectedId:current?view.selectedId:null,
    detail:current?view.detail:{kind:"loading" as const},onPeriodChange:setPeriod,onFilterChange:setFilter,
    onRefresh:()=>session.current!.refresh(),onSelect:(id:string|null)=>session.current!.select(id)};
}
