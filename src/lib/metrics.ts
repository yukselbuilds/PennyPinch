import type { ClientKind } from "./integrations";
import type { PreferenceLanguage } from "./preferences";
import type { RuntimeSnapshot } from "../runtime/snapshot";
import { invoke, isTauri } from "@tauri-apps/api/core";

export type MetricPeriod = "session" | "today" | "seven_days";
export type MetricStatus = "measured" | "no_comparable_events" | "mixed_bases" | "counter_overflow";
export type MetricsErrorCode = "invalid_input" | "unsafe_path" | "busy" | "unsupported_schema" | "corrupt" | "capacity" | "unavailable" | "invalid_local_time";
export type CollectionErrorCode = "busy" | "unavailable" | "ownership_lost" | "transport" | "invalid_snapshot" | "storage";

export interface MetricTotals {
  status: MetricStatus;
  comparable_events: number;
  excluded_events: number;
  before_tokens: number | null;
  after_tokens: number | null;
  saved_tokens: number | null;
  saved_percent: number | null;
}

export interface MetricBucket {
  at_utc: string;
  totals: MetricTotals;
}

export type AttributionStatus = "unknown" | "partial" | "verified";
export interface AttributionCounts { requests:number; events:number; model_observed:number; client_verified:number; agent_verified:number; conflicts:number }
export interface AttributionAgent { id:string; parent_id:string|null; role:"unknown"|"main"|"subagent" }
export interface AttributionRow {
  group_id:string; model:string|null; provider:string|null; client:ClientKind|null; upstream_id:string|null; agent:AttributionAgent|null;
  conflicted:boolean; is_other:boolean; coverage:AttributionCounts; totals:MetricTotals;
}
export interface AttributionSummary {
  scope:"observed_requests"; coverage:AttributionCounts; client_status:AttributionStatus; agent_status:AttributionStatus;
  groups:AttributionRow[]; omitted_groups:number;
}

export function decodeAttributionSummary(value:unknown,events:number,origins:number,main:MetricTotals):AttributionSummary|null {
  const keys = ["requests","events","model_observed","client_verified","agent_verified","conflicts"] as const;
  const hash = (v:unknown):v is string => typeof v === "string" && /^[0-9a-f]{64}$/.test(v);
  const label = (v:unknown):v is string => typeof v === "string" && v.length>0 && v.length<=128 && /^[A-Za-z0-9._\-/:+@]+$/.test(v)
    && !v.includes(":/") && !/^(?:sk-|ghp_|github_pat_|unknown$)/i.test(v);
  function counts(v:unknown):AttributionCounts|null {
    if (!record(v) || keys.some(k=>!count(v[k]))) return null;
    const c = Object.fromEntries(keys.map(k=>[k,v[k]])) as unknown as AttributionCounts;
    if (c.requests>c.events || (c.requests===0)!==(c.events===0) || c.conflicts>c.requests
      || c.model_observed>c.requests-c.conflicts || c.client_verified>c.requests-c.conflicts || c.agent_verified>c.client_verified) return null;
    return c;
  }
  const level = (known:number,total:number):AttributionStatus => known===0?"unknown":known===total?"verified":"partial";
  if (!record(value) || value.scope!=="observed_requests" || !Array.isArray(value.groups) || value.groups.length>64 || !count(value.omitted_groups)) return null;
  const coverage=counts(value.coverage);
  if (!coverage || coverage.events!==events || coverage.requests!==origins || value.client_status!==level(coverage.client_verified,origins)
    || value.agent_status!==level(coverage.agent_verified,origins) || (origins===0)!==(value.groups.length===0)) return null;
  const sums:AttributionCounts={requests:0,events:0,model_observed:0,client_verified:0,agent_verified:0,conflicts:0};
  const groups:AttributionRow[]=[],ids=new Set<string>();let before=0,after=0,saved=0,comparable=0,excluded=0,others=0;
  for (const raw of value.groups) {
    if (!record(raw)||!hash(raw.group_id)||ids.has(raw.group_id)||typeof raw.is_other!=="boolean"||typeof raw.conflicted!=="boolean"
      || !(raw.model===null||label(raw.model)) || !(raw.provider===null||label(raw.provider))
      || !(raw.client===null||member(raw.client,["codex","claude_code","opencode"] as const))||!(raw.upstream_id===null||hash(raw.upstream_id))) return null;
    const c=counts(raw.coverage),part=totals(raw.totals);let agent:AttributionAgent|null=null;
    if (!c || c.requests===0 || !part || c.events!==part.comparable_events+part.excluded_events) return null;
    if (raw.agent!==null) {
      if (!record(raw.agent)||!hash(raw.agent.id)||!(raw.agent.parent_id===null||hash(raw.agent.parent_id))
        || raw.agent.parent_id===raw.agent.id||!member(raw.agent.role,["unknown","main","subagent"] as const)
        ||raw.agent.role==="main"&&raw.agent.parent_id!==null) return null;
      agent={id:raw.agent.id,parent_id:raw.agent.parent_id,role:raw.agent.role};
    }
    if (raw.is_other) {
      others+=1;
      if (others>1 || value.groups.length!==64 || groups.length!==63 || value.omitted_groups<2 || value.omitted_groups>c.requests
        || raw.model!==null||raw.provider!==null||raw.client!==null||raw.upstream_id!==null||agent!==null||raw.conflicted) return null;
    } else {
      if ((raw.client===null)!==(raw.upstream_id===null)||agent!==null&&raw.client===null
        || c.model_observed!==(raw.model===null?0:c.requests)||c.client_verified!==(raw.client===null?0:c.requests)
        || c.agent_verified!==(agent===null||agent.role==="unknown"?0:c.requests)||c.conflicts!==(raw.conflicted?c.requests:0)
        || raw.conflicted&&(raw.model!==null||raw.provider!==null||raw.client!==null||raw.upstream_id!==null||agent!==null)) return null;
    }
    ids.add(raw.group_id);for(const k of keys)sums[k]+=c[k];
    comparable+=part.comparable_events;excluded+=part.excluded_events;
    if (main.status==="measured" && part.comparable_events>0) {
      if (part.status!=="measured") return null;
      before+=part.before_tokens!;after+=part.after_tokens!;saved+=part.saved_tokens!;
    }
    groups.push({group_id:raw.group_id,model:raw.model,provider:raw.provider,client:raw.client,upstream_id:raw.upstream_id,
      agent,conflicted:raw.conflicted,is_other:raw.is_other,coverage:c,totals:part});
  }
  if (keys.some(k=>sums[k]!==coverage[k]) || comparable!==main.comparable_events||excluded!==main.excluded_events
    || (value.omitted_groups>0)!==(others===1) || main.status==="measured"&&(before!==main.before_tokens||after!==main.after_tokens||saved!==main.saved_tokens)) return null;
  return {scope:"observed_requests",coverage,client_status:value.client_status as AttributionStatus,agent_status:value.agent_status as AttributionStatus,groups,omitted_groups:value.omitted_groups};
}

