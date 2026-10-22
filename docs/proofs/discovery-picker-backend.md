# H016-c1: backend client file picker

Date: 2026-10-20. Scope: Windows native picker and in-memory discovery
integration only. The UI command has not been wired in H016-c2 yet.

`select_client_file` accepts only a UUID request ID and one of `codex`,
`claude_code`, or `opencode`. Tauri injects the calling `WebviewWindow`; the
backend accepts only the `main` window and uses its HWND as the dialog owner.
The renderer cannot provide a path, process arguments, or HWND. The blocking
dialog runs on a dedicated STA thread with `CoInitializeEx`/`CoUninitialize`
RAII. It uses `IFileOpenDialog`, a single executable/script filter,
`FOS_FORCEFILESYSTEM`, `FOS_FILEMUSTEXIST`, `FOS_PATHMUSTEXIST`,
`FOS_NOCHANGEDIR`, `FOS_NODEREFERENCELINKS`, and `FOS_DONTADDTORECENT`. The
dialog's `SIGDN_FILESYSPATH` allocation is bounded to 32,767 UTF-16 units and
released with `CoTaskMemFree`. The explicit no-change-directory flag follows
the [Microsoft Common File Dialog documentation](https://learn.microsoft.com/en-us/windows/win32/shell/common-file-dialog).

The selected filename must exactly match the chosen client command name and
one of `.exe`, `.cmd`, or `.bat`, case-insensitively. Before filesystem
metadata is read, UNC/device paths are rejected; the existing discovery guard
then rejects non-local or non-fixed volumes and any symlink/reparse component.
The selected program is never run and its contents, configuration, credentials,
and conversation data are never opened. A valid selection adds an opaque
installation ID with `source=manual_file`, `version=null`,
`location_label=Selected file`, and `connection_status=not_verified`. The
canonical executable path stays in the backend's private map and the manual
candidate set is memory-only, capped at 32. Rescans revalidate that set,
deduplicate against automatic results, and drop disappeared files with the
existing fixed location warning. Cancel returns a correlated
`{schema_version:1,request_id,status:"cancelled"}` envelope; errors and cancel
leave the prior snapshot and generation unchanged. Scan and picker share one
mutation lock while snapshot reads remain available.

Fourteen focused discovery tests passed on Windows 10 Pro 22H2 x64, build
19045, using rustc 1.92.0 and Cargo 1.92.0. They include the six discovery
baseline cases and picker fixtures for cancellation/error preservation,
invalid request/client/path, a valid cross-client mismatch, UNC rejection,
Unicode/manual metadata, duplicate scan selection retained across an empty
rescan, missing-file removal, the manual cap, and shared-lock behavior. Tests
inject a selector closure and use fresh guarded UUID fixture directories; no
native file dialog was opened. The picker command was compiled but not invoked
from a real WebView. No client executable or external provider was run.

Commands, from the project root with project-local Cargo directories:

```powershell
$env:CARGO_HOME=Join-Path (Get-Location).Path 'work\caches\cargo'
$env:CARGO_TARGET_DIR=Join-Path (Get-Location).Path 'work\caches\cargo-target'
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --offline --manifest-path src-tauri/Cargo.toml integrations::discovery_tests::
cargo check --offline --manifest-path src-tauri/Cargo.toml --features custom-protocol
```

All three commands passed. The focused suite reported 14 passed, 0 failed.
The pinned Windows COM dependency is `windows=0.62.2` with only
`Win32_Foundation`, `Win32_System_Com`, `Win32_UI_Shell`, and
`Win32_UI_Shell_Common`; the lockfile package checksum is
`527fadee13e0c05939a6a05d5bd6eec6cd2e3dbd648b9f8e447c6518133d8580`.

Source/config SHA-256 fingerprints for this candidate:

| File | SHA-256 |
| --- | --- |
| `src-tauri/src/integrations/discovery.rs` | `caadea772b464df077550f6f3411a0926a18e1dd2c252f1dfcd4c1a4cb8519b8` |
| `src-tauri/src/integrations/discovery_tests.rs` | `883b0c512049b4a6205f0830e17b447ac6816cd7bc9b403e6f144d81b55302e2` |
| `src-tauri/src/integrations/picker.rs` | `c6dfb9bcbd707eff80883df3f800f1643d202e02dbfb6ba68e2e837b5aed0f20` |
| `src-tauri/src/integrations/mod.rs` | `4f507ddc4018f0992a7d128e436ca71012de653badb64a18eb18cc6dd7a63964` |
| `src-tauri/src/main.rs` | `7dab6cf099ac7c1f2ed8b20975161936d35c97b9603c15f9d9fa0cb0217131a0` |
| `src-tauri/Cargo.toml` | `8eae7ff57df5ff48be9c629a0250c17813abe0c0af1e8ba86add9d3abd130096` |
| `src-tauri/Cargo.lock` | `f4e0081f60dc2dba489f0133d4cf4068e75632f7076cfb97a77858355931d140` |

H016-c2 still needs to connect the Applications UI to this command and exercise
the real main-window dialog. Windows 11, a clean-machine install, and actual
selection/cancel interaction from the desktop are not proven by this slice.
