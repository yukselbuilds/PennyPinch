# H006-a — Portable Windows engine proof

Status: local proof PASS on the attached Windows 10 host, 2026-10-19. This covers a relocated portable runtime and hidden-helper lifecycle only. It is not a Windows 11, clean-machine, Tauri integration, or full H006 result.

The package is a copyable directory, not a zip. The pinned standalone CPython 3.13.13 base is 61.1 MiB and the locked environment's `site-packages` is 438.0 MiB; the resulting payload is 22,225 files / 523,394,231 bytes (499.1 MiB), excluding the small manifest. Keeping native-extension files extracted avoids adding an archive extraction layer to this first proof. The manifest records the engine/Python pins, source hashes, dependency-record and installed-distribution counts, and payload size. The packager's reuse path rejects reparse points and stale paths; its post-copy inventory must exactly match the pinned Python tree, locked `site-packages`, runner, and manifest.

The integration proof copies the package to a separate path containing spaces and starts it with a controlled environment whose `PATH` contains only Windows `System32`. The runtime invokes its adjacent Python by absolute path and does not need a developer Python, uv, or Node entry on `PATH`. Temporary state, workspace, config, HOME, and TEMP paths are also under a separate path containing spaces. The engine environment binds only `127.0.0.1` on a dynamically selected port, enables local telemetry, disables anonymous beacon shipping, disables update checks, and does not log message contents.

The proof launches an outer controller with `CREATE_NO_WINDOW`; that process uses a typed `GetConsoleWindow` call and confirms it has no console. It starts the runtime manager as a native hidden-console helper with `CREATE_NEW_CONSOLE` and `STARTF_USESHOWWINDOW/SW_HIDE`, redirecting stdin and stdout through pipes. The helper verifies its console window is not visible and accepts only a bounded stdin JSON command. The engine shares that hidden console while running in its own `CREATE_NEW_PROCESS_GROUP`. The manager accepts readiness only when `/readyz` reports version `0.41.0` and the exact engine PID owns the loopback listener. `/stats` reports local telemetry enabled and anonymous shipping disabled. On `{"command":"stop"}`, the manager sends `CTRL_BREAK_EVENT` only to the owned engine process group. Python's documented Windows process-group event behavior is described in [`GenerateConsoleCtrlEvent`](https://learn.microsoft.com/en-us/windows/console/generateconsolectrlevent), [console process groups](https://learn.microsoft.com/en-us/windows/console/console-process-groups), and Python's [`SIGBREAK`](https://docs.python.org/3/library/signal.html#signal.SIGBREAK) handling. The observed result is exit code 0, Headroom's proxy summary and Uvicorn lifespan-shutdown markers, and a closed owned port; the forced termination fallback is not used on this PASS.

A console-less manager that tried to attach to the engine's separate hidden console did not provide a working shutdown path in the earlier focused attempt. The passing path keeps the outer parent console-less and gives only the helper a hidden console shared with the engine process group. This test uses a small Python `CREATE_NO_WINDOW` fixture as the GUI-like parent; the Tauri GUI's native process creation and stdio wiring remain untested. H006/H014 must verify that Tauri can start this helper with the tested hidden-window flags and preserve the stdin control channel. The Job Object is configured with `KILL_ON_JOB_CLOSE` as an orphan-prevention fallback, but this test does not simulate an abrupt manager crash and does not claim that fallback was exercised. Windows 11 and a clean machine remain untested.

## Reproduction

Run from `D:\Projeler\Headroom` in PowerShell:

```powershell
powershell.exe -NoLogo -NoProfile -NonInteractive -File scripts/package-engine.ps1 -ReuseExisting
& work\proofs\engine\toolchain\cpython-3.13.13-windows-x86_64-none\python.exe -I work\proofs\packaging\test_portable_runtime.py PortableRuntimeTests.test_manifest_describes_a_relocatable_pinned_runtime
& work\proofs\engine\toolchain\cpython-3.13.13-windows-x86_64-none\python.exe -I work\proofs\packaging\test_portable_runtime.py PortableRuntimeTests.test_relocated_runtime_is_ready_and_stops_through_a_consoleless_controller
```

The manifest check reports `Ran 1 test in 0.000s — OK`; the focused integration test reports `Ran 1 test in 33.669s — OK`. The package command reports 22,225 payload files, 499.1 MiB, and 102 lock records. The manifest and an independent read-only inventory check agree on 22,225 payload files and 523,394,231 bytes, with 0 missing and 0 unexpected paths. The integration test confirms the `CREATE_NO_WINDOW` parent has no console, the helper window is hidden, relocation works, readiness and listener ownership match, telemetry flags are correct, graceful exit is 0, and the owned port closes. No model/API request was made and no client configuration was changed.

## Fingerprints

- Host: Microsoft Windows 10 Pro 22H2, 10.0.19045, x64. Windows 11 and clean-machine behavior are unknown.
- Engine and Python: [`headroom-ai==0.41.0`](https://pypi.org/project/headroom-ai/0.41.0/), CPython 3.13.13 x64. The manifest counts 102 lock records and 96 installed `.dist-info` distributions. The payload includes the standalone base runtime and project-installed locked environment.
- Environment: helper `PATH` is only `%SystemRoot%\System32`; engine `PATH` is the copied runtime root plus `%SystemRoot%\System32`. `HEADROOM_HOST=127.0.0.1`, dynamically allocated port, `HEADROOM_TELEMETRY=on`, `HEADROOM_BEACON=off`, `HEADROOM_UPDATE_CHECK=off`, `HEADROOM_STATELESS=false`, and `HEADROOM_LOG_MESSAGES=off`.
- Command fingerprint: the three reproduction commands above; Python runs with `-I`, and test paths are copied under `work\proofs\packaging\runs` with spaces in the relocated runtime and state directories.

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
