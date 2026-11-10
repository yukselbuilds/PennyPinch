use super::*;
use serde_json::{json, Value};
use std::io::Cursor;

const KEY: &str = include_str!("../../../tests/fixtures/update/fixture-release.pub");
const OTHER_KEY: &str = include_str!("../../../tests/fixtures/update/wrong-release.pub");
const STABLE: &[u8] = include_bytes!("../../../tests/fixtures/update/stable.json");
const STABLE_SIG: &str = include_str!("../../../tests/fixtures/update/stable.json.sig");
const PREVIEW: &[u8] = include_bytes!("../../../tests/fixtures/update/preview.json");
const PREVIEW_SIG: &str = include_str!("../../../tests/fixtures/update/preview.json.sig");
const INSTALLER: &[u8] = b"headroom-h042-fixture-installer-v0.1.1\n";

fn policy(version: &Version, channel: UpdateChannel) -> UpdatePolicy<'_> {
    UpdatePolicy {
        app_identifier: "dev.headroom.desktop",
        installed_version: version,
        channel,
        trusted_tauri_public_key: KEY,
    }
}

fn current_version() -> Version {
    Version::new(0, 1, 0)
}

fn stable_update() -> VerifiedUpdate {
    verify_manifest(
        STABLE,
        STABLE_SIG,
        &policy(&current_version(), UpdateChannel::Stable),
    )
    .unwrap()
}

#[test]
fn verifies_a_signed_stable_release_against_the_installed_app_and_engine() {
    let update = stable_update();
    let app: Value = serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
    let release_engine: Value =
        serde_json::from_str(include_str!("../../../runtime/manifest.windows.json")).unwrap();
    assert_eq!(
        app["identifier"],
        policy(&current_version(), UpdateChannel::Stable).app_identifier
    );
    assert_eq!(release_engine["engine"]["version"], EXPECTED_ENGINE_VERSION);
    assert_eq!(
        release_engine["engine"]["runtime_manifest_sha256"],
        EXPECTED_RUNTIME_MANIFEST_SHA256
    );
    assert_eq!(update.version(), &Version::new(0, 1, 1));
    assert_eq!(update.channel(), UpdateChannel::Stable);
    assert_eq!(update.notes(), "Yerel imza örneği.");
    assert_eq!(update.installer_name(), "Headroom_0.1.1_x64-setup.exe");
    assert_eq!(update.installer_size(), INSTALLER.len() as u64);
    update.verify_installer(Cursor::new(INSTALLER)).unwrap();
}

#[test]
fn separates_stable_and_preview_channels() {
    let installed = current_version();
    let preview = policy(&installed, UpdateChannel::Preview);
    let stable = policy(&installed, UpdateChannel::Stable);
    let candidate = verify_manifest(PREVIEW, PREVIEW_SIG, &preview).unwrap();
    assert_eq!(candidate.channel(), UpdateChannel::Preview);
    assert_eq!(
        candidate.installer_name(),
        "Headroom_0.1.1-rc.1_x64-setup.exe"
    );
    assert_eq!(
        verify_manifest(PREVIEW, PREVIEW_SIG, &stable).unwrap_err(),
        UpdateError::WrongChannel
    );
    assert_eq!(
        verify_manifest(STABLE, STABLE_SIG, &preview).unwrap_err(),
        UpdateError::WrongChannel
    );
}

#[test]
fn rejects_equal_older_and_prerelease_downgrades() {
    for installed in ["0.1.1", "0.1.1+build.9", "0.2.0"] {
        let installed = Version::parse(installed).unwrap();
        let policy = policy(&installed, UpdateChannel::Stable);
        assert_eq!(
            verify_manifest(STABLE, STABLE_SIG, &policy).unwrap_err(),
            UpdateError::VersionNotNewer
        );
    }
    let final_version = Version::new(0, 1, 1);
    assert_eq!(
        verify_manifest(
            PREVIEW,
            PREVIEW_SIG,
            &policy(&final_version, UpdateChannel::Preview)
        )
        .unwrap_err(),
        UpdateError::VersionNotNewer
    );
    let release_candidate = Version::parse("0.1.1-rc.1").unwrap();
    verify_manifest(
        STABLE,
        STABLE_SIG,
        &policy(&release_candidate, UpdateChannel::Stable),
    )
    .unwrap();
}

