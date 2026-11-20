<a name="014--model-istemci-ve-ajan-ataması"></a>

# 014 — Model, client, and agent attribution

5 November 2026. H026 is active. 23/51 remains unchanged. The lead agent owns data/provenance, origin mapping, persistent compatibility, and acceptance. Presentation work with a fixed contract may be assigned to the single Luna worker. H025 totals and trust boundaries are preserved.

<a name="kaynak-ve-kapsam"></a>

## Source and scope

Pinned 0.41.0 `/stats?cached=1` recent_requests/request_logs rows contain request_id, provider, and model. `server.py::_classify_agent_from_log` guesses the app from tags/model/provider. This result is neither identity nor route evidence and is not consumed. Raw tags, headers, prompt_cache_key, chat files, and account/auth are not read. Raw agent/client/role fields are not trusted witnesses. The current relay does not inspect byte streams. One open connection does not prove the client for every request.

Model/provider is only a bounded observed label. If native source provides owned route evidence for the same runtime+proxy origin, it may carry client, opaque upstream, and, when available, opaque agent/parent identity and explicit role. Do not infer role/client/provider URL from model name. The pinned adapter does not produce this route witness, so actual client/agent fields remain unknown. This gap is not an optimization filter. Different upstreams remain separate even when they use the same model. H051 may use this request-origin/opaque-identity contract. Chat/account listing does not start here.

<a name="uygulama-sözleşmesi"></a>

## Implementation contract

- Add default-unknown attribution to H023 observation/event. Continue reading schema 1 payloads. Do not change the SQL table/DB version. Preserve the existing 4KiB event/128MiB store limits and all pin/transaction protections. Do not add renderer write IPC.
- Route evidence is bound to origin_id. Raw upstream/agent identities are not stored. Identities in native witnesses are hashed within the runtime scope. Validate label size/characters and identity correlation. Do not echo errors/payload/secrets.
- Attribution enrichment or conflict does not alter token counts. Conflicting metadata is absorbed as unknown and is not revalidated by a later replay. An old unknown record may be enriched with new real metadata for the same origin. Counters and the initial measurement session do not change.
- The period query groups client/provider/model/upstream/agent from the same event set. Layer events for the same origin are grouped as one request identity. Role is not inferred from model. Show at most 64 groups. Combine overflow into one explicit “Other” row without silently dropping data. Amount/% use the same basis as H024 totals. Unknown counter bases are not upgraded.
- Coverage status is unknown/partial/verified only for the observed subset. It does not mean all computer/account/agent traffic is covered. Do not produce zero requests or optimization success for upstreams outside the proxy. A client connection record is not converted into attribution.
- Graphite shows a compact model/scope table, unknown attribution, and a clear source note. Do not add accounts, chats, cost, or a model selector. Do not show a second empty panel when there is no data.

<a name="kabul-ve-doğrulama"></a>

## Acceptance and verification

1. Two different evidenced agents using the same model do not collapse to one identity. Separate upstreams and missing roles remain distinct. A model/tags/client assertion alone is not native proof. Metadata conflict does not change token totals.
2. H025 owned HTTP → pinned adapter → SQLite → same-period attribution → read-only IPC/strict TS/UI works. Test old payload, replay/reopen, period, limit/Other row, wrong origin, and canary/safe-label acceptance. Native/proxy empty-source evidence, numeric fixtures, and real-provider tests are not presented as one another.
3. UI covers unknown/partial/verified and two agents with the same model, in TR/EN, narrow windows, and keyboard use. Run focused RED/GREEN tests, one integrated root review, the full local gate on the final candidate, and native checks. Do not repeat the valid PASS for 123 H025 inputs because of compaction or a new day. Validate changed inputs on the new candidate. Stop when H026 closes. H027 does not start.
