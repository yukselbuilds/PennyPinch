# H042-a — Update manifest and trust boundary

10 November 2026. The user approved the local H042-a slice while leaving H041 and H037 external verification open. The lead agent owns signature, source, and version trust. H042-b and H042-c are separate slices.

A Windows x64 application update is defined by manifest.json, a separate Tauri Minisign signature for that file, and an installer in the existing NSIS format. The signature covers the raw manifest bytes. The manifest binds application identity, target, channel, SemVer version, plain-text release notes, and the installer's exact size and SHA-256 digest. Tauri key/signature files use Base64. Pin minisign-verify 0.3.0, a dependency-free verifier for canonical Ed25519 signatures. [Tauri signing contract](https://v2.tauri.app/plugin/updater/#signing-updates), [Tauri 0.3 migration record](https://v2.tauri.app/release/updater/v2.13.3/).

The trusted public key and installed app identity/version come from the running build. The downloaded manifest cannot choose them. Bound signature size, verify the signature before reading JSON, and parse v1 fields with a closed schema. The stable channel accepts plain SemVer; the preview channel accepts prereleases. A candidate must be strictly newer than the installed version. Reject wrong identity, target, channel, and repeated/equal/older versions. Rollback to the local last-working version is a separate H042-c path.

Signed metadata includes the engine version required by the installer and the payload-manifest SHA-256. The verifier compares them with the runtime manager's existing fixed values. The initial update contract is for application packages that preserve this engine pin. Derive the installer download name from the verified application version. The manifest schema carries no key, URL, or separate engine-install command. Bind the distribution URL and download ownership separately in H042-b.

Acceptance order:

1. Accept real-Tauri-signed stable and preview fixtures in their respective channels. Match the expected app/engine identity and signed size/SHA.
2. A wrong key, tampered/invalid signature, validly signed but invalid schema/identity/channel/version, or engine-pin mismatch yields no candidate.
3. Reject missing, extra, unreadable, or wrong-SHA installer contents. The stream reads at most one byte beyond the signed size. H042-b preserves ownership and identity of the verified file before execution.

Fix limits before the run. Manifest is at most 32 KiB, signature text 4 KiB, release notes 4 KiB, and installer 1 GiB. Current signed samples use only a fixture key and synthetic installer bytes. Production trust key, download, and install action belong to later slices. Focused verification: cargo test --offline --manifest-path src-tauri/Cargo.toml desktop::update::tests -- --test-threads=1.

## H042-b — Updates screen and download

The user approved continuing after H042-a. The lead agent owns source trust, file ownership, and waiting behavior tied to engine state. This slice checks the channel selected by the user, shows verified version/notes/size, and prepares the signed package for this session. Running the installer, persistent transition record, and rollback are implemented in H042-c.

The release base and public key are bundled with the Windows x64 build. Do not take the URL from user settings or the manifest. The source serves direct HTTPS responses. There is no automatic redirect, user identity, or automatic check. Each channel retrieves manifest.json, manifest.json.sig, and the installer derived from the verified version from its own directory. A build without a defined production source clearly shows unavailable. A local fixture is enabled only by an explicitly selected debug feature, 127.0.0.1, and a separate fixture key.

A verified candidate is session-scoped. Checking another channel drops the old candidate. The download writes the signed size plus at most one extra byte to a temporary Windows file under the pinned app-data root, using sharing flags that allow writing but not deletion. Verify size and hash through the same open file handle. Close the handle to clean up on error and app exit. The UI does not receive local file path, signature, or key. H042-c separately validates file and process ownership at publication/execution.

The engine's active stream count is unknown. Therefore, after download, state is ready only when the owned engine is verified stopped. Wait while it is running, transitioning, or uncertain. This observation is not authorization to install. This check does not stop the engine or interrupt traffic.

Acceptance covers check→metadata→download→verification with a valid local fixture; rejection of invalid signature/wrong hash/partial download/redirect; preserving engine state while waiting; and the TR/EN Settings flow. Production signatures and real update/rollback require separate evidence.

