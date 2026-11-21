<a name="h006-c--gerçek-paketli-gui-sahibinin-beklenmedik-kaybı"></a>

# H006-c — Unexpected loss of the packaged GUI owner

15 November 2026. **The local slice passed. The main H006 task remains open.** The test used the real Tauri release app on Windows 10 22H2 x64, build 19045. The plan set a 5,000 ms limit for process and port closure. The lead agent handled process and settings ownership and acceptance. [Acceptance plan](../decisions/002-runtime.md#h006-c--gerçek-gui-sahibi-kaybı-için-yerel-kabul-planı)

| Scenario | Observed result |
| --- | --- |
| Native window → Start → manager → packaged engine | Both GUI runs began in Stopped. The UI action brought Headroom 0.41.0 to Ready. The test verified three owned Python process handles, the engine listener, the version at `/readyz`, and the fixed ingress owned by the GUI. |
| Forced termination of the GUI owner only | The test called TerminateProcess on the held Popen child handle. The GUI exited with code 1. The test observed three engine processes close within 57.429 ms. The engine and fixed ingress ports closed within 58.299 ms. |
| Control for false success | The observer kept its external Job handle open throughout measurement. The test used no external Job termination or cleanup before it observed shutdown. The observer closed the Job after it became empty. The foreign listener received the expected witness bytes. |
| Reopen with the same isolated profile | The app opened in Stopped. The endpoint identity and port matched. Start, Ready, Stop, and normal Exit passed with a new runtime generation. The GUI exited with code 0. |
| Final state | Both arms ended with 0 owned processes. The engine, fixed ingress, and WebView debug ports closed. The test used the same engine settings. The fixture received 0 model requests. |

This experiment covered the real packaged GUI. H006-b used a separate console-less fixture owner. The native WebView driver used the visible Start, Stop, and Exit controls. After Ready, it read only the existing snapshot IPC. The test added no hook to the Tauri app or runtime.

The external Job contained the test process tree and served only for failure cleanup. This experiment left the effects of EOF and the production Job unresolved. The control-pipe and extra-handle arms isolated the Job mechanism. [H006-b](windows-engine.md#h006-b--ani-sahip-süreç-kaybı-14-kasım-2026)

The lead agent built the existing source and staged Windows runtime as `dev.headroom.desktop.gui-crash-a8e23e494d` with `cargo build --release --locked --offline --features custom-protocol`. The build used the existing release profile. The packaged runtime tree contained 22,332 files. The source stage had its own package receipt.

The GUI PATH contained only System32. The engine PATH contained only the packaged runtime and System32. The observer used Python and Node on the development host. Clean Windows acceptance remained open.

The successful native run matched 22,689 inputs before and after the test. Those inputs covered source, tests, config, payload, and artifacts. Tool and OS fingerprints also matched.

The existing H009 gate applied because 344 product inputs, the tools, and the environment matched. Its recorded results were 222 Node and 462 Rust tests passed, 19 ignored, and formatting, frontend, and embedded build checks passed. The new benchmark provided separate native evidence. The product test results came from the prior run because the product code matched. [Programmatic result](windows-gui-crash.json) · [Review](../reviews/H006-c.md)

The first native run used an incorrect TCP table class in the test driver. The real GUI and engine reached Ready. Port inspection failed before TerminateProcess. The observer used the external Job only to clean up that failed run. It verified that no processes remained.

It kept byte-exact archives of `native-7928c92f86` and that run's state under the same local application state root. The corrected driver used `TCP_TABLE_OWNER_PID_LISTENER` from the pinned runtime. The test reduced full payload reads to one scan at the start and one at the end.

The build driver's original bytes and hash stayed in the record. The changed observer driver received its own native fingerprint. The product, payload, UI driver, and build command matched. The final run passed in `native-bb9c843392/result.json`.

Use the pinned runtime stage and the final quality record to repeat this test. Create a new test identity from the project root.

```powershell
& work/proofs/packaging/artifacts/headroom-engine/runtime/python.exe -I -B bench/windows/gui_crash.py --prepare
# Use the candidate path from the first command.
& work/proofs/packaging/artifacts/headroom-engine/runtime/python.exe -I -B bench/windows/gui_crash.py --candidate work/proofs/windows-gui-crash/candidate-<id>
```

Repeat this check only if a relevant input changes. The `work/proofs/windows-gui-crash/` directory contains the native evidence, the EXE, and empty client and WebView profiles. The isolated application test state stays under `%LOCALAPPDATA%/dev.headroom.desktop.gui-crash-<id>`.

The run used no real client account or settings. It kept the model and subagent choices. The run used no Actions, paid resources, or publication.

Clean Windows 10 and a separate Windows 11 test remain open. The startup race before Job assignment, active streams, CCR data, and long-term stability require separate checks.

The two observations apply only to this run. General performance requires separate evidence. The run measured no token savings, billing savings, or development chat savings. **The historical count stayed at 45/54.**
