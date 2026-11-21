<a name="h018--config-işlemi-kanıtı"></a>

# H018 — Configuration transaction evidence

October 22, 2026. **H018 local acceptance is complete.** The open items in stages A, B, and C record earlier progress. Section D records the final result. The tests kept user config, auth, and databases unchanged. They used only new project fixtures with UUIDs. The tests changed no Windows system, registry, or ACL settings and requested no elevation. [Decision 005](../decisions/005-config-transaction.md), [acceptance matrix](../validation/config-transaction-matrix.md)

<a name="d--son-aday-ve-native-kabul"></a>

## D — Final candidate and native acceptance

The final local gate followed the lead review and the same Luna worker's C3 label fix. The frontend passed 49 Node tests, type checking, and the Vite build. The run started no server.

Rust recorded 160 passes, 0 failures, and 3 ignored in 19.188 seconds. The embedded custom-protocol build passed in 15.021 seconds. The 3 ignored tests include separate opt-in tests for the pinned runtime and real discovery. Parent tests call the external filesystem child helper.

The build emitted two dead code warnings for `preview_config` and `config_was_missing`, plus a summary line. The wrapper counted 3 warning lines. Matching source, config, toolchain, and artifact hashes allowed reuse of formatting and frontend checks. The records contain 57 source and config fingerprints. Frontend (local receipt `work/proofs/config-transaction/frontend-quality.json`), full gate (local receipt `work/proofs/config-transaction/final-quality.json`)

The static UI passed 32 base layouts, 8 dialog layouts, and 7 flows. Coverage included Turkish and English, dark and light themes, and 1280×800 and 760×620. It recorded 0 JavaScript errors and no external requests. The run used Edge 155.0.4283.62 and Playwright 1.62.1. The lead reviewed the dark ownership view and the narrow light Turkish view.

Checks covered the pending lock for Apply and Undo, no-op and blocked or expired states, and snapshot errors. They also covered rereading after invalid, ambiguous, or rejected results, Undo with no installation, and focus return. The static test used synthetic IPC. The native evidence follows below. Static UI (local receipt `work/proofs/config-transaction/ui-review/summary.json`)

The tested EXE was 15,866,368 bytes with SHA `8cf08831ac4c32b53952c07bd76b0ebe6f042a381e175ae5b73465359ebdb624`. OCU sent real mouse and keyboard input. The flow ran from the UI through Tauri, the same manager, and TxF to actual disposable files. Final native acceptance (local receipt `work/proofs/config-transaction/native-proof.json`). 12 independent file and record checks (local receipt `work/proofs/config-transaction/native-checks.json`). Run A normal exit (local receipt `work/proofs/config-transaction/native-run-a.json`). Run B reopen and exit (local receipt `work/proofs/config-transaction/native-run-b.json`)

- Apply wrote the Codex TOML endpoint. A normal editor's atomic save added `userNote`. Undo removed only the endpoint and kept the added field.
- Apply wrote the Claude JSON endpoint. Alt+F4 closed the app normally with exit code 0. The new process found ownership without Preview and ran Undo. The original file bytes matched exactly.
- Apply wrote the OpenCode JSONC endpoint. After the fixture changed the endpoint as a user would, Undo showed a conflict and kept the config and record hashes unchanged. The fixture restored the managed endpoint value and saved an unrelated edit atomically. Reload and Undo then worked. The comment and the new field remained.

The manifest records the native fixture UUID and all verified paths. The child process used its own CODEX_HOME, CLAUDE_CONFIG_DIR, XDG_CONFIG_HOME, and other isolation roots. The test ran no real client or hook and sent no engine, provider, or model request. The npm version metadata came from a synthetic fixture. Live client compatibility requires separate acceptance. Manifest (local receipt `work/proofs/config-transaction/native-fixture.json`)

Both native runs exited normally with code 0. Preferences stayed the same. The final three ownership records showed `released`. The fixtures had no endpoint. The app process count was 0, and port 51778 had 0 stable listeners. The test retained its files and three secret-free released tombstones. No monitoring or test window stayed open.

OCU reported `appNotFound` after closure, and an independent check confirmed exit code 0. The seven cleanup targets that automated review rejected remain frozen. The test attempted no alternative cleanup.

The local scope was Windows 10 Pro 22H2 x64 build 19045 with normal privileges on the same fixed local NTFS volume. Decision 005 records TxF maintenance, future support risk, and fail-closed behavior.

