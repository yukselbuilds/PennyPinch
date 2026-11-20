<a name="012--yerel-metrik-deposu-ve-dönem-sorgusu"></a>

# 012 — Local metrics store and period query

The H031 addendum dated 7 November records a user preference of 7/30/90 days while retaining the default of 30. SQLite schema 2 migrates schema 1 while preserving measurements. One cutoff prevents old dated replays from returning when retention is extended. The initial H024 acceptance below is historical. The current decision is [019](019-operating-preferences.md).

4 November 2026. The lead agent implements H024 and owns persistent storage and measurement semantics. The single Luna xhigh worker writes only unit tests for the fixed weighted-total contract.

<a name="kapsam-ve-sınırlar"></a>

## Scope and limits

- H023 schema 1 normalized events are stored locally. A read-only native query returns period totals for the H025 screen. A live collector/polling, real model request, UI, model/agent attribution, and account/chat list are outside this task. Uncertainty about the basis in source metadata is not upgraded.
- SQLite transactions/upserts are used. The store uses an event_id key and time index instead of a JSON store that rewrites all history on every poll. `rusqlite=0.41.1`, bundled SQLite, and `chrono=0.4.45` already in the lockfile are pinned directly. A system SQLite installation is not required. [rusqlite](https://docs.rs/rusqlite/0.41.1/rusqlite/) and [package metadata](https://crates.io/crates/rusqlite/0.41.1) were checked on 4 November. MSRV 1.89 is compatible with the current Rust 1.92. Cargo.lock records checksums for downloaded packages.
- Records contain only H023's bounded numeric/provenance metadata. Prompt, response, auth, file path, URL, and free-form engine tags are not read. The UI receives no event-write/import command. Persistent writes remain within the crate's normalization/collector boundary.
- Use an app-owned metrics directory. The constructor and empty query do not create files. Existing reparse/link/multi-hardlink files, foreign/database schemas, and malformed records produce a fixed error. There is no silent reset/overwrite. Directory pins use the existing no-delete protection. SQLite transactions roll back partial writes. Files owned by another app are not claimed.

<a name="kimlik-dönem-ve-saklama"></a>

## Identity, periods, and retention

- `event_id` is the upsert key. The same `origin_id` links layers for a request. H023's duplicate/conflict merge is shared with the store. Identical source measurements remain once. A conflict becomes unknown and is not restored to measured by a later good replay.
- A session here is this app process's **measurement session**, not the client's chat or account session. Replay/reopen does not change the session of the first record. Engine generation remains on the event. The same request in another generation is a new event. Reopening the app starts a new measurement session while preserving today's/7 days' history.
- Today runs from the start of today in the operating system's local calendar through now. The last 7 days include today and the previous six local calendar days. Records are stored in UTC. Period boundaries and daily chart queries use the same local day start. For an ambiguous midnight, choose the earlier instant. If midnight or a full day is skipped, use the first valid local instant. If the timezone boundary cannot be resolved, the query returns an error. Future-dated records are not included in period totals.
- Default retention is **30 days**. Old metadata is removed in-transaction based on UTC age. For records without a timestamp, use the first-seen time. Retention duration is visible in query responses. The settings screen is part of H025/later settings work. No automatic disk recovery or secure forensic deletion is claimed.
- Store limits are explicit. The store accepts at most 100,000 events, a 128MiB SQLite file, and the H023 batch limit. Capacity/permission/malformed-schema errors roll back the transaction. Partial success or incomplete data is not returned as if it were all traffic. Load/long-duration performance acceptance belongs to H035/H041.

<a name="toplam-ve-kapsam"></a>

## Totals and coverage

- Only main_metric_eligible Headroom input pairs are summed, and only within the same counter kind, tokenizer identity, and component. Multiple bases/components remain separate groups. The combined primary number is unknown. Provider usage, RTK/cache/output are not added to primary input.
- At most 64 group details are returned. The remaining count appears in `omitted_basis_groups`. Different bases are never summed into one number, even to check for overflow. Mixed_bases has precedence. JS safe-integer overflow within one group is a separate counter_overflow.
- The weighted rate is `100 × (ΣB − ΣA) / ΣB`. Request percentages are not averaged. Percentage is null for `B=0`, empty, or incomparable data. Measured zero and negative values are preserved. A total beyond the JS safe-integer limit becomes unknown/overflow.
- Stored/comparable/excluded/unplaced-in-time event counts and rolling/aggregate/unidentified-source warnings are returned separately. A measurable subset is not all traffic. Without completeness evidence, `complete_traffic=false`. Raw lifetime/snapshot aggregates do not become additional savings events.

<a name="kabul-ve-doğrulama"></a>

## Acceptance and verification

1. New record → same proxy/MCP replay → reopen from the actual file, yielding one event and preserved period total. No old event is moved into the new measurement session.
2. Local-midnight/7-day calendar boundary and partial data → weighted 14% example, with empty/zero/negative/mixed-basis/overflow distinguished. Missing or ambiguous time is not treated as complete coverage.
3. Retention, conflicting replay, foreign schema, DB lock/partial transaction, and unsafe file → fixed error or correct coverage. Existing data is not corrupted.

First run focused Rust RED/GREEN tests, then lead-agent diff/connected-flow review, and one local Rust fmt/test/embedded-build gate on the final candidate. Unchanged frontend test/build evidence is reused only when the relevant source/tool fingerprints match. The new native query is checked through the real app IPC path without a model call. Fixture metrics are not real user savings.
