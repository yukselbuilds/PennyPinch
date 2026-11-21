<a name="h033--istatistik-ve-tanı-dışa-aktarma-kabulü"></a>

# H033 — Statistics and diagnostics export acceptance

8 November 2026. **Complete, 32/51.** [Plan 021](../decisions/021-export.md), [review](../reviews/H033.md). The lead agent owns capture timing, record and file security, integration, and acceptance. The current single Luna worker completed the frontend/client/test slice. H034 did not start.

The Export menu in Overview provides a selected-period summary in JSON or CSV, or a sanitized diagnostics JSON file. The report uses the native metrics snapshot shown at the time of the click. New metrics that arrive while the Save As dialog is open do not change the totals in the file.

The RAM cache holds up to 8 aggregate records. The summary covers aggregate period metrics only. Request and model lists and billing calculations remain outside this report. Unknown values use null in JSON and an empty field in CSV. The report keeps actual 0 values and signed increases distinct. Diagnostics can record public status and categories even when metrics cannot be read.

The app writes only to a new file selected by the user through Save As. An existing target, unsafe or linked directory, wrong extension, busy state, or expired capture is not a success. The app completes and syncs the file before it performs a non-replacing rename through the same open handle. It preserves parent pinning and race protection. The review checked the pinned SDK source and real filesystem tests. [Microsoft API contract](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info) No power-loss durability or new OS support is claimed.

<a name="son-adayın-kanıtı"></a>

## Final candidate evidence

Records are under `work/proofs/export/`. `acceptance.json` binds 16 checks to 180 current source/config inputs, the final binary/dist, the UI bundle, and the tool/OS environment. The run recorded results and before/after fingerprints programmatically in the same steps. No raw auth, prompt, HTTP body, or error dump was kept.

| Scenario | Observed result | Evidence / limit |
| --- | --- | --- |
| Strict client envelope, correlation, byte limit, single-flight, cancel/error | 8 new client tests PASS | `worker-f8c322f6-68da-47e2-9b2d-01a6f1e668e5.json`. The lead agent reviewed the wire correction. The final full Node gate includes the same test. |
| Capture pinning/8 records/expired, JSON and TR/EN CSV 0-negative-null, canary/formula, cancel/busy/exit rollback | 5 report + 3 real FS tests PASS | `focused-3.json`. A new/racing target, Unicode filename, extension, and capacity were checked. The old file did not change. The test did not fill the real disk. |
| Final local quality gate | 161 frontend/371 Rust PASS, 8 opt-in ignored. Fmt PASS | `final-1.json`. `build-1.json` binds the TS/Vite/custom-protocol PASS to the same source/config/tool fingerprint. The successful build was not repeated without a change. |
| TR/EN/light/dark ×1280/1024/819/683/507, ESC/focus/pending/error/period/unmount | 20 combinations, 6 behavior groups PASS. No overflow or page error. | `ui-c2cbe56d-dfbc-445f-8ef4-531837db18de/result.json`. It used the real App and an explicitly isolated sample transport. The lead agent reviewed the TR-dark and EN-light images. |
| JSON/CSV/diagnostics through native Save As and Cancel | The real mouse and keyboard produced 3 files. Their contents matched the committed UI capture at click time. | Run `b90930b8-1015-4bab-b182-143d2738f541`, `files-acceptance.json`: 18 checks. The read-only/passive observer did not change results. The native invoke remained immutable. |
| Period/source/coverage/percentage/amount, new snapshot arriving while waiting | JSON/CSV kept the snapshot from the same click. The report kept that snapshot timestamp when a newer UI snapshot arrived. | Same run. A local SQLite fixture used 100 to 50 (50%). This sample is not live provider evidence or development savings. |
| CSV encoding/locale and diagnostics allowlist | UTF-8 BOM/CRLF/TR delimiter and numbers. No credential/content/path/identity fields. | Same 18 checks plus renderer/client tests. The native dialog used TR. The EN UI/client/renderer are covered. |
| Cancel | The fingerprints of three existing reports were unchanged. No fourth file appeared. | `cancel-before.json`, `cancelled-final.json`, `files-acceptance.json`. |
| Exit/state | After UI exit confirmation, no process remained and owned debug/stable ports were closed. Runtime/metrics/preferences baseline bytes, protected state, and Run were unchanged. | `baseline.json`, `cleanup-recovery.json`. A helper script failed to restore a directory because its SQLite context manager left the connection open. A separate safe recovery completed. The native application exit code remains unknown. The record supplies no evidence for exit code 0. |

The first run, `0c93ba02-1869-4b00-90b5-9969bb50e96f`, stopped during a credit/session interruption. It did not count as PASS. `interruption-recovery.json` restored the originals and kept the test state found at that time in separate directories. The final run's recovery record confirmed the same original-state chain.

The helper problem occurred because Python's SQLite context manager did not close the connection. The old artifact stayed unchanged. The helper produced `seed_closed.py` and `native-safe.py`. `seed-smoke-c0df9be9-738e-4410-9710-b7c0026432c6/result.json` confirmed that the directory could be moved after the connection closed. This QA correction did not replace a product test.

Environment: Windows 10 x64 build19045, Rust/Cargo1.92.0, Node25.2.1, pnpm11.25.0, Python3.14.0. Windows 11 was not_tested under the H047/decision 006 free-use exception. Other OS, installer/release, live provider, long-load, and real disk-full behavior were not verified. No client/auth/model/startup settings changed. Development token savings were not measured.

H033 closed. The next task, H034 error and recovery flows, did not start. Work stopped at the end of the main task.
