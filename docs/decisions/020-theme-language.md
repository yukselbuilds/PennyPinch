<a name="h032--tema-ve-iki-dil-son-kabulü"></a>

# H032 — Final theme and bilingual acceptance

7 November 2026. Complete, 31/51. The lead agent owned persistent-store concurrency, implementation/review, and acceptance. The single Luna worker performed only a bounded read-only copy/locale review. Ponytail full, DESIGN.md, and Emil's guide were applied. H033 did not start. Work stopped at the end of the task.

<a name="kapsam-ve-korunacak-davranış"></a>

## Scope and behavior to preserve

Verify the four main screens in TR/EN and light/dark combinations, system-theme following, priority of manual selection, and preference persistence across reopen. Check dates/numbers/signed percentages/durations, unknown versus 0, long text, and dialog/focus/overflow behavior. Do not move existing component-local bilingual dictionaries or theme tokens solely to satisfy task-file suggestions. Add no i18n/framework/dependency. Make the smallest connected fix only for a concrete gap. Do not change model/auth/measurement semantics, OS appearance settings, startup registration, or H031 operating preferences.

Native acceptance found a concrete startup race. Overview/Activity measurement reads for the retention preference and App language/theme reads concurrently requested the named mutex from cloned PreferenceStores, and the zero-wait OS lock returned busy to one caller. Read/save calls on the same store are now serialized with a shared std Mutex. The Windows cross-process lock, CAS/atomic replacement, and error contract do not change. The store does not call back into runtime/metrics, and the lock guard is released before returning the reply. Records are not overwritten with defaults. Retry behavior and time limits are not loosened. The lead agent owned the fix and the 4×16 parallel-read regression.

<a name="kabul"></a>

## Acceptance

1. Real App with isolated transport. Cover Overview/Applications/Activity/Settings in every theme/language, data/empty/error/unknown states, 1,280/1,024 viewports, and narrow 819/683/507 viewports. Compare locale numbers/percentages/dates with actual Intl output. Preserve negative values and 0. Main controls and long text do not overflow. This is fixture coverage, not real traffic/savings evidence.
2. When the system changes from light to dark, system selection follows it. Manual dark/light selection remains unchanged. A failed/stale preference response does not claim persistence. Root updates lang/theme correctly. A headless media-query simulation is not an OS-setting change.
3. In the final native build, verify real theme/language selection, main-screen navigation, close/reopen, and matching saved preferences. Restore normal preferences/runtime/metrics state after the isolated run. Do not write real user startup/client/auth settings. Use Sky only for UI input. IPC records only the minimum metadata needed. Windows 10 is local. Windows 11 remains not_tested under the prior free-work exception. Do not infer real OS DPI/other OS/installer results.

Evidence is in `work/proofs/appearance/`, the report is `docs/proofs/theme-language.md`, and the review is `docs/reviews/H032.md`. Record the result and current source/config/build/environment fingerprints programmatically in the same verification step. Reuse PASS for unchanged H031 source/tests/config only when tool and artifact fingerprints match. For a concrete product change, rerun affected focused checks and the final candidate's WORKFLOW.md gate. When acceptance is complete, update TASKS/checkpoint, notify the user, and stop. Do not report an unmeasured development-savings percentage.

Acceptance result. 48 UI screen/state combinations × 5 widths and locale/system/manual/failure checks passed. 3 native sessions and 2 reopens passed. The race regression failed before the fix. 12 store tests and the final full gate passed with 153 frontend and 363 Rust tests (8 opt-in ignored). 26 checks linked 171 source/config inputs to the final artifact/environment. Details are in the [acceptance report](../proofs/theme-language.md). This is not evidence of a real OS theme change, a second OS, or the installer.
