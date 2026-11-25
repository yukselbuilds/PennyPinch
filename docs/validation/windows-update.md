# H042 — Windows update validation

## H042-a — Local update trust

10 November 2026. Local manifest and installer integrity checks passed on Windows 10 x64 build 19045. The application flow, actual update and rollback were still open at this stage. [Decision](../decisions/028-update-trust.md) · [Evidence](../proofs/windows-update-trust.json)

The Rust verifier checks the raw manifest bytes against the trusted public key before parsing them. It compares the signed version, channel, app identifier and Windows x64 target with the installed application. It derives the installer name from the application version. The runtime constants supply the engine version and payload manifest SHA-256. The first update schema accepts application packages that carry the same engine pin.

The stable and preview manifests were signed separately with Tauri CLI 2.12.3. Fixtures also cover a duplicate signed version field and an incorrect engine pin. The repository contains the public keys and signatures. The temporary fixture private keys were deleted. Git preserves LF bytes in the signed JSON files. The test installer consists of synthetic bytes.

Eight focused tests covered the correct channel and signature, incorrect or malformed keys, modified manifests, incorrect targets and schemas, older versions and mismatched engine pins. The installer verifier found a same-length modification using SHA-256. Missing, extra and unreadable data produced separate errors. The verifier stopped after reading one byte beyond the signed size. All 281 source and tool fingerprints matched before and after verification.

Run these commands from the repository root with the documented project Cargo cache to repeat the check.

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --offline --locked --manifest-path src-tauri/Cargo.toml desktop::update::tests -- --test-threads=1
```

The lead agent verified that signature checks precede parsing, that the app controls the trust key and URL, and that the same Rust type carries validated metadata into the size and hash check. The implementation uses `minisign-verify` 0.3.0 with the existing Base64, SemVer and SHA-256 dependencies. H042-b connects the verifier to a trusted source, an app-owned download and the UI. H042-c tests an installed upgrade and return to the last working version using real packages.

## H042-b — Updates screen and download

10 November 2026. On Windows 10 x64 build 19045, 13 focused Rust tests, 177 application tests and 8 native checks passed. Before and after verification, 336 focused source and tool inputs and 182 native inputs matched. The two runs shared 165 identical inputs. [Evidence](../proofs/windows-update-flow.json)

In the Turkish Settings screen of an isolated Tauri/WebView2 window, the stable channel showed version `0.1.1`, the signed release note and the 39-byte fixture. An active local model stream stayed open while the update downloaded and verified, and delivered its final response. The package waited while the engine was running and became ready when the engine stopped. The English screen showed the ready state and preview version `0.1.1-rc.1`. A normal exit removed the temporary files and closed the engine ingress and WebView test ports. The test driver was corrected for early-run connection counts, stop-button handling and port observation.

Focused tests rejected an incorrect signature, incorrect hash, interrupted download, HTTP redirect and a candidate identifier from the previous channel. A mismatched engine pin was also rejected when the package and installed app had the same version. The lead agent traced the UI action through the Tauri command, the source and file-handle checks and the engine state. An engine-stop observation is required before leaving the waiting state. The installation step checks that state again.

The local test uses a signed synthetic package and loopback. The production build takes the direct HTTPS base and trusted public key from `HEADROOM_DESKTOP_UPDATE_BASE_URL` and `HEADROOM_DESKTOP_UPDATE_PUBLIC_KEY`. Release configuration, an actual installer upgrade and rollback, and Windows 11 validation were still open at this stage.

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --offline --locked --manifest-path src-tauri/Cargo.toml desktop::update -- --test-threads=1
pnpm.cmd test:app
```

## H042-c1 — Signed NSIS preparation

10 November 2026. On Windows 10 x64 build 19045, NSIS 3.13 built a 2,943,643-byte update package from the previously verified H038 QA executable. Without the preparation command it exited with code 2 and preserved the existing destination file. Normal extraction produced the same SHA-256 as the source executable. [Evidence](../proofs/windows-update-package.json)

