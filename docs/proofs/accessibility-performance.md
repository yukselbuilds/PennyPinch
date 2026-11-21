<a name="h035--erişilebilirlik-ve-performans-kanıtı"></a>

# H035 — Accessibility and performance evidence

November 8, 2026. **Complete. The total was 35/51.** The lead agent owned performance, runtime, Windows checks, and acceptance. The single Luna worker reviewed accessibility in read-only mode. Development token savings remain unmeasured. [Plan 023](../decisions/023-accessibility-performance.md), [test conditions](../validation/performance.md), [review](../reviews/H035.md)

<a name="güncel-adayın-kapanışı"></a>

## Final candidate acceptance

H036 changed the application code and binary. The final candidate therefore repeated the performance checks and OS DPI checks for the changed Applications screen. `work/proofs/accessibility/h036-final-acceptance.json` records 46/46 checks. `closure-acceptance.json` links the final documentation changes. The recorded H036 results are 173 frontend tests, 386 Rust tests, 8 ignored, and 84 audits with 0 violations. The team reused those results because the source, tools, and OS matched.

The earlier H035 result stays with its binary. Programmatic checks found matching UI source, app shell, and focus code for the unaffected areas. The earlier narrow-window, keyboard, and other-screen DPI checks apply to those areas.

| Final check | Observed result | Evidence |
| --- | --- | --- |
| User tray check | The user confirmed that X, Open from the tray, Pause, and Resume all worked. The tests observed two enabled, paused, and enabled cycles with the same generation and revisions from 0 to 4. | `work/proofs/compatibility/native-runs/2dd91219-d59e-47be-8ccb-c2b6d6a57fcd/manual-tray-user-report.json`, `tray-observations.json` |
| Engine off and ready, idle for 60 s | UI CPU was 7.54% and 9.85%, respectively. Engine CPU was 0% and 0.26%. Each state had 59 samples. Fixed memory limits passed. | Run `93ce1a01-e141-42b3-b8fe-4afeea51fd0a`, `idle_off_settled/idle_ready.json` |
| Warm-up and local load | During the separate 60 s warm-up, UI CPU was 10.78% and engine CPU was 15.09%. The run sent 2+10 local requests. The warmed p95 was 35.88 ms. Peak private memory was 185.30 MiB for the UI and 1,157.14 MiB for the engine. | Same run, `warmup/load.json`. The synthetic input used the same 31,925 bytes and 128 lines. The proxy received 12 requests. The exact-response cache limited fixture upstream calls to 1. External provider calls were 0. |
| Startup and navigation | The actual Sky Start action reached ready in 16.72 s. Current UI navigation p95 was 21.8 ms. | `startup.json` and H036 `ui-5c51934c-954b-4f74-8997-ee61ebad2fb9/result.json` |
| Changed Applications screen at 125% and 150% | The screen used 120 and 144 DPI, with DPR 1.25 and 1.5. The test opened and closed Compatibility and refreshed 12 records. Critical actions were accessible horizontally, with no overflow. | Same run, `catalog125*`, `catalog150*`. Sky sent real mouse and keyboard input. The test injected no API or IPC calls. |
| Restoration and exit | Original scaling returned to 100%, 96 DPI, and DPR 1. Settings closed. Both final native runs exited normally with code 0 and forced=false. | Both `result.json` files. State, protected inputs, and the actual HKCU Run value matched. Owned ports closed. The foreign listener stayed active. The final Sky list had no Headroom or Settings window. |

Ready-state UI CPU is close to the 10% limit. Sustained load, release performance, provider performance, and savings require separate evidence.

At closeout, only documentation changed. The team corrected the H035 status in README. Product inputs stayed the same. Verification recorded source, evidence, artifact, tool, and OS fingerprints before and after. The user confirmed visual tray behavior. The passive DOM visibility value alone could not establish that the window was hidden. Work stopped after H035 and before H037.

<a name="ilk-adayın-tarihsel-kaydı"></a>

## Historical record for the first candidate

The changes fixed the skip link, focus and announcement for Activity filter errors, and light theme contrast. Passive UI runtime polling uses 3 s. Passive native actor health polling uses 2 s. Explicit lifecycle commands wake the actor immediately through the channel and show their own acknowledgement. The 5 s health validity and per-request ownership checks stayed the same. Authentication, model, routing, stream, and CCR contracts also stayed the same.

