<a name="h023--içeriksiz-sayaç-normalizasyonu"></a>

# H023 — Content-free counter normalization

November 3, 2026. **Accepted. The product total was 20/51.** Under the new AGENTS instructions, the lead agent implemented the change and owned decisions on token measurement and provenance. The lead was the only writer. Luna had no assignment. [Decision 010](../decisions/010-metrics-normalization.md), [contract](../contracts.md), [review](../reviews/H023.md)

<a name="kabul-edilen-davranış"></a>

## Accepted behavior

| Scenario | Result | Evidence |
| --- | --- | --- |
| Explicit matching basis, 120,000 to 75,000 | 45,000 tokens and 37.5% savings in an arithmetic fixture. Live savings remain outside the result. | `metrics_comparable_reduction_zero_baseline_and_increase` |
| Actual zero, zero baseline, missing values, and increase | The test kept 0%, a null percentage, a null amount, and a negative difference distinct. | Same test and empty batch test |
| Different or unknown basis or component | The main amount and percentage remain unknown. The normalizer keeps the original observation counts. | Basis, component, and legacy regression tests |
| Numeric format of actual H021 rows | Rows 2306/12/saved0 and 12/12/saved0 have an unknown basis. They yield no derived percentage. | Git fixture and `review-boundaries.json`. The check matched the numeric sequences with H021 `traffic.json`. |
| Content-free projection | The output excludes request and response content, authentication, raw IDs, free-form tags, and false claims about basis or origin. It also excludes lifetime totals and supplied savings amounts and percentages. | Canary fixture, raw-origin tests, and limit tests |
| Verified proxy and MCP origin, and replay | The same measurement yields one event. MCP with no proven matching origin is ineligible for the main total. | `metrics_proxy_mcp_replay_deduplicates_only_proven_origin` |
| Conflicting replay | The amount and percentage become unknown. Ambiguous time and basis also become unknown. The conflict stays visible. | The initial regression failed. The fix passed. |
| Runtime restart and equivalent UTC time | The same ID from a different generation remains distinct. Different offsets for the same UTC instant merge. | Restart and equivalent UTC tests |
| Layer and provider counts | RTK, schema, novel, cache, and output counts stay outside the main input total. Provider usage remains separate. | Layer tests and review of novel input source. Only one of the three measurements for the same origin is eligible for the main total. |
| Aggregate, partial window, and row without identity | An aggregate does not become an event. The normalizer counts missing identities. Snapshot coverage stays partial. | Partial-window test and `complete_traffic=false` |
| Limits and invalid metadata | The bounds are 2 MiB and 10,000 observations. The checks require JSON-safe integers, UTC, and UUIDs. Invalid data returns a fixed error. | Limit, type, and identity tests |

The Rust check verified this path `bounded JSON → allowlist metadata → typed Observation → normalize → schema1 event/batch`. Use `recent_requests` when available. Otherwise use `request_logs`. Count only one list.

Headroom 0.41.0 observations without tokenizer and component provenance cannot establish comparability. A new collector with trusted evidence can supply an explicit basis. The API exposes no Tauri command for raw engine or renderer claims.

<a name="doğrulama-ve-fingerprint"></a>

## Verification and fingerprints

The records are in `work/proofs/metrics-normalization/`.

- `entry.json` records the canonical root, implementer decision, and initial source and config hashes. The six pinned engine files in the source note matched.
- `red.json` records the 8 expected failures before the behavior existed. The 8 tests passed in `focused-first.json`.
- `review-red.json` reproduces the time and basis error across three conflicting records. The fix and expanded 12 tests passed in `focused-review.json`. Sources matched before and after.
- `review-boundaries.json` shows that only the `main.rs` module line and Cargo's existing time parsing feature changed among H022 inputs. Cargo.lock and the other H022 sources stayed the same.
- `frontend-reuse.json` records 32 frontend source and config inputs and 3 distribution inputs that match the H022 evidence. Node 25.2.1, pnpm 11.25.0, and Windows 10 also match. **The team reused the 97 frontend results and build evidence.**
- `final-quality.json` records the single full Rust gate for the final candidate. Cargo formatting, **285 tests with 3 ignored**, and the offline custom-protocol build passed. Rust and Cargo were 1.92.0.

  Sources, tools, and environment matched before and after. The record contains 81 source, fixture, and config inputs and the binary hash. The build reported 6 existing warnings and 0 errors. The candidate hash also matched after QA.
- `acceptance.json` records scope, evidence hashes, and limits. The project launcher synced an up-to-date CodeGraph index. The graph result documents source discovery only.

The team reused passing checks across context compaction, the new day, and documentation closeout. Changes to tests or code required new checks only in the affected scope. The team retained no raw failures, HTTP content, or authentication data.

<a name="sınır-ve-sonraki-bağ"></a>

## Scope and next integration

This result covers the pure calculation and adapter. H023 introduced no app polling, persistent storage, or working dashboard. The UI stayed the same. The task did not repeat the desktop flow.

H024 tracks storage, upsert, periods, and gaps. H025 connects the screen. A live comparable token collector and the measurement basis of Headroom 0.41.0 remain unverified. A new collector must show its counting method, tokenizer, and component provenance. Earlier observations keep their existing scope.

The work preserved the model, authentication, routes, user settings, and live processes. Client, model, and subagent coverage stayed the same. It used no provider or model call or paid resource.

The Windows 10 embedded build covers its local environment. Other operating systems, installation, and sustained performance need separate evidence. Development token savings remain unmeasured. H008 Codex version eligibility, H012 external capabilities, and the H019 amber status remained open.
