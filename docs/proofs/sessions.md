<a name="h051--oturumlar-yerel-kanıt-ve-açık-kabul"></a>

# H051 — Sessions: local evidence and open acceptance

13 November 2026. **H051 complete — Windows 10 22H2 x64 local development.** [Machine record](sessions.json), [decision](../decisions/031-session-metadata.md), [review](../reviews/H051.md).

Sessions → Chats/Accounts is ready. For the supported Codex server, the app distinguishes open, working, waiting, recorded, ended, and unknown states. It shows the platform, model, project basename, and any parent and subagent relationship. Last traffic and chat savings stay unknown because request association is not verified.

The app does not store chat titles, history, or auth records. The parser drops extra fields from the metadata API. The user selects the chat in the official Codex chat center. Headroom leaves chat resume and new turns under user control.

| Scenario | Observed result | Evidence / scope |
| --- | --- | --- |
| Strict client/IPC, errors, and model-independent metadata | 221 Node tests passed | `work/proofs/sessions/final-reviewed.json`. The later React visibility-helper change is not imported by the Node tests. Other inputs match. |
| Runtime, process trust, parser/manager, and existing product | 441 Rust tests passed, 17 opt-in ignored. Fmt, frontend, and embedded debug build passed. | `final-candidate.json`. Only an ignored native assertion and the UI test harness changed later. Production code stayed the same. The new assertion compiled and passed in a real opt-in native run. |
| Real App fixture: TR/EN, light/dark, empty/error, hidden, focus, confirmation, and launch error | 6 chat, 3 account, and 4 general-screen scenarios. 122 Axe checks, 0 violations. | `ui-stable-theme.json` and three UI reports in the machine record. This is not native IPC/provider evidence. |
| Navigation | p95 to the second paint frame was 16.9 ms across 60 samples, against a 250 ms threshold. | Isolated browser-click measurement. It excludes test-driver wait time, native DPI, and long-term stability. |
| Real Codex metadata, two same-model chats, silent open chat, server loss | 3 empty chats, 2 same-model chats, the existing User-Agent stayed the same. The state became unknown after the server connection dropped. | `work/proofs/s51/7aa331aa/result.json`, 0 inference POST, 0 external network attempts. |
| Official Codex center | A real `codex-tui` initialize witness on the isolated server, preservation of model/provider/reasoning for three chats, and continued operation after Headroom exited. | `work/proofs/s51/12e3ea28/result.json`, fixed-command/owned-parent receipt is in `ui-center.json`. A guard blocked the public TUI announcement CONNECT before TLS began. Inference POST: 0. |
| Final confirm/cancel/open/exit in the real Headroom window | **Passed** | The test kept three open records/two same-model chats and navigated between Accounts and Chats. It selected Cancel in the focused confirmation dialog without launching the client. After reconfirmation, it opened the official center and showed a success message. Headroom exited normally with code 0. The opened client remained after Headroom and the test-chat owner exited. The test cleaned up only owned test processes. |

Valid UI evidence for the chat/account components can be reused. Production sources stayed the same. Only the unimported general accessibility test and `preserveOutput` artifact policy changed. The test recreated the old config bytes programmatically and checked their hash.

A general-screen contrast run captured two palettes during a theme transition. The team reran 100 checks after waiting for fonts and two paint frames. The same contrast rules passed. The team did not lower any contrast rule or performance threshold. Raw Playwright error context was not retained.

The JSON record contains before/after source/config, dependency, and tool fingerprints. The run used Cargo/rustc 1.92.0, Node 25.2.1, pnpm 11.25.0, and Edge 157.0.4341.46. The team verified real Codex `0.167.0-alpha.4.1` signature/version against fixed public sources. This evidence does not extend to other versions.

The team visually reviewed the dark/TR and light/EN renders in `work/proofs/sessions/tr-dark.png` and `en-light.png`. These use explicitly isolated sample data. They are not real chat/account or native success images.

<a name="son-kabul-ve-sınır"></a>

## Final acceptance and limits

After the user unlocked the session, the team verified the WTS unlock state. A Sky capture timed out, so the lead agent controlled the real window through the available open-computer-use actions. The lead agent also inspected the native image.

The first locked run, `e782f555`, failed. In run `4ba2fa29`, the owner of the empty test chats exited early, so the UI showed 0 loaded chats. This run failed acceptance. The final fixture keeps the connection that owns the empty chats alive until the UI finishes.

Only ignored test setup changed. Production code did not change. The new opt-in test compiled and ran. The current fmt check passed. The earlier valid Node/Rust/default build and browser checks were not repeated.

`12e3ea28/observations.json` records real UI observations as counts/booleans without collecting content. The same run's source and fixture-config fingerprints match before and after. H051 closed at **42/54**. The next main task awaits the user's continuation.

The team postponed H049 Linux work because about $0.10 remained in the user's Actions quota. No CI, real login, or model request started in this task. This task does not accept Windows 11, macOS/Linux, real provider/subagent turns, other profiles/versions, or a release package. Unknown Claude/OpenCode chat status does not decide model support for the compression engine. Development token savings were not measured.
