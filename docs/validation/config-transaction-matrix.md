<a name="h018--config-işlemi-kabul-matrisi"></a>

# H018 — Configuration transaction acceptance matrix

22 October 2026. **Local acceptance complete.** The test used Windows 10 Pro 22H2 x64 build 19045 under normal privileges. The disposable project fixtures shared one local fixed NTFS volume. The test preserved real client, authentication, and chat settings.

The checks covered A1 native TxF probe, A2 transaction_fs_tests, B1 transaction_tests, B2 ownership_tests, and C1 connection_actions_tests. The final 160 Rust tests include these checks.

| State | Required behavior | Result / evidence |
| --- | --- | --- |
| Disabled, expired or missing plan | Config and ownership remain unchanged. A fixed error appears. No false success occurs. | **PASS** - C1 blocked/expired/replay/stale. The UI disables Apply. |
| Scan, installation, endpoint or environment root changed | Report stale/conflict. Do not write to an arbitrary target from the old plan. | **PASS** - C1 root/discovery/endpoint drift. Lease and current-installation checks ran. |
| Different file identity with the same bytes / JSON↔JSONC slot drift | Reject the old fingerprint. Do not silently select an alternative file. | **PASS** - C1 same-bytes/new-ID and alternate-slot appearance. |
| Custom endpoint in config | Block the operation. Do not write the previous URL or auth to any ownership record. | **PASS** - B1 rejects a custom endpoint. Planner blocked. |
| The endpoint already exists without ownership | Do not write or create ownership. Do not undo a user setting as if Headroom owned it. | **PASS** - C1 no-op/store passive. UI has no unowned Undo action. |
| Normal TOML/JSON/JSONC apply | Change only the endpoint. Preserve comments, unrelated bytes, model, effort, auth and hooks. Keep the file parseable. | **PASS** - B1 byte-exact/Unicode/CRLF/comment/comma. C1 and Native cover all three formats. |
| Config file does not exist | Create without clobbering. If a user file appears at the same time, preserve it. | **PASS** - C1 missing Codex and B1 missing Claude. A1/A2 absent-slot external CREATE_NEW guard. |
| Open writer or concurrent replace/rename/delete | Prove the selected primitive's actual Windows behavior. Do not claim lost-change prevention or unmeasured CAS. | **PASS** - A1 separate racer and A2 parent/child intent lock. Earlier writer admission fails closed. |
| Error before commit | Keep the old config valid and unchanged. Do not report ownership success. | **PASS** - C1 separate precommit rollback for Apply and Undo. A2 rejects poisoned/sealed commit. |
| Process interruption at commit/record boundaries | Use a real child process for the crash and reopen check. Keep config valid. Report unowned routing or uncertain results as recovery or conflict. | **PASS** - A1 real child exit 73 kept both old files. Exit 74 left both new files. A Rust kill and power loss require separate evidence. |
| Undo after an unrelated user change | Preserve new user fields and comments. Remove only the endpoint that is still owned. | **PASS** - B1/C1. Native Codex/OpenCode atomic-save preserves a new inode and userNote. |
| User changed or removed the endpoint | Do not overwrite the new value. Report conflict or an explicit no-op. Do not restore the full backup. | **PASS** - B1/C1. Native OpenCode detects a changed endpoint conflict. Config and record hashes stay unchanged. |
| User added data to an app-created container/file | Do not delete the container or file with the user's content. | **PASS** - B1 created-ancestor edits and C1 missing-file leaf undo. The full file is not deleted. |
| Repeated apply/undo or installations that share a config | Do not create duplicate ownership, duplicate fields or excess authority for the wrong installation. | **PASS** - C1 replay/no-op/revision and B2 owner guard. UI allows one pending dispatch. |
| Ownership record is corrupt, oversized, uses an unknown schema or changed | Fail closed. Do not change config or expose raw record/secrets in errors. | **PASS** - B2 strict codec/schema/duplicate-key/source/mask/16KiB/root/revision. |
| Junction/reparse/hardlink/unsafe root or read-only file | Reject unsupported writes. Do not silently weaken ACLs or attributes. Do not change targets outside the root. | **PASS** - A2/B2 real junction/hardlink/read-only and parent-rename lock. EFS guard had source review. No real EFS fixture ran. |
| Application restart | Find persistent ownership and required undo state. Do not revive an expired plan automatically. | **PASS** - B2 reload. Native Claude can Undo from the same record after a new process, without Preview. |
| UI action → command → manager → real file | Observe apply/undo under native disposable roots. Do not treat synthetic discovery metadata as real-client compatibility. | **PASS** - Native Apply/Undo for all three formats, OpenCode conflict/Reload and Claude restart. Both exits were 0. |

See the final gate (local receipt `work/proofs/config-transaction/final-quality.json`), static UI (local receipt `work/proofs/config-transaction/ui-review/summary.json`), and native checks (local receipt `work/proofs/config-transaction/native-proof.json`). The 12 file checks (local receipt `work/proofs/config-transaction/native-checks.json`) and [A1 and later slices](../proofs/config-transaction.md) support this record.

All 160 Rust and 49 Node tests passed. Frontend, typecheck, and embedded builds passed. The team reused the results with matching format, frontend hash, and toolchain inputs.

Windows 11, cross-volume operation, EFS, power loss, and live auth, model, and agent behavior require separate evidence. The [TxF maintenance and future support risk](../decisions/005-config-transaction.md) remains. H019 was pending at this checkpoint.