Three temporary Tauri signatures were used to test the real NSIS package against a local HTTP fixture. After verifying the signature and package digest, NSIS ran in an isolated directory. The extracted app passed another size and digest check. A same-size wrong digest and an incompatible data format were rejected separately. A data-format mismatch blocked the package request. The first run left an NSIS temporary directory. Using the built-in NSIS extraction command without changing the working directory resolved it. The fixture private key was deleted.

In an isolated Tauri window, test version `0.0.9` downloaded and prepared `0.1.0` through Settings. The wrong digest on the preview channel produced an error and left the retry button available. The engine stayed stopped. Normal exit cleared the staging files and closed both test ports. The checks passed for 14 focused Rust tests, 1 real NSIS package, 177 application tests, the production UI build and 5 native checks. There were 314 matching package and tool inputs, 189 matching native inputs and 166 matching inputs shared by the two runs.

The package builder passes `APPLICATION` and `OUTPUT` to `src-tauri/windows-update.nsi`, using pinned NSIS and the real release executable for the same version. The production design at this stage used the v2 signed manifest and application digest for that version. The next checks exercise executable replacement, startup and rollback in an isolated QA installation. Disk-full and interruption scenarios, a live release source and Windows 11 desktop acceptance were still open.

## H042-c2 — Installed upgrade and recovery

10 November 2026. A real NSIS installation ran on Windows 10 x64 build 19045 with the isolated identifier `dev.headroom.desktop.h042-c9eb52e7`. The new and previous app versions were compiled from the same sources with different version metadata. The current engine resources were verified against their pins in a separate staging area. The QA build used its local-release-fixture build option and fixture key. Release LTO was disabled, with 16 codegen units. This run covered a local fixture source. [Evidence](../proofs/windows-update-install.json)

The normal run covered 8 checks, including signed package preparation in Settings, rejection while the engine was active, cancellation, another confirmation, starting the new version and rollback. The preferences SHA-256 matched throughout `0.1.0 → 0.1.1 → 0.1.0`. A sample user file was preserved. The registered Windows version rolled back. The first installed executable and the restored executable had matching SHA-256 values.

The second run covered 5 checks. After the new version opened, the only app process in the QA Job ended with code 42. The stored recovery executable verified the signed record and files again and opened the previous version. The preferences and user file digests matched. Both runs ended with no staging files and their test ports closed. An interruption at the file replacement boundary was tracked separately.

The final local gate passed 179 app tests, TypeScript/Vite, formatting, 411 Rust tests and the custom-protocol build. It skipped 11 opt-in tests. File tests covered normal replacement and rollback, partial staging, a modified source, a locked target and preservation of a live file changed by the user. Engine reservation and shutdown were checked separately. The gate and release QA build shared 165 matching inputs. Each test result is bound to its before-and-after fingerprints.

The NSIS bundle-type marker difference from the initial run was confirmed against `__TAURI_BUNDLE_TYPE_VAR_NSS` in pinned Tauri Utils 2.10.2. The engine pin check also rejected stale staged resources, which were prepared again using the current inputs. These diagnostic runs were excluded from the acceptance evidence.

The remaining Windows 10 fault checks are recorded below. Windows 11 x64 and the production source and trust key have their own acceptance criteria. The run made zero live provider requests.

Both QA installations were removed with their uninstallers. The two application registrations and the LocalAppData roots owned by the tests were removed. [Cleanup record](../../work/proofs/update/cleanup-installed.json)

## H042-c2 — Remaining Windows 10 fault checks

10 November 2026. The current source passed local acceptance on Windows 10 x64 build 19045. The gate passed 19 focused update and file tests, two signed executable and HKCU fixture checks, 413 Rust tests, formatting and the custom-protocol build. The 179 app tests and build results were reused after 123 frontend input and tool fingerprints matched. [Evidence](../proofs/windows-update-faults.json)

The test build raised `ERROR_DISK_FULL` after a partial write. The operation removed the partial file and preserved the live executable. An owned test process was terminated immediately before and immediately after `MoveFileExW`. The target held either the complete previous or the complete new file. The previous backup survived and restoration from the new file passed. The test covers process interruption at the API boundary. Sudden power loss remains untested.

