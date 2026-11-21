<a name="h018-işlem-bağı-tasarımı"></a>

# H018 configuration action design

22 October 2026. **The implementation completed C1–C3. The lead review, static UI checks, and native acceptance in stage D verified the binding.** The actual command/DTO summary is in [the contract](contracts.md). The final source fingerprints are in the [local quality-gate receipt](../work/proofs/config-transaction/final-quality.json). [Decision 005](decisions/005-config-transaction.md), [acceptance matrix](validation/config-transaction-matrix.md).

<a name="yetki-ve-güncellik"></a>

## Authority and freshness

An Apply request carries only the schema version, request UUID, plan UUID, and expected config fingerprint. The renderer does not send a file path, arbitrary URL, model, config body, or full plan. The backend owns a store with space for 64 plans. Each lease lasts 300 seconds and permits one use. Preview, Apply, and Undo share one operation guard. A Preview capacity error reports a preview failure only. A blocked, stale, replayed, or expired plan prevents writes.

Apply first verifies the current selected installation and scan, plus the stable endpoint identity and port. The backend derives the plan target from environment roots. It creates the H017 fingerprint from the held TxF snapshot. The fingerprint includes the source, presence, content, Windows file ID, and canonical target. Apply repeats the identity check from the earlier preview. The same transaction reserves the absence of the alternative JSON/JSONC slot. The config's parent directory must already exist. Only the app-owned ownership directory may be created on demand.

The H017 present fingerprint includes the canonical Windows target path, including the extended prefix. The missing fingerprint includes the original lexical target path. Stage C keeps the same fingerprint contract. For a missing config, the hash excludes the file ID that TxF would create. Ownership binds the persistent config path independently of its inode.

The store constructor requires an existing safe boundary. It does not assume that the app subdirectory exists at first installation. In debug mode, it can use the existing project root and bounded relative components under `work/app-state/ownership`. In release mode, it can use the existing LocalAppData parent and application identifier/ownership components to provide the target `app_local_data_dir()/ownership`. Setup creates no files. It pins and creates all app-owned relative components on demand. If root setup fails, Preview may continue. A write returns a fixed error. App setup handles this failure without panicking.

The child action module under `config_io` can use the parent's private lease, guard, and root helpers. The design uses no separate runtime, discovery, or plan manager. The renderer needs no broad path API. An ordinary read can encounter a sharing error while the transacted writer remains open. Compare against the transaction snapshot directly.

<a name="sahiplik-ve-geri-alma"></a>

## Ownership and undo

If the in-memory patch makes no change, including when the endpoint already matches, it creates no ownership record. For an existing record, the backend checks the root, source, owner, revision, and fingerprint. It preserves the record if those checks find a conflict. The config and record commit in the same kernel transaction. If the commit result is uncertain, the app returns a fixed uncertain result. It requires a fresh read. Automatic write retry stays disabled.

The backend accepts an Undo request only with the client enum, ownership UUID, expected revision, and expected record fingerprint. The request does not send a path, old config, or URL. The app verifies the current config source/root binding and record. If the endpoint is still the managed value, Undo removes only that field. Undo does not require the previous after-file ID or full config hash to match. A normal editor can save atomically, and the user can edit unrelated settings. If the user changed or removed the managed field, Undo preserves the resulting state. The app returns a conflict or an explicit no-op. Undo leaves the rest of the config file intact.

Undo removes past ownership. It does not require a new plan, an active model engine, a newly allowlisted client version, or an installed executable. The app can read the persistent record when it reopens. If the environment or root changed, the app does not apply the record to another target. The UI must let a user with ownership select Undo even when it finds no installation.

<a name="typed-ipc-ve-graphite"></a>

## Typed IPC and Graphite

The IPC exposes Apply, Undo, and one ownership snapshot per client. The TypeScript side rebuilds each response after validation. It checks schema/request/client correlation, canonical UUIDs, safe integers, fixed enums, the retryable flag, the message key, and the exact field set. It does not expose raw serde/OS errors, paths, auth, config, hooks, or old URLs. An “Applied/owned” record does not prove that the config or traffic is still active.

The UI enables Apply only for a current, supported, unexpired native Preview. During a write, the modal blocks Close and Escape. The UI guards parent navigation so it can show the result. After completion, the UI tells users in plain language that it saved the route. It also says that the client may need to restart and that the live connection is not verified. It briefly explains that the route cannot work while Headroom is not running because its port does not serve requests. It does not automatically undo the whole file.

The UI offers Undo only for a scope with an app ownership record. If the user added the same endpoint, the app does not grant Undo permission. A conflict never forces an overwrite. The UI offers an action to read the current state again. The browser-only Preview stays read-only. The UI supports TR/EN, dark/light themes, a 760×620 minimum size, focus, and Escape return.

The existing connection dialog serves this flow. A native user can open connection management and read the ownership snapshot even when the app finds no installation. Only a new Preview/Apply requires a selected installation. The ownership panel provides the source, local port, status, Undo, and Read again. The modal blocks Close/Escape during a write. A scrolling body and fixed footer remain on narrow screens. Static checks covered TR/EN, dark/light, and 760×620/1280×800. Acceptance covered real native actions.

Final acceptance follows the real UI action → Tauri → same manager → transaction/config/ownership path in disposable native roots. Synthetic npm manifests can open a version-gated code path. Real-client, authentication, model, and subagent compatibility require separate verification in H019–H021.
