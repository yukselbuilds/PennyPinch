<a name="h050--hesap-bağlantıları-kabulü"></a>

# H050 — Account connections acceptance

November 13, 2026. Local development acceptance passed on Windows 10 22H2 x64. The machine record links 317 source and config inputs, tool versions, command summaries, and the native binary. The account contract defines the account, route, and traffic states. [Machine record](accounts.json), [Decision 030](../decisions/030-account-connections.md)

<a name="kullanıcıya-sunulan-davranış"></a>

## User-facing behavior

The Accounts section in Applications selects from discovered clients automatically. The user can change the selection when several installations are available. The section shows local record states of present, absent, and unknown. It shows the record type and supported OpenCode provider records. Desktop and other CLI profiles need separate verification. Reading an account writes no connection settings and selects no model or subagent.

Before official sign-in, the app shows the selected CLI version and the risk of changing the current session. Codex opens the ChatGPT sign-in flow. Claude Code opens the Claude account flow. OpenCode asks the user to select the method and provider in its own console. Headroom accepts no passwords or codes.

Full quit and timeout stop only the owned CLI root process. The app leaves the browser open. Restoring previous authentication after cancellation is outside this contract. If the app cannot confirm shutdown, it retains ownership and lets the user retry Stop.

<a name="doğrulamalar"></a>

## Verification

| Layer | Result and scope |
| --- | --- |
| Frontend contract | 216 Node tests passed. The tests cover account correlation, explicit confirmation, rejection of extra and secret fields, and flow identity. |
| Rust | 432 tests passed. The direct run ignored 16 tests. The tests cover account parsers, pin and discovery regressions, stale selection, and native console ownership. Its parent test calls the ignored child-process helper. The other opt-in tests remain unverified in this run. |
| Build | TypeScript and Vite, Rust formatting, the locked offline native debug build, and the QA release build with a separate identity passed. The run used Rust 1.92.0, Node 25.2.1, and pnpm 11.25.0. |
| Isolated UI | 7 Edge scenarios and 100 accessibility checks passed with zero violations. The tests covered Turkish and English, light and dark themes, narrow viewports, focus, and cancellation from confirmation. They covered reopen with a pending flow, cleanup retry, loss of the start reply, and stale reads. Navigation timing covers the browser fixture only. OS DPI and provider latency have separate acceptance. |
| Native Windows 10 | The separate release identity ran the real Applications flow through AccountManager and IPC to the account rows. The test opened and cancelled Codex scope confirmation, visited Activity, returned, and confirmed normal quit. It read the existing default CLI ChatGPT record through the real Codex 0.167.0-alpha.4.1. OpenCode 1.19.3 data came from an owned synthetic XDG record. The test made no Claude status call. The app exited with code 0. Source and binary fingerprints matched. The checked auth and settings files retained their size, modification time, and file identity. The before-and-after check used auth file metadata only. It copied no auth contents and calculated no content hashes. |

The first Sky capture and click attempts hit timeouts and coordinate errors. Keyboard focus also stayed unchanged. The lead verified the native window with the installed open-computer-use tool's Invoke and ScrollIntoView accessibility actions and real native images. The run needed no user click assistance.

The QA copy had no engine payload. Its engine unavailable banner was expected and outside account acceptance. The installer and engine delivery have separate acceptance.

<a name="pasif-okuma-ve-inceleme-bulguları"></a>

## Passive reads and review findings

The official Claude Code 2.1.158 status command created settings and a backup inside the owned synthetic profile. OpenCode V1 auth-list created a global DB, WAL, and SHM in an isolated run with fetch off. Automatic account reads leave both commands unused. The product reports Claude as unknown. OpenCode uses a fixed read-only V1 metadata adapter. Zero accesses in the proxy observation alone do not establish complete network isolation.

During the UI handoff, the lead found that returning to the screen cancelled the flow. The lead fixed this. Other fixes prevent replacing a process whose shutdown is unknown and releasing resources early after a process state error. A refresh recovers the existing flow if the launch reply is lost. [Review](../reviews/H050.md)

Node tests passed in the first full gate. The team fixed TypeScript state narrowing. It reused the passing Node inputs that still matched.

The next product gate passed Rust tests, formatting, and the build. The old UI test expected 12 catalog records and found 13. The team updated the assertion to require every record from that catalog. It reran all UI scenarios. The records preserve failed runs. The team reused passing product checks when the relevant inputs matched programmatically.

<a name="doğrulanmayan-kapsam"></a>

## Unverified scope

A new live provider sign-in, live API-key access, Desktop accounts, and account adapters on Windows 11, macOS, and Linux remain unverified.

An unsupported version or client surface returns unknown. A local record establishes local status only. Remote access, subscriptions, chats, and Headroom traffic require separate evidence. The run started no new sign-in, model request, or CI. Linux remained deferred. Development token savings remain unmeasured.