#[test]
fn rejects_tampering_mismatched_signatures_and_untrusted_keys() {
    let installed = current_version();
    let mut trusted = policy(&installed, UpdateChannel::Stable);
    let mut tampered = STABLE.to_vec();
    tampered.push(b' ');
    for (body, signature) in [
        (&tampered[..], STABLE_SIG),
        (STABLE, PREVIEW_SIG),
        (STABLE, "!"),
    ] {
        assert_eq!(
            verify_manifest(body, signature, &trusted).unwrap_err(),
            UpdateError::InvalidSignature
        );
    }
    trusted.trusted_tauri_public_key = OTHER_KEY;
    assert_eq!(
        verify_manifest(STABLE, STABLE_SIG, &trusted).unwrap_err(),
        UpdateError::InvalidSignature
    );
    trusted.trusted_tauri_public_key = "";
    assert_eq!(
        verify_manifest(STABLE, STABLE_SIG, &trusted).unwrap_err(),
        UpdateError::InvalidTrustKey
    );
}

#[test]
fn rejects_signed_metadata_with_duplicate_version_or_mismatched_engine_pin() {
    let installed = current_version();
    let trusted = policy(&installed, UpdateChannel::Stable);
    let duplicate = include_bytes!("../../../tests/fixtures/update/duplicate-version.json");
    let duplicate_sig = include_str!("../../../tests/fixtures/update/duplicate-version.json.sig");
    let wrong_pin = include_bytes!("../../../tests/fixtures/update/different-engine-pin.json");
    let wrong_pin_sig =
        include_str!("../../../tests/fixtures/update/different-engine-pin.json.sig");
    assert_eq!(
        verify_manifest(duplicate, duplicate_sig, &trusted).unwrap_err(),
        UpdateError::InvalidManifest
    );
    assert_eq!(
        verify_manifest(wrong_pin, wrong_pin_sig, &trusted).unwrap_err(),
        UpdateError::EnginePinMismatch
    );
    let installed = Version::new(0, 1, 1);
    assert_eq!(
        verify_manifest(
            wrong_pin,
            wrong_pin_sig,
            &policy(&installed, UpdateChannel::Stable)
        )
        .unwrap_err(),
        UpdateError::EnginePinMismatch
    );
}

#[test]
fn refuses_unbounded_metadata_before_verification() {
    let installed = current_version();
    let trusted = policy(&installed, UpdateChannel::Stable);
    assert_eq!(
        verify_manifest(&vec![b' '; MANIFEST_LIMIT + 1], STABLE_SIG, &trusted).unwrap_err(),
        UpdateError::ManifestTooLarge
    );
    assert_eq!(
        verify_manifest(STABLE, &"A".repeat(SIGNATURE_LIMIT + 1), &trusted).unwrap_err(),
        UpdateError::InvalidSignature
    );
}

