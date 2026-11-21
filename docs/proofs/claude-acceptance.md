<a name="h009--claude-code-yerel-teknik-kabulü"></a>

# H009 — Claude Code local technical acceptance

November 15, 2026. **H009 is complete.** The scope covers Windows 10 22H2 x64, Claude Code 2.1.158, Headroom 0.41.0, and a synthetic Anthropic API route. The review compared the existing evidence with H009 requirements for connection, preservation, streaming, tools, and retrieval. TASKS K2 keeps unavailable client, version, and auth combinations experimental. The task allows the review to document this limit when an account is unavailable.

| Acceptance | Observed behavior | Evidence |
| --- | --- | --- |
| Effective settings scope and connection | The native UI confirmed user scope, an explicitly selected project, Preview, Apply, owned stable ingress, and a real CLI request. The CLI loaded project settings without extra model, endpoint, auth, or effort flags. | [H020](claude-product.md) |
| Preserve existing settings | The run preserved synthetic auth, the selected model and effort, hooks, and three unrelated settings files. It copied no user account or secret store. Evidence for preservation with live OAuth remains open. | [H020](claude-product.md) |
| Streaming and final response | The CLI displayed partial text before the terminal event. It completed the response and exited normally with code 0. | [H009-a](claude.md), [native H020](claude-product.md) |
| Tools and retrieval | The CLI made two calls to the single permitted MCP fixture tool. The test observed three ingress requests and four upstream messages. It verified the call ID, model, effort, thinking, synthetic auth, final response, and graceful engine shutdown. The log changed from 14,999 bytes to 103 bytes. Server-side CCR retrieved the original bytes with the same hash. | [H009-b JSON](claude-tools.json), [review](../reviews/H009-b.md) |
| Cancellation, conflict, and undo | Cancelling the native picker preserved the scope. Apply and Undo restored three original settings files byte for byte. In H022, Undo in the wrong project preserved the original files. Undo in the correct project kept the user's additions. | [H020](claude-product.md), [H022](onboarding.md) |
| First use and reopening | Read-only discovery and preview created no files. Explicit Connect created the missing parent directory within the transaction. A normal reopen remembered the correct project and installation. This passed the missing-parent and saved-selection cases beyond the earlier H020 scope. | [H022](onboarding.md) |
| Model and subagent scope | The connection manages only the endpoint field. The user continues to choose models and effort. The connection requires no model allowlist or separate agent setup. Live verification remains open for Claude subagents, role-specific endpoints and providers, and model changes. Full agent coverage remains unverified. | [Decision 008](../decisions/008-claude-connection.md), [catalog](../../compatibility/clients.json) |

The native UI run used the CLI with tools off. The H009-b tool and CCR run used a separate isolated CLI and MCP profile with a test observer. End-to-end acceptance with tools through Tauri and Rust ingress remains open. The fixture's `protected_tools=[]` setting leaves the product's protected-tool defaults intact.

Server-side CCR verified retrieval by the engine. CLI-initiated retrieval remains outside that result. The CLI also moved large MCP output to a file. The test assigned no Headroom savings to that client behavior.

Live Anthropic, API-key and OAuth use, actual model features and quality, and general hooks, plugins, and MCP remain `not_tested`. This status also applies to Claude Desktop and extensions, subagents, different route providers, Windows 11, and other OS combinations. The user previously deferred Claude sign-in. The test used no new sign-in or paid request. The byte reduction applies to a single block. Token savings, billing savings, and development-chat savings remain unmeasured.

The plan compared existing evidence with main acceptance and K2. It updated the catalog's earlier tools-off and onboarding limits and retained the historical artifact fingerprints. The lead agent owned auth, route, and process acceptance. The slice preserved model, auth, ownership of processes and settings, and atomic undo contracts. The product change was limited to the compatibility catalog. It added no engine, adapter, or UI behavior.

The catalog is an embedded product input. The final candidate reran the five-command local WORKFLOW gate once. It passed 222 Node and 462 Rust tests. The gate ignored 19 Rust tests. Formatting, the frontend build, and the embedded custom-protocol build passed.

The gate recorded 344 source, test, config, and tool inputs, with the environment and commands before and after. The H020 and H022 executable and source snapshots retain their historical scope. The check also compared hashes for unchanged H009-b source and retained native, focused, and onboarding results.

The [main acceptance record](claude-acceptance.json) contains the result. The command and before-and-after records are in `work/proofs/claude-acceptance/`. External environment and release gates for H006, H037, H041, and H042 remain open. **Total complete tasks 45/54.** Work stopped at the end of the main task. [Main review](../reviews/H009.md)