A real Windows fixture passed its preflight probe and then exited with code 7 during normal startup. The production apply and rollback functions ran with a valid signed manifest and an isolated HKCU installation registration. The previous executable, registered version and sample user file were preserved. The temporary fixture private key was deleted.

At a history size of 32 records, the latest rollback record was retained, equivalent duplicates were consolidated and old signed records were removed. The locked record, record with the wrong signature and foreign user file were preserved. After releasing the lock, a retry removed the eligible old record. File handles deny writes and deletes during verification. After gaining delete access the operation checks the same digests again, then deletes via open handles in the pinned directory.

The current candidate was tested in a real, isolated NSIS installation with identifier `dev.headroom.desktop.h042-7088752f`. Eight normal and five crash-recovery checks passed, preserving the preferences and sample user file digests. The second actual update removed the old record and left one new record. The test ports closed. The QA uninstaller removed the installation registration and test state root.

The QA build explicitly accepts the local release fixture, with LTO disabled and 16 codegen units. Engine resources were reused after their file and source digests matched the previously verified package. Windows 11 x64, the live update source and the production trust key remain separate checks.

<a name="h042-d--yerel-yayın-hazırlama-komutu"></a>

## H042-d — Local release preparation command

10 November 2026. On Windows 10 x64 build 19045, 7 Python tests, 9 standard Rust tests and formatting passed. Real NSIS 3.13, Tauri CLI 2.12.3 and the production Rust verifier prepared the stable and preview channels separately. The command rejected the wrong private signing key, a version equal to the installed one and a pre-existing output directory. The extracted x64 executable from each local package passed hash, version, identifier and data-format checks. Before and after the run, 231 inputs matched. [Evidence](../proofs/windows-update-release-preparation.json)

Maintainers use this command to prepare a release. Supply the real release executable and config with the same version and identifier. Embed the public key when building that executable. Build the application, set its version and generate the signing key before running this command. If required, pass the password to the signer through the `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` process environment. The command line and receipt omit that password, and the receipt omits private-key contents and fingerprints.

The paths below are examples. Provide the actual release inputs after selecting the production key and build. Run from the repository root.

```powershell
& .\work\proofs\engine\toolchain\cpython-3.13.13-windows-x86_64-none\python.exe -I -B .\scripts\prepare-windows-update.py `
  --application .\work\releases\inputs\headroom-desktop.exe `
  --config .\work\releases\inputs\tauri.release.conf.json `
  --output .\work\releases\0.1.1 `
  --public-key .\work\releases\keys\release.key.pub `
  --private-key .\work\releases\keys\release.key `
  --notes .\work\releases\inputs\notes.md `
  --channel stable --from-version 0.1.0 `
  --release-base-url 'https://yukselbuilds.github.io/PennyPinch/updates/'
```

For release preparation, provide the same exact HTTPS base to `--release-base-url` and `HEADROOM_DESKTOP_UPDATE_BASE_URL`. The script verifies the URL and public key in the input executable before signing and again in the extracted executable. Local QA commands can use the existing identifier, version and data-format checks.

The command writes to a new directory under the project's `work` root. The `stable` or `preview` channel contains only `manifest.json`, `manifest.json.sig` and `Headroom_<version>_x64-update.exe`. The parent `release-receipt.json` records file, source and tool digests and `published: false`. Preserve the raw JSON and signature bytes during delivery. Extra channel files or changed inputs abort preparation. The script preserves a conflicting destination and cleans up its own partial directory.

The test executable was a small native fixture implementing the update probe. The run covered package and probe verification, and the temporary fixture private keys were deleted. Binding the production executable to the real HTTPS source, delivery to an installation that trusts the production key and Windows 11 x64 acceptance remained open. The user selected local preparation, so there was no public repository change, release or CI run. The product runtime and UI were unchanged during that historical run. H042-c2 carries the earlier full-gate evidence for its own scope.

### H042-d2 — Real Headroom executable

