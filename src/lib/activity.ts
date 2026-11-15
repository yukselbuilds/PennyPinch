import type { ClientKind } from "./integrations";
import type { PreferenceLanguage } from "./preferences";
import type { AttributionAgent, MetricPeriod, MetricTotals, OverviewSnapshot, PeriodMetrics, MetricsUiError } from "./metrics";

export type ActivityOutcome = "unknown" | "no_error_reported" | "error_reported";
export type ActivityMeasurement = "measured" | "unmeasurable" | "conflict";
export type ActivityLayer = "headroom" | "rtk" | "cache" | "output";
export interface RequestMetadata {
  total_ms:number|null; optimization_ms:number|null; first_token_ms:number|null;
  outcome:ActivityOutcome; cache_reported_hit:boolean|null; transforms_count:number|null; conflicted:boolean;
}
export interface ActivityIdentity {
  model:string|null; provider:string|null; client:ClientKind|null; upstream_id:string|null;
  agent:AttributionAgent|null; conflicted:boolean;
}
export interface ActivityRow {
  origin_id:string; timestamp_utc:string|null; identity:ActivityIdentity; metadata:RequestMetadata;
  totals:MetricTotals; measurement:ActivityMeasurement; events:number; layers:ActivityLayer[];
}
export interface ActivityFilter {
  client:"any"|"unknown"|ClientKind;
  model:{kind:"any"}|{kind:"unknown"}|{kind:"exact";value:string};
  outcome:"any"|ActivityOutcome;
  measurement:"any"|ActivityMeasurement;
}
export const defaultActivityFilter:ActivityFilter = {client:"any",model:{kind:"any"},outcome:"any",measurement:"any"};
export interface ActivityData {
  monitoring_session_id:string; period:MetricPeriod; from_utc:string|null; to_utc:string; retention_days:number;
  coverage:PeriodMetrics["coverage"]; matched_requests:number; shown_requests:number; limit:100; has_more:boolean;
  unplaced_requests:number; rows:ActivityRow[];
}
export interface ActivitySnapshot {
  schema_version:1; request_id:string; status:"ok"; observed_at_utc:string;
  data:ActivityData; collection:OverviewSnapshot["collection"];
}
export type MeasurementStatus = "measured"|"missing_counter"|"unknown_basis"|"incomparable"|"uncorrelated"|"missing_timestamp"|"conflict";
export interface ActivityReading {
  tokens:number|null; basis_kind:"local_estimate"|"provider_tokenizer"|"unknown"; basis_id:string|null;
  component:"message_input"|"whole_input"|"tool_schema"|"novel_input"|"cache_input"|"output"|"unknown";
}
export interface ActivityMeasurementDetail {
  event_id:string; timestamp_utc:string|null; observed_via:("proxy"|"mcp")[]; optimizer:ActivityLayer; status:MeasurementStatus;
  before:ActivityReading; after:ActivityReading; saved_tokens:number|null; saved_percent:number|null;
  main_metric_eligible:boolean;
  provider_usage:{input:number|null;output:number|null;cached_input:number|null;cache_write_input:number|null;upstream_attempts:number|null};
  metadata:RequestMetadata;
}
export interface ActivityDetailSnapshot {
  schema_version:1; request_id:string; status:"ok"; observed_at_utc:string;
  data:{period:MetricPeriod; row:ActivityRow; measurements:ActivityMeasurementDetail[]; omitted_measurements:number};
}
export type ActivityError = MetricsUiError|"not_found";
export type ActivityResult<T> = {kind:"snapshot";snapshot:T}|{kind:"error";error:ActivityError}|{kind:"unavailable"};
export type ActivityState<T> = {kind:"loading"}|{kind:"ready";snapshot:T;stale:boolean;refreshing:boolean;issue:ActivityError|null}
  |{kind:"error";error:ActivityError}|{kind:"unavailable"};
export interface ActivityProps {
  language:PreferenceLanguage; period:MetricPeriod; filter:ActivityFilter; state:ActivityState<ActivitySnapshot>;
  selectedId:string|null; detail:ActivityState<ActivityDetailSnapshot>;
  onPeriodChange:(period:MetricPeriod)=>void; onFilterChange:(filter:ActivityFilter)=>void; onRefresh:()=>void;
  onSelect:(originId:string|null)=>void;
}
