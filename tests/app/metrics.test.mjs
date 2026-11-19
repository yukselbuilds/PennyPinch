import test from "node:test";
import assert from "node:assert/strict";
import {
  createMetricsClient,
  formatMetricNumber,
  formatMetricPercent,
} from "../../src/lib/metrics.ts";

const requestId = "7132bced-3ce0-4c1b-8f48-d93d7b96f7ce";
const sessionId = "7132bced-3ce0-4c1b-8f48-d93d7b96f7cf";
const generation = "7132bced3ce04c1b8f48d93d7b96f7d0";
const observedAt = "2026-11-04T12:00:00Z";
const startAt = "2026-11-04T00:00:00Z";

function totals(overrides = {}) {
  return {
    status: "measured",
    comparable_events: 1,
    excluded_events: 0,
    before_tokens: 100,
    after_tokens: 50,
    saved_tokens: 50,
    saved_percent: 50,
    ...overrides,
  };
}

function bucket(at, values = totals()) {
  return {
    at_utc: at,
    totals: {
      ...values,
      groups: [{ basis: "legacy-unused" }],
      omitted_basis_groups: 2,
    },
  };
}

function snapshot({
  id = requestId,
  period = "today",
  values = totals(),
  series = values.status === "measured" ? [bucket(observedAt, values)] : [],
  coverage = {},
  top = {},
  data = {},
  collection = {},
} = {}) {
  const eventCount = values.comparable_events + values.excluded_events;
  const origins=coverage.distinct_origins??eventCount;
  const counts={requests:origins,events:coverage.events??eventCount,model_observed:0,client_verified:0,agent_verified:0,conflicts:0};
  const attribution={scope:"observed_requests",coverage:counts,client_status:"unknown",agent_status:"unknown",groups:eventCount===0?[]:[{
    group_id:"a".repeat(64),model:null,provider:null,client:null,upstream_id:null,agent:null,conflicted:false,is_other:false,coverage:counts,totals:values,
  }],omitted_groups:0};
  return {
    schema_version: 1,
    request_id: id,
    status: "ok",
    observed_at_utc: observedAt,
    data: {
      monitoring_session_id: sessionId,
      period,
      from_utc: startAt,
      to_utc: observedAt,
      retention_days: 30,
      last_event_utc: eventCount > 0 ? observedAt : null,
      totals: { ...values, groups: [], omitted_basis_groups: 0 },
      attribution,
      series,
      coverage: {
        events: eventCount,
        distinct_origins: eventCount,
        unplaced_events: 0,
        rolling_window: false,
        aggregate_only: false,
        unidentified_rows_observed: false,
        complete_traffic: false,
        ...coverage,
      },
      ...data,
    },
    collection: {
      status: "recording",
      interval_seconds: 15,
      last_capture_utc: observedAt,
      runtime_generation: generation,
      last_error: null,
      sampling_interrupted_since_start: false,
      ...collection,
    },
    ...top,
  };
}

function metricsError(code = "busy", id = requestId) {
  return {
    schema_version: 1,
    request_id: id,
    status: "error",
    code,
    retryable: code === "busy",
  };
}

function client({ native = true, id = requestId, reply = snapshot(), invoke } = {}) {
  const calls = [];
  return {
    calls,
    metrics: createMetricsClient({
      isNative: () => native,
      createRequestId: () => id,
      invoke: invoke ?? (async (command, args) => {
        calls.push([command, args]);
        return reply;
      }),
    }),
  };
}

test("native read sends a correlated period request and returns a fresh UI snapshot", async () => {
  const source = snapshot();
  const { metrics, calls } = client({ reply: source });
  const result = await metrics.read("today");

  assert.deepEqual(calls, [["get_overview_snapshot", {
    request: { schema_version: 1, request_id: requestId, period: "today" },
  }]]);
  assert.equal(result.kind, "snapshot");
  assert.equal(result.snapshot.request_id, requestId);
  assert.equal(result.snapshot.data.period, "today");
  assert.equal("groups" in result.snapshot.data.totals, false);
  assert.equal("omitted_basis_groups" in result.snapshot.data.totals, false);
  assert.equal("groups" in result.snapshot.data.series[0].totals, false);
  assert.equal("omitted_basis_groups" in result.snapshot.data.series[0].totals, false);

  for (const reply of [
    snapshot({ id: "00000000-0000-4000-8000-000000000002" }),
    snapshot({ top: { schema_version: 2 } }),
    snapshot({ data: { period: "seven_days" } }),
  ]) {
    const invalid = client({ reply });
    assert.deepEqual(await invalid.metrics.read("today"), { kind: "invalid_response" });
  }
});

