# H006-a — Portable Windows engine proof

Status: local proof PASS on the attached Windows 10 host, 2026-10-19. This covers a relocated portable runtime and hidden-helper lifecycle only. It is not a Windows 11, clean-machine, Tauri integration, or full H006 result.

Current update: On November 14, H006-b also verified abrupt-shutdown cleanup through the real Rust start path for a ready engine, with a Job Object. The historical H006-a result and hashes below remain unchanged. New scope and sources are at the end of this document. Main H006 remains open for clean Windows and Windows 11.

The package is a copyable directory. The pinned standalone CPython 3.13.13 base is 61.1 MiB. The locked environment's `site-packages` is 438.0 MiB. The payload has 22,225 files and uses 523,394,231 bytes (499.1 MiB), excluding the small manifest. The package keeps native extensions extracted so it needs no archive extraction layer.

The manifest records the engine and Python pins, source hashes, dependency-record and installed-distribution counts, and payload size. The packager rejects reparse points and stale paths when it reuses a package. Its inventory after the copy must match the pinned Python tree, locked `site-packages`, runner, and manifest exactly.

The integration test copies the package to a separate path with spaces. It starts the package in a controlled environment. The `PATH` contains only Windows `System32`. The runtime calls its adjacent Python by absolute path. It needs no developer Python, uv, or Node entry on `PATH`. The temporary state, workspace, config, HOME, and TEMP also use a separate path with spaces.

The engine binds only `127.0.0.1` on a dynamically selected port. The environment enables local telemetry. It disables anonymous beacon shipping, update checks, and message logging.

The test launches an outer controller with `CREATE_NO_WINDOW`. A typed `GetConsoleWindow` call confirms that the controller has no console. The controller starts the runtime manager as a native hidden-console helper with `CREATE_NEW_CONSOLE` and `STARTF_USESHOWWINDOW/SW_HIDE`. Pipes carry stdin and stdout. The helper confirms that its console window is hidden. It accepts only a bounded JSON command on stdin.

The engine shares this console and runs in its own `CREATE_NEW_PROCESS_GROUP`. Before the manager accepts readiness, it checks that `/readyz` reports version `0.41.0` and that the exact engine PID owns the loopback listener. `/stats` reports local telemetry on and anonymous shipping off. On `{"command":"stop"}`, the manager sends `CTRL_BREAK_EVENT` only to the owned engine process group.

The platform references describe Windows process-group events and Python's response to them. [`GenerateConsoleCtrlEvent`](https://learn.microsoft.com/en-us/windows/console/generateconsolectrlevent), [console process groups](https://learn.microsoft.com/en-us/windows/console/console-process-groups), [Python `SIGBREAK`](https://docs.python.org/3/library/signal.html#signal.SIGBREAK).

The engine exits with code 0. It reports Headroom's proxy summary and Uvicorn's lifespan-shutdown markers. The owned port closes without forced termination.

In the earlier attempt, a consoleless manager tried to attach to the engine's separate hidden console. That attempt failed to shut down the engine. In the passing test, the outer parent has no console. Only the helper and engine process group share a hidden console. The test uses a small Python `CREATE_NO_WINDOW` fixture as the GUI-like parent. Tauri GUI process creation and stdio wiring remain open.

H006/H014 must check that Tauri uses the tested hidden-window flags and keeps the stdin control channel. The Job Object uses `KILL_ON_JOB_CLOSE` to prevent orphan processes. This test scope excludes abrupt manager failure and therefore leaves this fallback untested. Windows 11 and a clean machine remain untested.

## Reproduction

Run from `D:\Projeler\Headroom` in PowerShell:

```powershell
powershell.exe -NoLogo -NoProfile -NonInteractive -File scripts/package-engine.ps1 -ReuseExisting
& work\proofs\engine\toolchain\cpython-3.13.13-windows-x86_64-none\python.exe -I work\proofs\packaging\test_portable_runtime.py PortableRuntimeTests.test_manifest_describes_a_relocatable_pinned_runtime
& work\proofs\engine\toolchain\cpython-3.13.13-windows-x86_64-none\python.exe -I work\proofs\packaging\test_portable_runtime.py PortableRuntimeTests.test_relocated_runtime_is_ready_and_stops_through_a_consoleless_controller
```

The manifest check reports `Ran 1 test in 0.000s — OK`. The focused integration test reports `Ran 1 test in 33.669s — OK`. The package command reports 22,225 payload files, 499.1 MiB, and 102 lock records. The manifest and an independent read-only inventory agree on 22,225 payload files and 523,394,231 bytes. The inventory found 0 missing and 0 unexpected paths.

The integration test confirms that the `CREATE_NO_WINDOW` parent has no console, the helper window is hidden, and relocation works. It also confirms readiness, listener ownership, telemetry flags, graceful exit code 0, and closure of the owned port. The run made no model or API requests and changed no client configuration.

## Fingerprints