Windows 11, real EFS fixtures, cross-volume operations, power loss, live auth and models, and main and subagent use remained unverified. Installer and release evidence remained open. This task did not repeat H040's historical runtime start, stop, and pinned engine checks. Work stopped before H019. Token savings remain unmeasured.

<a name="a1--klasik-yayın-apisi-feasibility"></a>

## A1 — Conventional file publishing feasibility

The same Luna worker tested isolated file fixtures. With target shareREAD-only, normal writer open and ReplaceFile returned Win32=32. MoveFileEx replace returned 5. FileRenameInfoEx/POSIX returned 32.

Target shareREAD|DELETE with a weak parent pin allowed POSIX absolute rename and also allowed an external ReplaceFile. A stronger parent used LIST_DIRECTORY|ADD_FILE|READ_ATTRIBUTES|SYNCHRONIZE with shareREAD-only. It blocked normal writer open and replace. The relative RootDirectory/FileRenameInfoEx call returned 87. The absolute call returned 32. Safe atomic compare-and-publish remained unproven.

The final probe source has SHA `ddebc2c24d76a1c9c7ad19e406b2aa0a646fc1a904547b008655d45cb00ebf43`. A fixture PermissionError invalidated the intermediate isolation attempt. A separate narrow matrix checked the behavior. The records retained no raw failure output. Initial matrix (local receipt `work/proofs/config-transaction/a4c651f3-0407-475f-b71c-3d6bafd51782/proof.json`), final parent matrix (local receipt `work/proofs/config-transaction/e8a6f67c-8269-4339-9e02-94e20d1f1deb/proof-parent-only.json`), final probe source (local receipt `work/proofs/config-transaction/e8a6f67c-8269-4339-9e02-94e20d1f1deb/probe_isolation.py`)

<a name="a1--ktmtxf-native-feasibility-kabulü"></a>

## A1 — Native KTM and TxF feasibility acceptance

The lead's QA used Python 3.14.0 and Windows 10 Pro 22H2 x64 build 19045 on local NTFS. The two commands exercised different states. The new matrix reused the passing base probe with matching source and evidence hashes.

| Command and evidence | Measured result |
| --- | --- |
| `C:\Python314\python.exe work/proofs/config-transaction/txf_probe.py` — metadata (local receipt `work/proofs/config-transaction/txf-runs/f0d6431c-a2bc-49c2-bb1a-93548fa5f2fc/proof.json`) | Passed. The external reader saw the old config and no ownership record. Writer, MoveFileEx, ReplaceFile, and DeleteFile attempts returned 32 or 6800 while the handle was open. They also failed after the handle closed and before commit. Commit published both files. Explicit rollback and child exit code 73 with open handles kept the old config. |
| `C:\Python314\python.exe work/proofs/config-transaction/txf_crash_matrix.py` — metadata (local receipt `work/proofs/config-transaction/txf-runs/96150dc7-6c9f-4a9b-b7e3-0083a6bef9d6/proof.json`) | Passed. Process elevation=false. A separate child could not write, replace, or delete even after all transacted file handles closed. Before commit, child exit code 73 left both config and ownership at the old values. After commit, exit code 74 left both at the new values. A writer open before transaction admission caused rejection with 6800. The transaction reserved the missing filename. External CREATE_NEW returned 6800. After rollback, the file remained absent. |
| `C:\Python314\python.exe work/proofs/config-transaction/txf_absence_guard.py` — metadata (local receipt `work/proofs/config-transaction/txf-runs/47e7b5c5-8a14-4bbf-972a-f13ad832a6c0/proof.json`) | Passed. The alternate OpenCode slot used TxF CREATE_NEW with READ, WRITE, DELETE, and DELETE_ON_CLOSE. After the handle closed and before commit, a separate child received 6800 from CREATE_NEW. The file stayed invisible externally and was absent after commit. The test could then create a new file with the same name. |

The probes use only their own `txf-runs/<UUID>` roots. Fixture children terminate their own processes with `os._exit`. User and client processes stayed running. The metadata records source, Python, OS, hashes, and results. File hashes refer only to synthetic files. The records contain no real auth or raw failure logs. Base probe (local receipt `work/proofs/config-transaction/txf_probe.py`), additional matrix (local receipt `work/proofs/config-transaction/txf_crash_matrix.py`), absence guard (local receipt `work/proofs/config-transaction/txf_absence_guard.py`)

