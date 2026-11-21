<a name="h016--yerel-keşif-kabulü"></a>

# H016 — Local discovery acceptance

20 October 2026. Local development acceptance for H016 is complete on Windows 10 Pro 22H2 x64. This task does not verify Windows 11, a clean install, connection/config changes, or live model/subagent compatibility.

<a name="son-aday-ve-doğrulama"></a>

## Final candidate and verification

- Backend: final focused discovery suite 15 PASS/1 opt-in ignored. fmt and offline check PASS. A separate ignored real bounded-environment scan had 1 PASS for earlier equivalent behavior. It was not repeated because the environment matched and only the debug trace changed. [Diagnosis appendix](discovery-native-diagnosis.md) records source/command/toolchain hashes.
- UI: 14 focused tests/typecheck/build PASS. The lead checked 32 views and 9 flows with a memory fixture. The run reported no JavaScript errors and 0 external requests. It ran with the production CSP from the local dist without starting a server. Native IPC has separate evidence below. See `work/ui-review-h016/summary.json` and [UI evidence](discovery-picker-ui.md). The final debug trace change did not change UI/dist sources. The team reused valid evidence.
- New embedded build: 10.66s PASS. EXE 14,098,432 B, SHA256 `d2f33a9eef9adb07764d3b3a7ac788320f154991fc4fa159b0e0e179dac9eaae`. Discovery source `df5a3137d0b96c7b8884b829a162fc2a187d581aa4e0e976669d7b6d13e60c11`. Tests `7e306509471ca4bb49756460c960ef0e7bcc5359bc48fdd00d5273936c62e02c`. Other source/config hashes are in the proof files below.

<a name="gerçek-pencere-yolu"></a>

## Real window path

PID 1204/session 17716/window 1247184, embedded local Tauri WebView. In Applications, the user selected an OpenCode file, chose the same 0B test file, and used Scan again. The user confirmed the candidate remained. The lead compared fixed metadata stderr records with the real accessibility text:

| Event | Generation | Manual candidate | Rejection | Visible total |
| --- | ---: | ---: | ---: | ---: |
| First real automatic scan | 1 | 0 | 0 | 2 |
| File selection accepted | 2 | 1 | — | — |
| First rescan | 3 | 1 retained | 0 | 3 |
| Second rescan | 4 | 1 retained | 0 | 3 |

The accessibility tree showed Codex PATH, Claude Code user-local and OpenCode Selected file rows. The UI showed unverified connections and unknown versions. The OpenCode row represented an empty test file created only for the picker. The test file and client were not run. The user's first native attempt included picker cancellation. The only reported issue was loss after rescan.

The lead closed the final window with normal Alt+F4. The retained process handle reported exit0, and the final process query found no Headroom process. The earlier PID 25548 candidate also closed normally with exit0. Preference file SHA256 was unchanged from start to end: `04ba8cfd197a03cf18ac2e1870b890ba4c90d060e0ba7d766e495ca61b43e7ac`. Model/auth/agent settings and other sessions did not change.

Isolated test UUID5b415e24-b96b-46fb-8a26-6ceb7a68b5e4 was cleaned after the window closed. Checks covered original ancestor/reparse/absolute containment/exact single child/0B SHA. First only opencode.cmd was removed, then the empty UUID directory. No recursive deletion occurred, and old frozen targets did not change. The historical manifest is `work/ui-review-h016/native-fixture.json`. The test file no longer exists.

<a name="pratik-sınır-ve-yayın-kapısı"></a>

## Practical limit and release gate

In an earlier candidate, the user reported that the row disappeared after rescan. The lead later observed that it was absent. The root cause of that earlier loss is unknown. New 0B cmd/default-input and real bounded-environment tests passed. Only a debug metadata observation was added.

The final candidate passed two consecutive native rescans and an accessibility-tree check. The cause of the earlier failure remains unknown. H039 adds a repeated manual selection/rescan/screen-change check on a candidate packaged with debug tracing off. The release acceptance will remain open if the issue returns.

Computer Use capture and indexed-click errors were repeated with Headroom and an empty Notepad. No automated native mouse or screenshot success is claimed. [External record review](../research/computer-use-2026-10-20.md) is an information source only. No helper patch was applied. Source/static/native evidence is kept separately in the [H016-c2 review](../reviews/H016-c2.md).

The local H016 scope is complete. Work stopped here at the user's request. H040 did not start.
