<a name="015--içeriksiz-etkinlik-ve-ayrıntı"></a>

# 015 — Content-free activity and details

5 November 2026. H027 is complete. 25/51. The lead agent owns source/metadata, persistent compatibility, query/parser, and acceptance. The single Luna worker wrote the presentation with fixed props. It was verified stopped before being handed off. H028/H033 and account/chat tasks do not start. [Acceptance](../proofs/activity.md).

<a name="kaynak-ve-semantik"></a>

## Source and semantics

Pinned 0.41.0 RequestLog contains total_latency_ms (request wall-clock duration), optimization_latency_ms, cache_hit, transforms_applied, and nullable error. The 25-row recent_requests list has time/transform. Error/cache appear only in the last 10 request_logs rows. Keep the primary recent list as the event source. Enrich it with secondary metadata only when request_id + timestamp match and model does not conflict. Do not count the two lists twice. RequestLog does not publish time-to-first-token. Do not fill it from raw tags or an unnamed TTFT field.

Ignore error text during deserialization. Convert only the field's unknown/null/non-empty state to a fixed enum. A null/empty error record is not evidence of success or a completed stream. cache_hit is only a source-reported flag, not a provider cache-read discount. Transform names are free-form text and are not stored. Retain only a bounded count. A layer record or reported transformation is not evidence of the full app configuration or RTK installation. Raw error/command/header/body/personal path/conversation or raw request IDs do not appear in the list, details, or serializable data projection.

<a name="uygulama-ve-sınırlar"></a>

## Implementation and limits

- Add default-unknown request metadata to observations/events. Continue reading schema 1 payloads. Durations must be finite and within 0..86,400,000 ms, and transform counts within 0..128. Validate native typed bounds. Raw invalid durations remain unknown. Metadata enrichment/conflict absorption is separate from token and attribution calculation. SQL schema/pin/retention/transaction boundaries do not change.
- H024 period selection uses one shared helper. Overview and Activity use the same session/local-day/7-day and 30-day retention semantics. Records without time are not assigned to a period. The ambiguous-coverage count is visible. Layers from the same origin are one request record. Uncorrelated sources are not counted as physical calls.
- Read-only list/detail native IPC accepts only canonical UUID requests, typed period/filter, and opaque origin. Filters cover client/unknown, exact/unknown model, error-reported state, and measurement status. Apply filters to all selected records, then return the newest 100 matches. Show matched/limit/has_more explicitly. Do not add infinite list/pagination. Details contain at most 32 measurements plus an omitted count. All measurements remain part of main aggregation. No arbitrary path/SQL/export writes.
- List/detail DTOs contain only allowlisted metadata and the same weighted MetricSummary. An opaque basis identity may be shown instead of tokenizer name. Raw tokenizer text is not exposed. Model/client/agent retain H026's evidence limits. Do not aggregate percentages across different tokenizers/layers. An unknown percentage and 0% are separate states.
- Activity is a compact Graphite list with filters and keyboard-opened modal details. Native request correlation and latest-query/latest-selection checks prevent late responses from applying to a new filter or different request. An error for the same query may retain the last view as stale. The UI refreshes its local query every 5 seconds. It does not start the engine/model. File export belongs to H033. This task establishes its safe-serializable data boundary and canary checks.

<a name="kabul"></a>

## Acceptance

1. Recent/log matching, duration 0/missing/negative/large values, unknown TTFT/cache, fixed error enum, metadata replay/conflict, old payload/reopen, and canary cleanup. Do not infer client/agent from model.
2. Same period/filter, correct filtering before the 100 limit, stable newest-first order/unknown time, opaque wrong ID/not_found, and 32-detail limit. No secrets/content/raw logs in measurement or JSON projections.
3. Real owned collector→SQLite→native list/detail/read-only UI. Populated rows have separate isolated fixture evidence. Filter/late response/keyboard modal/locale/narrow view/native zoom. Focused RED/GREEN, one integrated root review, full local gate on the final candidate, and current native acceptance. Stop when H027 closes. H028 does not start.
