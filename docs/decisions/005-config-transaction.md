<a name="005--sahiplikli-config-işlemi-ve-geri-alma"></a>

# 005 — Owned config transactions and undo

22 October 2026. **Accepted — the H018 local product integration/native file flow is complete.** The KTM/TxF backend was tested on the same local NTFS volume on Windows 10 build 19045. This decision does not verify support for Windows 11, live clients/auth/model/subagents, or release. [Final acceptance](../proofs/config-transaction.md).

<a name="kapsam-ve-güven-sınırı"></a>

## Scope and trust boundary

The H017 user-config plan applies Codex `openai_base_url`, Claude `env.ANTHROPIC_BASE_URL` and OpenCode V1 `provider.openai.options.baseURL`. Model, reasoning, provider, auth, hook/shell, and agent count remain unchanged. Active chat databases and settings for other projects/sessions are not read or written. Unknown versions, custom gateways, ambiguous routes, and blocked plans cannot be applied. Version/catalog limits are not loosened just to enable a UI test.

The first supported change is adding a previously missing endpoint field. If the user already set the same Headroom endpoint, no new ownership is created. Overwrites that would require retaining a previous custom URL or secret remain explicitly blocked.

The renderer does not provide an arbitrary path, patch, URL, port, or config content. Apply uses only the time-limited plan ID/expected fingerprint and correlation metadata stored by the backend. Before commit, plan expiry, installation/scan, current endpoint identity/address, config source/slot, and file identity/fingerprint are checked again. Multiple installations may share the same user config. Selecting another installation does not authorize taking ownership of the file again.

<a name="veri-koruma-ve-geri-alma"></a>

## Data preservation and undo

The patch adds only the required field. Comments and unrelated bytes are preserved. Unparseable or unsafe-to-patch structures produce an error. Undo removes only a value that still belongs to Headroom in the current document. If the user changed the managed field, it returns conflict. Edits to other settings are preserved. A container or config file with content added later is not deleted wholesale. A created empty container/file may be removed only when its ownership and current state are proven safe.

The ownership/recovery record contains fixed client/source/field enums, UUID, loopback endpoint, and required hashes/file identities. Raw config, auth, previous custom URL, hook command, or a full file backup is not placed in app logs/state. H017's plan lease, held only in redacted memory, is not write authorization after restart. Ownership of a successfully changed field must, however, be discoverable after the application restarts.

<a name="yayın-primitivei-için-kabul-kapısı"></a>

## Acceptance gate for the publication primitive

The existing preferences store is for the application's own state. Its hash check → close handle → MoveFileEx sequence is not evidence of compare-and-swap atomic publication for a client config that can also be changed by an external editor. That path is not copied directly. Target sharing, the unsupported WRITE_THROUGH flag, and partial-failure cases for [ReplaceFileW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew) require separate evaluation. The ability to publish through an open handle with [FileRenameInfoEx](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info) does not by itself mean other renames are blocked.

The first experiment uses only synthetic files under a new UUID. Normal writer, replace, rename-away, and delete behavior must be measured separately while the target/parent handle is open. A successful API return is not treated as operation success until verified against file identity and final bytes. A strict CAS or power-loss guarantee is not inferred from an API example that has not been measured. A production command that writes user files is not enabled until the primitive and interruption/recovery model are accepted.

<a name="seçilen-dar-backend-ntfs-transaction"></a>

## Selected NTFS transaction backend

In the classic API experiment, strongly pinning the target/parent also blocked our publication. In a variant that relaxed sharing to allow publication, an external ReplaceFile also succeeded. This experiment did not establish strict compare-and-swap publication. The local KTM/TxF experiment could update config and a secret-free ownership file in the same transaction. After transacted file handles closed, separate-process normal write/replace/delete attempts were rejected before commit. A process death before commit left both files old. A process death after commit left both new. No administrator privileges were used. [Measurements](../proofs/config-transaction.md).

Therefore, H018 selected an isolated NTFS transaction backend with capability checks. Local fixed NTFS and transaction support are required. If the API/volume is unsuitable, writes fail with a fixed error. Network/ReFS/FAT, EFS files, links/reparse points, and other unverified paths do not fall back to a weaker replace. Support is not forced by changing user ACLs/attributes or system settings. The config fingerprint is checked against the locked current view inside the transaction. The config patch and ownership record are part of the same commit. The application does not need to keep a full config backup or a sibling staging file containing auth. This does not claim that Windows itself never creates filesystem undo/journal data.

