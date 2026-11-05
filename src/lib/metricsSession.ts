import type { MetricPeriod, MetricsResult, OverviewMetricsState } from "./metrics.ts";

export interface OverviewReadState { period: MetricPeriod; metrics: OverviewMetricsState }

/** One local query at a time. Period changes invalidate replies, not native work. */
export function createOverviewSession(
  read: (period: MetricPeriod) => Promise<MetricsResult>,
  publish: (state: OverviewReadState) => void,
) {
  let period: MetricPeriod = "session";
  let metrics: OverviewMetricsState = { kind: "loading" };
  let active = false;
  let epoch = 0;
  let pending = false;
  let dirty = false;
  const emit = () => publish({ period, metrics });

  async function pump(): Promise<void> {
    if (!active || pending || !dirty) return;
    pending = true;
    dirty = false;
    const requested = period;
    const stamp = epoch;
    if (metrics.kind === "ready") { metrics = { ...metrics, refreshing: true }; emit(); }
    let result: MetricsResult;
    try { result = await read(requested); } catch { result = { kind: "request_failed" }; }
    if (active && stamp === epoch) {
      if (result.kind === "snapshot" && result.snapshot.data.period === period) {
        metrics = { kind: "ready", snapshot: result.snapshot, stale: false, issue: null, refreshing: false };
      } else if (result.kind === "desktop_unavailable") {
        metrics = { kind: "unavailable" };
      } else {
        const error = result.kind === "metrics_error" ? result.code
          : result.kind === "snapshot" ? "invalid_response" : result.kind;
        metrics = metrics.kind === "ready"
          ? { ...metrics, stale: true, issue: error, refreshing: false }
          : { kind: "error", error };
      }
      emit();
    }
    pending = false;
    if (active && dirty) void pump();
  }

  return {
    target(next: MetricPeriod, enabled: boolean) {
      if (next !== period || active !== enabled) epoch += 1;
      if (next !== period) { period = next; metrics = { kind: "loading" }; emit(); }
      active = enabled;
      dirty = enabled;
      if (enabled) void pump();
    },
    refresh() { if (active) { dirty = true; void pump(); } },
  };
}