The evidence is in `work/proofs/accessibility/`. The final `checkpoint-acceptance.json` links the source, tests, config, local gate, UI results, images, and native candidates to matching before-and-after SHA256 values and tool and OS inputs. It records `core_passed:true` and main acceptance `passed:false`.

Only the tray menu check remained open at that checkpoint. The earlier `core-acceptance.json` is a historical record before closeout. The team reused successful checks.

| Scenario | Observed result | Evidence and scope |
| --- | --- | --- |
| Turkish and English, dark and light, data, empty, and error states, four screens and dialogs | The test ran 68 axe audits with 0 violations. Keyboard, Escape, focus, and error alert and focus checks passed. | `ui-36e174ba-f4b9-4c8a-a94f-a1463c7c008c/result.json`. The real app used isolated transport. WCAG certification and a real screen-reader session remain outside this scope. |
| Final local quality gate | 168 frontend and 379 Rust tests passed, with 8 opt-in tests ignored. Formatting, TypeScript, Vite, native build, and accessibility (a11y) checks passed. | `native-final-3.json`, `reused-frontend.json`, and `final-2.json`. The native polling change required a new native gate only. |
| Engine off and ready, idle for 60 s | UI CPU was 7.27% and 9.80%, respectively. Engine CPU was 0% and 0.23%. All memory limits passed. | Native run `156bb28f-b191-440c-b2ce-04699664ea26`. CPU uses one core as the basis, with 59 samples per state. Startup and discovery and model warm-up use separate 60 s windows. |
| Two warm-up requests and 10 sequential local requests | Input was 31,925 bytes and 128 lines. Warmed p95 was 36.09 ms. Peak private memory was 198.33 MiB for the UI and 1,156.18 MiB for the engine. | `load.json` in the same run. The provider call count was 0. Live providers and sustained load have separate acceptance. |
| Native Start and isolated UI navigation | Ready in 16.85 s. Navigation p95 was 23.30 ms. | `startup.json` and UI result. Startup includes the ready polling delay. |
| Actual narrow window and keyboard | The window used 1015×711 CSS pixels with no overflow. Shift+Tab and Enter moved focus to the main content. | `small-720-final*`, `keyboard-skip-focused.json`, and `keyboard-main.json`. The native window was slightly smaller than the 1024×720 target. |
| Actual Windows scaling at 125% and 150% | The window used 120 and 144 DPI and DPR 1.25 and 1.5. Pause, Resume, the four screens, the lower connection dialog actions, and Escape were accessible. | `os125*`, `os150*`, and native images. Sky sent actual mouse and keyboard input. The test left native IPC unchanged. At 150%, the display limited the window height to 650 CSS pixels. |
| Original OS scaling | Settings showed 100%, with native 96 DPI and DPR 1. Settings closed. | `os100-restored*` and `display-restored-100.png`. The WebView display scale stayed at 100%. |
| Separate final normal native exit | Actual Sky quit and confirmation produced exit code 0 and forced=false. Original state, protected inputs, and the Run value matched. Owned debug and stable ingress ports closed. The foreign listener stayed active. | Run `6d8f33c6-fe4e-4b2e-888b-ca92205c57c4/result.json`. The final Sky window list had no Headroom window. |

The report retains the first two candidates that failed CPU limits. The third run passed its measurement and DPI checks. Its 20 minute limit expired before the normal quit click. Run `result.json` therefore reports `forced:true, passed:false`.

The original state returned and the foreign listener stayed active. The measurements apply to phases with their matching source, binary, and load fingerprints. Normal exit needed the separate final run.

The separate historical run `6d8f33c6-fe4e-4b2e-888b-ca92205c57c4` belongs to that source, binary, and distribution. At that point the user had yet to confirm the tray menu. Passive observation of visible and enabled states supplied no menu action evidence. The lead closed the app normally and restored state. The old pending result remains in the record. The final acceptance above covers user confirmation, performance, DPI, and normal exit.

This is local evidence for a debug candidate on Windows 10 x64 build 19045. Windows 11, macOS, Linux, installation, sustained load, live providers, and a manual screen-reader session remain outside this result.