#[test]
fn enforces_installed_identity_and_closes_the_manifest_schema() {
    let installed = current_version();
    let mut trusted = policy(&installed, UpdateChannel::Stable);
    trusted.app_identifier = "dev.headroom.desktop.h042-qa";
    assert_eq!(
        verify_manifest(STABLE, STABLE_SIG, &trusted).unwrap_err(),
        UpdateError::WrongAppOrTarget
    );
    let trusted = policy(&installed, UpdateChannel::Stable);
    let cases = [
        ("/schema_version", json!(2), UpdateError::InvalidManifest),
        ("/schema_version", json!(3), UpdateError::UnsupportedSchema),
        (
            "/target",
            json!("windows-aarch64"),
            UpdateError::WrongAppOrTarget,
        ),
        ("/version", json!("latest"), UpdateError::InvalidManifest),
        (
            "/version",
            json!("0.1.1+build.9"),
            UpdateError::InvalidManifest,
        ),
        ("/version", json!("0.1.1-rc.1"), UpdateError::WrongChannel),
        (
            "/engine/version",
            json!("latest"),
            UpdateError::EnginePinMismatch,
        ),
        (
            "/installer/size_bytes",
            json!(0),
            UpdateError::InvalidManifest,
        ),
        (
            "/installer/size_bytes",
            json!(INSTALLER_LIMIT + 1),
            UpdateError::InvalidManifest,
        ),
        (
            "/installer/sha256",
            json!("A".repeat(64)),
            UpdateError::InvalidManifest,
        ),
        (
            "/notes",
            json!("a".repeat(NOTES_LIMIT + 1)),
            UpdateError::InvalidManifest,
        ),
    ];
    for (pointer, replacement, error) in cases {
        let mut manifest: Value = serde_json::from_slice(STABLE).unwrap();
        *manifest.pointer_mut(pointer).unwrap() = replacement;
        assert_eq!(
            parse_manifest(&serde_json::to_vec(&manifest).unwrap(), &trusted).unwrap_err(),
            error,
            "{pointer}"
        );
    }
    let mut manifest: Value = serde_json::from_slice(STABLE).unwrap();
    manifest["installer"]["url"] = json!("https://invalid.example/headroom.exe");
    assert_eq!(
        parse_manifest(&serde_json::to_vec(&manifest).unwrap(), &trusted).unwrap_err(),
        UpdateError::InvalidManifest
    );
}

#[test]
fn verifies_application_bytes_only_for_the_same_state_format() {
    let installed = current_version();
    let trusted = policy(&installed, UpdateChannel::Stable);
    let mut manifest: Value = serde_json::from_slice(STABLE).unwrap();
    let application = json!({
        "state_format": 1,
        "size_bytes": INSTALLER.len(),
        "sha256": format!("{:x}", Sha256::digest(INSTALLER)),
    });
    manifest["application"] = application.clone();
    assert_eq!(
        parse_manifest(&serde_json::to_vec(&manifest).unwrap(), &trusted).unwrap_err(),
        UpdateError::InvalidManifest
    );
    manifest["schema_version"] = json!(2);
    let update = parse_manifest(&serde_json::to_vec(&manifest).unwrap(), &trusted).unwrap();
    assert_eq!(update.installer_name(), "Headroom_0.1.1_x64-update.exe");
    assert_eq!(update.application_size(), Some(INSTALLER.len() as u64));
    update.verify_application(INSTALLER).unwrap();
    assert_eq!(
        update.verify_application(&vec![0; INSTALLER.len()][..]),
        Err(UpdateError::InstallerHashMismatch)
    );
    assert_eq!(
        update.verify_application(&INSTALLER[..INSTALLER.len() - 1]),
        Err(UpdateError::InstallerSizeMismatch)
    );

    let mut other_format = application;
    other_format["state_format"] = json!(2);
    for (replacement, error) in [
        (Value::Null, UpdateError::InvalidManifest),
        (other_format, UpdateError::StateFormatMismatch),
    ] {
        manifest["application"] = replacement;
        assert_eq!(
            parse_manifest(&serde_json::to_vec(&manifest).unwrap(), &trusted).unwrap_err(),
            error
        );
    }
}

#[test]
fn detects_changed_truncated_overlong_and_unreadable_installer_data() {
    let update = stable_update();
    let mut changed = INSTALLER.to_vec();
    changed[0] ^= 1;
    assert_eq!(
        update.verify_installer(&changed[..]),
        Err(UpdateError::InstallerHashMismatch)
    );
    assert_eq!(
        update.verify_installer(&INSTALLER[..INSTALLER.len() - 1]),
        Err(UpdateError::InstallerSizeMismatch)
    );
    let mut overlong = Cursor::new(vec![b'x'; INSTALLER.len() + 20]);
    assert_eq!(
        update.verify_installer(&mut overlong),
        Err(UpdateError::InstallerSizeMismatch)
    );
    assert_eq!(overlong.position(), update.installer_size() + 1);

    struct Unreadable;
    impl Read for Unreadable {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        }
    }
    assert_eq!(
        update.verify_installer(Unreadable),
        Err(UpdateError::InstallerRead)
    );
}
