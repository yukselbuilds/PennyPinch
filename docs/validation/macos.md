<a name="macos-doğrulaması"></a>

# macOS validation

<a name="h048-a--apple-silicon-kaynak-ve-derleme"></a>

## H048-a — Apple Silicon source and build

The first target is a Tauri binary with an embedded frontend for Apple Silicon on GitHub's standard macos-15 ARM64 runner. Win32 file pickers that require window ownership keep the Windows path. Settings operations without a safe macOS equivalent return unsupported. Do not infer engine or client capabilities from a successful build.

| Scenario | Expected result |
| --- | --- |
| Included use and account-spending stop verified, manually dispatched candidate | Typecheck/build with locked frontend dependencies and the locked Tauri build pass. The binary is ARM64 Mach-O only. |
| Approval input is false or verification is missing | The job does not start. Do not count a build or support status as passing. |
| Rust/macOS target differs, inputs changed or build failed | The job fails. Store before/after input fingerprints and a short compiler diagnosis. |

The workflow uses pinned checkout/setup-node, .node-version, pnpm 11.25.0 and Rust 1.92.0. It enforces one standard job with a 20-minute limit and no cache or artifact upload. Before each dispatch, it checks current included usage and the account-level Actions $0 spending stop under the [free-only decision](../decisions/006-free-ci.md). [Workflow](../../.github/workflows/macos-build.yml).

