# Existing CLI authentication and live Headroom smoke

23 October2026. The user explicitly asked to test through the currently signed-in CLI rather than sign in again. That authorizes this bounded normal-CLI test; the earlier deferral of a separate isolated login remains intact. No credentials were manually opened, extracted or copied. No new login, API key, provider/model/reasoning override, existing config edit or conversation resume was performed.

## Observed result

Installed CLI is now **0.163.0-alpha.11.2**,336,134,960B, SHA-256 **`c81ed464e5e7cafd09a5fa26af390c480bed8e9fab2a87a54b285fbd56e7122a`**. This differs from the earlier `.11.1` fixture, so its exact-version acceptance is not reused for the new executable. Fresh `codex login status` returned only the ChatGPT category. Raw status output was discarded.

1. A normal `codex exec --ephemeral --json` invocation with a read-only sandbox returned the fixed auth marker, `turn.completed`, exit0 and no tool calls. Model/provider/reasoning were not overridden. This proves working existing authentication, not proxy passage.
2. The accepted native binary `9558429`/SHA `2d0f64d44cd48943e43b16ce2f5f7a7fe25bd1d3ef877732c59ddbeed768116a` and its62 source/config/dist fingerprints still matched. Root launched one owned Headroom window with isolated discovery environment, clicked Start through PID-selected Open Computer Use and observed Engine Ready. No client Apply/config write was used.
3. A second normal ephemeral CLI process used only a process-scoped `-c openai_base_url="http://127.0.0.1:51778/v1"` override plus project-owned log/cwd paths. Its usual CLI login mechanism supplied authentication; no auth cache was copied into the isolated Headroom environment. The CLI returned the unique proxy marker, `turn.completed` and exit0, without error events, in8.3s. Observed model was the CLI default **gpt-6.1-sol**; no model/reasoning/provider selection was changed for this probe.
4. Windows TCP owner-table sampling observed the exact test CLI PID→owned native ingress51778 and native ingress PID→owned engine port53786 connections. A fresh engine had0 request records before the invocation; afterward two new Codex-tagged success-path records were present. The8-output-token record matched the CLI's8 output tokens. No unrelated model request was launched by the harness.

This is a bounded **development CLI→native ingress→pinned engine→real-response witness**. It is stronger than a TCP counter alone. `/stats` does not expose the upstream URL or exact HTTP response status; those were not directly observed or claimed. ChatGPT upstream routing is inferred from the normal CLI's ChatGPT auth category and the inspected pinned routing code. This is not a finished product verifier, GUI Apply→Verify acceptance, all-model coverage, Desktop authentication or subagent-route proof.

## Runtime and privacy

The same worker verified that all five relevant auth/routing files match between the inspected0.41 environment and the deployed packaged runtime: `openai.py`, Codex `runtime.py`/`endpoints.py`, `streaming.py`, `copilot_auth.py`. Its ChatGPT classifier selects the fixed ChatGPT Codex Responses endpoint and preserves the incoming bearer header. The native manager clears/rebuilds the engine environment; full-message logging is off, wire-debug/log-file capture is unset and the beacon is off. Compression's CCR retention is separate from logs and may retain source content for its configured TTL.

The harness reduced `/stats` immediately to fixed request/token/model/latency metadata. Raw tags, errors, config, auth headers, prompts and responses were neither printed nor saved as proof. The CLI's marker/turn result and usage were parsed in memory. Test artifacts are ignored under `work/proofs/codex-auth-check/`.

## Measurement and cleanup

For the actual8-output-token response record: input original=22,040, optimized=22,040, **tokens saved=0; savings=0%**. A separate zero-output record is not used as savings evidence for the marker response. Cached input tokens from the direct CLI test are provider cache usage, not Headroom savings; the two invocations are not a controlled A/B savings benchmark. No broad savings claim follows from this short probe.

The owned Headroom window closed through verified `CloseMainWindow`, exit0. Preferences remained byte-identical. Test CLI, GUI, known/project runtime processes and ingress listeners all0; no forced termination. Existing client configs and active conversations were not rerouted.

Evidence: direct `work/proofs/codex-auth-check/590d22ed-2a7f-42ed-9dab-1466ee5d7aa1/result.json`; live `work/proofs/codex-auth-check/live-5e40db35-712f-47f5-b4b2-9eb6e822cfb2/{owned-before,proxy-result,native-run,native-close,native-cleanup}.json`. Native source/test fingerprints were reused; product source did not change, so successful local gates/CI were not repeated.

Next acceptance remains the product live-verification feature and one separately correlated main/sole GPT-6 Luna xhigh subagent flow preserving each configured model/effort. Another browser sign-in is not the current blocker. The `.11.2` installed-binary discovery/allowlist and its broader compatibility have not been silently promoted by this one developer probe. H019 stays open.

Official guidance: [Authentication](https://learn.chatgpt.com/docs/auth) documents reuse of cached CLI sign-in; [configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference) documents the built-in provider's base-URL override. Local behavior above is measured, not inferred from documentation alone.
