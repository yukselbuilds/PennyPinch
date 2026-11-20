<a name="h045--yerel-öneriler"></a>

# H045 — Local recommendations

10 November 2026. H044 is waiting for a suitable RTK candidate. H045 relies on local evidence from H039, H024, H026, H034, and H041. The lead agent owns signal, timing, storage, and acceptance contracts. The single Luna xhigh worker's scope is fixed Overview presentation.

The feature is enabled in Settings and is off by default. The preference is added to the existing revision- and fingerprint-protected record. It is off when the field is absent from an older document. A legacy save request that omits the field preserves its current value. When Overview is active and the preference is on, the existing local metrics query also computes recommendations. Source rows or additional request content are not sent to the UI. A recommendation suggests a next step. Settings, model, route, and engine changes remain in the user's existing flows.

<a name="sinyal-sözleşmesi"></a>

## Signal contract

| Signal | Accepted evidence | Displayed scope |
| --- | --- | --- |
| Input increase | A `measured` total in the selected period with the same measurement basis and input component. Previous value is positive, next value is larger, and event times can be placed. | Count of recorded comparable events, before/after tokens, counter basis, period, and measurement time. |
| Local compression duration | At least 20 separate requests in the last 15 minutes for the recording engine generation, each with a positive `optimization_latency_ms`. Exclude conflicting and error-reported records. Nearest-rank p95 is at least 1,000 ms. | Engine-recorded compression duration, sample count, window, and time. |
| Drop in recorded cache hits | Same engine generation, model, provider, and, when evidenced, the same verified route. At least 20 separate `cache_hit` reports in each of the last 15 minutes and the preceding 15 minutes. Difference is at least 20 percentage points. Evaluate the nearest eligible model/provider group in the latest window. | Hit and record counts in both windows, model/provider, and measurement time. The 0.41.0 flag combines provider and local response-cache hits. |
| Stale installation check | H034 can no longer verify that the existing discovery record matches the file and returns `stale`. | Client name, local check time, and rescan action in Applications. |

Do not generate the relevant signal from empty, ambiguous, mixed, untimed, or stale snapshots. Numeric signals are limited to recorded samples. Measurement counts are not converted to total traffic or billing usage. Rows with fixed keys update under one heading. No notification, new background collection job, or automatic repair is used.

<a name="kabul"></a>

## Acceptance

1. The feature starts off with an isolated preference. When the user enables it, show the relevant row if measurement evidence is sufficient. Turning it off removes the row. The preference survives reopen. A legacy record, invalid value, or write conflict preserves the prior preference.
2. An input increase on a comparable basis, threshold-level latency, and cache samples appear with the correct scope. Empty, partial, conflicting, stale, and other-engine-generation records do not trigger the corresponding recommendation. New healthy samples and a rescan clear a stale recommendation.
3. Check TR/EN, keyboard, and narrow layout. Activity and Applications actions only navigate. Verify the native path and final quality gate with the local test engine/metrics store. Windows 11 and live-provider evidence remain outside the current acceptance.

Local acceptance completed on 10 November. The off value preserves the previous preference format. The on value contains the new preference field. Forward preference compatibility with an older binary is evaluated separately in production rollback acceptance. [H045 evidence](../proofs/recommendations.md)