10 November 2026. The first run built the frontend and stopped at a Windows-specific engine-health field during the Rust check. After the platform condition was fixed, the locked Tauri build passed on macOS 15.8.2 and Xcode 16.4. Before/after input fingerprints matched, and lipo verified the binary as ARM64. [First run](https://github.com/Protocol272/headroom-desktop/actions/runs/42867872838), [successful run](https://github.com/Protocol272/headroom-desktop/actions/runs/42868691135), [machine evidence](../proofs/macos-build.json).

<a name="h048-b--ilk-motor-kanıtı"></a>

## H048-b — First engine evidence

Install pinned CPython 3.13.13 and Headroom engine 0.41.0 on the standard ARM64 runner. Use the same dependency lock recorded in the Windows payload. Run the engine in a private directory inside the project. Before reading /readyz or /stats, verify that the socket peer PID matches the child process started by this test. Observe the ready version, local telemetry, disabled anonymous beacon and clean shutdown. Do not send HTTP to a listener with another PID. Do not use installed client settings or provider accounts.

10 November 2026. The relocated CPython 3.13.13 and Headroom 0.41.0 ran as a real process on macOS 15.8.2 ARM64. The locked ONNX Runtime 1.31.0 native module loaded. The test verified the child process directly with LOCAL_PEERPID before the ready response. It sent no HTTP bytes to a foreign socket. Local telemetry was on, anonymous beacon was off and normal shutdown passed. The provider-reachability check was off in this experiment. [Run](https://github.com/Protocol272/headroom-desktop/actions/runs/42870582936), [machine evidence](../proofs/macos-engine.json).

<a name="h048-b--tam-paket-envanteri-geçti"></a>

## H048-b — Full package inventory passed

10 November 2026. scripts/macos-payload.py creates a content summary independent of the package root. It measures regular-file contents and modes, relative symlink text and directory modes. It rejects out-of-root, absolute or broken links, directory links, hardlinks, special files, case-fold name collisions and secret files. Package inputs must belong to the current user and must not allow group or public writes.

The filesystem contract tests passed in the first two native inventory runs. Package preparation then stopped at the permission check. The pinned Python archive had 1,635 entries with group or public write permission. A private umask alone was not sufficient to extract the archive. After wheel installation, the preparation step removed group and public write permissions from copied regular files and directories. It does not follow symlinks during preparation. The final inventory applies the same ownership and permission checks.

The final native run measured 16,640 files and 597,415,285 bytes. The full package summary matched before engine startup and after clean shutdown. The inventory symlink contract tests were not skipped on macOS. Normal engine readiness, foreign-socket rejection, local telemetry and disabled beacon were verified again. [Successful run](https://github.com/Protocol272/headroom-desktop/actions/runs/42875188912), [two failures and final-candidate evidence](../proofs/macos-payload.json).

The inventory code ran 9 tests on Windows. The Windows test environment skipped 5 tests due to filesystem case folding, symlink permissions, and POSIX permission differences. Windows stat and fstat showed different ctime values for the rewritten file, so file-identity comparison handles this by platform. The macOS comparison still includes ctime. The local check includes a one-byte content-change test and a hardlink regression.

<a name="sıradaki-dikey-dilim"></a>

## Next vertical slice

H048-b3 passed. The shared bounded HTTP parser runs over UnixStream. Do not forward control or metrics requests until the code verifies the private 0700 directory, socket inode and UID, actual LOCAL_PEERPID and process liveness. The connection timeout is 300 milliseconds. The HTTP I/O timeout is 1,500 milliseconds. Use the current Windows TCP control contract for verification and responses. The managed engine-session implementation and app/relay connection follow this connection slice.

10 November 2026. The 9 native tests passed on macOS 15.8.2 ARM64, with no ignored tests. Control and metrics routes sent zero HTTP bytes to a wrong PID. The tests covered loss of liveness before and after connect and during a response. They checked rejection of response-ID mismatch, symlink sockets, and unsafe directories. The old metrics sample remained UnknownBasis, and private fixture fields did not enter the projection. The same run produced an ARM64 Tauri binary. On Windows, 40 focused tests passed and 3 opt-in tests were ignored. [Run](https://github.com/Protocol272/headroom-desktop/actions/runs/42883847159), [earlier failures and final-candidate fingerprints](../proofs/macos-unix-transport.json).

The first build stopped at direct Socket → UnixStream conversion. The test fixed it with the locked version's OwnedFd conversion. Mac test diagnostics showed that the timeout setting returned EINVAL for an already-closed Unix socket. The fixture accepts zero bytes only after it verifies real EOF or a closed connection. The live fixture stream is blocking and time-bounded. The test did not weaken the accepted identity or byte conditions. Native tests do not connect to a pinned model/provider account.

<a name="h048-b4--yönetilen-motor-oturumu-geçti"></a>

## H048-b4 — Managed engine session passed

11 November 2026. The pinned CPython 3.13.13 and Headroom 0.41.0 ran on macOS 15.8.2 ARM64 as a direct child of the Rust process. The app checked the resources against the manifest summary embedded in the build. It rejected an untracked overlay or changed file. A temporary control secret went through stdin without entering argv, the environment or a file. The private UDS peer PID check remained before HTTP. Authorized /health and /stats reads received detailed metadata. A wrong secret received only a redacted response.

The final run completed 18 Python checks and skipped 1 check. The workflow rejects skipped native symlink checks. The 9 Unix protocol and 3 managed-session Rust tests passed. The tests verified normal stop, repeated stop, reopening with the same state and preservation of the synthetic cache-marker file. Pause/resume revision changes passed, and a write with an old revision conflicted. Tests did not cover real CCR retrieval or an active model stream in this slice.

In the parent-loss test, the test stopped only its own engine. After the test terminated the owned parent process, the inherited flock lock still blocked a second open. When the engine resumed, kqueue NOTE_EXIT verified its real process exit after parent-pipe EOF. The state lock then opened. The kernel verifies real process exit. Model/provider traffic and automatic cleanup of endpoint leftovers from an exited parent are outside this slice.

The first native run stopped at the CLI's macOS malloc re-exec behavior. The launcher uses the real embedding entry name and sets the allocator environment before Python starts. This preserves bootstrap hooks and -I/-B flags. The successful run verified a 16,640-file/597,415,256-byte payload, the same CI input fingerprints before and after, and an ARM64 Tauri binary. [Run](https://github.com/Protocol272/headroom-desktop/actions/runs/42990619381), [machine evidence and first failure](../proofs/macos-managed-session.json).

<a name="h048-b5a--sabit-giriş-ve-unix-aktarımı-geçti"></a>

## H048-b5a — Stable ingress and Unix relay passed

11 November 2026. On macOS 15.8.2 ARM64, tests verified a private 0700 directory, 0600 record and lock, and the same loopback identity/port after reopening. On conflict, the record and foreign listener stayed intact. Before forwarding client bytes, TCP ingress verifies the Unix peer PID and owned process. The native fixture verified a delta before the terminal event, half-close and completion of the current connection during drain.

The first native run found a Darwin wildcard-listener conflict and an incorrect error for ownership loss during connection closure. The fix added a bounded listener check before and after binding to the registered port. For control, metrics and health reads, the code checks process loss before I/O errors. The final run passed 18 native protocol/ingress/relay tests and 3 managed-session tests. Python checks had 17 passes and 1 skip. Source and 16,640-file payload fingerprints matched before and after. Windows passed 30 focused tests and ignored 1 opt-in test. [Run](https://github.com/Protocol272/headroom-desktop/actions/runs/43002575695), [machine evidence and first failure](../proofs/macos-stable-ingress.json).

<a name="h048-b5b--uygulama-yöneticisi-ve-gerçek-motor-geçti"></a>

## H048-b5b — App manager and real engine passed

11 November 2026. On macOS 15.8.2 ARM64, the app's RuntimeManager and preferences store ran with the real pinned engine. The manager checked the saved protected-tool setting at startup. It opened and cleaned up a private Unix socket under a long user-data path. The stable loopback ingress returned /readyz. The manager read metrics from the same engine generation. Pause and resume changed the revision. It rejected an old write and a stop request for the wrong generation. After stop, ingress did not forward a new request to the engine. Restart and manager reopen kept the same port and identity. The test did not touch a foreign port owner.

The first run stopped at app startup because it used a Windows-specific preference lock. The fix added a macOS 0600 owned-file lock and private temporary write. The final run passed 31 protocol/relay/preferences tests and 4 managed-engine/app tests. Python checks had 17 passes and 1 skip. The ARM64 Tauri build, 16,640-file payload and matching before/after input fingerprints were verified. Windows passed 69 focused tests and ignored 5 opt-in tests. [Run](https://github.com/Protocol272/headroom-desktop/actions/runs/43009523997), [machine evidence](../proofs/macos-application-manager.json).

This run used local readiness and metrics traffic. It did not use live provider traffic or measure savings.

<a name="h048-b6a--apfs-config-yayını-deneyi"></a>

## H048-b6a — APFS configuration publication experiment

The first H048-b6 step tests APFS file operations on a standard Apple Silicon runner. [Decision 005](../decisions/005-config-transaction.md) defines the app's existing plan, Apply, and Undo contracts. Apple defines a conflict for `RENAME_EXCL` if the target exists. It defines an atomic swap of two names for `RENAME_SWAP`. The `flock` lock is cooperative. [Apple rename manual](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/man/man2/rename.2) · [Apple flock manual](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man2/flock.2.html).

| Initial state and action | Expected observation |
| --- | --- |
| While the existing file and parent descriptors remain open, a separate process attempts to lock, write, replace, and unlink | The second cooperative lock fails. Check the actual outcome and file identity for the normal file operations. |
| Publish exclusively with the target absent and present, then test independent replacement and swap after the final read | Publication places the complete file at the absent target. It preserves the existing target. After swap, use the moved file's actual identity and bytes to detect a race. When no more writes occur, swap back to preserve the competing file. |
| The child process exits after the config swap but before the separate ownership write | Observe the new config, the moved old config, and the old ownership record. The result measures process loss at that point. |

11 November 2026. The test verified local APFS and a 0700 fixture directory on macOS 15.8.2 ARM64. A separate process could write, replace, and unlink the held target with normal operations. It could not obtain a second `flock`. Exclusive rename returned `EEXIST` when the target existed and preserved both files.

Swap moved the file that changed after the last check and preserved its identity and bytes. The other process wrote to the moved version through the descriptor it opened before the swap. A controlled swap back preserved that edit at the target.

The child exited after the config swap. The config was new, and the moved config and ownership record remained old. The before and after inputs matched. An explicit macOS SDK path fixed the build error from the first run. [Successful run](https://github.com/Protocol272/headroom-desktop/actions/runs/43013886985) · [Machine evidence](../proofs/macos-config-transaction-probe.json).

This evidence leaves production writes to existing client files through `flock` and rename closed. H048-b6b needs a recovery model that checks ownership and target state after interruption with secret-free identities and fingerprints. Measure whether recovery preserves a write through an old descriptor. Keep writes for that client closed while evidence is incomplete. The experiment measured process loss on synthetic files. Power loss, actual client settings writes, and Undo still need evidence. [Manual workflow](../../.github/workflows/macos-config-transaction.yml).

<a name="h048-b6b--apfs-kurtarma-deneyi-geçti"></a>

## H048-b6b — APFS recovery experiment passed

11 November 2026. The local APFS fixture used the app's Codex settings patch and ownership record. The pending transaction record stores identities and hashes for both file versions. A separate process finished the config swap and then exited before publishing ownership. On reopening, recovery checked both versions and the prepared ownership record. It published only the ownership record. A retry gave the same result.

A write through the old descriptor before or after recovery returned a conflict and preserved both versions. A recreated file with identical bytes provided no ownership evidence. The actual Undo patch preserved a setting added later. Recovery published the ownership record left by the interruption.

Four tests and the locked Tauri build passed on macOS 15.8.2 ARM64. The relevant tests invoked the child helper in a separate process. The direct helper test stayed ignored. CI input fingerprints matched before and after. [Run](https://github.com/Protocol272/headroom-desktop/actions/runs/43020691783) · [Machine evidence](../proofs/macos-config-recovery.json).

Native tests include the recovery prototype. The test kept the moved file and pending record. Actual client Apply and Undo still require protection for late writes to the moved file. They also require native checks through the application commands.

<a name="h048-b6b--gerçek-okuyucu-ve-önizleme-kimliği-geçti"></a>

## H048-b6b — Production reader and preview identity passed

11 November 2026. Native fixtures exercised the application's config reader and app-data ownership factory. The reader uses macOS HOME and lossless POSIX path binding. It checks local APFS, UID, file type, permissions, link count, and the identities of open directories and files. Discovery leaves the app-data directory absent if it does not exist. The run used test files. It did not use real client settings or accounts.

| Initial state and action | Expected and observed |
| --- | --- |
| Read a user-owned 0755 directory and 0644 Codex config, then discover a missing config | The reader preserves bytes and permissions. It leaves missing directories and files absent. The actual app-data factory uses the supplied root. |
| After preview, replace the inode with the same bytes, edit the file in place, replace its parent, or create the missing file | Each change invalidates the saved preview identity. Reject the old preview. |
| Try to read a symlink, an intermediate directory symlink, a hardlink, a FIFO, or a group-writable or world-writable target | The reader rejects the unsafe path and returns without waiting on a FIFO. Two OpenCode config files leave the choice ambiguous. The reader preserves Claude's nested env bytes. |

In the first two native runs, five reader and factory tests stopped with UnsafePath. Diagnostics used only directory depth, UID, permissions, and errno. They reported EINVAL 22. Apple XNU returns EINVAL when the caller sets both O_NOFOLLOW and O_NOFOLLOW_ANY. The reader kept only O_NOFOLLOW_ANY, which blocks symlink traversal for the full path. A native test still verified rejection of intermediate symlinks. [Apple kernel source](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/vfs/vfs_vnops.c).

In the final macOS 15.8.2 ARM64 run, seven actual reader and factory tests, one path codec test, and four existing recovery tests passed. One child-process helper stayed ignored for direct runs. The recovery tests invoked it in a separate process. The frontend, locked Tauri build, and architecture check passed. CI inputs matched before and after. All 264 source inputs matched the candidate.

Only the macOS module changed after the 91 focused Windows tests. The other fingerprints and the toolchain matched, so the team reused the Windows results. The final candidate passed fmt and actionlint. [Run](https://github.com/Protocol272/headroom-desktop/actions/runs/43025755374) · [Machine evidence for the two failed runs and final candidate](../proofs/macos-config-reader.json).

This acceptance covers production config reading and preview identity. macOS FileTransaction and actual Apply and Undo are still open. Installed client discovery, provider accounts, the actual packaged window lifecycle, and Intel macOS support require their own evidence.

<a name="h048-b6b--ortak-manager-ile-config-yayını-geçti"></a>

## H048-b6b — Configuration publication through the shared manager passed

11 November 2026. macOS now uses the shared FileTransaction and OwnershipStore. The lead agent owned production code and real PlanManager acceptance. The same Luna xhigh worker wrote store and transaction tests.

The lead agent checked that the worker stopped before completing metadata and publication race checks. The first write scope covers Codex, Claude, and OpenCode config files in existing user-owned APFS directories. The Windows TxF path remains in effect. Creating a missing Claude project directory still needs separate acceptance. The patch excludes model, auth, reasoning, and agent settings.

The config and ownership files use separate atomic rename steps. A strict compare-and-swap across both files has no acceptance. The secret-free intent in the private app directory stores the required config path as lossless byte metadata. It also stores parent and file identities and content hashes. Config content stays in the unique staging file under the client's own directory.

The moved old inode remains available. A late write survives and causes a conflict on the next operation for the same target. History holds up to 64 intents. At the limit, reject a new write and keep existing data. Discovery never starts a recovery write.

An explicit user action can finish only the ownership step of an interrupted config publication. It never replays an old plan that did not publish its config.

| Initial state and action | Expected and observed |
| --- | --- |
| Private files for three adapters go through real PlanManager Apply → reopen → snapshot → Undo | Only the owned field changes. Undo restores the original bytes. A separate test preserves unrelated edits, 0644 permissions, ACLs, and xattrs. The files in app state exclude synthetic auth, model, and prompt content. |
| An error before commit, rollback, or process loss after seal, followed by an explicit user action | The source stays intact. The old plan receives an abort marker and cannot publish config on retry. In a separate process interrupted with exit 42 after config publication, the next action completes only the matching ownership step. |
| A late write to the old descriptor before or after publication, same-byte inode replacement, or a foreign reverse swap | The app preserves both versions and returns a conflict. Cancelled operations do not qualify as commits. For success, verify the final config and owner identities and the committed marker. |

The first native run passed 18 tests. ACL fixture setup stopped because it read the UID string as a username. Resolving the short username with getpwuid_r fixed setup. After the second candidate passed, the lead agent corrected the reverse-swap success condition and added a real regression test.

The final macOS 15.8.2 ARM64 run passed 19 tests, frontend, the locked Tauri build, and the architecture check. Two child helpers stayed ignored for direct runs. Relevant recovery tests called them in separate processes. CI inputs matched before and after and matched the final source candidate.

Windows passed 129 focused tests with 2 opt-in tests ignored. The team reused those results because source, tool, and environment inputs matched apart from the later macOS-only changes. The final candidate passed fmt and actionlint. [Run](https://github.com/Protocol272/headroom-desktop/actions/runs/43034274808) · [Earlier runs and machine evidence](../proofs/macos-config-apply.json).

The focused Windows gate uses `cargo test --offline --manifest-path src-tauri/Cargo.toml -- integrations::config_io state::ownership integrations::connection_actions --test-threads=1`. The native gate uses the macOS transaction test filter in the same manual workflow. Check current quota before each new CI run. Check the zero-charge limit. Before the final run, the account used USD 3.16 of USD 12 in included compute. Billable usage was 0. The Actions budget was USD 0 with Usage blocked active. The amount represents included quota use.

This acceptance covers the real file backend and shared manager. Npm client metadata and the ready endpoint object are test inputs. Installed client discovery, live providers, and window interaction have separate acceptance. Missing Claude project directories, safe resolution of preserved versions, and the 64-intent limit display remain open. The final H048 candidate uses the full local product gate. This slice used focused checks.

<a name="h048-b6b--eksik-claude-proje-dizini-geçti"></a>

## H048-b6b — Missing Claude project directory passed

11 November 2026. The lead agent added scoped directory publication for a missing `.claude/settings.local.json`. The existing file path stays in effect. The same Luna xhigh worker wrote four transaction and store tests and the separate process helper. The lead agent added the manager flow and the check for a different transaction token. The native RED run passed the prior 19 tests. Six new tests stopped on the unsupported project selection and directory creation path. The production change followed that run.

| Initial state and action | Expected and observed |
| --- | --- |
| Select a project, preview the missing config, Apply, reopen the manager, and Undo after user additions | Preview, create, and seal kept `.claude` absent. Commit published the private 0700 staging directory exclusively with the new 0600 settings. Undo preserved the user's JSON fields, sibling file, and directory. The user config stayed intact. |
| Fault or rollback before commit, or drop after seal. After directory publication, a separate process exits with code 42. | Before publication, the source destination remained absent. An explicit new action aborts the unpublished old plan without replay. After publication, it completes only matching ownership for that directory. |
| Another process creates `.claude`, a different inode replaces the project path, or a different transaction uses the directory token | Publication fails. Competing content and both project directory versions remain intact. The app creates no ownership. |

The intent records the project parent identity and the new directory identity. Old file intents remain readable without the optional fields. Open the saved path only if the selected project identity and active target match. Directory publication and ownership use separate rename steps. The acceptance provides no strict multi-file CAS or power-loss guarantee. Apple documents that RENAME_EXCL returns EEXIST if the target exists. [Apple rename manual](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/man/man2/rename.2).

The final macOS 15.8.2 ARM64 run passed 25 tests, frontend, the locked Tauri build, and the architecture check. Three child helpers stayed ignored for direct runs. Relevant tests called them in separate processes. CI fingerprints matched before and after, and all 266 source inputs matched the candidate.

A shared project selection file changed, so the Windows focused gate ran again. It passed 129 tests with 2 opt-in tests ignored. Formatting and actionlint passed. Each dispatch followed a check of free quota and the USD 0 hard stop. [RED run](https://github.com/Protocol272/headroom-desktop/actions/runs/43062156522) · [GREEN run](https://github.com/Protocol272/headroom-desktop/actions/runs/43063416122) · [Machine evidence](../proofs/macos-claude-project-directory.json).

This result covers a dedicated native fixture, actual manager, transaction and ownership code, and test picker and endpoint data. Real picker windows, installed client or provider traffic, and package acceptance require separate evidence. Resolution of preserved versions and the 64-intent limit display remain open. The run used no new auth, real user settings, global environment changes, or paid calls.

<a name="h048-b6b--korunan-sürümler-ve-kapasite-alt-kabulü-geçti"></a>

## H048-b6b — Preserved versions and capacity acceptance passed

For the H048-b6b preserved versions slice, the lead agent owns storage, path, measurement, and ownership decisions. Read-only review produces a review fingerprint for the selected client, scope, and config. It uses current config and intent and marker identities. The renderer receives no raw config, auth, or other project content. The user's Continue with current settings action checks the fingerprint again. It leaves the config and preserved old inode intact.

An explicit action can finish pending ownership only when the typed receipt matches the current document and recorded ownership transition. An unverifiable state returns a fixed error.

Before setting old records aside, copy the archive metadata to a private 0700 directory. Check the copy. Publish the archive manifest atomically.

Then move old metadata to the archive with fd-relative exclusive rename. An interruption preserves config data. On reopening, the app checks the same manifest. Old config versions stay in the client directory.

Process at most 64 active records and 64 archive groups at once. Keep existing data without automatic deletion. Show the saved count and capacity to the user. At the limit, offer the review path with an explicit explanation. Preserve the Windows TxF path and global and other project settings.

Normal acceptance covers review → explicit acceptance → reopening → normal Apply and Undo. Error acceptance covers changed config or records after review and process interruption during archiving. Boundary acceptance covers 64 records and full archive capacity. Native fixtures and the actual manager path have their own evidence. Renderer behavior and the actual window and package have separate acceptance.

11 November 2026. In native RED, the prior 25 tests passed and three new archive tests stopped with Unsupported. On macOS 15.8.2 ARM64, the final candidate passed 34 tests, the frontend build, and the locked ARM64 Tauri build. The relevant tests called four directly ignored child helpers in separate processes. All 275 source inputs matched the candidate. CI fingerprints matched before and after.

The native tests took 59.96 seconds. That duration records the test run. Performance and sustained reliability require their own evidence.

| Scenario | Observation |
| --- | --- |
| Three adapters with the real manager. Passive review → explicit archive → reopen → Undo | The manager preserves current config and ownership. The reopened manager reads the archive. Typed Undo restores the original setting. It rejects explicit acceptance for a different config or an old fingerprint. |
| A late write to the old descriptor, pending ownership, changed metadata or copy, and process exit on either side of archiving | The app keeps edits to the old inode. It completes matching typed ownership and rejects a changed owned endpoint. A corrupt copy prevents manifest publication. With exit 43 after copies are ready or after the manifest, reopening continues without data loss. |
| 64 active records and 64 archive groups | With full active records, archiving allows a new Apply and Undo without data deletion. At full archive capacity, a new archive returns the fixed limit error. Existing groups and config stay intact. Partially prepared groups also count toward the limit. |

The Windows config_io, ownership, and connection_actions filters passed 72/33/38 test executions with 2 opt-in tests ignored. The filters may overlap. The team reused 210 frontend tests and the build after inputs, tools, and environment matched. An earlier Rust build failure did not count as PASS.

The isolated Edge panel passed five tests. They covered explicit checkbox consent and one call with the full scope and fingerprint. They checked pending and parent-busy locks and a delayed response. They also covered Turkish and English keyboard use, errors, and capacity.

The first worker fixture reset the delay gate on mount. The lead agent fixed it and reran the five affected tests. The lead agent reviewed the image. Actual native window behavior requires its own evidence.

The focused Windows command is `cargo test --offline --manifest-path src-tauri/Cargo.toml config_io -- --test-threads=1`. The same command uses the `ownership` and `connection_actions_tests` filters. The panel command is `pnpm exec playwright test --config tests/e2e/retained-versions.config.ts --max-failures=1`. The native job uses the existing `macos-build.yml` workflow with `verify_config_recovery=true`.

Before the final dispatch, the team verified USD 3.78/12 in included compute, 0 billable usage, and an Actions budget of USD 0 with Usage blocked. The included compute figure records quota use. [RED](https://github.com/Protocol272/headroom-desktop/actions/runs/43068865595) · [GREEN](https://github.com/Protocol272/headroom-desktop/actions/runs/43074294190) · [Machine evidence](../proofs/macos-retained-versions.json) · [Review](../reviews/H048.md).

<a name="h048-c1--kurulu-cli-keşfi"></a>

## H048-c1 — Installed CLI discovery

The standard Apple Silicon runner places three official, pinned CLI binaries in a private test directory inside the project. DiscoveryManager scans extensionless commands under PATH, `.local/bin` and `.opencode/bin` in read-only mode. It identifies versions by matching file size and SHA-256. Discovery reads file permissions to determine if a file is executable. [Codex version](https://github.com/openai/codex/releases/tag/rust-v0.163.0-alpha.11.1) · [Claude install layout](https://code.claude.com/docs/en/setup#auto-updates) · [OpenCode V1 install source](https://github.com/anomalyco/opencode/blob/v1.19.3/install).

| Start state and action | Expected observation |
| --- | --- |
| Pinned Codex, Claude Code and OpenCode V1 binaries exist under PATH, `.local/bin` and `.opencode/bin` | A scan of the real environment finds all three tools with the correct versions. Two links to the same target produce one row, and the IPC response has no path. |
| An executable with an unknown version, a writable file, a non-executable file and a looped link are present | The unknown version appears separately. Reject unsafe candidates. Run none of them. Do not create a settings file during the scan. |
| Use an installation ID from a discovery result after the next scan | Reject the selection from the old scan. Do not show version recognition as live connection status. |

The optional native CLI step in `macos-build.yml` performs validation. It records pinned packages, binary digests, source commit, toolchain and environment. Focused Windows discovery tests check the shared source diff. The lead agent owns path and version trust. Luna prepares only official test artifacts and the preparation command. Codex Desktop, the real picker, persistent installation choice and package lifecycle belong to later H048-c slices.

11 November 2026. Four tests, the frontend and the locked ARM64 Tauri build passed on macOS 15.8.2 ARM64. The real DiscoveryManager found Codex CLI 0.163.0-alpha.11.1, Claude Code 2.1.158 and OpenCode V1 1.19.3 from the separate process PATH and HOME. The test placed Claude under its version directory and OpenCode directly under the user bin directory. Two Codex links produced one row. A same-size file with different content remained an unknown version. The scan rejected writable, non-executable and looped-link candidates. The response had no file path. It rejected the old scan ID. The test did not run a client command. Before/after digests for the three binaries and CI inputs matched.

The Windows run `cargo test --offline --manifest-path src-tauri/Cargo.toml discovery -- --test-threads=1` passed 30 tests and skipped two opt-in tests. fmt and actionlint passed. Before the manual macOS run, included compute usage was 3.97/12 dollars, paid usage was $0 and the Actions budget was $0 with status Usage blocked. [Run](https://github.com/Protocol272/headroom-desktop/actions/runs/43079382814) · [machine evidence](../proofs/macos-cli-discovery.json).

<a name="h048-c2--kurulum-seçiminin-kalıcılığı"></a>

## H048-c2 — Persistent installation choice

macOS uses the existing `ChoiceStore` and the same IPC contract for installation choices. The app writes the choice to its own `onboarding/selections.json` file. The first read is passive. The app writes only after an explicit choice. It uses a 0700 directory and 0600 file. It uses a local APFS directory handle, an interprocess lock and atomic publication. The existing record format and Windows TxF path remain.

| Start state and action | Expected observation |
| --- | --- |
| Empty app data → scan → choose a client → reopen with a new manager and scan | The first read creates no file. The choice maps to a fresh scan ID. Other client choices and any Claude scope remain. |
| The selected binary is edited without changing size or replaced with another inode | Show the old choice as unavailable. Do not select a new file by itself. Reject a save request from the old scan. |
| Two writers access the store together, a process stops before or after publication, the current record is corrupt or path safety changes | The lock stops the conflict. Reopen reads only the complete old or new record. Do not automatically change corrupt records or foreign files. |

Real scanning with the three official CLI binaries and reopening with a new manager passed on Apple Silicon macOS 15.8.2. The new `PlanManager` also restored the Claude project choice, and Undo preserved later user additions. APFS tests covered racing writes, interprocess locking, process exit on both sides of atomic publication and invalid records. The 15 macOS tests, the frontend and the locked ARM64 build passed. Two child-process helpers ran in their relevant tests. Windows passed 107 focused tests and skipped two opt-in tests. [Machine evidence](../proofs/macos-selection-store.json) · [Run](https://github.com/Protocol272/headroom-desktop/actions/runs/43083203659).

The test reopened a newly created manager and verified the discovery mapping. The Claude project test read the selected path from the test. The real file picker and reopening of the packaged window will be tested in their own acceptance.

<a name="h048-c3--codex-desktop-keşfi"></a>

## H048-c3 — Codex Desktop discovery

On macOS, inspect the system and user `Applications` directories for `Codex.app` and `ChatGPT.app` candidates. Verify the launcher and `Info.plist` signature locally against the OpenAI identity. Read the version from this metadata. Do not run the client during a scan. Version display is discovery evidence. Keep the macOS Desktop connection plan closed until live client acceptance.

| Start state and action | Expected observation |
| --- | --- |
| Real signed OpenAI package in an isolated application directory → scan | The Codex desktop record appears with the correct version. Keep CLI records separate. Do not include the local file path in the response. |
| Explicit choice → scan with a new manager → package changes | Map the same launcher to a fresh identity. A changed or removed launcher invalidates the old choice. |
| Unrelated, unsigned or changed metadata, symlink, unsafe location | Do not show the candidate. Continue the scan. Do not change client files or settings. |

The lead agent owns package verification, selection and connection boundaries. Luna prepares only version and digest evidence for the pinned official disk image. Native acceptance uses an isolated package installed from the real Apple Silicon disk image in the free `macos-build.yml` run. Run focused Windows discovery/plan and frontend contract checks.

Acceptance completed on November 12. The test pinned the official 26.1106.51428 disk image by SHA-256 and extracted it to an isolated directory. The Security framework verified the OpenAI team, bundle identity, launcher and Info.plist signature. The test covered the user Applications directory and a system Applications directory writable by the admin group. A new launcher with the same bytes invalidated the old choice. Changed version metadata was rejected in the old select/save request and in a fresh scan. The scan rejected unsigned copies, symlinks and writable metadata candidates.

The first private-directory run passed 17 tests. The extra system-directory test failed at the current permission check. After adding a local file-system check through the Contents directory, the final macOS 15.8.2 ARM64 run passed 17 tests, the frontend and the locked build. Two child-process helpers ran in their relevant tests. Windows passed 61 Rust tests, 29 frontend tests and typecheck. The 282 Windows inputs and 89 frontend inputs matched, except for two later macOS file changes. [Machine evidence](../proofs/macos-desktop-discovery.json) · [Final run](https://github.com/Protocol272/headroom-desktop/actions/runs/43196478298).

Signature checks are local. Online certificate revocation, all package sources, Intel architecture and real Desktop traffic need separate verification. The macOS Desktop connection plan remains closed at this stage.

<a name="h048-c4--yerel-dosya-ve-klasör-seçici"></a>

## H048-c4 — Local file and folder picker

Use the existing IPC, ownership and choice store for client files and Claude project folders. Open the macOS AppKit panel on the main thread. The file picker accepts extensionless CLI commands and uses the existing file-identity check. The folder picker saves a safe local project. This slice adds no report export or new connection method.

| Start state and action | Expected observation |
| --- | --- |
| Add from file → select a CLI in the real panel → scan and reopen | Verify the correct client file. Map the choice to a fresh identity. Do not run the client from the panel. |
| Claude project scope → select a folder in the real panel → reopen | Send the folder through the existing PlanManager and ChoiceStore flow. Do not create a config file during discovery or selection. |
| Cancel the panel, select the wrong file or change the file | Cancel preserves the previous choice. A wrong file returns a fixed error. Do not map a changed file to the old choice. |

The lead agent implements the main-thread flow, path validation and real manager path. Luna inspects only the pinned AppKit APIs. Free macOS Actions validation uses a real panel, controlled local files and a separate process with a fixed lifetime. It records input/tool/environment fingerprints. Run focused Windows tests for discovery and Claude scope.

The final macOS 15.8.2 ARM64 run on November 12 passed four real-panel scenarios. It verified file and folder cancellation, selecting an official extensionless Codex binary and selecting a Unicode project folder on the main thread. LaunchServices starts the test app. An external System Events driver checks that its test window is frontmost, then checks the path field and active Open button. The test matches the returned panel paths to the expected file and folder. Before/after SHA-256 for the selected binary matched the pinned official file.

The 16 focused tests and an extra manager test using real-panel results passed. The extra test saves a CLI choice returned by the panel through DiscoveryManager and verifies it in a new manager's fresh scan. It passes a project folder returned by the same panel to Claude scope selection and reads it with a new PlanManager. Choice does not create a project config file. Focused tests cover invalid files, cancellation and changed clients. The Windows evidence for 46 tests and 281 related inputs was preserved. The frontend and locked ARM64 build passed. [Machine evidence](../proofs/macos-file-picker.json) · [Successful run](https://github.com/Protocol272/headroom-desktop/actions/runs/43217388795).

This acceptance covers the AppKit test package that uses the production picker and the real manager flow. Interactions with the packaged Tauri main window belong to later H048 acceptance.

<a name="h048-c5--paketlenmiş-pencere-ve-motor-yaşam-döngüsü"></a>

## H048-c5 — Packaged window and engine lifecycle

The goal is to verify the real window and engine flow with the pinned resources in the Tauri `.app` package. The lead agent owns packaging, local state and process ownership. The single Luna xhigh worker prepares a read-only helper and focused tests that compare the packaged runtime inventory with its source. The test uses existing payload preparation and atomic staging. Native GUI control uses the external System Events method from the earlier picker acceptance.

| Start state and action | Expected observation |
| --- | --- |
| Open the `.app` package with new project-owned QA state | The embedded interface opens. Engine resources are in the package. The runtime manifest, file contents, modes and symlinks match the pinned source. |
| Click Start → ready → Stop → Start again in the window | Verify the real engine and owned local endpoint. The first process closes. The new start uses a new process. UI state matches observed processes. |
| Confirm real exit and reopen the package | The app and its engine processes close. State lock and endpoint do not block the next open. Do not change another process or client setting. |

Build with existing locks in the standard free Apple Silicon Actions run. The QA build uses Tauri configuration that places app data in a private project directory. This run builds the release profile with LTO off and 16 codegen units. It is not performance or release acceptance for the final production profile. Add and verify engine resources before signing. Use a local ad-hoc signature for the app. This slice does not cover a production release, Apple notarization, Intel or live provider traffic. Capture source, package and environment fingerprints programmatically. Bind process checks to the identity of the launched package and its verified children.

12 November 2026. Candidate `c036f72` passed with the real `.app` package on macOS 15.8.2 ARM64. The test used the normal interface and Tauri commands in two separate LaunchServices launches. Start, ready state, Stop, Start again and confirmed exit matched three different direct-child engine processes. The same registered loopback port remained after reopening. Engine and ingress locks stayed held while running and released at normal shutdown. The test could connect to the port again. It did not force-terminate a process at exit acceptance.

The package runtime inventory of 16,640 files and 597,415,256 bytes matched the manifest, overlays, modes and symlinks to the pinned source before and after. The app binary hash did not change. Strict ad-hoc signature checks passed. Native Python checks passed 28 and skipped 1. Preparation treats skipped native symlink checks as an error. Windows passed 8 tests and skipped 3 POSIX/symlink checks. The 292 local source/workflow inputs and native before/after fingerprints matched.

The first attempts found a test umask assumption, duplicate file descriptors for the same listener and an AX object that disappeared during rendering. The lead agent tied the fixture to observed modes. Port checks accept only the expected address set and reject other addresses. The driver takes a fresh snapshot only when a read object disappears. It does not retry a click error. The single Luna xhigh worker prepared the inventory helper and regressions, then stopped. The lead agent handled packaging, process/config ownership, native driver and acceptance. [Machine evidence](../proofs/macos-package-lifecycle.json) · [Successful run](https://github.com/Protocol272/headroom-desktop/actions/runs/43227012806) · [Review](../reviews/H048.md#paketlenmiş-pencere-ve-motor-yaşam-döngüsü).

<a name="h048-c6--macos-rapor-dışa-aktarma"></a>

## H048-c6 — macOS report export

The goal is to bring the existing Overview → Export → ReportManager → AppKit save panel → safe new-file path to macOS. The lead agent owns file and panel ownership and atomic publication decisions. Preserve the existing report field allowlist, snapshot ID, size limit, Turkish/English CSV and exit barrier. Add no measurement, provider call, auth or UI design. Do not enable the Linux writer in this slice.

| Scenario | Acceptance |
| --- | --- |
| Save statistics JSON, CSV and diagnostics JSON from the real window | The save panel appears. The real writer saves the complete selected file with a new Unicode name. Preserve JSON/CSV format and existing content-exclusion contract. |
| Cancel the save panel | The UI reports cancellation. Create no new file or staging artifact. Allow the next export to work. |
| Target already exists or another writer publishes to the same target | Do not overwrite. Keep one complete file and preserve the competing writer's bytes. Clean up failed staging. |
| Wrong extension, empty/oversized report, symlink/alias parent or untrusted file system | Reject without writing. Use local APFS and existing directory-identity checks. |

First run the macOS writer tests as RED under current `unavailable` behavior. The final candidate uses `macos-package.yml` with `verify_report_export`. Free Actions checks the focused Rust report tests and the real package and panel flow. On local Windows, run focused tests for the changed shared picker route and report behavior. The full product gate belongs to the final H048 candidate. Record source, tool, environment and result fingerprints before and after. The QA ad-hoc package and live-traffic limits are the same as H048-c5.

12 November 2026. Final candidate `cf4f4bc` passed on macOS 15.8.2 ARM64. The test canceled the NSSavePanel opened by the real Overview menu. It then saved statistics JSON, Turkish CSV and diagnostics JSON. The test read all three files with Unicode names. It verified format, measurement scope, 0600 modes and no staging artifact. These files contain local metrics observed by the isolated app. They are not live provider traffic or a savings benchmark.

The 11 native Rust report tests passed. They covered all formats through the real ReportManager and preservation of an existing file. Six concurrent writers to one target produced one complete result. The tests covered parent and target symlinks, unsafe directories, and path, extension, and size limits. They also checked changed staging identity and preservation of foreign changes. Five existing report tests covered negative/zero/unknown metrics, content exclusion, snapshot, cancellation and exit barrier. Native Python checks passed 28 and skipped 1. Windows passed 8 focused tests. Windows inputs and the picker branch matched except for later macOS-only/driver changes. The same successful tests were not repeated.

After the report panels, the test opened the real package twice. It verified three different engine processes through Start/Stop/Start again and normal exit. The runs released the same registered port and locks. The package runtime inventory and binary hash did not change. 294 local source/workflow inputs and native before/after fingerprints matched. Each native run started after a check of the free quota and zero-dollar spending stop.

The first four tests returned RED. During the first build, the independent picker example's report-DTO dependency and URL conversion type were fixed. The next run passed report tests. AX read indexes changed after Start. The external driver takes a new snapshot only after stale-object/element reads return -1728/-1719. It does not retry click errors. [Machine evidence](../proofs/macos-report-export.json) · [Successful run](https://github.com/Protocol272/headroom-desktop/actions/runs/43261107448) · [Review](../reviews/H048.md#macos-rapor-dışa-aktarma).

<a name="h048-c7--gerçek-macos-desktop-bağlantısı"></a>

## H048-c7 — Actual macOS Desktop connection

The lead agent owns auth, route, and process ownership decisions. First observe actual macOS startup of the pinned official Desktop package with separate Electron and Codex test state. Check its bundled app-server config contract. The single Luna xhigh worker reviews the existing DMG package and startup inventory in read-only mode. macOS Desktop needs its own evidence for the shared backend and CLI. Keep discovery and the existing connection block until the live client path has evidence.

| Step | Acceptance and scope |
| --- | --- |
| Official package, full signature, and isolated app-server config read | Check the pinned package and binary. The actual bundled backend must read one endpoint setting in the new private CODEX_HOME. This check uses no model calls, real credentials, or changes to user config. |
| Open the actual GUI with separate app state | Match the running executable and its children to the package. Observe visible startup and sign-in state. Observe normal exit. Accept isolation only with valid flags, pinned source, and native observation. |
| Actual Desktop → Headroom → controlled local provider | Observe a native client request and response. Check the model, effort, auth type, and route. This scope covers a local provider fixture. Live provider or existing account acceptance needs its own evidence. |
| Authorized real account and provider, with reopen | Check the live response. Check adoption of the saved endpoint after reopening. Keep account data and secrets out of CI. Leave this criterion open without the required environment. Fixture and app-server results have their own scope. |

The first native run checks startup and config capability only. The Rust and full product quality gates stay outside this run. Use standard free Actions. Check the current quota. Check the zero-dollar spending block.

Record source, command, environment, and package fingerprints before and after. Limit diagnostics to fixed codes and counts. Preserve other user profiles, model and effort, and auth settings. The scope excludes new auth flows, bypassing security checks, and skipping sign-in with a fake account.

The precheck uses only the short `/private/tmp/hr-c7-<rastgele>` state root that CI creates. This explicit QA state exception accounts for the macOS Unix socket length limit. Codex, Electron, home, and temp subdirectories are private. The environment inherits no real API or GitHub account secrets.

The regular config and app profiles must be absent before startup and after exit. Package and development files stay under project `work`. Wait for the observed owned processes to exit. Check the QA state's UID, inode, and mode before cleaning it up. This observation starts no model turn.

The first native observation failed full state isolation. Normal Codex and ChatGPT profiles and `.codex` remained absent. The private Electron profile contained Local State and Default. One unclassified entry appeared under `~/Library/Application Support/com.openai.codex`.

The purpose of that vendor area is unknown. The narrower follow-up allows this explicit, initially absent vendor root only for a clean CI user. Check the UID, inode, and canonical directory. Clean up after normal exit.

The core profiles must stay intact, and the app must use the private Electron profile. This precheck leaves full app state isolation, GUI config adoption, and traffic open. The record retains failure of the first strict check.

12 November 2026. Candidate `2a2e0b4` passed the narrow precheck on macOS 15.8.2 ARM64. The pinned official Desktop 26.1106.51428 passed strict and deep signature checks. Bundled CLI 0.167.0-alpha.4.1 ran a separate stdio app-server session. It read the explicit built-in `openai` provider and loopback endpoint. The observation showed no account and a normal exit.

The first provider guard expected an omitted field to return explicit `openai`. The raw read failed that equality check, so the test profile supplies an explicit provider. The exact read criterion still applies.

The actual Electron window opened with one observed bundled backend child. The limited AX observation found 1 window and 27 controls. It found no recognized sign-in or API-key button. GUI auth capability remains unknown.

CmdQ closed the app and all 13 observed owned children. Local State and Default appeared in the private Electron profile. The normal core profiles remained absent.

The test linked the new vendor state to its UID, inode, mode, and canonical path. After normal exit, cleanup removed that namespace and the short QA state. The selected four package inputs and before and after native fingerprints matched.

The 3 local config, environment, and metadata privacy tests passed. Native preparation passed 31 of 32 Python tests with 1 skipped. The lead agent owned implementation, auth, process, and scope decisions. The same Luna xhigh worker finished two read-only Asar reviews and stopped. Static markers alone leave regular profile resolution partly unverified.

The precheck left product source and the existing macOS Desktop connection block intact. It observed neither a model turn reaching Headroom nor a loopback model request. Desktop route, provider, and savings acceptance remain open. Next, verify that a real GUI request reaches the controlled local provider through Headroom. [Machine evidence](../proofs/macos-desktop-preflight.json) · [Successful run](https://github.com/Protocol272/headroom-desktop/actions/runs/43271658774).

The [OpenAI advanced config documentation](https://learn.chatgpt.com/docs/config-file/config-advanced) describes CODEX_HOME and the built-in provider endpoint. The [app-server protocol](https://learn.chatgpt.com/docs/app-server) describes initialization, stdio, and config reads. Real GUI traffic still needs evidence from the pinned version.

The first native H048-c7b observation passed. On its owned loopback target, the actual renderer showed `complete`, 5 root children, 4 visible buttons, and the “Sign in another way” control. Normal exit, private state cleanup, and matching inputs before and after passed. The run made no model calls. Traffic acceptance remains open at this checkpoint. [Limited evidence](../proofs/macos-desktop-renderer.json) · [Run](https://github.com/Protocol272/headroom-desktop/actions/runs/43278256460).

The external driver accepts only the private Electron profile's DevToolsActivePort record and the same owned app PID's 127.0.0.1 listener. It rejects other ports or hosts, URLs with credentials, different pages, and multiple app pages. The reader returns only fixed UI labels, visible control counts, and readiness. It records no unknown text, accounts, or content.

Verify readiness and the auth path before claiming request submission. Full route acceptance needs a run-specific match from actual GUI input to the local provider. Check the preserved model, effort, and synthetic auth. Observe a visible reply before the terminal frame. Check clean exit.

The test profile's synthetic API key only serves the local fixture. The run signs in to no real account or paid service. [OpenAI auth documentation](https://learn.chatgpt.com/docs/auth) describes the Desktop API-key route. The pinned version needs its own behavior check.

The first full GUI run displayed a partial and final reply through the actual packaged engine. The exact-byte check for the test config failed. A separate direct-fixture diagnostic found only a new `[desktop]` table. The endpoint, model, effort, provider, and credential storage file matched. That diagnostic has local fixture scope. Headroom routing needs separate evidence.

The new controlled candidate allows only addition of the previously absent Desktop table. Every other TOML field must match exactly. Reject a change to an existing Desktop table. The no-account precheck still requires exact config bytes. The new controlled run records separate before and after config hashes. It makes no exact-byte claim for the whole file.

The record [retains the first failure](../proofs/macos-desktop-traffic-attempt.json). Diagnostics keep only field names and match flags. They exclude code, keys, and content.

<a name="h048-c7b-kontrollü-gui-akışı-geçti"></a>

### H048-c7b controlled GUI flow passed

12 November 2026. On macOS 15.8.2 ARM64, official Desktop 26.1106.51428 used bundled CLI 0.167.0-alpha.4.1. Normal renderer input sent one matching user request through packaged Headroom 0.41.0 to a local WebSocket provider. The test model `gpt-5.2-codex`, `medium` effort, and synthetic API auth reached it intact. The test verified the nonce in input. It observed separate partial and final markers in the actual renderer that the prompt did not contain.

The fixture held the terminal frame until the partial response appeared. It rejected the other four requests. The run did not identify them as user turns, helper work, or subagents. Claims of successful replies for multiple models or real subagents need their own evidence.

The real Headroom collector recorded 2 same-model matches in content-free SQLite events. It recorded 5 events in total. The counts describe event matching. Savings and benchmarks use separate measurements.

The endpoint, provider, model, effort, file auth store, and all TOML fields outside Desktop stayed intact. The new `[desktop]` table changed the config byte hash, so the record keeps both hashes. Normal `.codex` and Codex and ChatGPT core profiles remained absent. Cleanup removed the private profile and verified owned vendor state. Full app state isolation remains `not_established`.

Codex used normal CmdQ. Headroom used its own exit confirmation. The observed 11 Codex children and the actual Headroom engine exited. The Headroom port and both ownership locks became free.

The vendor package and Headroom bundle and runtime inventories matched before and after. CI source, tool, and environment fingerprints matched. The native suite passed 44 of 45 Python tests and skipped 1. Windows evidence reused 15 relevant PASS results on matching inputs and tools, with 1 POSIX test skipped.

After final review and the collector diagnostic addition, only affected checks ran again. The QA profile uses ad-hoc signing, LTO off, and 16 codegen units. Production signing and performance need separate evidence. [Machine evidence](../proofs/macos-desktop-traffic.json) · [Successful run](https://github.com/Protocol272/headroom-desktop/actions/runs/43284385567).

<a name="h048-c7-ürün-bağlantısı--geçti"></a>

### H048-c7 product connection passed

Use c7b's pinned-version evidence to enable the macOS config contract for official signed Desktop `26.1106.51428` only. Reuse read-only discovery and the fresh selection check. Use the existing endpoint patch and ownership record. Version acceptance does not depend on model, effort, or agent count. Preserve model, provider, reasoning, the auth store, and unrelated TOML fields. Session and account checks require a user action. Saved config support and actual account or request acceptance require their own evidence.

Normal acceptance starts with real vendor bundle discovery and a private test config. Check Preview → Apply → OwnershipStore and PlanManager reopen → Undo. Undo keeps the Desktop preference added after Apply and removes the endpoint. The synthetic auth file stays byte-exact.

For error acceptance, change the file after preview and check rejection of Apply. Check that endpoint drift blocks Undo while preserving the file and ownership. Verify that unknown Desktop versions stay blocked on macOS. The tests launch neither the vendor app nor the real user client. They write no data to either app. The earlier c7b GUI traffic evidence applies to its own fingerprints.

The lead agent owns version, route, and implementation decisions. The same Luna worker writes only existing `config_io/macos_tests.rs` helper tests. Use local `plan_tests`, related catalog and recovery checks, and fmt. On macOS, use the `macos-build.yml` Desktop option and new `native_desktop_manager_` tests with the platform version contract. Run the full local main-task gate once in c8. Before starting a standard GitHub macOS run, check current included quota. Confirm the Actions `$0` hard stop for the account.

The first native candidate passed 13 tests. The catalog snapshot was the only failure. The catalog compared the macOS build-bound payload pin with the Windows manifest. The initial short build also lacked the prepared Mac pin. The evidence [retains the failed run](../proofs/macos-desktop-connection-attempt.json).

The fix binds Mac catalog availability to the runtime's build pin. Missing or invalid pins still return `unavailable`. The `MacRuntime` checks still verify real file, manifest, and version integrity before engine startup. Windows keeps its manifest and hash comparison.

Prepare the real locked Mac payload for positive native catalog acceptance. Engine lifecycle and new model calls need explicit scope.

On 13 November, run a53c08d stopped at a changed `Codex.dmg` size pin before tests. The record [retains the failure](../proofs/macos-desktop-connection-download-attempt.json). The official appcast URL from the verified vendor package listed the same version's `ChatGPT-darwin-arm64-26.1106.51428.zip`.

Local validation passed the Sparkle Ed25519 signature with the key from the prior signed Info.plist. Info.plist, launcher, and bundled backend hashes and sizes matched the prior DMG. The new helper checks a pinned URL, exact size, and SHA-256 before extracting with native ditto. Version, team, and architecture checks still apply.

Three workflows now use the ZIP fingerprint. The summary records lifecycle results only when the run requests that check. The team confirmed included quota again before the new native run.

The separate Desktop job has a 3-minute limit, with a conservative estimate of 4 minutes and USD 0.248 including cleanup. Evidence is in `work/proofs/macos/h048-c7-product/fixed-archive-local-acceptance.json`. The team reused 31 passing local tests at that step because all 197 product inputs matched. Native acceptance required a separate run.

On macOS 15.8.2 ARM64, final candidate 20729e7 passed 15 Rust tests. The suite passed 44 of 45 Python checks and skipped 1. Frontend typecheck and build and the locked Tauri build passed. Source, tool, environment, and payload fingerprints matched before and after. The ZIP and three critical bundle inputs matched local provenance.

Acceptance covered signed discovery, Preview, Apply, reopen, and Undo. It also covered TOML, auth, and preference preservation, rejection of stale Apply and drifting endpoint, and read-only catalog with a real compiled pin. This run did not request engine lifecycle or a model turn. The actual c7b GUI and Headroom evidence keeps its own scope. [Product acceptance](../proofs/macos-desktop-connection.json) · [Native run](https://github.com/Protocol272/headroom-desktop/actions/runs/43391633178). H048-c7 finished. The c8 gate for the main H048 task remained open.

<a name="h048-c8-son-kabul--geçti"></a>

### H048-c8 final acceptance — passed

The full local gate ran once on final candidate 20729e7. It passed 211 frontend tests, build/typecheck, fmt, 420 Rust tests and the offline locked custom-protocol build. The gate ignored 15 opt-in Rust tests. On Windows 10 build 19045, 338 source/config/test/tool inputs and tool binary/versions matched before and after. This local gate does not replace macOS support evidence. The final native run also passed on macOS 15.8.2 ARM64.

The product diff after the final real native package/report candidate was programmatically limited to five files: two production modules (`codex_binary`, `catalog`) and three test files. The final 15-test native run and full local gate covered these changes. UI, runtime, packaging, config writer and dependency product sources did not change. Earlier real window/engine/APFS/picker/report and GUI traffic evidence remains valid within its own scope. The CI helper's switch to the same-version ZIP was separately verified by signature/hash/size and native discovery. Documentation closure alone does not require a new product test. [Combined acceptance](../proofs/macos-acceptance.json).

**Apple Silicon development acceptance for H048 is complete, 40/54.** The final Actions UI observation showed billable `$0` and included compute `$11.90/$12`. It created no paid resource or new release. H048 stopped at this point. H049 did not start.

<a name="doğrulanmayan-kapsam"></a>

## Unverified scope

- Do not infer live macOS provider/account support or other model/real subagent identity from this synthetic flow. The product is model-independent. This does not mean that all models received live tests. The Desktop config contract covers only signed version `26.1106.51428`.
- An ad-hoc QA signature is not a production app/engine signature, notarization or release acceptance. Do not transfer the Apple Silicon result to Intel, other macOS versions, Windows 11 or Linux. These runs do not cover a production release or paid signing.
