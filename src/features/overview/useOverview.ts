import { useEffect, useRef, useState } from "react";
import { createNativeMetricsClient, type MetricPeriod, type OverviewConnection } from "../../lib/metrics";
import { createOverviewSession, type OverviewReadState } from "../../lib/metricsSession";
import { discoverySession, type ClientKind } from "../../lib/integrations";
import { connectionActionClient } from "../../lib/connectionActions";

export function useOverviewMetrics(period: MetricPeriod, active: boolean, showRecommendations: boolean) {
  const [state, setState] = useState<OverviewReadState>({ period, metrics: { kind: "loading" } });
  const session = useRef<ReturnType<typeof createOverviewSession> | null>(null);
  if (!session.current) session.current = createOverviewSession(createNativeMetricsClient().read, setState);
  useEffect(() => {
    session.current!.target(period, active, showRecommendations);
    if (!active) return;
    const timer = window.setInterval(() => session.current!.refresh(), 5000);
    return () => {
      window.clearInterval(timer);
      session.current!.target(period, false, showRecommendations);
    };
  }, [period, active, showRecommendations]);
  return {
    metrics: state.period === period ? state.metrics : { kind: "loading" as const },
    refresh: () => session.current!.refresh(),
  };
}

const clients: readonly ClientKind[] = ["codex", "claude_code", "opencode"];
export function useOverviewConnections(active: boolean) {
  const [connections, setConnections] = useState<OverviewConnection[]>([]);
  const [loading, setLoading] = useState(true);
  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    setLoading(true);
    void (async () => {
      const discovery = await discoverySession.loadInitial();
      const found = discovery.kind === "snapshot" && discovery.snapshot.status === "partial" ? discovery.snapshot.installations : null;
      const next: OverviewConnection[] = [];
      for (const client of clients) {
        if (cancelled) return;
        const result = await connectionActionClient.snapshot(client === "opencode" ? "opencode_v1" : client);
        const ownership = result.kind === "success" && result.reply.status === "snapshot" ? result.reply.ownership : undefined;
        next.push({ client, detected: found === null ? null : found.some(item => item.client_kind === client),
          state: ownership === undefined ? "unknown" : ownership?.state === "active" ? "saved" : "not_saved",
          scope: ownership?.scope ?? null });
      }
      if (!cancelled) { setConnections(next); setLoading(false); }
    })().catch(() => {
      if (!cancelled) { setConnections(clients.map(client => ({ client, detected: null, state: "unknown", scope: null }))); setLoading(false); }
    });
    return () => { cancelled = true; };
  }, [active]);
  return { connections, loading };
}
