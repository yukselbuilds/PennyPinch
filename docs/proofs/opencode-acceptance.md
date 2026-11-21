<a name="h010-kabulü"></a>

# H010 — OpenCode acceptance

November 15, 2026. H010 is complete within the technical scope below on Windows 10 22H2 x64. The test used engine 0.41.0 and a local synthetic provider fixture. [Acceptance record](opencode-acceptance.json), [lead agent review](../reviews/H010.md), [connection decision](../decisions/009-opencode-provider-routes.md)

| Main acceptance | Evidence and scope |
| --- | --- |
| Preserve provider and model selection | OpenCode 1.19.3 CLI and TUI verified model changes in the same main session, inheritance by a new subagent, and a different provider and model pair. OpenCode 2.3.0 CLI verified model inheritance and explicit selection of the subagent model. [V1](opencode-subagents.md), [V2](opencode-v2-subagents.md) |
| Record JSON, JSONC, and auth scope | The native test verified Preview, Apply, preservation of comments and user fields, reopen, removal with conflicts, and restoration of the original bytes. Synthetic auth matched on the wire. Real OAuth and account use have separate acceptance. [V1](opencode-retrieval.md), [V2](opencode-v2-binding.md) |
| Preserve streaming, tools, and CCR | The tests verified Chat streaming and separate Responses routes, correct call IDs, completed tool calls, log compression, and byte-exact retrieval. [V1](opencode-retrieval.md), [V2](opencode-v2-binding.md) |
| Verify main agent, subagent, and model changes | Tool metadata matched a separate native session. The full V1 TUI verified the model menu and a change within the same parent session. A separate check confirmed the V2 background subagent completion notice. [TUI](opencode-tui.md), [V2 subagents](opencode-v2-subagents.md) |
| Record the scope for normal service and Desktop | The 2.3.0 CLI and full TUI passed against the same registered service. The Desktop from the official package used the config saved by the CLI through its own bundled service. Separate checks confirmed the final response in the GUI and normal exit. [Service and TUI](opencode-v2-service.md), [Desktop](opencode-v2-desktop.md) |
| Preserve error, ownership, and scope limits | The records cover cancellation, conflicts, an absent bridge, pause, reopen, an incorrect version, and cleanup after normal and failed runs. The final catalog limits each record to the verified client, version, OS, and auth combination. [Discovery](opencode-v2-discovery.md), [undo](opencode-retrieval.md) |

K2 keeps unavailable client, version, and auth combinations experimental. Decision 009 limits the initial profiles to standard OpenAI Chat and Responses routes. The support record leaves the combinations below open. The previous H010 remaining-acceptance entry treated each new combination as a requirement for closure. This decision corrects that scope.

- Live providers and auth, custom reasoning variants, and actual response quality.
- Automatic discovery on a machine with only Desktop installed, and installer behavior.
- V2 subagents in the TUI or Desktop, model changes within a subagent session, and nested subagents.
- Immediate adoption by a running service, other providers, custom upstreams, and additional OS combinations.

This closeout kept the product and catalog behavior. Reuse the local gate results while its 344 inputs and the toolchain and OS fingerprints match. The gate passed 222 Node and 462 Rust tests. It ignored 19 tests. Formatting, frontend, and embedded builds passed.

Each native record keeps its historical executable and source fingerprints. The final local gate supplies regression evidence for the current candidate. This review used the recorded evidence for traffic and operating system behavior.

The reason for one Apply failure in the Desktop test is unknown. The same action passed on the final candidate. The failed run restored the original config and test settings. [Run limits](opencode-v2-desktop.md)

The checks measured the reduction in log block bytes. Real token and billing savings remain unmeasured for H010. H041 records local token, cache, integrity, and latency comparisons with their own scenarios and measurement bases. [Optimization measurements](../validation/optimization-benefit.md)
