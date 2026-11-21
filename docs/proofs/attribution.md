<a name="h026--model-ve-ajan-kapsamı-kabulü"></a>

# H026 — Model and agent coverage acceptance

5 November 2026, Windows 10 22H2 x64/build19045. [Plan 014](../decisions/014-attribution.md), [lead review](../reviews/H026.md). The H025 totals/route were preserved. Model/provider observation and evidence-based attribution were added. No model/client/agent selection or filter was installed.

The lead agent owned provenance/origin, normalization, persistent storage, the parser, and acceptance. The same single GPT-6 Luna xhigh worker wrote only the Coverage.tsx/coverage.css presentation, then stopped and handed it over. The worker model is not a product support condition or a required test model. Development token savings/duration were not measured.

<a name="davranış"></a>

## Behavior

- The app keeps bounded model/provider labels from an owned pinned metadata row. It does not use engine model-name guesses for client/agent assignment, tags, or raw agent/client/role or attribution objects.
- When a native request-route witness is bound to the same runtime/proxy origin, the app can carry the client, opaque upstream, and any opaque agent/parent/role. The pinned adapter in this test produced no such witness. Actual client and agent attribution remains unknown. A user connection or the same model name does not make it verified. This unknown state does not block optimization.
- The app reads old records. Enrichment/replay does not count the same event again. A metadata conflict does not corrupt token values and does not resolve itself through metadata replay. The persistent table/version did not change.
- Period totals and groups use the same event set. Layers from one origin count as one request identity. The app does not merge uncorrelated sources as physical calls. Different upstreams and two agents with the same model stay separate. If the group count exceeds 64, the table retains excess groups in the Other row. These values are not API bills/account usage.
- The Graphite table shows client/model, an explicit role and short opaque identity, request record, and comparable input/% amount. Uncomparable legacy counters stay `—`. Unknown/partial/verified attribution covers only observed subsets. It is not an open-chat/account list.

<a name="kanıt"></a>

## Evidence

| Scenario | Observed result |
| --- | --- |
| Same model/two agents | Two separate roles/opaque identities. No merge by model. Different upstreams with the same model also stay separate. Native witness input was a fixture, not live provider evidence. |
| Missing and fabricated metadata | The words codex/claude in a model name, tags/agent/client claims, and raw attribution objects do not create a role/client. A missing role is partial. A fully missing assignment stays unknown. |
| Wrong origin/identity | Rejected a witness moved to a different request ID, raw URL upstream, self-parent/main-parent, and invalid/long labels. Raw ID/auth canaries did not reach output/storage/UI. |
| Metadata/metrics independence | Model enrichment from unknown and conflicts in model, agent, or upstream kept the difference of 50 tokens. The conflicts persisted. Layers from one origin yielded 2 events and 1 request. The metrics count stayed the same. |
| Store/reopen/period | The app decoded an old payload without attribution fields. The same event gained metadata. Reopen kept the model and one record. A new monitoring session and previous-day scope stayed separate. |
| Group limit | 80 groups → 63 detail rows + 17 groups in Other. The total still showed 80 requests and a difference of 4,000. The strict decoder rejected missing/truncated groups and fabricated certainty. |
| Real owned HTTP | Fixed stats GET/exact peer/lease → pinned adapter → SQLite → Overview. Model/provider stayed intact. Raw client/role/agent canaries were not witnesses. Two captures remained one event. This used a local HTTP fixture. |
| Native app | Real session/today/seven_days replies matched attribution event/origin totals with history. Pinned engine ready → collection recording. The renderer read the new DTO. Real source: 0 requests, 0 model calls. No live identity/percentage was invented. |
| UI | Two same-model agents, separate route, partial/unknown, metadata conflict with preserved percentage, increase, and Other table. TR/EN, keyboard scroll focus, 1280×800/1024×720/narrow viewport, and real WebView125/150/100 scale passed. Numeric/identity rows used an isolated module-mock fixture. |
| Exit | The final native window received WM_CLOSE and exited 0. Owned Job/debug-port cleanup completed. Normal Codex config/preferences/onboarding/ownership fingerprints stayed intact. |

The first 8 new Rust tests were RED, followed by 50 metrics PASS. New actual HTTP metadata assertions also passed. The 5 frontend attribution tests first had 1 PASS/4 RED, then 5 PASS. The existing metrics client had 17 PASS.

The final candidate's only full gate had **125 frontend/335 Rust PASS**, 5 existing opt-in ignored, plus TypeScript/Vite build, fmt, and offline embedded build PASS. The gate for earlier slices was not repeated. The source/copy changed after the final review, so native evidence was refreshed on the new EXE.

`work/proofs/attribution/` contains `pure-red.json`, `pure-green.json`, `metrics-green.json`, `pure-green2.json`, `source-green.json`, `decoder-red.json`, `decoder-green.json`, `decoder-integrated-green.json`, `final.json`, `ui-latest.json`, and `acceptance.json`. The final source/config input count was 128. The same verification step recorded before/after fingerprints, Rust1.92.0/Node25.2.1/pnpm11.25.0/Python3.14.0, and Win10 environment. No raw failure/HTTP/secret record was kept.

CodeGraph was synced through the project launcher. Discovery is not test evidence. No browser/Codex/Vite server was installed. No new dependency was added.

This acceptance does not claim a new live model/auth/agent or support for another OS. No paid provider or CI call occurred. Windows 11/macOS/Linux, installer/power loss, and long load were not measured. API-key remains in H041. Account/chat coverage remains in H050/H051. H027 did not start. Work stops at the end of the main task and awaits the user.
