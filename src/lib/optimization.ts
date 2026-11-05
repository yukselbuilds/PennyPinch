import type {EngineStatus,RuntimeSnapshot} from "../runtime/snapshot";
import type {PreferenceLanguage} from "./preferences";

export type RuntimeAction="start"|"stop"|"pause"|"resume";
export interface OptimizationView {
  phase:EngineStatus|"enabled"|"paused";
  primary:"start"|"pause"|"resume"|"refresh";
  pending:RuntimeAction|null;
  disabled:boolean;
  canStop:boolean;
  pauseUnavailable:boolean;
}
export function optimizationView(runtime:RuntimeSnapshot|null,pending:RuntimeAction|null):OptimizationView {
  const engine=runtime?.engine.status??"unknown";
  const control=runtime?.compression.status;
  const known=engine==="ready"&&(control==="enabled"||control==="paused");
  const phase=known?control:engine==="ready"?"unknown":engine;
  const primary=known?(control==="enabled"?"pause":"resume"):runtime?.capabilities.start?"start":"refresh";
  return {phase,primary,pending,disabled:pending!==null||engine==="starting"||engine==="stopping"||known&&!runtime!.capabilities.pause,
    canStop:Boolean(runtime?.capabilities.stop),pauseUnavailable:known&&control==="enabled"&&!runtime!.capabilities.pause};
}
export interface OptimizationControlProps {
  language:PreferenceLanguage;view:OptimizationView;
  onPrimary:()=>void;onStop:()=>void;
}
