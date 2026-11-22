# PennyPinch — Product and Feature Specification

**Scope date** 11 November 2026\
**Document version** 0.6\
**Development workspace** `D:\Projeler\Headroom`\
**Product name** PennyPinch, an independent desktop app that uses the Headroom engine for context compression.

**Status reviewed on 20 November 2026.** The task list tracks acceptance and evidence for each build and platform. [Task list](TASKS.md) · [Current handoff](docs/IMPLEMENTATION_STATUS.md)

## 1. Product idea

Developers using Codex, Claude Code, or OpenCode should be able to connect through PennyPinch without handling terminal commands or configuration files. The desktop panel should let them start and pause compression and see the measured reduction in input tokens.

The product's core promise: **“Connect the coding assistant you already use, keep working with your current model, and see the savings.”**

The desktop app manages the connection, Headroom engine lifecycle, reversible client settings, and metrics. Reimplementing the compression algorithms is outside the initial scope. Headroom already provides standalone proxy and local monitoring surfaces. [Headroom proxy documentation](https://docs.headroomlabs.ai/docs/proxy).

Success means more than displaying a large savings percentage: it means easy setup, working streaming and tool calls, access to original content when needed, and preserving the model behavior chosen by the user.

## 2. Initial product decisions and assumptions

| Topic | Initial decision |
| --- | --- |
| Platform | Windows 10 22H2 x64 and Windows 11 x64 are both in the first-release scope; macOS and Linux belong to later distribution phases. |
| User | A developer who uses one or more AI coding clients. |
| Language | Turkish and English; translation keys are used from the start. |
| Operation | Local desktop UI + local management layer + Headroom engine. |
| Account | No new account is required to use the app. The existing client sign-in is used. |
| Initial clients | Codex, Claude Code, and OpenCode; UI variants are validated separately. |
| Connection | Supported client settings or process-scoped launch options. |
| Default optimization | The coding approach, cache stability, retrieval access, and file-reading protections. |
| Model and effort | Preserve the user's choice; never automatically downgrade the model or reasoning effort. |
| Privacy | Metrics stay local; raw conversations and code content are not saved by default. |
| Name and brand | PennyPinch is the independent desktop app. Credit the Headroom project for the compression engine. |

“Easy for everyone” means a working setup flow for supported versions and sign-in methods. It does not promise identical behavior across every version, extension, and account type of every client.

Windows 10 support is not deferred to a later release. Windows 10 22H2 is the initial validation baseline; older Windows 10 releases and ARM/32-bit support are not considered supported until separately validated. Each client's operating-system requirements are tracked separately from those of the desktop manager.

## 3. Core user flow

1. **Open:** The app automatically finds supported clients, their versions, and previous connection preferences.
2. **Connect and start once:** Found applications appear in a simple list. The user sees which applications will be connected and a short impact summary, then selects **Connect and start**. Required preparation, engine startup, settings changes, and ownership recording happen in the same flow. Any required client restart is stated clearly; partial success across applications is shown separately.
3. **Start on later use:** Previous connections are retained; the user resumes with **Start**. Setup, CLI/Desktop, and model choices are not repeated; optional auto-start is offered separately. Starting the engine alone does not route a client that has not been connected.
4. **Monitor:** The connection is verified in the background through new real requests, and savings are shown. A previously connected client is not asked for setup or a paid verification request every time the app opens.

Automatic scanning is limited to known installation/configuration locations and accessible client version information. Found clients are not connected automatically. The user's one-time selection determines which programs may have their settings changed. A newly installed client is not connected until the user selects it.

Users are not expected to enter terminal commands, set ports manually, edit config files, or create a new API key solely for this product. The existing supported sign-in method is preserved. If automatic detection fails, **Choose application** is available. If the adapter does not support the existing sign-in method, it does not silently switch to a different paid provider.

**One logical connection:** CLI/Desktop installations verified to use the same active settings scope and supported route are shown as one application connection in the normal flow; binary and version details are under Advanced. Meaningful selection/recovery remains available for different profiles/projects/routes, stale or missing installations, and settings conflicts. A binary is never selected automatically based only on version. Traffic verification for one surface is not evidence for another. The [3 November UI/UX review](docs/reviews/UX-2026-11-03.md) explains how this goal differs from the current implementation and identifies related tasks; the target flow is not yet considered complete.

**Verify connection** is also available as a troubleshooting action. If the user wants to test immediately, they can start a small real verification request; the action states that it may count toward account usage. In normal operation, existing requests are preferred for verification.

**First-time setup and daily use are distinct.** The client may need to be reopened during setup. For clients connected once with the background engine ready, daily use stays at opening, pausing, and monitoring. The app does not assume it can automatically capture connections already established by an open process.

## 4. Core features — first usable release

| ID | Feature | User outcome |
| --- | --- | --- |
| F01 | Automatic client detection | Installed program, version, and connection eligibility are visible; file selection is offered if no client is found. |
| F02 | Guided setup | Establish a supported connection without asking the user to enter commands. |
| F03 | Reversible connection | Remove changes made by the app while preserving later user changes. |
| F04 | Headroom engine management | Start, health checks, port selection, and controlled recovery are managed in one place. |
| F05 | Start / Pause | Compression controls do not unnecessarily interrupt the model connection. |
| F06 | Savings view | Show input reduction, period totals, data source, and last-update time. |
| F07 | Access to original content | Verify that content compressed through CCR can be retrieved when needed. |
| F08 | System tray | The connection continues when the window closes; status and basic actions remain available from the tray. |
| F09 | Compatibility and connection verification | Show “found,” “configured,” and “real traffic verified” as distinct states. |
| F10 | Preserve model settings | Do not silently change the main model, subagent model, reasoning, permissions, or existing sign-in method. |
| F11 | Troubleshooting | Show the reason and an actionable next step instead of a raw stack trace. |
| F12 | Export | Export a content-free statistics summary as JSON/CSV; remove personal paths and secrets from diagnostic reports. |

Headroom's compress, retrieval, and statistics tools are existing engine capabilities. Their presence alone does not prove that all model traffic is compressed automatically. [MCP tools](https://docs.headroomlabs.ai/docs/mcp).

## 5. Client integrations and support policy

| Client / surface | Product target | Particular validation focus |
| --- | --- | --- |
| Codex CLI | First-release adapter | Process/user routing, Responses streaming, WebSocket, tool calls, and existing auth. |
| Codex desktop app | First-priority real-use target | Where the setting actually takes effect, first restart, and main/subagent traffic. |
| Claude Code CLI | First-release adapter | Anthropic protocol, streaming, retrieval, and preserving the sign-in method. |
| OpenCode CLI / TUI | First-release adapter | Active provider/model, JSON/JSONC settings, and OpenAI/Anthropic routes. |
| VS Code / JetBrains extensions and other desktop variants | Separate compatibility entries | CLI support does not automatically extend to these surfaces. |
| Cursor, Copilot, and other compatible tools | Later adapters | Add by version and protocol; do not give a universal support badge in the first release. |

Each adapter keeps a compatibility record containing client version, operating system, UI surface, sign-in method, protocol, and date of last successful verification. States are **Verified**, **Experimental**, and **Unsupported**. The existence of a file path is not support evidence.

A verified publisher signature or hash pinned from a trusted source verifies file provenance; a real config/protocol/feature test verifies behavioral compatibility. A version-number change alone does not establish general incompatibility. Preserve known breakages and security limits; after an update, previous behavior evidence becomes stale and controlled revalidation may be needed. The normal flow does not make users choose a binary.

### Subagent coverage

**User decision — default scope:** When the user connects a client, the connection covers all supported models, the main agent, and all subagents together. No separate setup is required per model or subagent. This is a product acceptance condition; it does not mean that every client/auth/agent combination has already been validated.

The goal is to include subagents that use the same route as the connected client's main agent. Users are not asked to perform a separate **Connect** action for each subagent. Regardless of the model used by a subagent, eligible requests that pass through the same supported provider/proxy route are included in optimization.

Do not assume coverage for a subagent running in a separate process, through a separate provider, or in a cloud environment. Until that route is separately connected and verified, the UI must not claim full coverage. If subagent identity cannot be measured, show only verified aggregate traffic; do not infer agent identity from the model name. The app does not create subagents or change their count, model, or effort settings.

The connection plan accounts for known subagent/provider route differences; supported routes are prepared in the same setup plan when possible. A local or cloud route that bypasses the proxy is not covered automatically. The UI shows main/subagent coverage as verified, partial, or unknown. A newly started subagent using the same verified route does not need a separate connection. If agent attribution is unavailable for all traffic, do not say “all subagents verified.”

### Model coverage

The product is not pinned to a short list of model names. It aims to cover models used through the connected client and supported provider protocol. Changing models inside the client alone should not require reconnecting; a different upstream or sign-in route may require new verification.

The user selects **which client or connection scope to connect** in PennyPinch. They continue choosing models in Codex, Claude Code, or OpenCode. A supported connection includes the main agent and subagents using the same verified route, regardless of their model names. Verify new capabilities against the underlying protocol and route.

The scope criterion is the request protocol, sign-in method, and transformations the engine can safely apply to that content—not the model brand. Do not claim every feature of every model can be compressed. Preserve encrypted reasoning or blocks the engine does not support without modification; report no savings for those blocks. If a new/unknown model is not sufficiently supported, use the supported raw pass-through path rather than changing models, and show the optimization boundary. If raw pass-through cannot be verified either, report incompatibility clearly.

A fixed model list, mandatory low effort, or automatic low-cost model selection is not a connection requirement. The model name, reasoning level, and subagent selection belong to the user. Savings rates are not expected to be the same for every model.

### Binding decision for Codex

Current Codex documentation says provider and base URL settings in a project .codex/config.toml file are ignored. Therefore, the Codex adapter must not present such a project setting as a working connection; it must use a supported user or process scope. [Official Codex configuration](https://learn.chatgpt.com/docs/config-file/config-advanced#project-config-files-codexconfigtoml).

Users may assign a different model and reasoning level to their main agent and each subagent and change them in other conversations. The app does not choose agent count or model and does not require reconnecting per model/agent. The model preference of the worker developing this project is not a product support condition. Verify separately that main and subagent requests use the same route; do not infer agent identity from a model name alone.

Model-scope acceptance tests different models for main/subagents, model changes in a connected client, and preservation of a new model name using the same protocol. A fixed allowlist based on model names cannot be a connection condition. Do not hide protocol or feature incompatibility by changing models; show measured scope. Representative tests must not be presented as validation of every model's every feature.

### Binding decision for Claude Code

Claude Code reads user, project, and local project settings from different scopes. The adapter must find the active settings source and offer the narrowest supported scope. [Official settings scopes](https://code.claude.com/docs/en/settings).

Plugin and desktop variants may have distinct issues, such as displaying tool-search content. Do not place these surfaces in the same support entry as the CLI. [Headroom's plugin compatibility note](https://docs.headroomlabs.ai/docs/persistent-installs#claude-code-vscode-extension-caveat).

### Binding decision for OpenCode

Headroom's OpenCode documentation defines a default model for the provider it creates. The product adapter must not blindly run a wrapper and change the user's model. Verify that the active upstream model is equal before and after setup; record any required provider-alias change separately. [Headroom OpenCode integration](https://docs.headroomlabs.ai/docs/opencode).

OpenCode supports JSON and JSONC. Preserve comments and user-owned fields; overwriting the entire file with an old backup cannot be the normal disconnect method. [Official OpenCode configuration](https://opencode.ai/docs/config/).

## 6. Start, pause, and shutdown behavior

User-visible actions:

- **Start:** If the proxy is ready, enable supported compression for new requests.
- **Pause:** Stop compression for new requests without interrupting requests already in progress; preserve the connection and retrieval service.
- **Disconnect:** Undo PennyPinch's routing changes and explain when the current process needs a restart.
- **Close window:** Minimize to the system tray; do not terminate the engine.
- **Exit completely:** Explain the effect if a client is connected or a request is active; do not silently cut off a running process.

Implementing pause by simply toggling the --no-optimize option is insufficient. Preserve the prefix of previously compressed history, CCR records, and in-progress streams. Prove the engine's appropriate behavior in a technical prototype.

A client permanently routed through the proxy may be unable to connect while the engine is off. Background operation and optional Windows startup are therefore separate, clear settings. Show this preference explicitly during setup. On an unexpected engine crash, attempt a bounded restart; do not hide failure. Automatic fallback to a direct connection is not a product promise until separately proven.

Headroom has persistent operation options; how the desktop product owns and reverses them is managed separately. [Persistent operation options](https://docs.headroomlabs.ai/docs/persistent-installs).

## 7. Connection states

Connection, engine, and compression states are separate. Do not represent them all with a single “active” label in the UI.

| Visible state | Meaning | Primary action |
| --- | --- | --- |
| Not found | The client was not detected automatically. | Choose application |
| Not connected | The client was found; routing is not configured. | Connect |
| Preparing | The engine or client settings are being prepared. | View progress |
| Restart required | Settings were written; the open process has not refreshed them. | See how to continue |
| Waiting for verification | Configuration is ready; there is no real-request evidence. | Verify connection |
| Compression on | New compression is enabled on a verified route. | Pause |
| Paused | The connection continues; new compression is off. | Start |
| Problem | An engine, authentication, or protocol error occurred. | Resolve issue |
| Waiting for data | The connection is healthy; there is no measurable traffic in the selected period. | Continue normal use |

Show the times of **last verification** and **last request**. If configuration or version changes, do not treat old verification as current. A quiet session is not an error.

## 8. Modern, minimal UI direction

**Design character:** A calm, technical but accessible desktop tool for developers. Spacious layout, strong typographic hierarchy, few colors, and clear states.

Three comparable visual directions and interactive previews are in [UI_CONCEPTS.md](UI_CONCEPTS.md). The user selected **Graphite**; develop light and dark themes within the same layout. Decision: [001 — UI direction](docs/decisions/001-ui-direction.md).

### Visual foundation

- Light and dark themes; follow the system theme by default.
- Dark theme: graphite background #111315, panel #1A1D21, primary text #F0F2F4, secondary text #A2AAB6, accent #47C7A5.
- Light theme: background #F7F8FA, panel #FFFFFF, primary text #1C2027, accent #147D68.
- Segoe UI Variable / Segoe UI on Windows; tabular numerals for metrics. Do not require a font license or download.
- Windows 11-specific window effects are not required for core functionality. Preserve the same layout, readability, and actions on Windows 10 through supported visual alternatives.
- Fine dividers and restrained 8–12px corner radii; no heavy shadows or persistent glow decorations.
- Functional transitions of 120–180ms; respect reduced-motion preferences.
- Keyboard access, visible focus, and a WCAG AA text-contrast target; never communicate states by color alone.

### Four main screens

1. **Overview:** Connection state, one primary Start/Pause action, selected-period input reduction, one time-series chart, and a short application list.
2. **Applications:** Codex, Claude Code, and OpenCode connections; version, scope, last verification, and connect/disconnect actions.
3. **Activity:** A request list that does not show content; model, client, duration, compression result, and errors. Filters and a details panel.
4. **Settings:** Theme, language, background operation, data retention, and simple compression preferences. Advanced details expand on demand.

These four screens are in the UX001 visual-acceptance scope. The Windows 10 development release has a fifth **Sessions** screen combining Chats and Accounts views. [H050 account acceptance](docs/proofs/accounts.md) · [H051 session acceptance](docs/proofs/sessions.md)

### 24 October user direction — simple use and sessions

This section describes target behavior; implementation status is limited to TASKS.md. The primary experience answers two questions: **How much did I use and save by platform/model? Which accounts are connected, and which chats are open or working?** Preserve the Graphite visual direction.

- Overview shows tokens used, tokens saved, and the percentage from the same basis, broken down by platform → model. Label cost as measured only when based on official usage/billing data; label it estimated when based on a versioned price calculation. Do not convert subscription quotas to token cost. Missing data is not zero.
- The normal connection view shows scope, a short impact summary, and Connect/Undo actions. Settings layers, protocol/ownership metadata, and diagnostics are optional under Advanced; the target of a change and critical conflicts remain clear before the action.
- Sessions shows account connection and sign-in status separately from open or working chats. PennyPinch opens the supported client's official sign-in flow for the user's existing account and subscription. The client handles passwords, codes and stored credentials.
- The chat list uses only required metadata. Open, working, last traffic observed, ended, and unknown states are not conflated. A process, file, or long period without traffic alone does not prove an active chat. Use an officially supported open/resume path only by user action; do not collect content or change provider/model settings silently.
- Show separate models and subagents under a platform only when route/identity evidence exists; do not require setup per model. Keep unassigned usage visible. Do not present traffic that bypasses the proxy as total usage.

Implementation order: H022 simple connections, H023–H026 real metrics/breakdowns, H050 account connections, and H051 session metadata/open flow. The existing H020 technical acceptance screen is not the completed end-user experience.

Overview layout:

~~~text
┌──────────────────────────────────────────────────────────┐
│ PennyPinch                 Status   [Start]              │
├───────────────┬──────────────────────────────────────────┤
│ Overview      │ Input token reduction      [Today ▾]    │
│ Applications  │ value / waiting for data   measured at   │
│ Activity      │                                          │
│ Settings      │ One time-series chart                    │
│               │                                          │
│               │ Connected apps and last traffic          │
│               │                                          │
│ Engine status │ One clear status message when needed     │
└───────────────┴──────────────────────────────────────────┘
~~~

Do not show a fake chart or sample savings as real data on first launch. Use a “No requests yet” empty state. First use, loading, pause, errors, and disconnect are part of the design. Target workspace: 1280×800; core controls remain accessible at 1024×720 and Windows 125%/150% scaling.

## 9. Savings and usage measurement

The primary metric is **input-token reduction calculated by Headroom**. It is not the net savings on a provider bill or a subscription quota.

~~~text
B = input tokens before compression, in the same scope
A = input tokens after compression, in the same scope
S = B - A
Rate = 100 × S / B  (only when B > 0 and both values are known)
~~~

**User request — percentage display:** Overview shows both the number of tokens saved and the percentage. For example, 37.5% · 45,000 tokens means 120,000 before and 75,000 after; this is an illustrative example only. When the selected period changes, both values update together from the same scope. Before/after values, data source, and measurable scope appear in details; JSON/CSV uses the same percentage. If data is missing, show — / Waiting for data; for a measurable zero, show 0%; for a negative difference, show “token increase.” Format the percentage according to the TR/EN locale.

- Calculate period percentages from total B and A; do not average per-request percentages.
- Preserve the source and precision of each token counter: provider tokenizer, local estimate, or unknown.
- Do not turn missing data into zero. Show a negative difference as an increase; do not silently discard it.
- For estimated dollar equivalents, show the model, price source/date, and cache assumption. Do not invent an amount for an unknown price.
- Provider-reported input/output/cached tokens, if available, are separate fields. Do not add them to local compression counters.
- Show that retrieval and retries may add usage. Do not claim net cost savings based only on removed content.
- Show session, today, seven days, and as much history as is available. Longer periods require the app's own local retention layer.

Headroom's persistent event records, live /stats, and MCP snapshots are different sources. The app must normalize record types; do not blindly combine proxy, MCP, and subagent totals. [Savings record structure](https://docs.headroomlabs.ai/docs/savings), [metrics endpoints](https://docs.headroomlabs.ai/docs/metrics).

Optional layers such as terminal filters/proxies are not added to engine measurements to produce one percentage. Command-output bytes, RTK's bytes/4 estimate, comparable same-basis total input tokens, provider cache/output counters, and billing usage remain separate. Compare layers side by side only when the same request/workload relation and comparable basis are proven; otherwise mark them unknown.

Show client/model breakdowns when evidence exists. Show project, chat, main-agent, and subagent breakdowns only when trusted metadata or an app-managed launch mapping exists. Otherwise use “Unassigned traffic” or “Proxy total.” The same model may be used by two different agents.

The panel reads statistics directly from local records; it does not call an LLM to prepare a report. Adding a savings line to every GPT final response is not a first-release requirement. If requested later, it may be a short, conditional statement that identifies snapshot time.

## 10. Differentiating features

### In the first release

- **Real connection state:** Do not say “working” because a settings file exists; show the last real request and verification.
- **Continue with your chosen model:** Compare the model and effort before/after setup; report incompatibility clearly.
- **Undo in one place:** Disconnect and preserve user changes.
- **Cache view:** Show cache usage beside token reduction when data exists; do not invent a rate when it does not.
- **Quiet system tray:** Do not produce constant notifications during healthy operation; notify only about meaningful state changes.

### In later small releases

- **View by content type:** Show where gains came from in JSON, logs, search, and other types when metadata is sufficient.
- **Local sample experiment:** Preview compression for a user-selected file without sending a provider request. Do not present it as a response-quality or billing-savings test.
- **Project preferences:** Recommendations based on client and verified project mapping. Do not create conflicts between global settings for a shared proxy.
- **Exportable diagnostics:** A content-free report of version, connection test, and settings differences.
- **Compatibility checks:** After a client update, report when revalidation is needed; do not force an engine change while it is running.
- **Optional terminal filter:** An additional layer within H043/H044 scope, for validated clients and shell commands. It does not change the connected model, reasoning, auth, or agent settings; do not assume it captures local/built-in tool calls automatically.

Rewriting long history in Claude Code, Codex, and other clients may reduce cache gains. The initial strategy favors cache preservation and working on new content; do not assume “more aggressive is always better.” [Cache behavior](https://docs.headroomlabs.ai/docs/cache-optimization).

## 11. Compression and original-content policy

- Initial configuration: coding profile and cache mode; verify active settings against the engine version.
- Prioritize protection for edited source files, full identifiers, and exact numeric decision data.
- Advanced settings may let users choose tool names whose output must be preserved. Use the active engine's supported tool-result protection; show selected names and which connection/engine group the change affects. Preserve numeric data through a verified file-reading/tool policy; do not guarantee automatic detection of all numeric content. numeric_report in experiments is a synthetic tool name, not a production default.
- Test that original content stored through CCR is accessible through the retrieval tool; producing a marker alone is not sufficient.
- Initial TTL target is 7200 seconds. Verify that the active value took effect; do not hide that expired records cannot be restored.
- Pause and window close do not automatically clear existing retrieval records.
- Output shaper, automatic effort reduction, model switching, continuous learning, and automatic instruction-file writes are off initially.
- Offer simple preferences: recommended coding settings and advanced details. Do not expose raw environment variables in the primary user flow.

CCR allows original content to be fetched later; it does not guarantee that every detail is present in the compressed view or that a model will always request every omitted detail. [CCR and retention](https://docs.headroomlabs.ai/docs/ccr).

## 12. Proposed technical architecture

H012 recorded the choice of a Tauri 2 desktop shell, Rust management layer, React + TypeScript UI, pinned Headroom engine, and local SQLite metrics store. Clean-install acceptance on both Windows targets is tracked under H006 and H037. [Architecture decision](docs/decisions/002-runtime.md) · [H012 acceptance](docs/proofs/architecture-acceptance.md)

Tauri supports packaging external executables as sidecars. That capability alone does not prove that Headroom and optional ML dependencies package cleanly on Windows; a clean-machine test is required. [Tauri sidecar documentation](https://v2.tauri.app/develop/sidecar/).

~~~mermaid
flowchart LR
    C[Codex] --> R[Local routing]
    A[Claude Code] --> R
    O[OpenCode] --> R
    R --> H[Headroom engine]
    H --> P[Model provider]
    U[Desktop UI] <--> M[Local manager]
    M --> H
    M --> D[Metrics and connection records]
    M --> I[Client adapters]
~~~

### Component responsibilities

- **UI:** Shows state and calls defined actions. Does not generate free-form shell commands from file paths.
- **Local manager:** Engine ownership, ports, health, controlled shutdown, config operations, and metric normalization.
- **Client adapters:** Detect → plan changes → apply → verify real behavior → undo. Keep client-specific differences encapsulated.
- **Headroom engine:** Supported protocol routing, compression, and retrieval. Record behavior that depends on the upstream version.
- **Local records:** Connection manifest, versions, metric events, and last verifications. Not a raw prompt/response store.

Do not handle conflicting upstream, profile, or sign-in settings with a shared proxy where the last writer wins. Start with shared trusted settings; if separation is needed, create a separate app-owned engine group. Do not change an active session's settings because of another connection.

### Distribution target

Target a Windows installer that does not require end users to install Python, Node.js, Rust, or a terminal. The installer manages prerequisites such as WebView2/runtime clearly. uv may be used during development, but it must not remain a permanent end-user requirement.

Do not make Windows 11 the installer's minimum version. Check for WebView2 on Windows 10 22H2 and Windows 11; provide a supported installation flow if the runtime is missing. Shell support is not enough: separately test Headroom, packaged Python/native dependencies, system tray, and update path on both operating systems. [Tauri system prerequisites](https://v2.tauri.app/start/prerequisites/), [Windows installer and WebView2 distribution](https://v2.tauri.app/distribute/windows-installer/).

The 19 October 2026 prototype used Headroom 0.41.0, which required Python 3.10 or later. On 16 November, the Windows development and package manifests pinned Headroom 0.44.0 and embedded CPython 3.13.13. The current branded Windows 10 x64 ZIP is a verified unsigned local preview. The earlier native macOS 15.8.2 ARM64 acceptance covers 0.41.0. CircleCI built the 0.44.0 ARM64 package and verified local engine traffic. Native window acceptance for that macOS version and clean Windows 10 and Windows 11 x64 verification remain open. [Initial package record](https://pypi.org/project/headroom-ai/0.41.0/) · [Windows manifest](runtime/manifest.windows.json) · [Pin transition](docs/proofs/headroom-044-upgrade.json) · [Windows 10 delivery](docs/proofs/pennypinch-windows10-delivery.json) · [Platform validation](docs/validation/desktop-platforms.md)

## 13. Configuration, data, and application boundaries

- Local connections stay on loopback. Management actions are accepted only through the app's authorized local channel.
- Sign-in tokens and API keys are not written to UI logs or the metrics store. Copying existing client auth files is not the normal setup method.
- **User scope as of 11 November:** H052–H054 plan a dedicated Cloudflare Workers/D1 report API and authorized panel, user-previewed feedback submission, and optional daily savings summaries. After explicit selection of the data shown at first launch, submission becomes automatic; users can opt out and clear pending reports. This is separate from the local metrics-sharing preference. Daily installation summaries are retained for 30 days; older statistics are aggregated without installation IDs/daily detail. Code/conversations, project paths, auth secrets, and account/chat IDs are excluded from remote summaries. Participation and measurement basis remain visible; different bases are not combined. Free-tier quotas, data region, access, deletion, and retention must be accepted against the real deployment. [Feedback and sharing tasks](TASKS.md#geri-bildirim-ve-isteğe-bağlı-kullanım-paylaşımı--11-kasım-kullanıcı-kararı).
- **Privacy acceptance for remote reports.** Show a notice and obtain separate consent before sharing usage data. Use measurements made after opt-in. Make withdrawal, access, and erasure available. Apply the 30-day limit to active records and verify backup retention. Confirm the legal basis for the real hosting route before collection. Keep remote sharing off until these checks pass. [Privacy decision](docs/decisions/032-usage-sharing-privacy.md)
- Local statistics are on and the upstream anonymous usage beacon is off by default: on supported versions, verify HEADROOM_TELEMETRY=on and HEADROOM_BEACON=off together. These are distinct features. [Headroom telemetry description](https://docs.headroomlabs.ai/docs/proxy#starting-the-proxy).
- The engine sends requests to the selected model provider; package/model downloads, when needed, are shown separately. Do not promise “the app never accesses the internet.”
- User data is not written to the installation folder. The suggested root is %LOCALAPPDATA%\HeadroomDesktop, with config, state, engine, and diagnostics subdirectories.
- Track app-owned processes and files in a manifest. Do not terminate unknown port listeners or claim other Headroom installations.
- Config edits are atomic and validated. Record before/after values and fingerprints for fields changed by the app.
- Undo removes app-owned fields. If a later user change conflicts, show a clear diff instead of silently overwriting it with an old backup.
- Track engine and application updates separately. Check upstream licenses/NOTICE files when preparing distribution; do not manually change upstream files.

Compare the real privacy and local-control behavior with the selected engine version's security documentation. [Headroom security model](https://docs.headroomlabs.ai/docs/security-model).

## 14. Proposed project layout and code contract

The tree below is the initial design proposal. Current source paths and implemented slices are tracked in the task list. [Task list](TASKS.md)

~~~text
HEADROOM_DESKTOP_SPEC.md       Product scope and acceptance criteria
src/                          React + TypeScript UI
  features/                   overview, integrations, activity, settings
  components/                 Shared accessible UI components
  i18n/                       Turkish / English text
  lib/                        Types and local API client
src-tauri/                    Rust desktop and management layer
  src/integrations/           Codex / Claude Code / OpenCode adapters
  src/runtime/                Engine processes and health
  src/metrics/                Counter normalization and local records
tests/fixtures/               Config files and protocol examples
tests/e2e/                    Setup, connection, and UI flows
scripts/                      Packaging and release helpers
docs/                         Technical decisions and short user guide
work/                         Temporary workspace; not source or user records
~~~

TypeScript strict mode is used; client/engine output is validated as untrusted external data. Use typed actions in Rust instead of free-form command strings. Keep UI copy out of code. Preserve counter uncertainty in the data type.

Example in-app contract; this does not claim that Headroom has a ready-made response schema:

~~~ts
type SavingsSnapshot = {
  scopeId: string;
  observedAt: string;
  source: "proxy" | "mcp";
  counterBasis: "provider_tokenizer" | "local_estimate" | "unknown";
  inputTokensBefore: number | null;
  inputTokensAfter: number | null;
  tokensSaved: number | null;
  modelId: string | null;
  agentKind: "main" | "subagent" | "unknown";
};
~~~

An adapter preserves not only request/response format but also error behavior, auth, model identity, and metric semantics. Test the mapping between raw data and the UI contract against the selected engine version.

## 15. Development commands

The following entry points are used from the Windows workspace root. Node and pnpm versions are pinned in the repository. Project-local caches, offline Rust commands, and the final candidate's local quality gate are defined in the development workflow. [Node pin](.node-version) · [Package commands](package.json) · [Local quality gate](docs/development/WORKFLOW.md#bugün-çalışabilen-yerel-ürün-kapısı)

| Purpose | Current command |
| --- | --- |
| Install locked dependencies | `pnpm.cmd --store-dir work/caches/pnpm install --frozen-lockfile` |
| UI or desktop development | `pnpm.cmd dev` or `pnpm.cmd tauri:dev` |
| TypeScript check and UI build | `pnpm.cmd build` |
| Application contract tests | `pnpm.cmd test:app` |
| Browser accessibility and UI checks | `pnpm.cmd test:a11y` |
| Windows preview package with the prepared engine | `pnpm.cmd build:windows` |

## 16. Validation strategy and acceptance criteria

Application contract tests use Node's built-in test runner, browser checks use Playwright, and Rust uses local tests. Installer and tray acceptance require native execution. Track the following product acceptance criteria separately for Windows 10 22H2 x64 and Windows 11 x64 in the task list. [Task list](TASKS.md)

### Initial technical validation

- [ ] The packaged Headroom engine starts in clean Windows 10 22H2 x64 and Windows 11 x64 environments; dependency and download requirements are recorded.
- [ ] Show a small real request routed from the Codex desktop app; preserve model name and streaming.
- [ ] In a sample using a main agent and a subagent, verify both routes separately and preserve the subagent's model and effort.
- [ ] Run the same small model/stream/tool validation for Claude Code CLI and OpenCode CLI.
- [ ] Compression occurs in a recorded example, and the original content is retrieved from the same installation.
- [ ] Pause works without breaking existing history or retrieval.

Real model checks are small and run separately; the automated test suite does not make paid API calls on every run. Do not present an unverified combination as supported.

### First usable release

- [ ] Users can install and connect without opening a terminal in clean Windows 10 22H2 x64 and Windows 11 x64 environments.
- [ ] Preservation of user settings, conflict behavior, and undo pass against Codex TOML, Claude JSON, and OpenCode JSONC fixtures.
- [ ] Wrong ports/processes, malformed config, auth errors, engine crashes, and post-update states are understandable.
- [ ] Supported clients are detected automatically; after a one-time user connection, the same choice is remembered on the next launch. Newly detected clients are not changed automatically.
- [ ] Subagents on the same routed scope do not require separate manual setup; show status accurately when a separate provider/cloud route is not covered.
- [ ] Model changes within a supported route do not require setup again; handle unknown models and unsupported content without changing the model/effort.
- [ ] Empty, missing, and negative counters display correctly; no double-counting or treating unknown as zero.
- [ ] Do not show fake savings/connection badges when there is no valid data.
- [ ] Preserve streaming, tool calls, error responses, and CCR retrieval protocol contracts.
- [ ] Check dark/light themes, both languages, keyboard, focus, high DPI, and reduced motion.
- [ ] Closing the window and using the system tray do not unnecessarily terminate the engine.
- [ ] Uninstalling does not delete user-owned new settings or client installations.
- [ ] Record installation, startup, connection, streaming, retrieval, pause, tray, high-DPI, and uninstall results separately on Windows 10 and Windows 11; a PASS on one system is not evidence for the other.

Performance targets are not published benchmarks: on the reference Windows machine, warm visible-window startup ≤2 seconds; metric freshness in the focused panel ≤2 seconds; average idle UI CPU <1%. Measure UI memory separately from engine memory; do not retain optional ML downloads. Refine targets against reference hardware in a technical prototype.

Use focused checks during implementation. After review and fixes, run the final candidate's full local quality gate once. Reuse a deterministic PASS only when the relevant source/config, environment, and toolchain fingerprints match; refresh live connections at the appropriate stage.

## 17. Scope boundaries

**Always:** Preserve existing user settings; show the correct scope; provide undo; redact secrets; identify metric sources; detect model/effort changes; prove real connections.

**Requires a new product decision:** Cloud accounts or cloud sync, automatic model routing, taking over client auth management, broad paid A/B testing on real traffic, enterprise multi-user management, or an upstream engine fork. Record a scope change in this specification first.

**Will not do:** Present a compression percentage as credit savings; silently change the user's model; treat extension support as proven by CLI evidence; claim to capture all computer traffic; install general HTTPS interception by changing client certificate trust; save raw conversations by default; overwrite later client config changes with an old backup.

Audio, images, cloud Codex sessions, AI memory sync, and automatic instruction learning are not first-release requirements. Add them in later releases only if requested and with a capability matrix.

## 18. Product phases

1. **Technical feasibility:** Codex desktop, Claude Code, and OpenCode routes; evidence for auth, model, streaming, subagents, retrieval, pause, and packaging.
2. **First daily use:** Setup wizard, connection adapters, local engine management, simple dashboard, system tray, and undo.
3. **Release quality:** Separate clean-environment tests for Windows 10/11, installer, compatibility catalog, localization, accessibility, and version management.
4. **Evidence-based expansion:** Content types, local preview, verifiable project breakdowns, and new clients.

The task list maintains phase order, dependencies, files, and acceptance evidence. [Task list](TASKS.md)

## 19. Open decisions

- First-download, WebView2, and offline-start acceptance for the packaged engine on clean Windows 10 and Windows 11 x64. The current Windows engine's file count and total bytes are in the versioned inventory. [Package inventory](runtime/windows-payload.json)
- First-release combinations verified for Codex desktop/CLI and each sign-in method.
- Release order for macOS/Linux and additional editor extensions.

H031 made Windows startup opt-in. Local metrics default to 30-day retention, with 7-, 30-, and 90-day options. CCR has a separate 7,200-second TTL. [Operation and retention preferences](docs/decisions/019-operating-preferences.md)

## 20. Product requirements drawn from another desktop app

The [source comparison dated 20 October 2026](docs/research/desktop-comparison-2026-10-20.md) reviews commit 49a6161 of the gglucass/headroom-desktop repository. Features found in that source are not capabilities implemented by us. Preserve the existing 39 tasks and add H040–H045; this document does not complete any new feature.

Requirements added to the first release:

- **Stable connection endpoint:** Do not leave a client on an old temporary port when the engine restarts. H040 identifies ownership, performance, and shutdown evidence for a persistent entry point and alternatives, then implements the selected approach. A global port 6767 or two-proxy architecture is not mandatory in advance. H017 config planning follows this acceptance.
- **Verified failure path:** Offer direct pass-through only when proven for the same auth/model/reasoning/upstream and protocol. Never blindly retry a request already sent upstream or send a second POST in the middle of a stream. An unknown route fails; it is not redirected to another provider. Separate the CCR/prefix effects of pause, disconnect, and failure.
- **Benefit and integrity tests:** H041 evaluates source/numeric content, tool identifiers, CCR, cache prefix, cold/warm sessions, and request latency together. Do not infer response quality from byte-exact retrieval. If there is no real model experiment for quality, keep it open; a benchmark does not automatically start paid model requests.
- **Signed updates:** H042 covers app/engine manifests, stable/preview channels, signature/hash checks, application after stream completion, and rollback to the last working version. Do not update clients on the user's behalf or silently change the engine version through a floating pip dependency.

Optional scope after the first usable release:

- **RTK:** H043 gathers client-specific technical evidence; H044 covers managed installation and explicit enablement. Do not change model/auth/reasoning or agent count. Preserve source, exact numeric output, and test/error data. Show RTK and Headroom savings separately unless equivalent scope and connection evidence exist. Do not modify global hooks/shell settings automatically.
  RTK-OSS Apache-2.0 terminal filters are an optional candidate; RTK engine and the separate proxy are BUSL-1.1 and are not included or distributed automatically in this scope. Documented rtk claude usage does not prove coverage for Codex/OpenCode, PowerShell, or local tools. [Source-to-target mapping](docs/research/optimization-targets-2026-11-04.md).
- **Local recommendations** (H045) produce health and optimization recommendations from content-free metrics. They make no additional model call and do not themselves change profile, model, or config. Do not estimate unknown traffic or savings.
- **Learn preview** runs on Windows when a user selects a Codex or Claude Code JSONL file in Settings. It shows a repeated failed-call pattern from one file, the first occurrence's line, and the result of the next equivalent call. Analysis stays on the device and the result lasts only as long as this screen. Engine background learning and memory tools are off by default. [Decision](docs/decisions/027-optimization-regression.md#learn-önizlemesi).

- **Experimental rules** are a separate, **off by default** feature under Settings → Learn. The Windows local preview shows rule candidates from one user-selected Codex or Claude Code JSONL file. A separate preference is saved and restored on startup. Selecting a Learn file or starting the engine does not enable the preference. Canceling file selection preserves the prior result. If the preference is turned off during preview, the result is not accepted. **Try locally** tests the duplicate-log-line rule against the same file's original bytes held in memory; it shows exact round-trip equality and the stdout bytes tested. One record is limited to 16 MiB, access lasts 15 minutes, and it is released when the screen closes. Ambiguous error/channel outputs, stderr, and full source reads are preserved. This local experiment does not change live traffic. Quality/usage acceptance and versioned enablement in the real task remain in H041-e3–e4. Those steps preserve model/reasoning/auth/agent count, full source/numeric reads, and client permissions. Enablement applies to a new session in the supported connection/project scope. Turning it off preserves active streams and prior original/CCR access. [Scope, order, and acceptance](docs/decisions/027-optimization-regression.md#h041-e-deneysel-kurallar).

Product principles:

- Do not bulk-retag Codex's existing chat database. Any migration needs separate scope, preview, and undo; the existence of current sessions is not permission.
- Do not adopt the approximately 50% savings or 2x quota claims found in comparison sources as our product promise. Main input-token reduction, cache effects, output estimates, and monetary equivalents remain separate; preserve negative differences and unknown metrics.
- The setup screen shows every client/hook/shell area that will change and the size of the core engine/optional components. Heavy ML/MCP tools are not required in the core installation.
- Do not add account/subscription requirements or remote telemetry to the local core because of this comparison. macOS/Linux/Grok and other extra tools do not expand the initial Windows 10/11 scope.
- If licensed source is redistributed, record its pinned source and notices in the component/NOTICE inventory; do not add a dependency on a proprietary service. Tests from the reviewed app are not our test results.

New task IDs were appended to preserve previous records; read task order from dependencies. The rule to stop at the end of each main task and wait for the user's continuation remains in force.