test("grouped session chart keeps the date range and rejects overlapping or invalid points", async () => {
  const start = "2026-09-01T00:00:00Z";
  const through = "2026-09-02T00:00:00Z";
  const grouped = bucket(start, totals({
    comparable_events: 2, before_tokens: 200, after_tokens: 100, saved_tokens: 100,
  }));
  const read = (throughUtc) => client({ reply: snapshot({
    period: "session",
    values: totals({ comparable_events: 3, before_tokens: 300, after_tokens: 150, saved_tokens: 150 }),
    series: [{ ...grouped, through_utc: throughUtc }, bucket(observedAt)],
    data: { from_utc: null, retention_days: 90 },
  }) }).metrics.read("session");

  const result = await read(through);
  assert.equal(result.kind, "snapshot");
  assert.equal(result.snapshot.data.series[0].through_utc, through);
  assert.equal(result.snapshot.data.series[1].through_utc, undefined);
  for (const invalid of [null, "not-a-date", start, observedAt, "2026-11-05T00:00:00Z"]) {
    assert.deepEqual(await read(invalid), { kind: "invalid_response" });
  }
});

test("browser mode is unavailable without invoking the desktop bridge", async () => {
  let invokes = 0;
  const { metrics, calls } = client({
    native: false,
    invoke: async () => { invokes += 1; return snapshot(); },
  });

  assert.deepEqual(await metrics.read("session"), { kind: "desktop_unavailable" });
  assert.equal(invokes, 0);
  assert.deepEqual(calls, []);
});

test("opted-in recommendations match measured scope and malformed advice does not hide valid metrics", async () => {
  const values = totals({ comparable_events: 40, after_tokens: 120, saved_tokens: -20, saved_percent: -20 });
  const advice = [
    { kind: "input_growth", basis: "local_estimate", component: "whole_input", before_tokens: 100, after_tokens: 120, events: 40 },
    { kind: "slow_optimization", p95_ms: 1001, requests: 20 },
    { kind: "cache_share_drop", model: "fixture-model", provider: "fixture-provider", previous_hits: 12,
      previous_requests: 20, recent_hits: 8, recent_requests: 20 },
    { kind: "installation_stale", client: "codex" },
  ];
  const { metrics, calls } = client({ reply: snapshot({ values, top: { recommendations: advice } }) });
  assert.deepEqual((await metrics.read("today")).snapshot.recommendations, []);
  assert.deepEqual((await metrics.read("today", true)).snapshot.recommendations, advice);
  assert.equal(calls[0][1].includeRecommendations, undefined);
  assert.equal(calls[1][1].includeRecommendations, true);
  for (const row of [
    { ...advice[0], after_tokens: 123 }, { ...advice[1], requests: 19 }, { ...advice[2], recent_hits: 9 },
    { ...advice[2], model: "sk-fixture-secret" }, { ...advice[3], client: "unknown" }, { ...advice[3], content: "canary" },
  ]) {
    const result = await client({ reply: snapshot({ values, top: { recommendations: [row] } }) }).metrics.read("today", true);
    assert.equal(result.kind, "snapshot");
    assert.deepEqual(result.snapshot.recommendations, []);
    assert.equal(result.snapshot.data.totals.saved_tokens, -20);
  }
  const stopped = await client({ reply: snapshot({ values, collection: { status: "stopped" }, top: { recommendations: [advice[1]] } }) }).metrics.read("today", true);
  assert.deepEqual(stopped.snapshot.recommendations, []);
});

test("only canonical non-nil request IDs are sent", async () => {
  for (const id of [null, "not-a-uuid", "00000000-0000-0000-0000-000000000000"]) {
    let invokes = 0;
    const { metrics } = client({ id, invoke: async () => { invokes += 1; return snapshot(); } });
    assert.deepEqual(await metrics.read("today"), { kind: "request_failed" });
    assert.equal(invokes, 0);
  }
});

