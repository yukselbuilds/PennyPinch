<a name="h027--içeriksiz-etkinlik-kabulü"></a>

# H027 — Content-free activity acceptance

5 November 2026. Decision [015](../decisions/015-activity.md), lead review [H027](../reviews/H027.md). H033 file/CSV export is outside this task. The tests used content canaries for the serializable list/detail projection.

| Scenario | Expected and observed result | Evidence |
| --- | --- | --- |
| Source metadata | The parser counts each primary record once. It adds secondary metadata only when identity and timestamp match and the model does not conflict. It preserves 0 and bounded duration. Missing, negative, and excessive durations remain unknown. Time to first token remains unknown. The parser drops raw error, transform, and content fields. The first tests found 2 behavior failures. The tests passed after the fix. | `work/proofs/activity/metadata-red.json`, `metadata-green-2.json`, final candidate Rust tests |
| Persistence and query | Replay, enrichment, and persistent metadata conflicts leave token values unchanged. The app reads old schema 1 payloads after reopen. The monitoring session, local periods, and retention match Overview. The store filters before it applies the limit of 100 rows. Unknown times sort last. Detail shows up to 32 measurements and an omitted count. Totals include all events. | `metrics-green-2.json`, final candidate `activity_tests` |
| Sensitive fields and client | The decoder rejects model/personal path, unsafe ID, wrong correlation/period, malformed counter/percentage. Export-safe DTO drops unknown secret/body/agent/tokenizer fields. A late list/detail reply does not enter a new context. | `node-focused-1.json`, final candidate133 frontend PASS |
| Real owned HTTP chain | Metadata fixture passed the loopback peer/generation boundary and reached SQLite → manager list/detail. Replay stayed one event. No raw content/secret appeared. Legacy token basis stayed unknown. | Final candidate `owned_http_metadata_reaches_sqlite_without_payload_or_replay_inflation` |
| Local product gate | `pnpm test:app`, `pnpm build`, `cargo fmt --check`, offline Rust test, and custom-protocol build. Result: 133 frontend/344 Rust PASS, 5 existing opt-in ignored. | `work/proofs/activity/final-1.json` |
| Real desktop | Native strict client checked all three periods against the shared Overview totals. Not-found/invalid-input produced fixed errors. The owned pinned collector recorded. Real source: 0 requests, 0 model calls. | `ui-runs/0c6068a4-27ad-479d-85bf-c1202b7f912c/result.json` |
| Populated UI fixture | Real Activity hook/client with isolated core mock: filter/exact validation, 0/unknown/negative, late detail, stale/recovery, metadata conflict, limit100, empty/error/unavailable, Enter/Space/Escape/focus. TR/EN, dark/light, and 1280/1024/819/683 views had no overflow. Native125/150/100 zoom passed. | Same native run, `synthetic-*.png`, real empty screen `native-activity.png` |

`work/proofs/activity/acceptance.json` programmatically compares the final gate's 142 source/config and tool/environment fingerprints with the current EXE/dist/fixture/runner and pinned models/server/outcome sources. It does not report PASS unless before/after and current candidate match.

The window exited with code 0. The owned debug port closed. Existing Codex config and app preferences/onboarding/ownership did not change. No raw error/provider/HTTP dump was kept. A failed test kept fixed classes/markers and only test-owned screenshots.

This is local development acceptance on Windows 10 22H2/build19045. It is not evidence for Windows 11/macOS/Linux, installer, long load, or live-provider savings.

TTFT stays unknown without a supporting source. A null error field is not success. A cache flag is not a discount. Real user/development savings percentages were not measured. H028/H033 did not start. H041 live API-key acceptance remains open.