export interface PeriodMetrics {
  attribution:AttributionSummary;
  monitoring_session_id: string;
  period: MetricPeriod;
  from_utc: string | null;
  to_utc: string;
  retention_days: number;
  last_event_utc: string | null;
  totals: MetricTotals;
  series: MetricBucket[];
  coverage: {
    events: number;
    distinct_origins: number;
    unplaced_events: number;
    rolling_window: boolean;
    aggregate_only: boolean;
    unidentified_rows_observed: boolean;
    complete_traffic: boolean;
  };
}

export interface OverviewSnapshot {
  schema_version: 1;
  request_id: string;
  status: "ok";
  observed_at_utc: string;
  data: PeriodMetrics;
  collection: {
    status: "waiting" | "recording" | "stopped" | "interrupted" | "unavailable";
    interval_seconds: number;
    last_capture_utc: string | null;
    runtime_generation: string | null;
    last_error: CollectionErrorCode | null;
    sampling_interrupted_since_start: boolean;
  };
}

export type MetricsUiError = MetricsErrorCode | "invalid_response" | "request_failed";
export type OverviewMetricsState =
  | { kind: "loading" }
  | { kind: "ready"; snapshot: OverviewSnapshot; stale: boolean; issue: MetricsUiError | null; refreshing: boolean }
  | { kind: "error"; error: MetricsUiError }
  | { kind: "unavailable" };

export interface OverviewConnection {
  client: ClientKind;
  detected: boolean | null;
  state: "saved" | "not_saved" | "unknown";
  scope: "user_config" | "project_local_config" | null;
}

export interface OverviewProps {
  language: PreferenceLanguage;
  runtime: RuntimeSnapshot | null;
  pending: "start" | "stop" | null;
  period: MetricPeriod;
  metrics: OverviewMetricsState;
  connections: readonly OverviewConnection[];
  connectionsLoading: boolean;
  onPeriodChange: (period: MetricPeriod) => void;
  onRefresh: () => void;
  onConnect: (client: ClientKind) => void;
  onOpenApplications: () => void;
  onStart: () => void;
  onStop: () => void;
}

