<a name="h035--erişilebilirlik-ve-performans"></a>

# H035 — Accessibility and performance

Started 8 November 2026. 33/51 complete, H035 open. The lead agent owns acceptance, real Windows/DPI/tray checks, and process-owned performance measurement. The same single Luna worker performs only a read-only accessibility source review. The lead agent implements product changes. Ponytail full, Emil, and DESIGN.md are preserved. If a fix slice exceeds five files, split it by behavior and handle the slices in order.

<a name="amaç-ve-korunacak-sözleşmeler"></a>

## Purpose and contracts to preserve

Check keyboard access, accessible names, readable contrast, and narrow/high-scale layouts across the four main screens and the connect/disconnect/request-details/exit dialogs. Do not change measurement, model/auth, user config, runtime routing, or owned teardown. Mock data is only an explicitly isolated UI test. It is not live account or savings evidence. Measure CPU/memory separately for the existing real engine and app. Add no backend, auto tuning, or continuous monitor.

axe-core is a pinned development dependency for tests and is not packaged with the app. Separately check the four main screens in TR/EN/light/dark and empty/data/error states, along with open modals and the export menu. The target is no critical/serious violations among the relevant WCAG 2 AA automated rules, assessed together with manual review of names/focus/ESC and important text contrast. Automated checks are not WCAG certification or real screen-reader testing.

<a name="önceden-belirlenmiş-ölçüm-koşulları"></a>

## Predefined measurement conditions

Use the local Windows 10 development binary, one app window, and only a loopback fixture instead of a network provider. Record reference CPU/RAM/OS/tool versions in the fingerprint. The UI group is the app and its own WebView processes. The engine group contains only owned helper/Python/engine processes. Do not sample external apps or other Codex sessions. CPU percentage is equivalent to one logical core over wall-clock time. Do not divide it by 16 cores to make it look smaller. Record memory as private bytes and working set separately.

Thresholds for short runs are fixed before observing results.

- Engine stopped for 60s and ready/idle for 60s. Average CPU for each group must not exceed 10% of one core. UI private bytes are at most 512 MiB and engine private bytes at most 1,536 MiB.
- Send two warm-up requests, then 10 local chat-completions requests sequentially at no more than one per second. Each request carries the same fixed 128-line synthetic tool JSON. Record content hash/size/line count. Warmed loopback end-to-end p95 must be at most 5s. UI private bytes are at most 512 MiB and engine private bytes at most 1,536 MiB. This is not provider latency or performance across all workloads.
- Native startup remains within the existing START_REPLY_TIMEOUT 100s limit. Short UI navigation response p95 is at most 250ms in an isolated real-App run. Do not conflate it with network or startup time.

In the first measurement, the first 60s after the ready signal was incorrectly counted as an idle window. Pinned 0.41.0 `proxy/server.py:1974` and `:2204` show that model warm-up continues in a daemon thread after startup returns. A CPU spike was also recorded during measurement. Preserve this initial failure. On the final candidate, report the fixed 60s after startup separately as “warm-up,” then measure 60s idle. CPU/memory/latency thresholds remain the same, and warm-up CPU does not count toward idle acceptance. This distinction does not hide slow/heavy startup cost. The performance report includes both phases.

The second run's first engine-off minute also overlapped with WebView startup/initial scan. The first 10 samples had a UI average of 41%, while later windows were 6–8%. Preserve this failure too. The idle precondition is now verified using actual committed Overview props, a stopped engine, ready metrics, and three completed connection reads (`frontend-settled.json`). Then measure a new 60s window. Report initial startup/scan cost separately. Do not turn the old failed window into a PASS.

Do not loosen thresholds for a failed measurement. Record the condition/root cause and small fix. This run does not prove long-term stability, production release-binary performance, or real provider cost.

<a name="kabul-senaryoları"></a>

## Acceptance scenarios

1. Across four screens and modals, important actions are available with Tab/Shift+Tab/Enter/Space/ESC, names and focus return are correct, and there are no critical automated accessibility errors in either theme/language. At 1,024×720 and 125%/150% scale, there is no horizontal overflow or inaccessible critical action. Record real OS DPI separately from app WebView zoom.
2. For the native window/tray, record actual Windows DPI and check access/reopen plus the relevant actions/focus. If the test changes OS display scale, do not exit before restoring the initial value. Preserve user-specific persistent settings/state. Acceptance requiring unavailable external conditions remains open/not_tested. Do not treat WebView zoom as OS DPI evidence.
3. Measure app and engine separately under the fixed data/load/duration/thresholds above. Verify normal graceful shutdown, absence of owned ports/processes, and preservation of other listeners and original state. Record final source/config/test/tool/OS/binary fingerprints with the result in the same step. Run the final product candidate's local WORKFLOW gate once. Do not rerun unchanged successful blocks.

Evidence is in `work/proofs/accessibility/`. The repeatable UI gate is `tests/e2e/accessibility.spec.ts`. Measurement conditions/results are in `docs/validation/performance.md`, acceptance in `docs/proofs/accessibility-performance.md`, and review in `docs/reviews/H035.md`. When H035 is accepted, update TASKS/checkpoint, notify the user, and stop. H036 does not start.

Checkpoint from 8 November. Accessibility/local quality gate, fixed performance phases, actual Windows 125%/150% DPI, and restoration to 100% passed. In the final separate native check, the app exited normally with code 0 and preserved state and port behavior. The performance run's forced termination at its 20-minute limit remains a separate FAIL. `checkpoint-acceptance.json` links all valid inputs. Only the tray-menu check is missing. Sky did not expose the system tray as a targetable window and no user response arrived. Do not loosen this acceptance. H035 remains open/33/51. H036 does not start.

<a name="h036-sonrası-h035-kapanış-adımı-8-kasım"></a>

## H035 closure step after H036 — 8 November

H036 was completed under the user's explicit order exception. The current total is 34/51. The user returned to the PC and reported that the full X → reopen from tray → Pause → Resume flow worked on the current candidate. A passive committed record verified enabled/paused transitions in the same engine generation with revision0→4. A separate run ended with normal exit0/state/Run/port preservation. Evidence is in `work/proofs/compatibility/native-runs/2dd91219-d59e-47be-8ccb-c2b6d6a57fcd/`.

The lead agent completes the remaining acceptance. No new worker or product-code change is needed. H036 changed the EXE/dist and Applications screen, so refresh the current candidate's local phases once using the existing `native.py`, `settled.mjs`, and `measure-settled.py` conditions/thresholds instead of treating the old performance candidate as current. Source/binary change is the concrete reason to rerun. On real Windows at 125%/150% DPI, check the changed Applications screen's collapsed/open compatibility details and access to critical actions, then restore the initial scale. Preserve recorded DPI evidence for other unchanged screens within its scope. Reuse H036's current 173 frontend/386 Rust/84 audit gate only if its source/tool/environment fingerprints still match. Do not rerun product tests. A short performance run is not a long-load/provider/release guarantee. Stop after H035 acceptance. H037 does not start.

Result from the current run. `93ce1a01-e141-42b3-b8fe-4afeea51fd0a` completed with normal exit code 0. Off/ready UI CPU was 7.54%/9.85%, engine CPU 0%/0.26%, and p95 under the same load was 35.88 ms. Memory thresholds passed. The real 125%/150% DPI Applications check and restoration to 100% passed. 46 technical checks passed. The final documentation entry separates the README status correction from product changes. H035 is complete, 35/51. H037 did not start. Cross-platform native Actions work is manually planned under H048/H049 with the current free-quota/hard-stop verification. CI was not run for this task.