<a id="h042-c1--imzalı-uygulama-paketi"></a>
## H042-c1 — Signed application package

10 November 2026. The lead agent owns package format, executed-file trust, and temporary-area ownership. The existing full NSIS installer retains clean-install and repair paths. The first update protocol prepares only the embedded UI executable while keeping the same engine pin, installation resources, and local data format.

The signed schema_version: 2 manifest carries application.state_format: 1, the application's exact byte size, and SHA-256. The size limit is 128 MiB. Package name pattern is `Headroom_<sürüm>_x64-update.exe`. V1 manifests retain the H042-a/b download contract. Updating the installed app requires a prepared v2 application. Reject an incompatible data format before downloading the package.

src-tauri/windows-update.nsi runs only with the /HEADROOM-EXTRACT command. Full installer paths and registry operations remain separate. After package verification, run it with an open handle that allows only read sharing, in a separate pinned directory under the app-data root. NSIS extracts to a temporary directory and fails if the target name already exists. If the extracted file's size and SHA-256 match the signed manifest, transfer it to a one-session file that is deleted on close. [NSIS rename contract](https://nsis.sourceforge.io/Reference/Rename).

Local acceptance uses a real NSIS 3.13 package and a previously verified QA executable. Test a valid signed package, same-size wrong app digest, incompatible data format, running without the command, and occupied target name. Observe that temporary files are removed after normal preparation and invalid-output rejection. The user's actual installation files and data root remain separate.

The next slice verifies ownership of an installed Windows app. After engine and connection operations stop safely, acquire the private .desktop-use lock. Atomically replace the single application file on the same disk. Bind the previous working executable and installed-version data to rollback without touching existing user data. Process interruption, disk full, and startup incompatibility are separate error scenarios. Acceptance on both desktop Windows versions remains open.

On 20 November 2026, release manifest v3 bound application.runtime_resources_sha256 into the signature. The digest is derived from sorted SHA-256 values for six engine helper files and two fixed Windows manifests in the Windows package. The installed executable compares its build-bound digest with the signed digest of the downloaded v3 candidate. If resources differ, do not offer an executable-only update. Such versions require the full installer package. V1/v2 parsing support remains for development fixtures and existing recovery records. Executable-only updates from the real HTTPS source require v3.

## H042-c2 — Installed-app transition and rollback

The lead agent owns process, file, and persistent recovery-record behavior. Offer installation only for the app's own HKCU NSIS record, a valid installation directory, and executable running from that directory. This action is disabled in development builds. The first transition supports app versions that use the same source package and data format 1.

The user selects a version in Settings and confirms the restart. Under a mutex, the engine manager rechecks that the engine is stopped and clean. The same mutex prevents engine restart. Exit is refused while a settings or export operation is pending. The previous executable, new executable, signed manifest, and content-free recovery record are prepared under the app's update-recovery root. After normal app shutdown succeeds, a copy of the previous executable starts as the recovery process.

The recovery process accepts only its own UUID directory and the app-owned .desktop-use lock. It revalidates the signed manifest and file digests. The new executable is prepared in the same installation directory, checked in a short child process for identity/version/data format, and published as a single file with MoveFileExW. Update HKCU version and reopen the app. On early startup failure, return to the previous executable. On the next launch, Settings offers rollback to the recorded previous version. [Windows file transition](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw), [sharing lock](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew).

Normal acceptance is install→update→open→rollback→open with two release builds under a separate NSIS app identity. Preferences and a sample user file must retain matching digests across both transitions. A running engine must reject the install request; cancel must not start exit. Focused file tests cover partial preparation, a changed file, and a locked target. Disk full, process termination during transition, startup failure, and Windows 11 x64 are recorded as separate acceptance scenarios.

### H042-c2 failure acceptance