- Host: Microsoft Windows 10 Pro 22H2, 10.0.19045, x64. Windows 11 and clean-machine behavior are unknown.
- Engine and Python: [`headroom-ai==0.41.0`](https://pypi.org/project/headroom-ai/0.41.0/), CPython 3.13.13 x64. The manifest counts 102 lock records and 96 installed `.dist-info` distributions. The payload includes the standalone base runtime and project-installed locked environment.
- The helper `PATH` contains only `%SystemRoot%\System32`. The engine `PATH` contains the copied runtime root and `%SystemRoot%\System32`. The engine uses `HEADROOM_HOST=127.0.0.1`, a dynamically allocated port, `HEADROOM_TELEMETRY=on`, `HEADROOM_BEACON=off`, `HEADROOM_UPDATE_CHECK=off`, `HEADROOM_STATELESS=false`, and `HEADROOM_LOG_MESSAGES=off`.
- The command fingerprint covers the three commands above. Python runs with `-I`. The test copies paths under `work\proofs\packaging\runs` and uses spaces in the relocated runtime and state directories.

SHA-256 of the final proof inputs:

| File | SHA-256 |
| --- | --- |
| `work/proofs/engine/pyproject.toml` | `7d9802162b931a2c2b8ceb08852f44c587668290d1763eaec49786d089aeeaa9` |
| `work/proofs/engine/uv.lock` | `161d3b1543b1f6620c1ce40e38d9a2fba26d5c64e3e103a41dcd81dff18ced3f` |
| project CPython `python.exe` | `282c2635fbd49b4aeefda4a965d84a745508c87e6e5eb95947efd0b5a1c1fb07` |
| `scripts/package-engine.ps1` | `b154493968e0ce0f374521c74189582c036833b5a6b941fdbe5d2b0664bdd152` |
| `work/proofs/packaging/headroom_runtime.py` | `bf887b6f783f4385debc84836353340f39e3773433782305376a3247be0f4ac1` |
| `work/proofs/packaging/test_portable_runtime.py` | `68156d35bc3b5d3460dd56c4b1a7c4ef448e24bba5815febc4696b8400c9e623` |
| packaged `runtime-manifest.json` | `8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908` |

<a name="h006-b--ani-sahip-süreç-kaybı-14-kasım-2026"></a>

## H006-b — Abrupt owner-process loss (November 14, 2026)

Local acceptance passed on Windows 10 build 19045 x64. The test uses production `ControllerSession::launch`, the current staged Windows package and state separated by UUID. It force-terminates the consoleless fixture owner with `TerminateProcess`. It does not call Rust `Drop`, send a stop request or call `TerminateJobObject`. This experiment is not a GUI crash launched from a Tauri window.

On Windows, `KILL_ON_JOB_CLOSE` ends processes in the group when the last Job handle closes. [Microsoft Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects). The test observer keeps the control-pipe writer handle open in its own process. Pipe EOF and normal shutdown cannot replace Job protection. [DuplicateHandle contract](https://learn.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-duplicatehandle). The test verifies live process handles for three core processes in the same Job. It also verifies that the Job is not inherited and does not contain the fixture owner. It keeps identities only in test metadata.

| Scenario | Preset acceptance | Observed |
| --- | --- | --- |
| Force-close the only Job owner while the control pipe stays open | Three core processes and the engine listener close within 5000ms | Processes closed in 53ms. The port-closed observation occurred at 357ms. |
| Observer holds an extra Job handle | Three processes and the port stay alive for at least 1000ms after the owner closes | Process handles stayed live. Readiness and port owner stayed the same. |
| Observer closes the last Job handle while the control pipe stays open | Three processes and the listener close within 5000ms | Processes closed in 54ms. The port observation occurred at 370ms. |
| Independent test listener | Preserve it in both arms | TCP connection and 13-byte content equality passed |

We measured each time from just before the trigger call through the end of observation. It is not the actual kernel termination time or the first time the port became free. Each arm ran once. This is not a long-term stability, speed/quality or token-savings benchmark. The test observed handles for only three core processes. It did not count all operating-system processes.

Package: **22,333 files / 536,626,939 bytes**. The packager's full content verification was reused as startup evidence with the same package inputs, commands and tools. After the test, it independently hashed every package file. The inventory and source/tool/environment fingerprints matched before and after. The final candidate passed the opt-in test, 441 Rust tests/18 ignored, fmt and embedded build. The earlier 221 Node tests and frontend build were reused because their inputs and Node/pnpm versions matched. [Machine-generated acceptance](windows-engine-crash.json), [review](../reviews/H006-b.md).

The first attempt was rejected because the old `work/build/windows/resources` helper pin did not match. A separate current stage was produced, and the old package was preserved. The second attempt timed out because the test reacquired its own Job mutex. The test moved the liveness check before the lock. Both runs failed. The final passing evidence is under `verification-6fdcd6e65d`, and the current local gate is under `quality-28623d163c`. Records are in `work/proofs/windows-engine-crash/`. No raw error dump was kept.

<a name="tekrarlama"></a>

### Repeat

Use the [Windows packaging record](../validation/windows-install.md) for pinned-payload prerequisites. Renew passing evidence only when relevant inputs change. If the packager reports `existing_stage_conflict` for an old target, keep the old artifact in the project and build the current package at an empty target. Do not weaken the pin or guard.

```powershell
& work/proofs/engine/toolchain/cpython-3.13.13-windows-x86_64-none/python.exe -I -B scripts/package-windows.py --output work/build/windows/h006-crash-resources
$env:CARGO_HOME = 'D:\Projeler\Headroom\work\caches\cargo-home'
$env:CARGO_TARGET_DIR = 'D:\Projeler\Headroom\work\caches\cargo-target'
cargo +stable-x86_64-pc-windows-msvc test --locked --offline --manifest-path src-tauri/Cargo.toml runtime::manager::windows_crash_tests::packaged_controller_job_closes_after_owner_crash -- --exact --ignored --test-threads=1
```

Main H006 remains open. This slice does not verify a clean Windows 10 or Windows 11 system, the startup crash race before Job assignment or a real GUI crash. No live provider or client setting changed. No release or Actions run occurred.

<a name="h006-c--gerçek-paketli-gui-kaybı-15-kasım-2026"></a>

## H006-c — Real packaged GUI loss (November 15, 2026)

The local slice passed after forced loss of the ready Tauri release GUI. The three engine process handles closed within 57.429 ms. The engine and fixed ingress ports closed within 58.299 ms.

The external observer kept the Job open during this measurement. The same test identity and profile opened as Stopped. The endpoint stayed available. Start, Ready, Stop, and normal Exit passed. The foreign listener stayed intact.

The run made 0 model requests. This is evidence from the real GUI. The H006-b controller fixture has separate evidence. [Scope and reproduction](windows-gui-crash.md), [programmatic result](windows-gui-crash.json), [review](../reviews/H006-c.md).

The main H006 item remains open. This new experiment did not verify clean Windows 10, separate Windows 11, the startup race before Job assignment or active stream/CCR data. Total remains 45/54.

<a name="h006-d--job-atamasından-önce-sahip-kaybı-15-kasım-2026"></a>

## H006-d — Owner loss before Job assignment (November 15, 2026)

The test verified startup behavior of the real packaged controller on Windows 10 22H2 x64 build 19045. It paused the production start function after `spawn` returned and before assignment to the newly created production Job. The `start` message had not yet been sent. [Acceptance criteria](../decisions/002-runtime.md#h006-d--job-atamasından-önce-sahip-kaybı).

| Scenario | Observation |
| --- | --- |
| Owner was force-closed and the pipe write end closed | The controller exited with code 2 within 147 ms. |
| Observer held the write end | The controller stayed alive during a one-second observation after owner loss. It exited with code 2 within 8 ms after the write end closed. |
| Process ownership and protocol | The external observer kept the Job open. No process appeared except the test owner at startup, the controller and two `conhost.exe` processes. In both arms, final live-process count and controller stdout bytes were 0. |

The first precheck rejected the `windows-hooks.nsh` pin in the earlier stage. The test continued with the current H006-c stage. The first native test could not distinguish the two `conhost.exe` processes in the Windows count. The test then observed process types separately. The final test recognizes startup members and treats every increase in the cumulative Job counter as a failure. It kept the five-second close limit. The canonical JSON contains references to failed records.

The 22,334 files including the package record, sources, validation commands and tool/OS fingerprints matched before and after. The final candidate passed 462 Rust tests, formatting and embedded build. The normal run skipped 20 opt-in tests. H006-d passed in a separate opt-in run. The team reused 222 application tests and the frontend build because frontend inputs and Node/pnpm matched. [Programmatic evidence](windows-engine-startup-crash.json) · [Review](../reviews/H006-d.md).

Use the existing `work/build/windows/h010-retrieval-resources` stage and project Cargo cache for validation. The [evidence record](windows-engine-startup-crash.json) contains the command. This local test covers the paused checkpoint after spawn. Separate clean Windows 10/11 x64 environments remain open for main H006.

<a name="temiz-windows-ortamı-için-taşınabilir-test-paketi"></a>

## Portable test package for a clean Windows environment

`work/proofs/windows-engine-crash/clean-test-a8e23e494d/Headroom_0.1.0_Windows_x64_H006_test.zip` contains the H006-c candidate previously tested with a real window and engine, the same engine files, license notices and short test instructions. All 22,335 files in the 197,348,721-byte archive passed exact SHA-256 comparison. Windows `tar.exe` read the entries, and PowerShell verified the archive hash. [Package and hash record](windows-engine-clean-test-package.json).

The tester extracts the ZIP to a short folder on a separate clean Windows 10 22H2 x64 or Windows 11 x64 computer. The tester opens `headroom-desktop.exe`. The archive's `README.txt` lists startup, engine readiness, stop and normal exit steps. Microsoft Edge WebView2 Runtime is a prerequisite. Record the Windows build number and architecture with the result. Compare the archive SHA-256 with `SHA256SUMS.txt` in the same folder.

```powershell
Get-FileHash -Algorithm SHA256 -LiteralPath .\Headroom_0.1.0_Windows_x64_H006_test.zip
```

The package uses the byte-matched, unsigned H006-c executable with its separate app identity. Independent clean-system acceptance still requires the archive's engine test on both target operating systems. NSIS and missing-WebView2 recovery tests belong to H037.
