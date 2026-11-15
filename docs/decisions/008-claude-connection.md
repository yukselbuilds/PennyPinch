# Decision 008 — Claude Code connection scope

24 October 2026. H020 accepted within the local Windows10 CLI/fixture scope; see docs/proofs/claude-product.md.

15 November 2026. H009 local technical acceptance is complete for Claude Code 2.1.158 and Headroom 0.41.0 on Windows 10 22H2 x64. [Acceptance and limits](../proofs/claude-acceptance.md) bind the historical native UI transport, H022 onboarding and isolated MCP/server-side CCR proofs. No live account, provider, subagent or extension support is added. K2 retains untested combinations as experimental.

## Required behavior

Use the existing discovery, plan, atomic Apply and conflict-preserving Undo path. Do not introduce a second config writer or a copy of the Codex verifier. The selected Claude executable and configuration scope are independent identities; a detected executable does not identify another running session's working directory or effective settings.

For a user-selected project, prefer its personal `.claude/settings.local.json` over a machine-wide user edit. A user-settings connection must be an explicit, separately labeled scope. Never infer a project from Headroom's own working directory or scan unrelated project trees. Project shared settings, user settings and managed policy are observations, not automatically writable targets. A managed restriction or an unknown/unsupported route cannot be silently overridden.

Scope must remain consistent through the preview, guarded lease, config target, ownership receipt, Apply, Undo and UI. Retain existing user-config receipts and preserve Codex/OpenCode behavior. A project-scoped receipt must not be labeled `user_config`. Any supported concurrency/remembering limitation must be explicit; do not implement H022's general onboarding and connection persistence as an incidental part of this task.

The initial H020 project adapter required an existing safe `.claude` parent directory and manual project reselection after reopening. H022 superseded these two limitations with [transactional first use and remembered context](../proofs/onboarding.md). Preview remains read-only. Explicit Connect creates missing parents inside the shared transaction, and Undo preserves later user additions. One active Claude ownership record is supported. Remembered scope does not prove another running client's effective settings or adoption.

## Preservation and evidence

Only the endpoint field is managed. Existing auth, arbitrary model identifiers, effort, hooks and unrelated settings remain unchanged. Inspect only bounded settings data and report fixed metadata categories; never copy credential stores or expose configured secret values. CLI flags, external process environment, remote policy and an already-running session's adoption cannot be inferred from disk alone. Config application is not a live-traffic success.

An unobserved policy source is not a blanket prohibition on authorized user/project config edits. Preserve unknown applicability and block observed route/policy conflicts; do not claim the edit overrides policy or proves adoption. A local file observation never establishes the absence of registry, remote or host-supplied managed settings. The official [managed-settings deployment page](https://code.claude.com/docs/en/managed-settings) names `C:\Program Files\ClaudeCode\managed-settings.json` as the Windows file source. Any implemented observation is restricted to that bounded backend-owned source; newer policy composition features are outside the pinned CLI evidence.

H009-a proved Claude Code 2.1.158 → pinned Headroom 0.41.0 → a local Anthropic SSE fixture with synthetic auth and tools disabled. H020 separately accepted actual UI→manager→owned-runtime/config→CLI streaming and scoped Apply/Undo. H009-b accepted two native MCP calls and byte-exact server-side CCR through an instrumented ingress. Its synthetic profile overrides protected tools and is not a native UI tools/retrieval proof. These records keep their own source/executable fingerprints. The final current-source gate is recorded separately; old executables are not represented as the current candidate. OAuth, live provider/model behavior, general hooks/plugins, client-side retrieval, subagents and Desktop remain untested.

Separate successful local fixture/config coverage from untested live auth and agent routes in `compatibility/clients.json`. No model allowlist or per-agent setup is introduced. A chosen model label is fixture input, not proof of model feature support. No new paid service, login, provider account or model call is required for the local fixture acceptance.

## Source boundary

The official [settings and precedence](https://code.claude.com/docs/en/settings) and [LLM gateway](https://code.claude.com/docs/en/llm-gateway) documentation were checked on 24 October 2026. Current documentation describes several newer releases; behavior specific to those releases is not inferred for the pinned 2.1.158 executable. Verify the supported settings-loading behavior with that exact local CLI in the isolated product acceptance.

## Shared dialog behavior

Extend the existing Graphite connection dialog only for Claude. Show user settings versus project-local scope, the chosen project directory and the exact file that will be managed. The project picker belongs to the native main window and accepts no renderer-supplied config path. Cancel preserves the current scope and selection; a changed context invalidates the displayed preview and refreshes ownership/layer metadata before mutation. Selection and Apply/Undo cannot race. Use existing theme tokens, keyboard/focus behavior and TR/EN copy.

Show observed local setting layers separately from unknown runtime adoption and external policy/session overrides. Model/auth values remain private; declarations and their source categories are sufficient. Keep config saved, actual traffic and live provider/agent compatibility distinct. Explain the single-active-Claude limitation before Apply. H022's remembered project and installation restore the selected context after normal reopen; cancellation or a different selected project must not redirect Undo to another configuration.
