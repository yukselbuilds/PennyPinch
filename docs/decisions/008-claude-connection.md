# Decision 008 — Claude Code connection scope

24 October 2026. H020 accepted within the local Windows10 CLI/fixture scope; see docs/proofs/claude-product.md.

## Required behavior

Use the existing discovery, plan, atomic Apply and conflict-preserving Undo path. Do not introduce a second config writer or a copy of the Codex verifier. The selected Claude executable and configuration scope are independent identities; a detected executable does not identify another running session's working directory or effective settings.

For a user-selected project, prefer its personal `.claude/settings.local.json` over a machine-wide user edit. A user-settings connection must be an explicit, separately labeled scope. Never infer a project from Headroom's own working directory or scan unrelated project trees. Project shared settings, user settings and managed policy are observations, not automatically writable targets. A managed restriction or an unknown/unsupported route cannot be silently overridden.

Scope must remain consistent through the preview, guarded lease, config target, ownership receipt, Apply, Undo and UI. Retain existing user-config receipts and preserve Codex/OpenCode behavior. A project-scoped receipt must not be labeled `user_config`. Any supported concurrency/remembering limitation must be explicit; do not implement H022's general onboarding and connection persistence as an incidental part of this task.

The initial project adapter requires an existing safe `.claude` parent directory. Missing parents fail with a fixed unavailable result; preview must not create directories and Apply must not create them outside the shared transaction boundary. H022 tracks the first-use parent-creation experience. The UI must explain this limit rather than representing every empty project as ready to connect. One active Claude ownership record is supported; reselect the original project after reopening to Undo until H022 adds remembered context.

## Preservation and evidence

Only the endpoint field is managed. Existing auth, arbitrary model identifiers, effort, hooks and unrelated settings remain unchanged. Inspect only bounded settings data and report fixed metadata categories; never copy credential stores or expose configured secret values. CLI flags, external process environment, remote policy and an already-running session's adoption cannot be inferred from disk alone. Config application is not a live-traffic success.

An unobserved policy source is not a blanket prohibition on authorized user/project config edits. Preserve unknown applicability and block observed route/policy conflicts; do not claim the edit overrides policy or proves adoption. A local file observation never establishes the absence of registry, remote or host-supplied managed settings. The official [managed-settings deployment page](https://code.claude.com/docs/en/managed-settings) names `C:\Program Files\ClaudeCode\managed-settings.json` as the Windows file source. Any implemented observation is restricted to that bounded backend-owned source; newer policy composition features are outside the pinned CLI evidence.

H009 proves Claude Code 2.1.158 → pinned Headroom 0.41.0 → a local Anthropic SSE fixture, with synthetic auth and tools disabled. It does not establish OAuth, live provider/model behavior, tools, hooks, plugins, retrieval, subagents or Desktop support. H020's native functional acceptance must use the actual UI→manager→owned-runtime/config path and a supported small CLI/fixture stream. Reuse unchanged proof only with matching relevant fingerprints; do not repeat the old helper-only E2E as a substitute for the product path.

Separate successful local fixture/config coverage from untested live auth and agent routes in `compatibility/clients.json`. No model allowlist or per-agent setup is introduced. A chosen model label is fixture input, not proof of model feature support. No new paid service, login, provider account or model call is required for the local fixture acceptance.

## Source boundary

The official [settings and precedence](https://code.claude.com/docs/en/settings) and [LLM gateway](https://code.claude.com/docs/en/llm-gateway) documentation were checked on 24 October 2026. Current documentation describes several newer releases; behavior specific to those releases is not inferred for the pinned 2.1.158 executable. Verify the supported settings-loading behavior with that exact local CLI in the isolated product acceptance.

## Shared dialog behavior

Extend the existing Graphite connection dialog only for Claude. Show user settings versus project-local scope, the chosen project directory and the exact file that will be managed. The project picker belongs to the native main window and accepts no renderer-supplied config path. Cancel preserves the current scope and selection; a changed context invalidates the displayed preview and refreshes ownership/layer metadata before mutation. Selection and Apply/Undo cannot race. Use existing theme tokens, keyboard/focus behavior and TR/EN copy.

Show observed local setting layers separately from unknown runtime adoption and external policy/session overrides. Model/auth values remain private; declarations and their source categories are sufficient. Keep config saved, actual traffic and live provider/agent compatibility distinct. Explain the existing-parent and single-active-Claude limitations before Apply. After restart a project-scoped connection can be removed by reselecting its original directory; do not make a remembered-context claim before H022.