**This stage accepts native API feasibility.** At this checkpoint the production Rust primitive, ownership schema, PlanManager and typed UI, and final local gate remained open. The findings cover NTFS under the tested conditions. Non-NTFS environments, EFS, other operating systems, kernel-level hostile writes, and power loss remain outside the result. Decision 005 records Microsoft's recommendation against TxF and its possible future removal.

<a name="b1--saf-patchundo"></a>

## B1 — In-memory patch and undo

The lead reviewed the source and 9 focused tests. **This stage passed.** For Codex TOML, Claude JSON, and OpenCode V1 JSONC, the patch adds only a missing endpoint. An identical endpoint keeps the exact bytes and creates no ownership. The core rejects custom, blocked, and mismatched plans. It supports missing Codex and Claude sentinels and keeps the missing OpenCode route blocked. At this stage the test build used an in-memory core. Production disk Apply and Undo remained closed.

The review fix removes the JSONC comma and property token spans separately. It preserves user comments and a single property's trailing comma. The added JSON separator contains no whitespace, which preserves comments and exact bytes on roundtrip. Undo keeps a created container if the user added data or comments later. A user endpoint change produces a conflict. The parser supplies the TOML root key and value spans.

All 9/9 focused `integrations::transaction_tests` passed, along with format and check. The slice added no dependency. The check reported 1 historical dead code warning for the `plan::preview_config` test adapter. The lead reused matching passing checks. File publishing, crash handling, and client adoption require separate evidence. Command, source, and config fingerprints (local receipt `work/proofs/config-transaction/patcher-b1.md`)

## A2 — Rust filesystem transaction primitive

The lead reviewed the Rust primitive from the same Luna worker. **This stage passed.** 11 focused tests passed and 1 child helper was ignored by default. Parent tests ran the helper for external writer, move, replace, delete, and create attempts. Format and check passed. The lead reused matching passing commands. A2 source and manifest fingerprints (local receipt `work/proofs/config-transaction/writer-a2.md`)

Review fixes gave absent-slot reservations and file entries separate indexes. They bound each token to its transaction identity. An invalid mutation after sealing invalidates commit and causes rollback. Commit checks the pins and deadline again.

Codes 32, 33, and 6800 return a fixed Conflict. File handles close before commit. NTFS keeps the transaction intent lock. Limits are 4 paths, 256 KiB, and a 10-second deadline. The primitive uses the same volume and `TRANSACTION_DO_NOT_PROMOTE`. It rejects EFS, reparse points, hardlinks, read-only targets, and unsupported filesystems. A weaker rename fallback remains unavailable.

Native child crash behavior has separate evidence in A1. The source hash changed during the B2 diagnosis. The old A2 pass applies only to its original hash. The team removed temporary instrumentation. The final H018 gate will cover the current primitive's A2 tests.

**New frozen remnants.** Guarded cleanup for seven UUID roots from the earlier failed A2 tests returned `blocked by policy`. Under `work/proofs/config-transaction/runs/` these are `06efb0f5-20be-45f8-9716-75dedf0fb6d1` (config.json/replacement.json), `892ef8e9-f5e5-46d0-9a8f-c61bf191ce9d` (linked.json/source.json), `abef4a2a-169d-4bcb-8a4e-1df19b8e11e7` (original.toml), `be7ba9a2-29df-4355-8d02-1c607da12d7b` (empty), `c1084172-e7eb-4f4f-904e-323690989733` (config.toml), `589a9f5d-a46f-437e-8ac0-a2cceab44861` and `e6aa4a1f-7d6e-4246-830a-81d26aa01236` (preflight.json). These targets, their children, and cleanup through a parent path remain frozen. The later passing tests cleaned only their own new UUID directories.

<a name="b2--kalıcı-sahiplik-alt-kabulü"></a>

## B2 — Persistent ownership acceptance

The lead reviewed the source and accepted 16 focused ownership tests. The store uses 3 fixed client slots and a record limit of 16 KiB. It checks schema, owner tag, UUID, source, presence, mask, revision, and hash. It rejects transitions to a different active root or owner.

The store records the required hash and validated patch receipt. It retains no raw body, auth, URL, hook, or path. The read and stage API uses the caller's transaction. It has no independent commit or backup. Construction and reads of missing records are passive. The service skips prepare for an identical-endpoint no-op.

