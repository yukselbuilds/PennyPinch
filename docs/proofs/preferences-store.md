# H015-a: backend preferences store

**Date:** 20 October 2026. **Scope:** local backend store and Tauri command registration only. The preferences UI has not been connected yet; H015-b remains open.

`get_preferences(request_id)` and `save_preferences(request_id, language, theme, expected_revision, expected_fingerprint)` now use a versioned store. In debug builds its file is `work/app-state/preferences/preferences.json`; release builds resolve `preferences/preferences.json` below Tauri's app-data directory. Missing state returns Turkish/system defaults at revision 0 without creating a file. The stored allowlist is `language = tr|en` and `theme = system|light|dark`. Schema 1 also carries the secretless pinned runtime reference: engine `0.41.0`, manifest SHA-256 `8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908`, cache mode, coding profile, and CCR TTL 7200. That reference is immutable metadata, not evidence that a runtime is installed or ready.

The store rejects unknown fields, malformed/oversized JSON, changed pinned metadata, invalid language/theme values, non-RFC request IDs, and revisions above JavaScript's safe integer maximum (`9007199254740991`). Saving at that maximum returns `config_conflict`; an on-disk revision above it returns `unsupported_schema`. Fixed errors are `validation_failed`, `unsupported_schema`, `environment_blocked`, `busy`, and `config_conflict`; they contain no filesystem path or parser detail. A save compares both revision and a SHA-256 fingerprint of the exact current file, serializes cooperating processes with a path-derived Windows mutex, writes a create-new same-directory temporary file, flushes and syncs it, rechecks the root and source fingerprint, then uses `MoveFileExW` with replace-existing and write-through flags. Failure cleanup removes a temp only when this call created it and it remains a normal file inside the verified state root. A failed pre-replace write leaves the previous valid bytes intact. Corrupt or unknown state is preserved and returned as `unsupported_schema` rather than overwritten.

Root and each existing descendant are checked for canonical containment, symlinks, and Windows reparse points before directory creation or file replacement. Test cleanup first audits the complete new UUID fixture subtree and only removes it when every descendant resolves inside the project and contains no reparse point. No prior proof or diagnostic run directory is used by these tests.

Focused verification passed:

- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- `cargo test --manifest-path src-tauri/Cargo.toml --bin headroom-desktop state::store_tests` — 9 passed
- `cargo check --manifest-path src-tauri/Cargo.toml`

Toolchain/environment fingerprint: Windows 10 Pro 22H2 x64; `rustc 1.92.0 (ded5c06cf 2025-12-08)`; `cargo 1.92.0 (344c4567c 2025-10-21)`; `CARGO_HOME=work/caches/cargo`; `CARGO_TARGET_DIR=work/caches/cargo-target`. No network/provider request was made. The real application preferences file was not written, and no `.preferences-test-*` fixture roots remained after the run.

Source/config fingerprints (SHA-256):

| File | SHA-256 |
| --- | --- |
| `src-tauri/src/state/mod.rs` | `8ce49f9f4989fb044fb1b66140ebd2be82996f2f7eccd4535a206ef9eecf593c` |
| `src-tauri/src/state/store.rs` | `d093b7636e2b72e7d5c167025a635bec0f69b3eaf5e7e1de1c705a704a94976e` |
| `src-tauri/src/state/store_tests.rs` | `c6192790cd7e71a20e42599b6167b74f53805f3663dfe69fbd4ac23697ad0ca8` |
| `src-tauri/src/main.rs` | `eea7ae303cdb229886bb4ab807b004addbcdf2615631db70229e98022a377b29` |
| `src-tauri/Cargo.toml` | `5c90cbff72abf7492037668c8c5a0a5439723fa5a42b7102698a941cb21d41` |
| `src-tauri/Cargo.lock` | `61f5fd42ecdff941fa80f0ade6a16c0611c403106b2f3448a95e68a93ae90319` |
| `runtime/manifest.json` | `e57249e194eb5e5ac8c0396956338c40f35d144b357adb3db826bfb4ce133c57` |

The failed-replace test injects an error immediately before the atomic replacement; it verifies previous-file preservation and temporary-file cleanup. It is not a power-loss or process-kill test. Cross-process serialization is implemented with a named Windows mutex; the local concurrency test uses parallel store calls in one process. Windows 11/clean-machine behavior and frontend save/reopen wiring remain untested in this slice.
