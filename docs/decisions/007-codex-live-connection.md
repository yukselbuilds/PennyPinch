# Decision 007 — Codex connection status and live verification

22 October 2026. **H019 active; implementation and acceptance pending.**

## Scope and evidence boundary

H018 provides config planning, owned atomic apply/undo and persistence. Applying an endpoint does not prove model traffic. H040 ingress counts TCP connections, not requests, client identity, auth, models or agents. H019 must expose these distinctions through actual manager/IPC/UI paths.

H008 is partial fixture evidence for Codex CLI `0.163.0-alpha.10.2`; no real provider, ChatGPT auth, Desktop or main/subagent route was accepted. The current installed CLI reports `0.163.0-alpha.11.1`, executable SHA-256 `e757246b4de87359e660647fa9514955a0971ff6a30affdd8c145d1d5ece7b84`. A read-only `codex login status` reported ChatGPT. Only this category was returned; raw auth output was discarded. This does not establish inference traffic. The isolated built-in-provider route through pinned Headroom 0.41.0 and a loopback SSE fixture passed with model/reasoning/sentinel preserved and config unchanged; this justifies adding only the exact `.11.1` CLI fixture scope. [Evidence](../proofs/codex-connection.md).

## Product behavior

The backend derives a Codex-specific snapshot from trusted discovery, config ownership and ingress state. Bind status to the selected installation/config/endpoint; stale or conflicting state cannot produce success. Keep config ownership, runtime readiness, new-process/restart scope and traffic verification separate. Preserve model/reasoning/provider/auth declarations without exposing secrets. Unknown effective values remain unknown.

The UI distinguishes CLI from Desktop and ChatGPT from API-key evidence. Coverage of the main agent and subagents remains unknown/not tested until observed. A shared endpoint or matching model alone is insufficient. No per-model or per-agent setup is introduced. No live verifier exists yet; never manufacture a verified state from counters, config application or a local fixture.

## Ordered acceptance

1. Backend state and focused tests, including wrong version/surface, missing or conflicting ownership, stopped/wrong endpoint and unverified traffic.
2. Strict IPC decoder and Graphite UI, with scope and restart/unknown evidence visible.
3. Isolated current-CLI path through real owned runtime and loopback fixture. Record this as fixture evidence; do not substitute it for provider acceptance.
4. One small natural real-model flow, followed by reopen/persistence and UI-to-manager-to-engine-to-client checks. Keep H019 open if external auth or route acceptance remains missing.

No paid API call/key acquisition, existing credential copying, provider switch, model/reasoning change or current conversation restart. Existing user settings and other active sessions remain unchanged during experiments. New test profiles/state stay beneath the project, with generated credentials held only in their explicitly owned auth store if the user signs in normally. Raw prompts, model content, auth and failure output are not persisted as proof. The previously rejected H008 inner PowerShell/file-read path is not retried through another entry point.

The user deferred sign-in; both isolated login attempts were stopped without creating an auth file. A generic continuation does not revoke that decision. Independent implementation and fixture/native checks may continue, while real auth/provider acceptance stays open.

## H019-d1 — Read-only current setting observation

The next independent prerequisite is to recheck the currently owned Codex endpoint setting. A saved ownership record and a matching runtime endpoint do not establish that the current config still contains that endpoint. Reuse H018's safe config-root resolution, bounded reads, ownership path binding and transaction/rollback rules. Observe the record and config consistently; stale scope, busy access, unsafe paths and unreadable/invalid files must not report a match. Never fall back to another config or credential store.

Return only fixed redacted setting-state codes. Do not return a path, URL, config content, model/auth literal, parse error or credential. Released/absent ownership and unsupported installation surfaces remain distinct from an active matching setting. A matching setting is a point-in-time disk observation, not effective adoption by an open Codex process, auth support, traffic verification or agent coverage. This operation neither changes files nor launches a client/provider request. The UI must explain changed/missing/unreadable settings without automatically repairing them.

## H019-d2 — Ordered next-step guidance

23 October 2026. Use the existing fresh, strictly decoded status snapshot to give one actionable next step. This is a read-only diagnosis, not a live-verification executor or an authorization to send a request. Reuse the current update action and request/selection invalidation; introduce no ticket, timer, polling, extra IPC or unused future verifier state.

Unsupported surfaces/versions, inaccessible ownership, absent ownership, unsafe/changed/missing settings, mismatched endpoints, unavailable ingress and an unready engine must each prevent a local-ready recommendation. Config differences require reviewing the existing setting/record while preserving user edits; do not recommend blind overwrite or automatic repair. Only a supported fixture CLI with active ownership, matched disk setting, matching endpoint and verified ingress/engine readiness can report that local prerequisites are ready. TCP counts never affect this decision. Even then, traffic, process adoption, auth and main/subagent coverage remain unverified and the product live-test action remains unavailable.

The compact localized guidance appears in the existing status panel. Its useful output is the next corrective step, rather than another table of readiness facts. Backend/protocol/auth behavior remains unchanged. Acceptance covers a blocker-priority test matrix, the final frontend/build candidate and real native UI/IPC with isolated fixture config; it cannot close H019 or replace deferred real-provider validation.

## Sources checked

Current official [configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference) and [advanced configuration](https://learn.chatgpt.com/docs/config-file/config-advanced) describe `openai_base_url` for the built-in provider and exclude provider/auth overrides from project config. The [authentication guide](https://learn.chatgpt.com/docs/auth) separates subscription and usage-billed API-key access. Documentation describes possibilities; the installed client and pinned Headroom runtime still require their own route proof. The existing gateway/Sign-in-with-ChatGPT integration examples do not authorize copying the user's existing tokens or changing their provider.
