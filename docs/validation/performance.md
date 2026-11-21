<a name="h035--yerel-erişilebilirlik-ve-performans-ölçümü"></a>

# H035 — Local accessibility and performance measurements

On 8 November 2026, **H035 closed**. This record covers a Windows 10 development binary. Release, installer, sustained load, and live provider performance require separate evidence. [Plan 023](../decisions/023-accessibility-performance.md) · [current acceptance](../proofs/accessibility-performance.md).

<a name="sabit-koşullar"></a>

## Fixed test conditions

The test computer had an Intel Core i7-10875H with 8 cores, 16 logical processors, and approximately 16 GB of RAM. It ran Windows 10 x64 build 19045 with a 1920×1080 display. The reference record stores the hardware and original OS scale of 100% in `work/proofs/accessibility/reference.json`.

CPU percentages use one logical processor as the basis, with no division by 16. The UI group contains Headroom and its WebView processes. The engine group contains the other helper, Python, and console hosts in the owned job. The debug console host uses a small amount of memory while the engine is off.

The sampler reads GetProcessTimes and GetProcessMemoryInfo from the processes in the held test job every 1 s. It excludes other applications, sessions, command lines, auth data, and content. It keeps private bytes and working set as separate measures. The fixture POST measures local response performance. It provides no live provider performance or cost.

Measure 60 s of idle time after readiness and 60 s with the engine off. Each group must average ≤10% of one core. The private memory limit is ≤512 MiB for UI and ≤1536 MiB for the engine.

Send two warm-up requests. Then send 10 sequential requests at most once per second. Each request contains 128 lines of synthetic tool JSON. Require warmed local p95 ≤5 s and the same memory limits.

Require native readiness in ≤100 s. Require isolated App navigation to the second frame at p95 ≤250 ms. The thresholds stayed fixed after the test.

<a name="güncel-h036-sonrası-aday"></a>

## Current candidate after H036

Run `93ce1a01-e141-42b3-b8fe-4afeea51fd0a` started after frontend loading completed. Over 60 s with the engine off, UI CPU was 7.54% and engine CPU was 0%. The separate 60 s model warmup measured 10.78% UI and 15.09% engine CPU. The next 60 s ready period measured 9.85% UI and 0.26% engine CPU. Every phase had 59 samples. The idle limit excludes warmup.

For 2+10 identical local requests, the p95 latency was 35.88 milliseconds. Peak private memory was 185.30 MiB for UI and 1157.14 MiB for the engine. Load CPU was 13.28% and 4.89%, respectively. The idle limit applies to the idle periods.

The proxy received 12 requests. With exact caching, 1 reached the local upstream fixture and 0 reached an external provider. The Sky Start action reached ready in 16.72 s. The verification step recorded the source and binary hashes.

The changed Applications screen passed open, close, and refresh at real Windows scale of 125% and 150%. The corresponding readings were 120 and 144 DPI and DPR 1.25 and 1.5. The test restored 100%, 96 DPI, and DPR 1. This run and the user's tray run both exited with code 0 and forced=false. Both preserved owned ports, state, and Run.

The team reused the 173/386/84 source gate after it matched current inputs. It matched the relevant unchanged screen sources, app shell, and focus code programmatically. Historical narrow-window, keyboard, and DPI results apply only to those areas. The record binds 46 technical checks and the final document acceptance.

Short debug tests and UI CPU close to its limit leave sustained load and release stability open.

<a name="tarihsel-bulgular-ve-geçerlilik"></a>

## Historical findings and evidence scope

