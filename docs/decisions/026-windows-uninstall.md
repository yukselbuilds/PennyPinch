<a name="h038--kaldırma-ve-veri-tercihleri"></a>

# H038 — Uninstall and data choices

9 November 2026. While external acceptance for H037 remained open, the user approved moving to H038. The lead agent connects the existing undo operation to the Windows uninstaller. Because this work involves settings ownership and local-data deletion decisions, the lead agent implements it.

The Windows uninstall screen keeps Headroom history and preferences by default. If the user chooses deletion, clean the current user's Headroom LocalAppData root. Exported files and client-app data are outside this scope. Silent uninstall keeps history. The Tauri path marked `/UPDATE` skips this cleanup. Update acceptance belongs to H042.

The uninstaller detects open Headroom instances in the same profile using Windows file sharing. If one is open, it requests a normal tray exit. While the uninstaller holds its lock, a new instance cannot write in the same profile. The installed app's headless entry point reads the ownership records for the three clients and calls the existing `undo_connection` operation for active ones. Client-owned new fields and comments are preserved. Scope, ownership, config-path, and transaction checks remain in force. Remove the app's startup registration only when its command line exactly matches the installed EXE.

The uninstaller stops before deleting installation files or the selected local data if it encounters a client lock, an ownership conflict, or a failed rollback. Each connection already undone counts as a separate success. A retry processes only the remaining active records. Locked data during deletion produces an error and preserves the installation. App data removed before the error stays deleted.

Acceptance uses Windows 10 with a separate app identity, installation directory, and synthetic client config. Verify connect → disconnect → reconnect in the installed app. Test uninstall rejection while the app is open, byte equality under locks or conflicts, and the NSIS uninstaller's keep and delete options separately. Test a failed commit with a focused Rust test. Store before/after input and environment fingerprints under `work/proofs/windows-uninstall/`. Run the final candidate's local gate with the `WORKFLOW.md` commands. Windows 11 and clean-Windows acceptance await their own environments.

Use the Tauri CLI 2.12.3 NSIS template from the pinned commit. Adapt uninstall confirmation, protected preparation, and deletion steps for Headroom. Include the source and MIT text in the packaged NOTICE. [Tauri template](https://github.com/tauri-apps/tauri/blob/59b63ab500da9859f12d8888c3298bc29bba0b92/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi) · [Windows file sharing](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew).