10 November 2026, Windows 10 x64 build 19045. The real Headroom executable was built from unchanged source at version `0.1.1`, with isolated identifier `dev.headroom.desktop.h042d-557cd46e` and a temporary public key. The build used process-scoped `TAURI_CONFIG` and the public-key input. This build omitted the `update-fixture` feature and an HTTPS source. The release-mode QA profile recorded LTO off, 16 codegen units and stripping enabled. [Evidence](../proofs/windows-update-real-application.json)

H042-d packaged the 9,915,392-byte application with NSIS. The real Rust verifier checked its signature, engine pin and executable and package digests. The real executable passed the identifier, version and data-format probe. An incorrect version or identifier made the executable exit with code 2. Preparing either config exited with code 1 and produced no ready output. An incorrect public key was also rejected. There were 236 matching source, test, tool and embedded-asset inputs before and after, with the same tool versions. The earlier 7 Python and 9 Rust tests were reused with 231 matching inputs and the same tools. The 60 frontend source, asset and lockfile inputs matched.

The temporary fixture private key was deleted. The run left no partial staging directory or QA app data folder. It exercised the executable probe only. The launch UI, installation, engine traffic and installed transition were outside that run's scope. The real release profile, key, HTTPS delivery and Windows 11 acceptance remained open. The artifact was local QA. Product source had not changed, so the existing product tests and CodeGraph index were reused.

### 19 November 2026 — 0.44.0 Windows 10 QA package

The earlier real-app preparation used 0.41.0. The team compiled a real Headroom release executable from `6702b64` with the 0.44.0 pin and an isolated test identifier. It built the local NSIS update for version 0.1.1. The production Rust verifier accepted the manifest signature, engine pin and both file digests. The extracted executable was byte-for-byte identical to the source and passed the identifier, version and state-format probe. The command rejected the wrong identifier, version and public key. The temporary private key was removed after testing. [Evidence](../proofs/windows-update-044-qa.json)

Seven focused Python tests passed. There were 364 matching build inputs and tool versions before and after, with 304 matching source-gate inputs and embedded frontend files. The local acceptance covers validating and extracting the signed package. The installed Windows 11 x64 transition, production HTTPS source and production key remain open.

<a name="19-kasım-2026--açılış-bildirimi-ve-tek-depodan-dağıtım"></a>

### 19 November 2026 — Startup notification and single-repository distribution

At startup, an idle, configured updater checks the channel for the installed version once. SemVer prereleases use the preview channel and stable versions use the stable channel. When the updater finds a newer version with a valid signature, the app displays it and offers to open Settings. The user starts the download.

The Windows 10 check used the real WebView and a local signed 0.1.1 manifest. The notification appeared before Settings opened, and the download button was ready in Settings. The server saw two GET requests, for the manifest and signature. Normal exit and input-fingerprint checks passed. [Native evidence](../../work/u42/ad38ae49/result.json)

