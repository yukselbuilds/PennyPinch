# H019 — Codex connection, partial evidence

22 October 2026. **H019 remains open; 15/49 main tasks complete.** This is development evidence on Windows 10 Pro 22H2 x64 (19045), not a release or universal client/model compatibility claim.

## Current client and transport fixture

The installed CLI reports `0.163.0-alpha.11.1`; its executable has SHA-256 `e757246b4de87359e660647fa9514955a0971ff6a30affdd8c145d1d5ece7b84` and size 336,011,568 bytes. Its PE FileVersion/ProductVersion fields are empty. A read-only login-status check returned the category ChatGPT; no existing credentials were copied or saved as evidence. This category does not prove inference.

An isolated CLI home used the built-in provider's `openai_base_url`, `gpt-6-luna`, `xhigh`, a synthetic sentinel and a local HTTP/SSE fixture behind the pinned Headroom 0.41.0 engine. The current CLI completed one marker turn, exited 0 and left its config unchanged in 6.60 seconds. Both the developer-only `generate:false` warmup and the subsequent user turn kept the model, reasoning and sentinel. The owned engine listener closed afterward. The fixture returned synthetic output and made no real provider request.

Local metadata: [acceptance](../../work/proofs/codex-connection/runs/372028c6-5a75-43a2-8a8c-8abca3d64587/acceptance.json), [runner](../../work/proofs/codex-connection/probe_builtin_route.py). No raw prompt, output or auth was persisted. Earlier failed harness attempts are not PASS evidence: the first validator did not classify Codex's separate warmup request. The fixture was corrected to distinguish this bounded warmup from a real user turn.

This extends the exact CLI fixture allowlist to `.11.1`, alongside H008's `.10.2`. Neighboring versions remain unverified; Desktop and main/subagent routing do not inherit this result. This standalone engine fixture does not prove the native UI → stable ingress → provider chain.

## Implementation and focused checks

- The real Tauri status command derives a selection-bound snapshot from DiscoveryManager, PlanManager ownership and RuntimeManager observations. It separates ownership, endpoint binding, engine readiness, config that has not been re-read, auth scope and traffic that has not been verified. TCP counters cannot produce a verified result.
- Backend focused checks: 8 status tests and 17 plan tests PASS after the exact-version update. Stale generation/wrong client are rejected before ownership/runtime snapshot construction; missing/conflicting ownership and stopped engines remain distinct.
- The strict TypeScript decoder binds responses to the request and installation, rejects unknown fields and excessive fixture claims, and drops stale UI completions. Its 11 focused tests PASS, including ready-string/false-flag contradictions and independent ingress/engine failure states.
- A full-width Codex panel below the compact discovery cards exposes four primary facts and an accessible details disclosure. Auth and agent unknowns remain visible. Config apply does not silently restart or reroute an existing Codex conversation.
- Read-only bundled-CLI discovery examines at most 64 direct entries and 16 direct version directories under the known local install root. Exact size and digest are required to identify this binary; its directory name is not evidence. Manual EXE selection uses the same check; unknown binaries retain an unknown version. Discovery's 20 focused Rust tests and the 14 Node integration tests PASS.

## Local candidate checks and anomaly

The frontend gate passed 60 Node tests and the TypeScript/Vite build. The first full Rust run returned 172 pass, 1 fail and 3 ignored. The wrapper retained counts but failed to retain the failing test name. An unchanged-source diagnostic recheck returned 173 pass, 0 fail and 3 ignored (4.77 seconds of tests). Read-only inspection found no new residual fixture directory that identified the first failure. Short-deadline tests exist, but scheduling/OS contention is only a hypothesis. **The first failure remains unexplained and is not marked resolved.** Future required gates collect bounded failed-test identifiers without raw output.

Formatting and the embedded custom-protocol build passed. Native visual review then changed only three TSX/CSS layout inputs. The frontend and embedded builds were refreshed for that concrete change; matching Node/Rust/format results were not repeated. Current executable: 16,000,000 bytes, SHA-256 `7d86eb77ab09e4dc6d21e6205fb21fb939294fcf0f704f9e5aef9647863c1858`. Two existing dead-code warnings and their Cargo summary remain. The 62 source/config hashes, tools and dist hashes are in [final-quality.json](../../work/proofs/codex-connection/final-quality.json); [test-anomaly.json](../../work/proofs/codex-connection/test-anomaly.json) preserves the limitation.

## Native UI evidence and limits

Open Computer Use operated the actual embedded Tauri window with physical mouse/keyboard input, not a browser mock. Disposable child environment/configs and a byte-identical copy of the real CLI stayed under the project. The copied CLI was discovery data and was never executed in this native test. At 1280×800, automatic discovery reported `.11.1`; actual IPC showed engine ready/stopped separately from ingress and consistently kept client traffic unverified. The compact cards' connection actions remained visible, and the four-fact status panel displayed auth/agent limits. Mouse expansion and Space collapse of details worked; endpoint binding, user config and values not re-read were explicit.

Native Apply changed only the owned test `openai_base_url`; model `gpt-6-luna` and reasoning `xhigh` remained. Phase A reached engine ready and exited normally (0). Reopening with the reviewed layout retained identical config/ownership hashes without reinstalling. Undo worked without a new preview and restored the initial config byte for byte. Config/record checks: [native-checks.json](../../work/proofs/codex-connection/native-checks.json). Runtime Rust/config inputs did not change with the layout refresh, so phase A's normal shutdown is reused only for that unchanged runtime path.

Manual-selection/rescan native acceptance is **not tested**. Its attempt opened a Windows warning because the disposable profile initially lacked Desktop. The profile directory was corrected, but automation refused input to the covered child dialog. A later close check also hit the foreground guard. Those phases are not normal-exit PASS results: only their identified owned processes were stopped after Undo, with preferences preserved. No test process or listener remained on port 51778. These automation/setup limits do not demonstrate a product defect or successful manual selection. See [native-proof.json](../../work/proofs/codex-connection/native-proof.json), which is explicitly PARTIAL.

![Native Codex status after reopen, before Undo](../../work/proofs/codex-connection/native-status.png)

## Deferred and missing acceptance

The user explicitly deferred ChatGPT sign-in. Both isolated login attempts were stopped; neither created `auth.json`, and the login tab was closed. No new sign-in or real request is authorized by a generic continuation. Existing account credentials are not substituted for that declined test.

A product action that proves a real completed client request through the bound endpoint is not implemented yet. Live ChatGPT/API-key inference, the effective Desktop route, main/subagent coverage and real savings remain unverified. Native status UI and a local sentinel fixture cannot substitute for those checks. H019-d and the main H019 checkbox remain open.

See [decision 007](../decisions/007-codex-live-connection.md). Current official [Codex configuration](https://learn.chatgpt.com/docs/config-file/config-reference), [authentication](https://learn.chatgpt.com/docs/auth) and [Responses warmup documentation](https://developers.openai.com/api/docs/guides/websocket-mode) describe configuration and protocol possibilities; support claims here are limited to the observed local results.