Microsoft [recommends alternatives to new TxF dependencies and notes that TxF may be removed in a future Windows release](https://learn.microsoft.com/en-us/windows/win32/fileio/transactional-ntfs-portal). The project accepted that compatibility risk because the tested alternatives could not preserve concurrent file changes. TxF is not used throughout the product. It is limited to this write backend. Unsupported environments may allow preview only. Any future backend replacement must meet the same acceptance matrix. Windows 11, other file systems, and physical power loss have not been tested here.

Under the [TxF programming contract](https://learn.microsoft.com/en-us/windows/win32/fileio/programming-considerations-for-transacted-fileio-), file handles are closed before commit. The transaction's intent/ancestor protection remains effective until commit/rollback completes. Synchronous commit, bounded transaction duration, RAII rollback, and fixed error mapping are required. A native Python experiment is not product success until the Rust production primitive and real manager flow are separately tested against the same conditions.

<a name="kabul-sırası"></a>

## Acceptance sequence

<a name="macos-apfs-yazım-alt-kabulü--h048-b6b"></a>

### macOS APFS write sub-acceptance — H048-b6b

The Windows TxF contract is not generalized to macOS. The APFS candidate publishes config and ownership under separate atomic names. An interruption between the two steps is recognized through bounded intent and inode/birth/metadata/content summaries in the private app directory. Restart does not revive an old plan to write config. An explicit user action may complete only an ownership step that matches the published config. An intent that has not yet been published is marked for cancellation. Read-only discovery/snapshot does not write during recovery.

To bind recovery to the correct target, lossless absolute config path bytes are stored as minimal metadata in private state. A stored path alone does not authorize opening a file. The active backend target, path binding, and parent identity must match. No other project target is opened. Raw config, auth, and xattr values are not written to this state. The staging copy of an existing file is kept in its own client directory. ACL/permission/xattr metadata is copied from the open source descriptor using `fcopyfile(COPYFILE_METADATA)`. [Apple copyfile guide](https://github.com/apple-oss-distributions/copyfile/blob/main/copyfile.3).

The old inode moved by swap is not deleted automatically. A late write through an open descriptor is preserved and blocks a new action on the same target with conflict. The 64-intent limit is not handled by deleting data. This is not a strict multi-file CAS or physical power-loss guarantee. The initial write scope is an existing config file. Production write code was verified on macOS 15.8.2 ARM64 with the real PlanManager and private fixture files. This is not acceptance for an installed client or native window. [Initial write candidate and limits](../proofs/macos-config-apply.json).

For a missing Claude project directory, the directory and new settings.local.json are prepared in a private 0700 staging directory. The visible `.claude` is published only at commit using fd-relative RENAME_EXCL. A competing target is preserved. The operation record binds the selected project and published directory identities. An interruption before publication aborts the old plan. An interruption afterward may complete only matching ownership through an explicit user action. Undo preserves the new directory and user additions. Older file intents remain readable without the optional project-identity field. Native RED/GREEN and manager reopen were verified. [Directory publication evidence](../proofs/macos-claude-project-directory.json).

Retained-version review returns only counts and the current review fingerprint for the selected client/target. The explicit “Continue with current settings” confirmation revalidates that view. Config bytes and displaced inodes are preserved. Pending ownership is completed only when the typed receipt matches the current document and the recorded transition. App-owned metadata is copied to and verified in a private 0700 group. Root records are not moved before the manifest is published exclusively. After publication, metadata may be moved with an fd-relative exclusive rename. Completed entries in the manifest index leave the active 64-record limit. At most 64 archive groups are retained. Partially prepared groups count toward this protective limit. There is no automatic cleanup, and old config files remain in the client directory. On reopen, the manifest's structure/identity is verified. All archive copies are not rehashed during every ordinary settings operation. The three adapter managers, process interruption, stale data, and both capacity limits were verified with native fixtures. Confirmation/scope/keyboard behavior was verified in an isolated panel. [Archive acceptance and limits](../proofs/macos-retained-versions.json).

H018-a covers the real Windows primitive/publication experiment. H018-b covers byte-preserving patch/undo and a secret-free ownership record. H018-c covers manager/typed command/UI integration. H018-d covers review, the final quality gate, and the real native fixture flow. A sub-slice PASS does not complete the main task. [Task list](../../TASKS.md), [checkpoint](../IMPLEMENTATION_STATUS.md).