test("invoke failures and thrown secrets become a fixed request failure", async () => {
  const { metrics } = client({
    invoke: async () => { throw new Error("fixture-secret C:\\private\\metrics.db"); },
  });
  const result = await metrics.read("today");
  assert.deepEqual(result, { kind: "request_failed" });
  assert.equal(JSON.stringify(result).includes("fixture-secret"), false);
  assert.equal(JSON.stringify(result).includes("metrics.db"), false);
});

test("fixed metric errors are returned as codes; unknown codes and mismatches are rejected", async () => {
  const { metrics } = client({ reply: metricsError("busy") });
  assert.deepEqual(await metrics.read("today"), { kind: "metrics_error", code: "busy" });

  for (const reply of [
    metricsError("fixture-secret"),
    metricsError("busy", "00000000-0000-4000-8000-000000000002"),
  ]) {
    const invalid = client({ reply });
    const result = await invalid.metrics.read("today");
    assert.deepEqual(result, { kind: "invalid_response" });
    assert.equal(JSON.stringify(result).includes("fixture-secret"), false);
  }
});

test("weighted totals match the measured buckets and the requested period", async () => {
  const first = totals({ before_tokens: 1_000, after_tokens: 500, saved_tokens: 500, saved_percent: 50 });
  const second = totals({ before_tokens: 9_000, after_tokens: 8_100, saved_tokens: 900, saved_percent: 10 });
  const weighted = totals({
    comparable_events: 2,
    before_tokens: 10_000,
    after_tokens: 8_600,
    saved_tokens: 1_400,
    saved_percent: 14,
  });
  const reply = snapshot({
    values: weighted,
    series: [bucket("2026-11-04T10:00:00Z", first), bucket(observedAt, second)],
    coverage: { events: 2, distinct_origins: 2 },
  });
  const { metrics } = client({ reply });
  const result = await metrics.read("today");
  assert.equal(result.kind, "snapshot");
  if (result.kind === "snapshot") {
    assert.equal(result.snapshot.data.totals.saved_percent, 14);
    assert.equal(result.snapshot.data.totals.saved_tokens, 1_400);
  }

  const wrongAverage = snapshot({
    values: totals({ ...weighted, saved_percent: 30 }),
    series: reply.data.series,
    coverage: { events: 2, distinct_origins: 2 },
  });
  const invalid = client({ reply: wrongAverage });
  assert.deepEqual(await invalid.metrics.read("today"), { kind: "invalid_response" });

  const wrongPeriod = client({ reply: snapshot({ period: "seven_days" }) });
  assert.deepEqual(await wrongPeriod.metrics.read("today"), { kind: "invalid_response" });
});

test("measured zero, zero baseline, and negative savings retain their meaning", async () => {
  const cases = [
    { id: "equal", values: totals({ before_tokens: 100, after_tokens: 100, saved_tokens: 0, saved_percent: 0 }), percent: 0 },
    { id: "zero-baseline", values: totals({ before_tokens: 0, after_tokens: 7, saved_tokens: -7, saved_percent: null }), percent: null },
    { id: "increase", values: totals({ before_tokens: 100, after_tokens: 150, saved_tokens: -50, saved_percent: -50 }), percent: -50 },
  ];
  for (const item of cases) {
    const { metrics } = client({ reply: snapshot({ values: item.values }) });
    const result = await metrics.read("today");
    assert.equal(result.kind, "snapshot", item.id);
    if (result.kind === "snapshot") {
      assert.equal(result.snapshot.data.totals.saved_percent, item.percent, item.id);
      assert.equal(result.snapshot.data.totals.saved_tokens, item.values.saved_tokens, item.id);
    }
  }
});

test("empty, mixed, and overflow periods have no aggregate values or chart buckets", async () => {
  const cases = [
    { status: "no_comparable_events", comparable_events: 0, excluded_events: 2 },
    { status: "mixed_bases", comparable_events: 3, excluded_events: 1 },
    { status: "counter_overflow", comparable_events: 2, excluded_events: 0 },
  ];
  for (const state of cases) {
    const values = totals({
      ...state,
      before_tokens: null,
      after_tokens: null,
      saved_tokens: null,
      saved_percent: null,
    });
    const reply = snapshot({ values, series: [], coverage: { events: values.comparable_events + values.excluded_events } });
    const { metrics } = client({ reply });
    const result = await metrics.read("today");
    assert.equal(result.kind, "snapshot", state.status);
    if (result.kind === "snapshot") {
      assert.equal(result.snapshot.data.totals.status, state.status);
      assert.equal(result.snapshot.data.totals.saved_tokens, null);
      assert.equal(result.snapshot.data.totals.saved_percent, null);
      assert.deepEqual(result.snapshot.data.series, []);
    }
  }
});