export type MetricsResult =
  | { kind: "snapshot"; snapshot: OverviewSnapshot }
  | { kind: "metrics_error"; code: MetricsErrorCode }
  | { kind: "desktop_unavailable" }
  | { kind: "invalid_response" }
  | { kind: "request_failed" };

export interface MetricsTransport {
  isNative: () => boolean;
  createRequestId: () => string | null;
  invoke: (command: string, args: Record<string, unknown>) => Promise<unknown>;
}

const periods = ["session", "today", "seven_days"] as const;
const statuses = ["measured", "no_comparable_events", "mixed_bases", "counter_overflow"] as const;
const errors = ["invalid_input", "unsafe_path", "busy", "unsupported_schema", "corrupt", "capacity", "unavailable", "invalid_local_time"] as const;
const collectionErrors = ["busy", "unavailable", "ownership_lost", "transport", "invalid_snapshot", "storage"] as const;
const collectionStatuses = ["waiting", "recording", "stopped", "interrupted", "unavailable"] as const;
const record = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);
const member = <T extends string>(v: unknown, choices: readonly T[]): v is T => typeof v === "string" && choices.includes(v as T);
const count = (v: unknown): v is number => typeof v === "number" && Number.isSafeInteger(v) && v >= 0 && v <= 100_000;
const token = (v: unknown): v is number => typeof v === "number" && Number.isSafeInteger(v) && v >= 0;
const signedToken = (v: unknown): v is number => typeof v === "number" && Number.isSafeInteger(v);
const uuid = (v: unknown): v is string => typeof v === "string" && /^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/.test(v) && v !== "00000000-0000-0000-0000-000000000000";
const runtimeGeneration = (v: unknown): v is string => typeof v === "string" && /^[0-9a-f]{32}$/i.test(v) && v !== "0".repeat(32);
const utc = (v: unknown): v is string => typeof v === "string" && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?Z$/.test(v)
  && Number.isFinite(Date.parse(v)) && new Date(v).toISOString().slice(0, 19) === v.slice(0, 19);

function totals(value: unknown): MetricTotals | null {
  if (!record(value) || !member(value.status, statuses) || !count(value.comparable_events) || !count(value.excluded_events)) return null;
  const { before_tokens: before, after_tokens: after, saved_tokens: saved, saved_percent: percent } = value;
  if (value.status === "measured") {
    if (value.comparable_events === 0 || !token(before) || !token(after) || !signedToken(saved) || saved !== before - after) return null;
    if (before === 0 ? percent !== null : typeof percent !== "number" || !Number.isFinite(percent)
      || Math.abs(percent - 100 * saved / before) > 1e-9 * Math.max(1, Math.abs(percent))) return null;
  } else {
    if ([before, after, saved, percent].some(v => v !== null)) return null;
    if ((value.status === "no_comparable_events") !== (value.comparable_events === 0)) return null;
  }
  return { status: value.status, comparable_events: value.comparable_events, excluded_events: value.excluded_events,
    before_tokens: before as number | null, after_tokens: after as number | null,
    saved_tokens: saved as number | null, saved_percent: percent as number | null };
}

