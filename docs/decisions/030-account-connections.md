<a name="h050--hesap-durumu-ve-resmî-giriş"></a>

# H050 — Account status and official sign-in

13 November 2026. The lead agent owns implementation and auth/process decisions. The same single Luna worker performs only a read-only review of fixed Claude/OpenCode command contracts. H049 was deferred by user decision. CI is not used.

<a name="sözleşme"></a>

## Contract

- Installation, local sign-in record, Headroom routing, and measured traffic are separate states. Reading account status does not select a model/reasoning/provider or send a model request. A local credential record does not prove a valid remote account/subscription.
- Require a fresh discovery selection and repin the trusted CLI executable before running it. Never take a client path, argv, or URL from the WebView. Report unknown for unsupported Desktop, OS, or version contracts.
- Run the official Codex status command with 10-second and 64 KiB per-stream limits, using only its own process tree. Do not use the Claude Code 2.1.158 status command for passive reads because an isolated experiment showed it writing settings/backups. Report unknown with reason `read_not_passive`. Do not use OpenCode 1.19.3 `auth list` because it creates a global DB/WAL/SHM. Read only local provider metadata from the pinned V1 schema, with bounded parsing and file-identity checks.
- Do not copy raw stdout/stderr, key fragments, email, tokens, or auth files into Headroom state/logs. Reduce OpenCode values during streaming to presence/absence only. Unknown provider names become `other`. The UI receives only fixed categories/states. Passwords, API keys, or OAuth secrets are not entered in Headroom.
- Official sign-in requires an explicit user action. Show the affected CLI profile and explain that the official client may change its current session. Cancel stops only the launched CLI process. It does not cancel the browser, affect other sessions, or roll back the auth store. Do not infer success from a canceled sign-in. Read status again.
- Do not infer account status from an old installation record or process presence. Read fresh metadata after reopen and make clear that live account access was not verified. Preserve the user's prior choice not to sign in to Claude.
- Open sign-in in a separate visible official CLI console. Headroom manages only the root process handle it started. Do not put the browser in a kill-on-close job. Normal full quit and the 600-second timeout stop the CLI root process. Do not claim sign-in will end automatically if the process crashes abruptly. If closure cannot be verified, preserve ownership, block new operations, and allow stop to be retried. Screen changes/tray hiding do not cancel the flow. If the IPC reply is lost, refresh finds the existing flow.
- The Codex sign-in action opens ChatGPT. Claude Code sign-in opens a Claude account. OpenCode lets the user choose provider/method in its own screen. State clearly in the confirmation screen that the existing API-key method may change. Even a successful exit code does not prove a valid remote session. Read the local record again.

<a name="kaynak-ve-kritik-sınır"></a>

## Sources and critical limits

Reviewed [OpenAI authentication](https://learn.chatgpt.com/docs/auth), [CLI login/status](https://learn.chatgpt.com/docs/developer-commands?surface=cli), and the [official CLI login source](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/cli/src/login.rs). Status reports credential presence and may also print API-key-formatted fragments. Do not retain those fragments. Login may clear existing auth before the flow completes. Cancel does not mean the old auth was preserved. Pin the reviewed version sources and hashes in task evidence.

Do not project V2 behavior in the current [OpenCode documentation](https://opencode.ai/docs/cli/providers/) onto V1 1.19.3. Reviewed the [pinned V1 auth schema](https://github.com/anomalyco/opencode/blob/e3c4eb242585dcd77be1a867391b7c25fd47ce52/packages/opencode/src/auth/index.ts), global data root, and xdg-basedir 5.1.0 sources. If `OPENCODE_AUTH_CONTENT` is set or the store is unsafe/unknown, keep status unknown. Reviewed the Claude command contract using help output from the pinned installed client. No command was run against a user's account.

Windows CLI auth capability is limited to Codex `0.163.0-alpha.11.1`, `0.163.0-alpha.11.2`, `0.167.0-alpha.4.1`, Claude Code `2.1.158`, and OpenCode `1.19.3`. These are not limits on model/subagent support. Require the real `.exe`, version/digest or signature verification, and pinned parent directories. If there is no trusted adapter for wrappers, Desktop accounts, or another OS, keep status unknown.

<a name="kabul-ve-doğrulama"></a>

## Acceptance and verification

1. Fresh installation selection → read status → allowlisted metadata only. Distinguish present/absent/unknown and Desktop/unsupported. Malformed, oversized, failed, or timed-out output does not become connection success.
2. Show the user the scope → open official sign-in → cancel/exit/read status again. Reject stale selection, modified executable, and competing login. Do not close another process/browser.
3. Accounts UI uses the real IPC path. Check TR/EN, empty/unknown/error, keyboard, and final native rendering. Keep fixture and real-client evidence separate. Clearly state when no live sign-in was performed.

Run focused Rust `integrations::accounts` and pin-regression tests, frontend accounts contract/interaction tests, and the final candidate's full local WORKFLOW.md gate once. Local evidence is from Windows 10. Do not project it onto Windows 11, a new macOS feature, or Linux. Reuse previous passing tests only when their relevant fingerprints match.
