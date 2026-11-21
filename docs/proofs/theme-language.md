<a name="h032--tema-ve-iki-dil-kabulü"></a>

# H032 — Theme and two-language acceptance

7 November 2026. **Complete, 31/51.** [Plan 020](../decisions/020-theme-language.md), [review](../reviews/H032.md). The lead agent owns application behavior, persistent-record concurrency and acceptance. The single Luna worker reviewed only the localization in read-only mode. Work stopped before H033.

The existing Graphite light/dark tokens and TR/EN component dictionaries were preserved. No new i18n layer or dependency was added. Engine error text points to the translated message shown to the user, not to an unseen raw code. Theme/language use the existing atomic revision/fingerprint store. Root `lang`, `data-theme` and CSS `color-scheme` reflect the same selection.

The team reviewed a native startup where the saved dark preference failed to load and the app showed system/light. The screen correctly showed a busy error, did not change the saved file, and offered Refresh. Parallel reads by clones of the same PreferenceStore caused the race on a Windows named mutex with zero wait.

A shared std Mutex now serializes reads and writes on the same store. Existing cross-process lock, CAS, atomic replace, validation and external-conflict behavior remain. The lead agent made this fix.

<a name="kanıt"></a>

## Evidence

Records are under `work/proofs/appearance/`. `acceptance.json` binds 26 checks to 171 current source/config inputs, the final binary/dist, UI bundle and tool/OS environment. The run recorded results and before/after fingerprints programmatically. No raw auth, prompt, HTTP body or error dumps were kept.

| Scenario | Observed result | Evidence / limit |
| --- | --- | --- |
| TR/EN × light/dark × data/empty/error × four main screens | 48 states passed at widths 1280/1024/819/683/507. No horizontal overflow. | `ui-a613a3ad-734e-4ccd-a14c-274bebf0cf1d/result.json`. It used the real App, isolated typed transport and explained sample measurements. 16 images were produced. The lead agent visually reviewed examples from four different screens/themes/languages. |
| Rendered number/percentage/date/duration, 0/increase/unknown and details/ESC | The displays matched TR and EN Intl output. Unknown values stayed unknown. | Same UI record, Europe/Istanbul timezone. Duration details show 12,5/12.5. The saved activity list rounds to whole numbers. Measurement semantics did not change. |
| System media light→dark→light, manual selection, language change, failed save | Followed system mode. Manual dark stayed fixed. root-lang updated. Error restored the previous saved value. | Same UI record. Media simulation does not change Windows appearance settings. |
| 4 readers ×16 parallel reads on the same store | First FAIL/busy. Then all reads showed en/dark/revision1. File bytes matched. | `read-race-red.json`, `preferences-focused.json`: 12 store tests passed, including stale/concurrent save and CAS checks. |
| Final full local gate | 153 frontend/363 Rust PASS, 8 opt-in ignored. Fmt/TS/Vite/custom-protocol build PASS | `final-2.json`. The concrete race found in native testing invalidated the earlier candidate. The full gate ran once on the final candidate after the fix. Unchanged frontend UI PASS was not repeated. |
| Real native four theme/language combinations and four screens in both languages | PASS. Selected value, saved preference, root-lang/theme and color palette matched. | `native-runs/41c2d1c7-4a15-4ae4-8afd-64716e997d80/phase-*.json`. Windows input used only Sky mouse/keyboard. Typed IPC was read-only observation. |
| Manual EN/light save→exit→reopen and TR/system save→exit→reopen | Both reopens showed saved selections, “Preferences loaded.” / “Tercihler yüklendi.” and the correct appearance. | Same native run. All three sessions exited with code 0. Debug/stable ports closed. No forced close. The initial TR/dark state also loaded without a user retry. |
| State and ownership | Runtime/metrics/preferences bytes were restored. Protected state and real HKCU Run matched. Foreign listener remained intact. | Same run `result.json`. 0 model calls. The initial diagnosis run `3c632979-7ff4-441e-bc95-7f5a60303afe` ended by closing only its owned job after the race appeared. State was restored, and the run was not counted as PASS. |

Local environment: Windows 10 x64 build19045, Rust/Cargo 1.92.0, Node 25.2.1, pnpm 11.25.0 and Python 3.14.0. Windows 11 was not_tested under the earlier H047/decision 006 free-use exception. Native testing read the system preference at that checkpoint. Actual OS theme transitions and DPI have separate acceptance. Other operating systems, installer and release, sustained load, and live providers remain outside this result. No global client/auth/model setting changed. Development token savings were not measured.

H032 closed. The next task, H033 statistics and diagnostics export, did not start. Work stopped at the end of the main task.