The store pins existing ancestors before lazy directory creation and checks them afterward. Root rename returned Win32=32. An existing real NTFS junction and a hardlinked slot failed closed. The fixed-stage diagnosis returned Win32=0 for a mismatch between `/` in a nested path and canonical `\`. Pushing each normal component separately fixed it. The nested create and rollback regression remains. The OpenCodeMissing test covers the codec only. The planner keeps that route closed.

Scoped rustfmt and offline check passed. **Full-crate formatting still remained open.** C1 would format changed files including `transaction.rs` and `state/mod.rs`. The current `transaction_fs.rs` SHA was `8b5f8f567855ddbfebe0d7baa0bb18f2748d11203f785a4b025d2ca25cc041e5`. It differs from the earlier A2 hash.

The final gate will cover new and related receipt tests and A2 regressions. The lead reused the passing B2 suite. C1 manager Apply, Undo, and snapshot were active. C2 typed IPC, C3 Graphite, and D native and final quality acceptance remained open. **H018 remained open at this checkpoint.** Final command and source fingerprints (local receipt `work/proofs/config-transaction/ownership-b2.md`)

<a name="c1--gerçek-yönetici-işlemleri-alt-kabulü"></a>

## C1 — Native manager action acceptance

The October 21 candidate remained valid on October 22. The 9 source and dependency hashes matched, as did Rust and Cargo 1.92.0, rustfmt 1.8.0, and Windows 10 build 19045. The team reused checks across the credit and date change. **14 connection-action and 16 ownership tests passed.** They covered 3 config formats and missing Codex.

They covered blocked, expired, replayed, and stale plans and a new identity with identical bytes. They covered root, scan, and endpoint changes and an alternate OpenCode slot that appeared. Tests also passed for an unowned no-op and Undo after an unrelated atomic save. They covered managed-field conflicts and rollback before commit in both directions. Commands and fingerprints (local receipt `work/proofs/config-transaction/actions-c1.md`)

After the lead review, one mapping connects each fixed error code to its retryable status and message key. The manager checks the record source against the current config and plan. The live plan helper checks both expiry clocks and verifies freshness even on no-op. It uses the version allowlist from the planner.

The manager prepares the reply summary and fingerprint before commit. Config and record share one TxF commit. Undo does not require the old after-ID or full-file hash. The snapshot contains only content-free record metadata. Traffic verification remains separate.

`cargo fmt --check` and offline check passed. **Formatting scope.** C2 will replace `include!` with a normal `#[path] mod` declaration and use a complete Rust hash list to verify format coverage. The current primitive added a canonical-target getter. The old A2 pass remains bound to its earlier hash. The final gate will run all regressions.

Credit limits paused C2 before it began. On October 22, the same Luna worker enabled the typed Tauri and TypeScript binding. C3 Graphite and D final quality and native acceptance remained open. **H018 remained open at that point. H019 had not started.**

<a name="c2--dar-masaüstüts-bağı-alt-kabulü"></a>

## C2 — Desktop and TypeScript binding acceptance

October 22. The lead reviewed main, locator, and the TypeScript decoder and tests. **11 focused Node tests, type checking, formatting, and offline check passed.**

The same `Arc<PlanManager>` handles `apply_connection_plan`, `undo_connection`, and `get_connection_ownership`. Filesystem work uses spawn_blocking. A join error returns fixed config_unavailable. The setup locator is passive. Debug uses a project root and release uses app-local data. A store setup failure lets Preview and app startup proceed and creates no state directory.

TypeScript sends only the narrow C1 request fields. It checks schema, request, client, operation, and exact key sets. It checks UUID, hash, revision, port, source, ownership state, and exact fixed error tuples. These checks produce a new DTO. Browser invoke, raw exception or secret forwarding, and automatic mutation retries remain unavailable. Actual client config and traffic remain outside this check. Command and source fingerprints (local receipt `work/proofs/config-transaction/typed-actions-c2.md`)

The action module replaced `include!` with a normal `#[path] mod` declaration. The format record covers 30 Rust sources plus build.rs and Cargo.toml. If C3 leaves Rust unchanged, the final gate can reuse this result.

Offline check reports 2 known unused helper warnings for preview_config and config_was_missing. The code keeps warning reporting active. C3 Graphite was active. Final frontend, full Rust, embedded build, and native UI acceptance remained open at this point. Format record (local receipt `work/proofs/config-transaction/format-pass.json`)