For remaining local acceptance, the lead agent owns signature, records, and process ownership. Test disk-full by returning Windows ERROR_DISK_FULL after a partial write to a real file, only in a test build. The previous executable must remain unchanged and the partial file must be removed. Terminate a separate owned test process immediately before and after the publication API. The target must contain either the complete previous or complete new file, and rollback must remain possible. This experiment does not cover sudden power loss inside the API.

For early-startup testing, a real Windows executable passes identity checking and exits with an error code during normal startup. Exercise the production application/rollback path using a signed package and separate HKCU test record. Preserve the previous executable, recorded version, and sample user file.

History maintenance removes only signed records and validated files under fixed UUID directories. Keep the record that allows return to the current executable. Duplicates for the same backup may be reduced to one record. Preserve unknown files, wrong signatures, and locked records. If the 32-record limit remains full because records could not be cleaned, fail the new preparation safely. Windows 11 and a real release source remain separate acceptance items.

## H042-d — Local release preparation

On 10 November, the user chose local preparation for now instead of a separate public repository. The lead agent owns package/signature trust. scripts/prepare-windows-update.py prepares one channel directory from the explicitly supplied release executable, app config, key files, and notes. Public repository, network upload, generating a real release key, and running CI are out of scope. Existing installation and account settings are not touched.

Compare version/identity/data format against the executable's update probe and public key against the value in the executable. Package the app using the same NSIS template. Validate manifest signature, engine pin, channel, SemVer, and package/app digests with the production Rust verifier. After extraction, verify the executable digest and probe again. Bind inputs and tools with before/after fingerprints. Do not copy private keys/passwords into output or receipt; pass the password only to the signing subprocess.

Acceptance scenarios:

1. Local x64 fixture with temporary key → preparation → only manifest, signature, and package in the channel, plus a content-free verification receipt. This is not proof of a production release executable or real HTTPS delivery.
2. Wrong channel/key/version, invalid signature, or a different extracted executable → error → no ready output.
3. Existing output, tool failure, or input changed during preparation → reject → preserve previous output and original inputs, remove the owned partial directory.

Focused gate uses pinned Python, test_windows_update_package.py, normal update Rust tests, and real NSIS/Tauri signer/Rust-verifier execution. Product runtime/UI do not change. Windows 11 x64 and production source/key remain open in the main H042 task.

### H042-d2 — Local preparation with the real app

At the user's request to continue, verify the remaining connection within local preparation: use a release-mode executable built from current Headroom source in the same command instead of a small probe fixture. Without changing source, use TAURI_CONFIG for a separate QA identity/version and a temporary public key in the build environment. Do not add an HTTPS source or the update-fixture feature. Preserve the previously used QA profile with LTO off and 16 codegen units; this is not a distributable production build or production performance acceptance.

Pass a valid app through preparation, signature/hash/extraction/probe, and before/after input equality. A mismatch in config version/identity or executable public key must not produce ready output. Delete the test key's private part afterwards; do not write to existing install/data roots. Reuse unchanged tests and frontend build only when source/tool inputs match. Build and preparation evidence comes from a new run; external acceptance remains open.

## H042-ao — Older releases in the signed channel

On 25 November, preview.5 showed a verification error when it read the signed preview.2 metadata. The earlier release requires a different engine and different runtime resources.

Verify the signature before parsing the manifest. Check the application, platform, channel, schema, and artifact fields. Return no candidate when the signed release is older. Check engine and resource compatibility for equal and newer releases. Offer an update only when its version is newer and all checks pass.

## H042-ar — Full installer guidance for engine changes

Verify the signature before reading the manifest. Check the application, platform, channel, schema, artifacts, and version. For a newer schema 1 or 3 release, show the full installer instruction if the engine pin or runtime resource digest differs. Keep the candidate empty. An invalid signature, an invalid manifest, or an incompatible state format still causes a verification error. Keep the existing version and channel rules.

The Windows 10 native Settings test used a signed loopback feed. It checked the instruction in Turkish and English, an invalid signature, and a compatible preview. The parser tests also cover the schema 3 resource digest and malformed fields. The change is ready in source. [Evidence](../proofs/windows-update-full-installer.json)
