<a name="h024--kalıcı-metadata-ve-dönem-sorgusu"></a>

# H024 — Persistent metadata and period queries

November 4, 2026, Windows 10 22H2 x64. The lead agent owned persistent storage, measurement semantics, implementation, and acceptance. The existing Luna xhigh worker wrote only the weighted totals test file. One correction kept mixed bases separate even for overflow checks. The lead stopped the worker, checked its final state, and added boundary regressions. Development token savings and time remain unmeasured.

<a name="kapsam"></a>

## Scope

- The path starts at the H023 normalized event, uses a guarded SQLite transaction and upsert, and ends at a local period query through native IPC. The work opened no UI and started no live engine polling or model request. [Decision 012](../decisions/012-metrics-store.md), [API](../contracts.md#h024-kayıt-ve-sorgu-apisi)
- The store records each event_id once. It combines proven proxy and MCP replay sources and keeps conflicts unmeasured. It checks the origin and slot identity again. The first monitoring session and first-seen time stay the same. The monitoring session identifies a monitoring period, separate from account and chat identity.
- Queries cover today or 7 local calendar days including today. The store uses UTC and resolves local boundaries at query time. The native reply shows 30 days of metadata retention. Group details have a limit of 64. The store keeps different bases and components out of a combined percentage.
- Actual zero, missing values, negative differences, zero baseline, and JavaScript integer overflow stay distinct. Rolling, aggregate, unidentified, and untimed rows appear in coverage. Complete traffic coverage requires separate evidence.

<a name="doğrulama"></a>

## Verification

| Scenario | Observed result |
| --- | --- |
| Weighted total | Synthetic inputs of 1,000 to 500 and 9,000 to 8,100 yield a difference of 1,400 and savings of 14% in the fixture. Real user savings remain outside this result. |
| Replay, restart, and conflict | The same origin yields one record. Different runtime generations remain separate. A later valid replay cannot turn a conflict into a measured value. |
| Reopen in a separate process | One test process wrote to SQLite and exited with code 0. The second read the total from the same file. The new monitoring session had 0 events. The period total stayed at 1,400 and 14%. The fixture did not enter the real app state. |
| Local boundaries and coverage | The tests covered midnight at UTC+3 and 7 calendar days. They excluded future fractional timestamps and recalculated the selection when the local zone changed to UTC. Partial coverage stayed visible. |
| Persistent write errors | Checks rejected a foreign or future schema, hardlinks, and altered event_id and batch inputs. The tests covered busy state and an error during the second SQL write. The whole transaction rolled back and preserved existing data. |
| Retention and limits | Checks covered the 30-day threshold, exclusion of older replay, and cleanup on a later query. The store rejects files over 128 MiB before SQL. Load and performance need separate evidence. |
| Content-free legacy adapter | The actual H023 projection leaves the 2306/12 basis unknown. SQLite receives neither prompt nor authentication canaries. Lifetime aggregates and unidentified rows stay outside the savings total. |

The first RED run had 12 existing passes and 17 new failures. Then 29 focused tests passed. Review regressions failed before the fix. After the fix, 34 tests passed. Separate process evidence brought the focused count to 36 passes.

The team fixed build and API errors and a test fixture type error. It retained no raw error output.

The full local Rust gate ran once on the final source. **313 tests passed and 5 opt-in tests were ignored.** Rustfmt and the offline embedded build passed. `work/proofs/metrics-store/final.json` records 108 inputs before and after, including 107 sources, tests, and config inputs plus the verification helper. It also records tools and environment.

Frontend sources, tests, config, and distribution stayed the same. The team reused 98 frontend tests and earlier build evidence with matching inputs, Node, and pnpm.

<a name="gerçek-native-sorgu"></a>

## Native query

The test opened the new Headroom build twice. Each process used a separate WebView profile and an owned Job. The actual `get_metrics_summary` IPC returned the correct DTO for session, today, and seven_days. It showed the same in-process monitoring identity and retention of 30 days. Invalid schema returned a fixed error.

The second launch had a different identity. Both windows closed normally with WM_CLOSE and exit code 0. The Job processes and two debug listeners closed.

The normal Codex config, app preferences, ownership, onboarding, and metrics database state retained their hashes. The test started no engine or model.

Evidence is in `work/proofs/metrics-store/`. See `focused4.json`, `final.json`, `native-query.json`, the process fixture UUID directories, and final `acceptance.json`. Verification recorded the source, artifact, and environment fingerprints in the same step. The team retained no raw authentication data, conversations, or errors.

H024 backend acceptance is complete.

H025 adds the owned runtime metadata collector, polling and gap checks, and the actual screen connection. Live dashboard savings and tokenizer provenance for Headroom 0.41.0 remain unverified here. The native check queried an empty store. Separate process evidence used the production store code and a synthetic fixture for populated history. Windows 11, macOS, Linux, installation, power loss, and sustained performance remain untested. [Lead review](../reviews/H024.md)

<a name="19-kasım-2026--atlanan-yerel-gece-yarısı"></a>

## November 19, 2026 — Skipped local midnight

The lead agent used the same start of day in the shared Overview and Activity period query and in the daily chart. When clocks advance, the boundary moves to the first valid local second. The chart resolves the boundary once for each date.

The test wrote synthetic records to a real SQLite store on Windows 10 22H2 x64. A test simulated the September 8, 2024 clock change in Santiago. It checked the UTC 04:00 start for Today and Last 7 days, records on both sides of the boundary, and matching Activity results.

Two tests covered a clock jump of 30 minutes and 1 second within the week and a first date that was skipped entirely. Three tests failed with `InvalidLocalTime` before the fix. All three passed after the fix. The tests injected timezone transitions.

The final local gate passed `test:app`, the frontend build, `cargo fmt --check`, 476 Rust tests, and the `custom-protocol` build. The gate ignored 24 opt-in Rust tests. All 259 source, test, and config inputs matched during verification. The candidate input SHA-256 is `e27622a184702a6ead40e97df356580acbd27eb0cf92b9c2905b2bbdd6e5f4ca`.

The verification records are in `work/proofs/metrics-local-calendar/`. See `red-0f8caa6a-8b57-4646-bb6d-0425c36241be.json`, `focused-dfba4cda-20af-4cfa-a97f-e3bbf29ba685.json`, and `gate-c071872a-e0ae-4a72-aa2c-ac4efabca450.json`. The final report SHA-256 is `92341974f131cf367ab6677e256898b0fdb716f186a936f31bfe98d8381ff99f`. [Period contract](../decisions/012-metrics-store.md)

<a name="19-kasım-2026--uzun-oturum-grafiği"></a>

## November 19, 2026 — Long session chart

The test wrote synthetic measurements to a real SQLite store on Windows 10 22H2 x64 with 90 days of retention. It displayed 64 days in separate points. After the test added day 65, the chart kept 64 points and grouped the two oldest days. The period and chart matched for 65 records, 6,600 input tokens, 3,500 optimized input tokens, and a difference of 3,100. The test verified a difference of −50 for the first point and its weighted percentage.

A separate test wrote and reread a 119 to 50 record. Pinned serde_json 1.0.154 previously read the last bit of the stored ratio differently. SQLite validation rejected the record. With `float_roundtrip`, the store reads the valid record with its exact ratio. The chart and fractional ratio tests gave `Capacity` and `Corrupt`, respectively, before the fix. Both tests and the date range resolver test passed after the fix.

The real React chart displayed 64 bars in an isolated Edge run. The tooltip and screen-reader list showed the grouped date range in Turkish and English. The last daily point showed one date. The UI run used synthetic chart data.

The final local gate passed 225 frontend and 478 Rust tests, formatting, the frontend build, and the `custom-protocol` build. It ignored 24 opt-in Rust tests. The 259 source, test, and config inputs matched before and after with digest `8c949f63569d334936e8db3809b7b5ff59ac905ac16f49eaa72122a739bb974b`.

The verification files are under `work/proofs/metrics-long-session/`. Pre-fix `red-fde16b58-26c0-45d3-a360-97808dc42115.json`, focused `focused-9230138e-512d-4692-91d8-970e79afc8b4.json`, fractional ratio `roundtrip-e9795755-f04f-4e6c-b6cb-cadc93772eb2/result.json`, UI `ui-d2a7dcfe-bcfe-40f4-b0b0-7da459f594ad/result.json`, and the full gate `gate-41100360-1907-4f99-b3dc-43053b9ff53e.json`. The final gate report SHA-256 is `7fe7559a0cb2a633c64fb2d13def915466d3e3e351d936feabae82bd9d1de6ba`. [Chart contract](../contracts.md)