One public GitHub repository can host the source and installers in Releases. Signed updates use a fixed HTTPS address in the same repository's GitHub Pages site. The release build receives `HEADROOM_DESKTOP_UPDATE_BASE_URL` and `HEADROOM_DESKTOP_UPDATE_PUBLIC_KEY`. The source must serve the channel's `manifest.json`, `manifest.json.sig` and the update package at direct URLs without redirects. The production signing key, Pages deployment and live HTTPS end-to-end check are open. [GitHub Pages](https://docs.github.com/en/pages/getting-started-with-github-pages/what-is-github-pages)

### 20 November 2026 — Build-bound HTTPS update source

A Windows 10 x64 run used an isolated QA identifier and temporary key. The real release-mode executable prepared a signed 0.1.1 package with the expected HTTPS address and public key. The probe rejected an incorrect HTTPS path, plain HTTP and a different key. An incorrect release address produced no ready package. The existing three-argument recovery probe also passed. [Programmatic evidence](../proofs/windows-update-release-source.json)

The run passed 478 Rust tests, Windows packaging tests and the custom-protocol build. It skipped 24 opt-in Rust tests. Matching inputs let it reuse the 227 app tests and three production assets. The existing Windows 10 installer kept its SHA-256 and the temporary private key was deleted. A preflight request to the planned Pages source returned 404 for the manifest. Production signing and live HTTPS delivery remain open.

### 20 November 2026 — Engine resources in executable-only updates

The release script sorts the SHA-256 values of six helper files and two Windows manifests and produces a combined digest. In a v3 update prepared with `--release-base-url`, the signed manifest includes that digest. Verification covers the input and extracted executable. The installed app offers the package when the signed digest matches its build. Use the full Windows installer when the resources change. [Contract](../decisions/028-update-trust.md#h042-c1--imzalı-uygulama-paketi)

The Windows 10 x64 check used a real QA executable and temporary key. The compatible 0.44.0 package passed signature, NSIS extraction and executable verification. A fixture re-signed with a different digest for `desktop_helper.py` returned `RuntimeResourcesMismatch`, even with the same engine manifest and package bytes. The run passed 479 Rust tests, the package tests and custom-protocol build. It skipped 24 opt-in Rust tests. The 227 frontend tests and three production files were reused after their inputs matched. [Programmatic evidence](../proofs/windows-update-runtime-resources.json)

## 22 November 2026 — Public preview and live signed download

GitHub Releases serves the v0.1.0 Windows 10 preview for manual installation. Both anonymous downloads matched their recorded SHA-256 digests. [Publication evidence](../proofs/pennypinch-windows10-preview-publication.json)

The v0.1.1-preview.1 installer embeds the HTTPS source and public key. The preview channel serves a signed v0.1.1-preview.2 manifest and app package. The Windows 10 update manager accepted the live manifest, downloaded the signed package, and extracted the executable. The staged digest matched the release build. The manager deleted the staged file when the test ended. Five local flow tests also passed. [Preview evidence](../proofs/pennypinch-windows10-signed-preview.json)

The download test used the same check and download methods as Settings. It used a test state directory. The installed transition has separate evidence below.

For a live preview check, set the build-time update URL and public key described above. Set `PENNYPINCH_LIVE_UPDATE_FROM_VERSION` to the installed version. Set `PENNYPINCH_LIVE_UPDATE_VERSION` to the published preview version. Set `PENNYPINCH_LIVE_UPDATE_APPLICATION_SHA256` to the application digest from the verified build. Run the following command from the project root.

```powershell
cargo test --offline --locked --manifest-path src-tauri/Cargo.toml desktop::update_flow::tests::verifies_live_preview_download -- --ignored --exact
```

## 22 November 2026 — Installed live update and public installer

A real Windows 10 22H2 x64 installation accepted the v0.1.1-preview.1 installer over v0.1.0. Settings verified and installed v0.1.1-preview.2 from GitHub Pages. The app reopened with the new version. The restore action reopened the previous version. The restored executable matched its original digest. The installer registration, test preferences, and local test file matched. [Installed evidence](../proofs/pennypinch-windows10-installed-update-and-release.json)

The original profile stayed in a local backup during the test. The test removed its installation and data. The 2,081 original profile files matched after restoration.

GitHub published the full installer ZIP as v0.1.1-preview.1. Both assets passed anonymous download verification. The unsigned installer embeds the public key for signed in-app updates. Windows 11 x64 update and rollback remain open. [Windows preview](https://github.com/yukselbuilds/PennyPinch/releases/tag/v0.1.1-preview.1)

## 24 November 2026 — Headroom 0.45.0 full installer

Before the fix, the 0.45.0 NSIS package returned exit code 0 after an in-place install. It left nine Headroom 0.44.0 metadata files. Packaged Python selected that version. The engine launcher rejected the version mismatch.

The installer now removes the known 0.44.0 metadata from the registered installation. It checks the target before copying. It preserves unknown files and rejects a conflict at the retirement path.

On Windows 10 22H2 x64, the fixed full installer replaced Headroom 0.44.0 with 0.45.0. Both installers used PennyPinch version 0.1.0. All 19,491 staged resources matched. Packaged Python selected 0.45.0. The installed controller started the engine, completed one local Responses SSE request, and stopped normally. The test preserved its separate state marker and removed its installation.

The executable-only update channel requires matching runtime resources. Use a full installer for an engine version change. Public release delivery and Windows 11 require separate checks. [Evidence](../proofs/windows-installer-045-upgrade.json).

## 24 November 2026 — Windows 10 preview.4 installed upgrade

The test used the public PennyPinch 0.1.1-preview.3 package that matches the release digest. The local full installer upgraded it to 0.1.1-preview.4 on Windows 10 22H2 x64. Packaged Python selected Headroom 0.45.0. All 19,491 staged resources and the package receipt matched.

The app kept the saved preferences and the test data file. Start opened the installed engine. The app completed one local Responses SSE request through its loopback listener. The engine stopped normally. The older installer exited with code 2. The installed version and its files matched after that attempt. The test removed the test profile and installation.

The candidate executable accepted the existing update source, the public key, and the new runtime resource pin. It rejected the pin for 0.44.0. The candidate installer has no Authenticode signature. Windows 11 x64 verification remains open. [Evidence](../proofs/windows-preview4-installed-upgrade.json).

## 24 November 2026 — Windows 10 preview.4 delivery package

The local ZIP contains the installer that passed H042-ai and the PennyPinch license. The package includes 673 indexed license and notice files. The Headroom 0.45.0 wheel and the staged runtime contain the same three Headroom notice files. The index records the new engine version and installer digest.

PowerShell extracted all 678 files to a folder with Unicode and spaces on Windows 10 22H2 x64. The test matched the 677 listed SHA-256 digests. The extracted setup file matched the tested installer. The external checksum and release notes are ready for publication. The package uses the full installer for this engine upgrade. Windows 11 x64 verification remains open. [Delivery evidence](../proofs/pennypinch-windows10-preview4-delivery.json).

## 24 November 2026 — Public Windows 10 preview.4

GitHub Releases serves PennyPinch v0.1.1-preview.4 as a prerelease. The tag points to the source of the tested installer. Both anonymous downloads matched their SHA-256 digests. The setup file in the downloaded ZIP matched the installer from the Windows 10 upgrade test.

The project page and update page link to the full installer. The signed preview feed still serves v0.1.1-preview.2 for compatible installations. Its three files kept their original digests. Use the full installer for the engine upgrade. Windows 11 x64 update and rollback remain open. [Release](https://github.com/yukselbuilds/PennyPinch/releases/tag/v0.1.1-preview.4) · [Publication evidence](../proofs/pennypinch-windows10-preview4-publication.json).

## 25 November 2026 — Public Windows 10 preview.5

GitHub Releases serves v0.1.1-preview.5 as a prerelease for Windows 10 22H2 x64. Both downloads matched their SHA-256 digests without an account. The installer matched the Windows 10 upgrade test. The desktop check script passed five cases on Windows 10. [Release](https://github.com/yukselbuilds/PennyPinch/releases/tag/v0.1.1-preview.5) · [Publication evidence](../proofs/pennypinch-windows10-preview5-publication.json)

Use the full installer to install preview.5. The signed channel serves v0.1.1-preview.2 to compatible installations. Its three files kept their recorded digests. Windows 11 x64 update and rollback remain open. [Update status](https://yukselbuilds.github.io/PennyPinch/updates/)

## 25 November 2026 — Check an older signed release

The public preview.5 app showed a verification error after it read the older preview.2 metadata. The earlier release uses Headroom 0.44.0. The installed preview.5 uses Headroom 0.45.0. The old check tested engine compatibility before it handled the version difference.

The lead agent repeated the check with a release-mode test executable in a separate installation on Windows 10 22H2 x64. The same channel showed that there was no newer release. The manifest and signature digests matched both runs. The engine stayed stopped. Both runs exited normally and removed their test profiles.

The full local gate passed 227 application tests and 481 Rust tests. The tests also checked the signature, engine compatibility for equal and newer versions, and the schema of the older manifest. The fix awaits the next installer. [Decision](../decisions/028-update-trust.md#h042-ao--older-releases-in-the-signed-channel) · [Evidence](../proofs/windows-update-older-preview.json)
