<a name="h038--kaldırma-ve-veri-tercihleri"></a>

# H038 — Uninstall and data choices

9 November 2026. H038 completed local acceptance, 36/51. The user approved the task while H037 external acceptance was open. [Plan 026](../decisions/026-windows-uninstall.md) · [evidence](../proofs/uninstall.json).

Before uninstalling the app, check for open Headroom instances owned by the same user. Then undo client routes by using the current ownership records. Keep the installation if a connection cannot be undone. Preserve user edits to other fields under the current transaction contract.

By default, the uninstaller keeps local history and preferences. If the user selects the delete checkbox, it deletes the app's LocalAppData root. Locked data can cause an error, and deleted data cannot be recovered during that attempt. Exported files and client data folders stay in their locations.

Four focused Rust tests passed on Windows 10 build 19045. Source and tool inputs matched before and after. Open app instances and the uninstaller excluded each other. Foreign data in the existing lock file stayed intact.

The test verified a delete error and retry on locked local data. Undo preserved user edits in all three clients. A retry completed undo for the locked client. Focused test record (local receipt `work/proofs/windows-uninstall/focus-655bfb8a4e9e46a78d2c62c13ac24825/result.json`).

The installed candidate has app ID `dev.headroom.desktop.h038-qa` and name Headroom H038 QA. Client fixtures use a separate run directory under `%LOCALAPPDATA%\HeadroomDesktop-QA-H038`. Normal user settings are not fixtures. Build record (local receipt `work/proofs/windows-uninstall/qa-build.json`).

The test used a mouse in the installed Windows 10 app. The test started the engine and connected the Codex fixture. In-app Disconnect released the ownership record. The test then reconnected the fixture. The app exited normally with code 0.

The real uninstaller stopped with code 2 for an open app, a locked settings file, and a user endpoint conflict. Uninstall with history retention completed with code 0. It preserved user edits to model, reasoning, and comments in the fixture. It also preserved history and preference files. Run records (local receipt `work/h038-test/7b843249/keep.json`).

The six-command final local gate passed. Python supervisor/package tests, 173 UI tests, TypeScript/Vite, Rust fmt, 392 Rust tests and the custom-protocol build passed. The gate left nine opt-in Rust tests ignored. Source and tool fingerprints matched before and after. Local gate (local receipt `work/proofs/windows-uninstall/local-quality.json`).

At the second installation, the engine and Codex connection restarted from the UI. The app then closed normally. The real uninstaller shows an empty delete checkbox by default. The test selected delete and started uninstall with a lock on the history file.

The client route was undone, deletion failed and the installation remained. After releasing the lock, we selected delete again in the same uninstaller. The UI showed success and exit code was 0.

The uninstaller removed the QA installation and data root. Before/after hashes for all three external client configs matched. Locked-file check (local receipt `work/h038-test/7b843249/delete-locked-postcheck.json`), delete check (local receipt `work/h038-test/7b843249/delete-postcheck.json`).

The acceptance script bound 40 checks to the same candidate ID. It matched 212 product/tool inputs, build and test records, two normal exits and installer identity. The test closed diagnostic ports and test windows. It did not rerun passing tests for full-candidate acceptance. Acceptance record (local receipt `work/proofs/windows-uninstall/acceptance.json`).

This record applies to a separate QA app ID on Windows 10 build 19045. Rust fixtures tested config/rollback behavior for all three adapters. The installed app connection path used the Codex fixture. Clean Windows, Windows 11 and production identity installation remain in H037 external acceptance. The `/UPDATE` path remains in H042. This uninstall does not claim secure forensic erasure. The uninstaller keeps the zero-byte sibling `.desktop-use` file for concurrency.
