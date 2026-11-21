<a name="h030--sistem-tepsisi-ve-kapanış-kabulü"></a>

# H030 — System tray and shutdown acceptance

November 7, 2026. **H030 is complete. The total is 29/51.** The local test ran on Windows 10 x64 build 19045. The paid Windows 11 run remained skipped and marked not_tested under the earlier H047 and Decision 006 exception. Windows 11, macOS, Linux, installer behavior, and live providers have separate acceptance. [Plan 018](../decisions/018-tray-lifecycle.md), [review H030](../reviews/H030.md)

<a name="uygulanan-davranış"></a>

## Implemented behavior

- With the tray available, X and Alt+F4 hide the window. The engine and stable ingress stay active. Without the tray, the app asks for exit confirmation.
- The tray menu shows Open, the current engine state, Start, Pause, Resume, and Quit. Status polling reads only the in-memory snapshot. It issues no metrics queries or model requests. Pause and Resume use generation and revision CAS and wait for the engine acknowledgement.
- Quit explains that routing settings stay in place and that quitting can affect connected clients and active responses. Cancel is the default. Escape cancels and returns focus. Quit returns busy during a client config operation. A stale token starts neither a config writer nor shutdown.
- Confirmation permanently blocks new config operations for the exiting process. The existing owned shutdown sequence runs once. The H029 stop, drain, and no-replay limits stay in effect. Incomplete shutdown returns exit code 1. A crash or forced shutdown has no guarantee of uninterrupted bypass or automatic config undo. H031 covers auto-start.

<a name="doğrulama-kayıtları"></a>

## Verification records

Evidence is in `work/proofs/tray/`. The test retained no raw authentication data, prompts, or HTTP bodies. Verification recorded source, config, tool, environment, command, and before-and-after fingerprints.

| Scenario | Result | Evidence and scope |
| --- | --- | --- |
| Nonce, cancellation, stale and busy responses, hide denial without the tray, acknowledgement state, and exit hold on a real filesystem | Passed | Final candidate `final-1.json`. 146 frontend and 356 Rust tests passed. The gate ignored 7 opt-in tests. Client settings bytes stayed unchanged. |
| Final TypeScript, Vite, and native build and formatting | Passed | `build-2.json` covers the frontend. `native-build-4.json` covers later native changes. `final-1.json` covers the final Rust and formatting checks. The team reused results with matching sources. |
| Component UI in Turkish and English, light and dark, at widths 1280, 819, 683, and 507 | Passed with an isolated fixture | The check covered Escape, focus, Tab, parent busy, duplicate pending actions, errors, and the hidden sidebar. `ui-d655c49c-795d-44f3-9822-8dcb4f76ca8f/result.json` records 6 behavior groups and one StrictMode subscription. The lead reviewed `tr-dark-507.png`. The fixture used an explicit mock transport for IPC. |
| Native event permission and initial tray state | Passed | The actual `get_desktop_state` response and accessibility tree confirmed tray availability and idle text. The earlier candidate lacked event permission and failed acceptance. |
| Native Alt+F4 during active SSE | Passed | `native-runs/fff9de67-e194-46bf-88f7-3de65bb997ee/sky-active-hide.json` records 0 Headroom windows in the Sky inventory. The process, engine generation, and endpoint stayed the same. The client received its first byte while the terminal gate stayed closed. The run made 1 upstream POST, and the foreign fixture listener stayed active. The first inventory ran during window close. A separate inventory confirmed the final state. |
| Native exit request, cancellation, and stale token during active SSE | Passed with one request, one POST, a complete terminal response, and exit code 0 | `native-runs/610ebbd4-091f-4135-9c02-e8baf91508fc/result.json` and `drain-before-release.json` record owned shutdown. The old SSE stayed open while stable ingress rejected a new request. The test then opened the upstream gate. Cancel and a stale token kept the same generation and endpoint. Earlier setup attempts have their own records. |
| Explicit quit before the active response completes | Passed with exit code 1, no replay, and owned cleanup | `native-runs/fff9de67-e194-46bf-88f7-3de65bb997ee/result.json` records the header and first frame with no terminal event. The app exited with code 1 after 1 POST. The test tool made no forced termination. The test restored the normal state. |
| Engine loss, visible error, and native Escape, Tab, Enter, and exit | Passed with exit code 0 | `native-runs/de1769d7-43c7-4cc4-be19-9337a446a6b0/result.json`, `crash-settled.json`, and `native-keyboard.json`. The test checked the pinned executable identity, the three-process ancestry chain, and test Job ownership. It terminated only the leaf engine. The UI showed failed and engine_not_ready. Escape cancelled the confirmation. Tab focused the actual confirmation button. Enter completed owned shutdown. |
| Native tray menu Open, Pause, and Resume | Passed with user interaction and an independent final state check | The user reported completing Start, X, Open from the tray, Pause, and Resume. In the open native window, the lead read the engine On state, enabled Pause, and tray text. There was no exit dialog. `manual-final.json` and `closeout.json` link the source, binary, distribution, tools, and OS fingerprints. The user performed the menu clicks. The lead directly observed only the final state. |