| Run | Observation | Result |
| --- | --- | --- |
| First candidate `c7de7336-3531-4dcb-9a46-3f66849288eb` | The first 60 s after readiness gave 22.41% UI and 14.72% engine CPU. Load used 10 requests plus 2 warm-up requests. The slowest took 41.55 ms. | The first CPU window failed. The team separately found background model warmup. |
| Second candidate `b2e52781-fd3c-41e2-be28-a6ead122eeb5`, first 60 s with engine off | UI CPU was 13.36%. The first 10 samples gave 41.10%. Later windows of 10 samples gave about 6–8%. | Startup and scanning overlapped with the window. The record remains a failure. |
| Same second candidate, 60 s with engine off after actual frontend loading completed | UI CPU was 5.50% and engine CPU was 0%. A separate 60 s model warmup measured 12.86% UI and 14.61% engine CPU. | Idle with engine off passed. Idle CPU excludes the separate warmup measure. |
| Second candidate, 60 s ready and idle after warmup | UI CPU was 13.51%. Engine CPU was 0.47%. | The UI CPU target failed. Startup alone did not account for this result. |
| Final correction | Passive UI polling changed from 1 s to 3 s. Passive native health polling changed from 1 s to 2 s. | Explicit commands update state on acknowledgement. Request ownership and the 5 s health validity period stayed the same. |
| Final candidate `156bb28f-b191-440c-b2ce-04699664ea26`, 60 s startup and scanning | UI CPU was 12.37% and engine CPU was 0%. The 60 s model warmup measured 10.58% UI and 15.34% engine CPU. | This window measures startup overhead. The idle CPU gate uses a separate window. |
| Final candidate, 60 s with engine off after actual frontend loading | UI CPU was 7.27%, engine CPU was 0%, and the window had 59 samples. | The fixed idle CPU and memory limits passed. |
| Final candidate, 60 s ready and idle after model warmup | UI CPU was 9.80%, engine CPU was 0.23%, and the window had 59 samples. | The fixed idle CPU and memory limits passed. The UI result was close to the limit. |
| Final candidate, two warm-up requests plus 10 sequential local requests | Requests had 31,925 bytes and 128 lines. Both p95 and the longest time were 36.09 ms. Private memory was 198.33 MiB for UI and 1156.18 MiB for the engine. | Local latency and memory passed. Load CPU was 13.16% for UI and 2.61% for the engine. The idle limit applies to the idle window. |
| Final candidate, actual Sky Start to loopback ready | Readiness took 16.85 s. UI navigation p95 was 23.30 ms. | The 100 s startup and 250 ms navigation limits passed. The ready observation includes polling delay. |

The record keeps two fixed 60 s periods for UI startup and scanning and model warmup. Idle measurement starts after the committed frontend ready condition and the warmup window. Early failures remain in the record as failures. Differences between the early and final conditions prevent a performance gain percentage.

<a name="erişilebilirlik"></a>

## Accessibility

The local `pnpm.cmd test:a11y` command runs the real App with an isolated transport. The axe-core 4.14.0 package is a development dependency. The native API guard rejects a real Tauri module in the test bundle. The final frontend candidate `ui-36e174ba-f4b9-4c8a-a94f-a1463c7c008c` passed 68 audits with 0 violations. It covered Turkish and English, light and dark themes, data, empty and error states, four screens, and dialogs.

Activity error focus and announcements, Tab, Shift+Tab, Enter, Esc, and focus restoration passed. Navigation p95 was 23.30 ms. Actions fit without horizontal overflow at viewports equivalent to 1024×720 at 100%, 125%, and 150% scale. Actual OS DPI, WCAG certification, and manual screen reader use require their own evidence.

Sky mouse and keyboard input narrowed the native window to 1015×711 CSS pixels, slightly below the 1024×720 target. The skip link showed focus. Enter moved focus to the main content. The test set 125% and 150% in Windows Settings. It recorded GetDpiForWindow values of 120 and 144 with WebView DPR 1.25 and 1.5.

Application view scale stayed at 100%. At 125%, the test observed Pause. At 150%, it observed Resume. The test covered four screens, bottom actions in the connection dialog, and closing the dialog with Esc. At 150%, the display reduced the window height to 650 CSS pixels. The actions fit without horizontal overflow.

Native evidence covers Turkish in dark mode. Automated UI checks cover the other language and theme combinations.

The historical run restored Windows Settings to 100% and the native window to 96 DPI and DPR 1. It closed Settings. Evidence includes `os125*`, `os150*`, `os100-restored*`, and `keyboard-*` JSON files and native captures.

The run reached its 20-minute limit and ended by forced termination. The overall run remains FAIL.

A separate shutdown of the earlier candidate exited normally with code 0. That exit applies to the earlier binary. The current runs above cover the user's tray confirmation and normal shutdown. The test left no engine running.
