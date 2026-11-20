<a name="010--içeriksiz-sayaç-normalizasyonu"></a>

# 010 — Content-free counter normalization

3 November 2026. The lead agent implements H023 and owns decisions about token-measurement meaning and provenance trust. The basis is H012-a, `docs/contracts.md`, and review of pinned 0.41.0 sources whose hashes match. This decision does not complete H012's other product capabilities.

<a name="sözleşme"></a>

## Contract

- The normalizer is pure and bounded. It does not manage files, HTTP/model requests, or persistent state. H024 covers storage and periods, H025 screen integration, and H026 model/agent attribution. This task does not change processes or routes for the current app or other chats.
- Before/after values are carried in separate `Reading` records with nullable tokens, counter kind, tokenizer identity, and covered component. A difference/percentage is not derived without the same identified basis and component. For a supported pair, `S=B−A`. When `B>0`, the percentage is `100×S/B`, otherwise it is null. Measured zero, missing values, and negative differences remain distinct.
- Only Headroom message-input or whole-input pairs are candidates for the primary input metric. Tool schema, novel, RTK, cache, and output remain separate. Provider usage fields are not added to input pairs. A message pair that does not measure whole input is not labeled as whole input. The normalizer does not combine different bases into one total. H024 owns weighted period totals.
- A proxy-provenance key is derived from runtime generation and request identity. Identities are not kept in plaintext and are hashed with SHA-256. `origin_id` links measurements for the same request. `event_id` also carries the optimizer/component slot using explicit fixed labels. The message/whole-input slot is shared. Overlapping pairs conflict instead of being added together. MCP may be linked to the same event only through an explicit proxy-origin mapping from a trusted collector. Matching model names or similar IDs do not count as a mapping. An exact duplicate is one event. A conflicting duplicate becomes unknown/conflict, and ambiguous basis/time is not retained. The same ID in another runtime generation is a different event. H024 uses these keys for repolling/persistent upsert.
- UTC timestamps are validated. Invalid identities/values/times produce a fixed error. Payload content is not copied into the error message. Token counts are bounded to the JSON/JS safe-integer range, batches to 10,000 records, and raw snapshots to 2MiB. Events without a timestamp are not candidates for period metrics.

<a name="0410-adaptörü"></a>

## 0.41.0 adapter

If the engine provides `recent_requests`, project that field. Otherwise project `request_logs`. Never count both. Read only request ID/time and explicitly numeric fields. Do not carry content, raw IDs, auth, agent inferred from model, URL/path, free-form tags, transformation details, or lifetime totals. Snapshot aggregates are not added to event totals.

These legacy rows do not carry tokenizer identity or trusted component/provenance information. Preserve `before/after` as raw observations, but leave basis/component unknown. `has_exact_tokens`, engine saved/% fields, or tags do not raise that trust. Do not infer desktop savings from H021 rows `2306/12, saved0/%0` and `12/12`. A newly evidenced collector may later provide measurements with an explicit basis to the same pure API. This task does not add token recounting to the engine.

The recent window is not complete history or all traffic. Aggregate-only and unidentified rows are visible metadata. A source reset/restart remains a new generation and separate count. A snapshot difference is not additional savings. H024 owns carrying buffer gaps/partial coverage into stored periods. Legacy lifetime values of 51 requests/4471 saved are not imported as current data.

<a name="kabul"></a>

## Acceptance

1. A same-basis measurement from 120,000 to 75,000 input tokens yields 45,000 saved tokens and 37.5% savings. Measured zero, a zero denominator, missing values and increases remain distinct.
2. Different/unknown bases, H021's actual row number formats, and the content-canary fixture do not produce invented percentages or leak content.
3. Provenance-verified proxy/MCP duplicates, conflicts, and two runtime generations are distinguished correctly. Layer/aggregate counters do not inflate primary input.

Focused Rust tests → lead-agent review → final local gate on changed source. Unchanged frontend evidence is reused only when source/config/tool fingerprints match. A fixture result is not live-provider/model/subagent or performance evidence.