function snapshot(value: Record<string, unknown>, expectedPeriod: MetricPeriod): OverviewSnapshot | null {
  if (!utc(value.observed_at_utc) || !record(value.data) || !record(value.collection)) return null;
  const d = value.data, c = value.collection;
  if (!uuid(d.monitoring_session_id) || d.period !== expectedPeriod || !(d.from_utc === null || utc(d.from_utc)) || !utc(d.to_utc)
    || !(d.last_event_utc === null || utc(d.last_event_utc)) || !token(d.retention_days) || d.retention_days < 1 || d.retention_days > 365
    || !record(d.coverage) || !Array.isArray(d.series) || d.series.length > 64) return null;
  const end = Date.parse(d.to_utc), start = d.from_utc === null ? -Infinity : Date.parse(d.from_utc);
  if (start > end || end > Date.parse(value.observed_at_utc)) return null;
  if (d.last_event_utc !== null && (Date.parse(d.last_event_utc) > end || Date.parse(d.last_event_utc) < start)) return null;
  const main = totals(d.totals), coverage = d.coverage;
  if (!main || !count(coverage.events) || !count(coverage.distinct_origins) || !count(coverage.unplaced_events)
    || coverage.distinct_origins > coverage.events || coverage.unplaced_events > coverage.events
    || main.comparable_events + main.excluded_events !== coverage.events || coverage.complete_traffic !== false
    || [coverage.rolling_window, coverage.aggregate_only, coverage.unidentified_rows_observed].some(v => typeof v !== "boolean")) return null;
  const attribution=decodeAttributionSummary(d.attribution,coverage.events,coverage.distinct_origins,main);
  if (!attribution) return null;
  if (!member(c.status, collectionStatuses) || !token(c.interval_seconds) || c.interval_seconds < 1 || c.interval_seconds > 60
    || !(c.last_capture_utc === null || utc(c.last_capture_utc)) || !(c.runtime_generation === null || runtimeGeneration(c.runtime_generation))
    || !(c.last_error === null || member(c.last_error, collectionErrors)) || typeof c.sampling_interrupted_since_start !== "boolean") return null;
  const series: MetricBucket[] = [];
  let previous = -Infinity, before = 0, after = 0, saved = 0, measured = 0;
  for (const raw of d.series) {
    if (!record(raw) || !utc(raw.at_utc)) return null;
    const at = Date.parse(raw.at_utc), part = totals(raw.totals);
    if (at < start || at > end || at <= previous || !part || part.status !== "measured") return null;
    previous = at;
    before += part.before_tokens!; after += part.after_tokens!; saved += part.saved_tokens!; measured += part.comparable_events;
    if (!token(before) || !token(after) || !signedToken(saved) || !count(measured)) return null;
    series.push({ at_utc: raw.at_utc, totals: part });
  }
  if (main.status === "measured" ? before !== main.before_tokens || after !== main.after_tokens || saved !== main.saved_tokens || measured !== main.comparable_events : series.length !== 0) return null;
  return { schema_version: 1, request_id: value.request_id as string, status: "ok", observed_at_utc: value.observed_at_utc,
    data: { monitoring_session_id: d.monitoring_session_id, period: expectedPeriod, from_utc: d.from_utc, to_utc: d.to_utc,
      last_event_utc: d.last_event_utc, retention_days: d.retention_days, totals: main, series,attribution,
      coverage: { events: coverage.events, distinct_origins: coverage.distinct_origins, unplaced_events: coverage.unplaced_events,
        rolling_window: coverage.rolling_window as boolean, aggregate_only: coverage.aggregate_only as boolean,
        unidentified_rows_observed: coverage.unidentified_rows_observed as boolean, complete_traffic: false } },
    collection: { status: c.status, interval_seconds: c.interval_seconds, last_capture_utc: c.last_capture_utc,
      runtime_generation: c.runtime_generation, last_error: c.last_error, sampling_interrupted_since_start: c.sampling_interrupted_since_start } };
}

export function createMetricsClient(transport: MetricsTransport): { read: (period: MetricPeriod) => Promise<MetricsResult> } {
  return { read: async period => {
    try {
      if (!transport.isNative()) return { kind: "desktop_unavailable" };
      const requestId = transport.createRequestId();
      if (!uuid(requestId)) return { kind: "request_failed" };
      if (!member(period, periods)) return { kind: "invalid_response" };
      const raw = await transport.invoke("get_overview_snapshot", { request: { schema_version: 1, request_id: requestId, period } });
      if (!record(raw) || raw.schema_version !== 1 || raw.request_id !== requestId) return { kind: "invalid_response" };
      if (raw.status === "error") return member(raw.code, errors) && typeof raw.retryable === "boolean"
        ? { kind: "metrics_error", code: raw.code } : { kind: "invalid_response" };
      if (raw.status !== "ok") return { kind: "invalid_response" };
      const parsed = snapshot(raw, period);
      return parsed ? { kind: "snapshot", snapshot: parsed } : { kind: "invalid_response" };
    } catch { return { kind: "request_failed" }; }
  } };
}

export function createNativeMetricsClient(): ReturnType<typeof createMetricsClient> {
  return createMetricsClient({ isNative: () => isTauri(), createRequestId: () => globalThis.crypto?.randomUUID?.() ?? null,
    invoke: (command, args) => invoke(command, args) });
}

export function formatMetricNumber(value: number | null, language: PreferenceLanguage): string {
  return value === null || !Number.isFinite(value) ? "—" : new Intl.NumberFormat(language === "tr" ? "tr-TR" : "en-US", { maximumFractionDigits: 0 }).format(value);
}

export function formatMetricPercent(value: number | null, language: PreferenceLanguage): string {
  return value === null || !Number.isFinite(value) ? "—" : new Intl.NumberFormat(language === "tr" ? "tr-TR" : "en-US", { style: "percent", maximumFractionDigits: 1 }).format(value / 100);
}
