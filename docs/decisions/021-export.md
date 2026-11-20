<a name="h033--istatistik-ve-tanı-dışa-aktarma"></a>

# H033 — Statistics and diagnostic export

Started 7 November 2026 and completed 8 November. 32/51. H034 did not start. The lead agent owned the measurement snapshot, allowlist, file/process ownership, native integration, and acceptance. The single Luna xhigh worker completed the frontend/client/test slice under the frozen IPC contract. Root's review covered one fix batch and final integration/acceptance. Ponytail full, Emil, and DESIGN.md were preserved.

<a name="kapsam-ve-sözleşme"></a>

## Scope and contract

Overview provides one Export menu for statistics JSON/CSV and diagnostic JSON. Statistics contain only a content-free total/coverage summary for the selected period. No request/model/chat list or cost estimate is added. Keep only the safe aggregate projection of each successful real overview response, with its current request UUID, in RAM, up to 8 records. Export selects the snapshot captured before the save picker opened and does not run a new measurement query. Traffic arriving later does not change that snapshot. A missing/evicted ID returns snapshot_expired. Diagnostics can be exported without a snapshot from the error screen. The frontend sends no path, file content, or measurement value.

JSON preserves signed saved_tokens/saved_percent, before/after, period/UTC boundaries, status/coverage, and a fixed local-source description. Unknown remains null. CSV uses UTF-8 BOM, CRLF, semicolons/decimal commas for TR, and commas/decimal points for EN. Protect metadata text cells against quotes/formula prefixes. The safe aggregate projection excludes raw basis/tokenizer, model labels, auth, config, personal paths, HTTP/body/log data, and chat identity. Diagnostics contain only allowlisted fixed app/OS architecture and public status/error categories, plus the same aggregate snapshot when available. No new model/provider request is made.

Native Save As uses the existing COM/Windows dependency. The selected path/name is not returned to the UI. Save only a new file. Reject an existing file, link/reparse point, or unsafe path. Preserve parent-directory ownership/pinning. In the same parent, flush/sync the staging file and target it with an atomic move that does not replace. Cancel creates no file, and an error is not success. Allow only one export at a time. Do not start a new export during exit, and return busy for exit confirmation while an export is open. Add no dependency/global file access/OS preference.

IPC is `export_report({request:{request_id,kind,language,snapshot_request_id}})`, with kind statistics_json/statistics_csv/diagnostics_json and language tr/en. Statistics requires a snapshot ID. Diagnostics allows null. The fixed saved/cancelled envelope contains kind and bytes_written. Errors are invalid_input/snapshot_expired/busy/unavailable/unsafe_path/file_exists/write_failed. Secrets and raw error/path data are not in the envelope. Report only OS support that was actually verified. Preserve the existing free Windows 11 exception.

<a name="üç-kabul-senaryosu"></a>

## Three acceptance scenarios

1. Select a period from the real store → Save JSON/CSV from the UI → the file contains the signed total/percentage, before/after, period/source/coverage values from that same capture. Later measurements or a period change do not change the file. Verify TR/EN, 0/negative/unknown/mixed values, and CSV formula/canary protections.
2. Cancel/busy/expired/disk error or unsafe-existing target produces no success. Preserve the old file/state and provide fixed feedback with a retry path. Check export/exit races while the picker is open and release the guards. Do not read/copy auth/model/client settings into the report.
3. In the final native build, use Save As for JSON, CSV, and diagnostic files, plus Cancel. Compare files with the backend/UI snapshot. Check narrow view and keyboard/focus/pending/error in all themes/languages. Run the final candidate's local WORKFLOW.md gate and record programmatic before/after source/tool/OS/artifact fingerprints. Restore normal state.

For focused verification, after the test is added, run `node --experimental-strip-types --test tests/app/export-report.test.mjs` and Rust `export::` filtered tests. Run the full local gate once on the final candidate. Evidence is in `work/proofs/export/`, the report is `docs/proofs/export.md`, and the review is `docs/reviews/H033.md`. Keep TASKS/checkpoint consistent with each slice. Do not report an unmeasured savings rate. Stop when H033 is complete.

Acceptance result. 161 frontend/371 Rust PASS (8 opt-in ignored), with build and fmt also passing on the same candidate. 20 UI combinations plus 6 behavior groups passed. 18 native checks covered three files and Cancel. 16 batched checks linked 180 source/config inputs. The initial native interruption and final helper SQLite cleanup error were not presented as product success. Original state was restored using separate fingerprints, and closed process/port were observed. The helper was fixed and the directory-move smoke check passed. A native exit code was not recorded. No result is inferred for another OS/live provider/installer. [Acceptance report](../proofs/export.md).

Win32 behavior was checked against pinned windows-sys0.61.2 source and the [Microsoft FILE_RENAME_INFO documentation](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info). `ReplaceIfExists=false` does not replace an existing target. Rename is performed through an open handle. Parent pinning and writer boundaries were also verified with real Windows filesystem tests. No power-loss durability is claimed.