test("non-finite, negative, and unsafe native counters are rejected", async () => {
  const malformed = [
    snapshot({ values: totals({ saved_percent: Number.NaN }) }),
    snapshot({ values: totals({ after_tokens: Number.POSITIVE_INFINITY }) }),
    snapshot({ values: totals({ before_tokens: -1 }) }),
    snapshot({ values: totals({ before_tokens: Number.MAX_SAFE_INTEGER + 1 }) }),
    snapshot({ coverage: { events: 100_001 } }),
    snapshot({ data: { retention_days: 0 } }),
    snapshot({ top: { observed_at_utc: "not-a-utc-timestamp" } }),
    snapshot({ collection: { runtime_generation: "not-a-uuid" } }),
    snapshot({ series: [bucket("not-a-utc-timestamp")] }),
  ];
  for (const reply of malformed) {
    const { metrics } = client({ reply });
    assert.deepEqual(await metrics.read("today"), { kind: "invalid_response" });
  }
});

test("series must be bounded, ordered, unique, and equal the measured aggregate", async () => {
  const valid = snapshot();
  const reversed = snapshot({
    values: totals({ comparable_events: 2, before_tokens: 200, after_tokens: 100, saved_tokens: 100, saved_percent: 50 }),
    series: [bucket("2026-11-04T11:00:00Z"), bucket("2026-11-04T10:00:00Z")],
    coverage: { events: 2, distinct_origins: 2 },
  });
  const duplicate = snapshot({
    values: totals({ comparable_events: 2, before_tokens: 200, after_tokens: 100, saved_tokens: 100, saved_percent: 50 }),
    series: [bucket(observedAt), bucket(observedAt)],
    coverage: { events: 2, distinct_origins: 2 },
  });
  const mismatch = snapshot({ values: totals({ saved_tokens: 49, saved_percent: 49 }) });
  const tooMany = snapshot({
    values: totals({ comparable_events: 65, before_tokens: 6_500, after_tokens: 3_250, saved_tokens: 3_250, saved_percent: 50 }),
    series: Array.from({ length: 65 }, (_, index) => {
      const at = new Date(Date.parse(startAt) + (index + 1) * 60_000).toISOString().replace(".000Z", "Z");
      return bucket(at, totals({ before_tokens: 100, after_tokens: 50, saved_tokens: 50 }));
    }),
    coverage: { events: 65, distinct_origins: 65 },
  });

  for (const reply of [reversed, duplicate, mismatch, tooMany]) {
    const { metrics } = client({ reply });
    assert.deepEqual(await metrics.read("today"), { kind: "invalid_response"});
  }
  const { metrics } = client({ reply: valid });
  assert.equal((await metrics.read("today")).kind, "snapshot");
});

test("runtime keeps its 32 hex generation; monitoring session IDs use canonical UUIDs", async () => {
  const good = await client().metrics.read("today");
  assert.equal(good.kind,"snapshot");
  assert.equal(good.snapshot.collection.runtime_generation,generation);
  assert.equal(good.snapshot.data.monitoring_session_id,sessionId);
  for(const invalid of [requestId,"0".repeat(32),"x".repeat(32)]) {
    assert.equal((await client({reply:snapshot({collection:{runtime_generation:invalid}})}).metrics.read("today")).kind,"invalid_response");
  }
});

test("metric number and percent helpers keep Turkish and English locale forms", () => {
  assert.equal(formatMetricNumber(45_000, "tr"), "45.000");
  assert.equal(formatMetricNumber(45_000, "en"), "45,000");
  assert.equal(formatMetricNumber(0, "tr"), "0");
  assert.equal(formatMetricNumber(null, "tr"), "—");
  assert.equal(formatMetricPercent(37.5, "tr"), "%37,5");
  assert.equal(formatMetricPercent(37.5, "en"), "37.5%");
  assert.equal(formatMetricPercent(0, "en"), "0%");
  assert.equal(formatMetricPercent(-50, "en"), "-50%");
});
