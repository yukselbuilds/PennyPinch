# H014-a: Desktop runtime controller proof

The Python control layer now exposes a bounded JSONL protocol v1 and starts the pinned engine through the proven hidden-console supervisor. The consoleless controller owns the helper process handle; the H006 helper owns the engine process, hidden console, job object, and loopback listener. A readiness frame is sent only after the helper and listener PIDs match, `/readyz` reports Headroom 0.41.0, the guarded cache shim reports its exact version and source hashes, and live health/statistics endpoints confirm the required profile and telemetry settings.

The controller accepts only `start` and generation-bound `stop` requests. Requests are limited to 4 KiB, helper frames to 8 KiB, request IDs are opaque printable tokens, duplicate JSON keys and unknown fields fail closed, and replies use a fixed event and error allowlist. Helper frames are read by one bounded reader thread: readiness has a 90-second deadline and the stop event a 30-second deadline, both returning the fixed `helper_timeout` error. On timeout only the owned helper process handle is terminated, with bounded terminate/kill waits; its stdout is closed only after the reader exits. The runtime manifest must match the fixed cache/coding profile, CCR TTL 7200, telemetry on, beacon off, output shaping/router off, and memory/learning off. Runtime resources are resolved from `runtime/manifest.json`; its pinned H006 package files and the H007 shim copy are SHA-256 checked before the supervisor starts. The adapter leaves the H006 runtime payload unchanged and extends its engine bootstrap with the exact cache shim, disabled model router check, console marker, and Ctrl+Break handler.

On the Windows 10 Pro 22H2 x64 host (10.0.19045), the focused protocol, timeout, incomplete-frame, profile-drift, and pin-guard tests passed: 8 tests, `OK`. The stall test used a test-owned sleeping Python child and confirmed the frame deadline and reader cleanup after that child exited. The final real smoke passed in 15.509 seconds. It launched the controller with `CREATE_NO_WINDOW`; the controller reported no console, then created the hidden helper with `CREATE_NEW_CONSOLE | CREATE_NEW_PROCESS_GROUP` and `STARTF_USESHOWWINDOW/SW_HIDE`. The helper’s PID differed from the engine PID, and the loopback listener PID equaled the engine PID. The observed engine was 0.41.0, cache mode with the coding profile, CCR default TTL 7200 seconds, local telemetry enabled, anonymous beacon disabled, memory and traffic learning disabled, output shaping disabled, and model routing disabled. The cache shim was active for the exact pinned `prefix_tracker.py` source. No upstream URL or provider credential was configured and no model/provider request was made.

The test sent a v1 `stop` request over stdio. The helper recorded the hidden console, lifespan-complete and proxy-summary markers, Ctrl+Break shutdown, engine exit code 0, and closed owned port; no force fallback was used. The test then confirmed the controller exited with code 0 and the port refused a connection. The generated run directory was within `work/proofs/runtime/runs/` and was removed after all owned processes exited; the runs directory was empty at proof completion.

The focused commands were:

```powershell
& .\work\proofs\packaging\artifacts\headroom-engine\runtime\python.exe -I -m py_compile runtime\desktop_controller.py runtime\desktop_helper.py tests\runtime\test_desktop_controller.py
& .\work\proofs\packaging\artifacts\headroom-engine\runtime\python.exe -I -u tests\runtime\test_desktop_controller.py DesktopProtocolTests
& .\work\proofs\packaging\artifacts\headroom-engine\runtime\python.exe -I -u tests\runtime\test_desktop_controller.py DesktopRuntimeSmokeTests.test_consoleless_controller_starts_and_gracefully_stops_owned_engine
```

The normalized argv SHA-256 is `3d355b7372a015d64aab7c33491d020a678d1ec14875c98c800edc50f776d5ea`. The normalized runtime environment fingerprint is `8aac0705e6e5b936e10b6eaf7e3386efe4cad634f1ae7582a1f8eeb458322a89`; dynamic port and temporary paths were replaced with placeholders. Relevant effective values were `HEADROOM_HOST=127.0.0.1`, dynamic loopback port, `HEADROOM_STATELESS=false`, `HEADROOM_MODE=cache`, `HEADROOM_SAVINGS_PROFILE=coding`, `HEADROOM_CCR_TTL_SECONDS=7200`, `HEADROOM_TELEMETRY=on`, `HEADROOM_BEACON=off`, `HEADROOM_OUTPUT_SHAPER=0`, `HEADROOM_MODEL_ROUTER_ENABLED=0`, and memory/learning disabled. `OPENAI_TARGET_API_URL` and `ANTHROPIC_TARGET_API_URL` were unset.

Source and toolchain fingerprints:

| Input | SHA-256 |
| --- | --- |
| `runtime/desktop_controller.py` | `a56a82bed8c2878416f99f817f899a294daf5907ba5c21ba53b06a22b2e2537d` |
| `runtime/desktop_helper.py` | `70c0d5b8c52c3f74cb417781c4e1fe60fce942ed6f2375871f90cafbdb1dc398` |
| `runtime/cache_control_shim.py` | `ebc80ca2958b178ed488fb953f1a09cf2dd803b23352dcc3f8f0a644414e30c1` |
| `runtime/manifest.json` | `e57249e194eb5e5ac8c0396956338c40f35d144b357adb3db826bfb4ce133c57` |
| `tests/runtime/test_desktop_controller.py` | `bdb1e75d9a2a9667586b87f9c9c0237c3439afa214aab761a0e15b2dbb402021` |
| Packaged H006 `headroom_runtime.py` | `bf887b6f783f4385debc84836353340f39e3773433782305376a3247be0f4ac1` |
| Packaged H006 `runtime-manifest.json` | `8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908` |
| Packaged CPython `python.exe` 3.13.13 x64 | `282c2635fbd49b4aeefda4a965d84a745508c87e6e5eb95947efd0b5a1c1fb07` |
| Pinned `prefix_tracker.py` | `f1af4ee279f879efc48ddfebbff125a088893bb64ee960ca2dbd4ad1cdb628ee` |

The exact packaged payload path remains a development resource reference. Rust/Tauri resource resolution, app-managed startup and UI lifecycle wiring are H014-b; installer and clean-machine packaging remain later gates. The smoke proves the normal consoleless-controller stop path on this Windows 10 host. It does not prove Windows 11 behavior, abrupt parent-crash recovery, or installed-app lifecycle.
