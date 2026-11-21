<a name="h031--çalışma-tercihleri-kabulü"></a>

# H031 — Operating preferences acceptance

7 November 2026. **Complete, 30/51.** [Plan 019](../decisions/019-operating-preferences.md), [review](../reviews/H031.md). The lead agent owned persistent and OS state, retention, engine connection, and acceptance. Luna completed the frontend and client tests. The lead then took over. Work stopped before H032.

<a name="son-davranış"></a>

## Final behavior

- Default X behavior keeps the app in the tray. If the preference is off, X asks for exit confirmation. ESC does not stop the engine. The footer and exit window explain the same active preference.
- Measurement metadata retention is 7/30/90 days, with 30 as the default. The UI explains and confirms permanent cleanup for a shorter period. CCR 7200 seconds is separate. SQLite schema1→2 preserves existing measurements. One persistent date floor prevents replay of older dated measurements after retention is extended. New captures without a known time keep the earlier unknown/first_seen semantics. They make no absolute-age claim.
- The advanced protected-tools list defaults to empty and accepts up to 32 validated full names. It applies at the next manual Start. The UI shows differences between the saved and active lists. Scope covers supported client/model/main-subagent traffic through the same Headroom engine. It does not guarantee results for calls without a tool name or calls outside that route. Synthetic test names are not production defaults.
- Windows startup is a separate user action. The app manages only the fixed HKCU Run value for its own exe command. It preserves stale/different values. Startup opens the window. The engine remains stopped. Windows startup policy still applies. Acceptance tests did not change the real user Run value.
- Operating preferences share the same atomic revision/fingerprint record as language/theme. Reads of old v1 records do not write. An old save call does not reset new fields. Corrupt/busy/conflict states are not silent success.

<a name="son-adayın-kanıtı"></a>

## Final candidate evidence

Records are under `work/proofs/settings/`. `acceptance.json` binds 26 checks to 171 current source/config inputs, the final binary/dist, UI bundle and tool/OS environment. The run recorded results and before/after fingerprints programmatically in the same steps. No raw auth/prompt/HTTP error dumps were kept.

| Scenario | Observed result | Evidence / limit |
| --- | --- | --- |
| Full local regression gate | 153 frontend, 362 Rust PASS, 8 opt-in ignored, fmt PASS | `final-1.json`. One full gate ran on the final candidate. The worker's 6 client tests are included in this total. |
| TS/Vite/embedded native build and Python boundaries | PASS | `build-3.json`, `python-1.json`, including the final modal text. Missing active protection or an enabled beacon does not count as ready. |
| Old records/CAS/tool boundary, SQLite migration/7–30–90/Overview/Activity/reopen | PASS | `operating_*` real FS/SQLite tests. Dated records did not return after retention was extended. |
| Startup enable/disable/reopen/stale/different/adjacent value | PASS | `autostart_*`: real Windows API and UUID-isolated HKCU test key. This is not a real Run or login/reboot test. |
| Two real pinned 0.41.0 generations and tool lists | Two exact 58,800-byte numeric outputs, 2 POST to the same endpoint with a new generation | `live-2.json`, `live-runs/7f2ed1ab-b71d-42db-ae94-bb3e0e8b19f9/result.json`. The old generation remained after the record changed. The next Start verified the new list. Model/reasoning/call ID and stats on/beacon off/TTL7200 were preserved. This used a loopback fixture, not a live provider. |
| Native real mouse: save/reopen in both directions, X confirmation/ESC/same generation, hide to tray | PASS, two owned exit0 and port cleanup | `native-runs/d771efba-25ef-4b6c-9cd8-61e008c33b1d/result.json`, `sky-ui.json`. Sky image/click worked this time. Settled inventory showed 0 visible windows. Runtime/metrics/preferences/protected state returned, and Run stayed unchanged. |
| Busy/retry | Safe error and same record after Refresh | Concurrent reads showed store busy. The UI did not save defaults and kept controls disabled. After Sky Refresh, the UI showed saved close=false/theme/language (`phase-1-reopened-ui-ready.json`). No performance claim is made about the cause of the busy state. |
| Turkish and English, light and dark, at widths 1280, 819, 683, and 507. The test checked pending state, parser, conflicts, and saved and active preferences. | 8 UI behavior groups PASS, no overflow | `ui-d61e2bec-b88f-4989-a2bd-c929497a4395/result.json`. It used a real typed client and isolated native transport. This check covers narrow viewports. Native OS DPI at 125 and 150 requires separate evidence. Earlier example images and final native screens were reviewed visually. |

The native UI test did not change retention or OS startup values. Real isolated backend/API tests and mock UI tests verified them. User state was restored. Auth/model/reasoning and other processes did not change. The first native candidate exposed a tray-text issue. The final native/UI run and full gate include its fix. Unchanged successful checks were not repeated.

Local environment: Windows 10 x64 build19045. The Windows 11 paid run was skipped/not_tested under the earlier H047/decision 006 free-use exception. This result is not evidence for that OS. Real login/reboot, installer/upgrade, other OS, long load and live API-key/provider acceptance remain open in their respective tasks. Development savings were not measured. No percentage is assigned.

H031 closed. H032 did not start. Work stopped at the end of the main task.