The automated native runs used a synthetic loopback upstream and a test Job. They made no live provider or model calls. The harness moved the project runtime and metrics directories to verified paths within the project and restored them after the tests. It compared fingerprints to check the restored bytes. Authentication, user client config, ownership, preferences, and other projects stayed unchanged. Real token savings and long-term stability require separate evidence.

`acceptance-1.json` uses 43 programmatic checks to match the current source and config, final binary and distribution, original verifier, and UI evidence. The team reused passing gates.

The first crash fixture compared the Windows `\\?\` prefix with a plain path and stopped at the process identity guard. It closed the Job and restored state before any direct process termination.

The final test harness checks executable identity with `samefile`, an open handle, process ancestry, and Job membership. The early setup failures remain failures. The final normal, timeout, and crash runs each record matching before-and-after fingerprints.

The automated runs finished and restored temporary state.

The final user menu check used the normal development app. The engine started in Stopped. The user started it and completed the steps. The window remained open in the On state with Pause available.

This last check ran outside the isolated synthetic traffic test. No test automation remained active. Live provider verification remains separate.

Sky screenshot and click geometry errors required user interaction with the tray menu. `manual-final.json` records only the necessary metadata from the independent final accessibility-tree check.

`closeout.json` matched the current source, binary, distribution, toolchain, Windows build, and verifier Python to the earlier PASS. The product code stayed the same. The team reused the 146 frontend and 356 Rust results and the build checks. The lead accepted the final run and closed the documentation. There was no new handoff or development savings measurement.

H030 closed. Work stopped before H031.

<a name="19-kasım-2026--tepside-genel-bakış-yenilemesi"></a>

## November 19, 2026 — Overview refresh while the window is in the tray

The test observed the same UI and native window flow before and after the fix on Windows 10 22H2 x64. The lead owned acceptance for the window lifecycle and metrics collector. Before the fix, WebView2 kept `document.visibilityState` at `visible` after Windows hid the window. It issued 2 automatic `get_overview_snapshot` requests in about 12 seconds.

The app reads the main window's native visibility after a Headroom desktop notification. When the window is hidden, the Overview session and its five-second refresh stop. Open in the tray starts a refresh immediately and restores the normal interval. The test saw 0 automatic queries while the window was hidden for 12,013 ms. It saw 1 query within 2.4 seconds after reopening. The next scheduled query also passed.

The test used the actual window-close message for X and the owned tray menu's Open command for reopening. It measured native window visibility directly. The isolated Headroom 0.44.0 engine was ready locally.

A diagnostic query while hidden verified that the latest metric timestamp advanced after the window was hidden. The query identity was excluded from the automatic panel count. The owned development window closed with exit code 0.

The first engine run reached the verifier timeout. In the second run, the tray icon's left-click message did not reach the Open step. The final run completed with the native Open menu command.

The final local gate passed 225 frontend and 478 Rust tests, formatting, and two builds. It ignored 24 opt-in Rust tests. The team reused the frontend build with matching source and distribution fingerprints. Isolated Edge tests across five screens passed 60 accessibility checks with 0 violations. All 304 source, test, and config inputs matched.

The local gate SHA-256 is `11b279528f8ed2014739238ce5494038fc4fb41e590c2a5296e7337b081cedfb`. Earlier observation (local receipt `work/proofs/panel-visibility/native-12be617fb1/baseline-result.json`) · Final native observation (local receipt `work/proofs/panel-visibility/native-12be617fb1/candidate-observation-872f98b0/result.json`) · Local gate (local receipt `work/proofs/panel-visibility/gate-1dcb0800-aeba-4132-b477-e35091a51d82.json`) · Screen checks (local receipt `work/proofs/accessibility/ui-eb4d3baf-c1bb-499b-8e35-135f7219026d/result.json`)
