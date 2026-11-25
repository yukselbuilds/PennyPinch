//! Validate signed update metadata and installer bytes before any apply step.

use crate::runtime::manager::{EXPECTED_ENGINE_VERSION, EXPECTED_RUNTIME_MANIFEST_SHA256};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use minisign_verify::{PublicKey, Signature};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Read};

pub(super) const MANIFEST_LIMIT: usize = 32 * 1024;
pub(super) const SIGNATURE_LIMIT: usize = 4 * 1024;
const PUBLIC_KEY_LIMIT: usize = 1024;
const NOTES_LIMIT: usize = 4 * 1024;
const VERSION_LIMIT: usize = 64;
const INSTALLER_LIMIT: u64 = 1024 * 1024 * 1024;
const APPLICATION_LIMIT: u64 = 128 * 1024 * 1024;
pub(super) const UPDATE_STATE_FORMAT: u32 = 1;
const WINDOWS_TARGET: &str = "windows-x86_64";

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UpdateChannel {
    Stable,
    Preview,
}

/// All fields come from the installed app or its explicit channel selection.
pub(crate) struct UpdatePolicy<'a> {
    pub(crate) app_identifier: &'a str,
    pub(crate) installed_version: &'a Version,
    pub(crate) channel: UpdateChannel,
    pub(crate) trusted_tauri_public_key: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UpdateError {
    ManifestTooLarge,
    InvalidTrustKey,
    InvalidSignature,
    InvalidManifest,
    UnsupportedSchema,
    WrongAppOrTarget,
    WrongChannel,
    VersionNotNewer,
    EnginePinMismatch,
    RuntimeResourcesMismatch,
    StateFormatMismatch,
    InstallerRead,
    InstallerSizeMismatch,
    InstallerHashMismatch,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    app_identifier: String,
    target: String,
    channel: UpdateChannel,
    version: String,
    notes: String,
    engine: EnginePin,
    installer: Artifact,
    #[serde(default, deserialize_with = "present")]
    application: Option<Application>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnginePin {
    version: String,
    runtime_manifest_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifact {
    size_bytes: u64,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Application {
    state_format: u32,
    size_bytes: u64,
    sha256: String,
    #[serde(default, deserialize_with = "present")]
    runtime_resources_sha256: Option<String>,
}

fn present<'de, T: Deserialize<'de>, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

pub(super) fn windows_runtime_resources_sha256() -> String {
    macro_rules! runtime_file {
        ($name:literal) => {
            include_bytes!(concat!("../../../runtime/", $name))
        };
    }
    // Keep this order aligned with the Windows staging overlays and pinned manifests.
    let inputs: [&[u8]; 8] = [
        runtime_file!("desktop_controller.py"),
        runtime_file!("desktop_helper.py"),
        runtime_file!("cache_control_shim.py"),
        runtime_file!("optimization_control.py"),
        runtime_file!("optimization-sources.json"),
        runtime_file!("retrieval_bridge.py"),
        runtime_file!("manifest.windows.json"),
        runtime_file!("windows-payload.json"),
    ];
    let mut digest = Sha256::new();
    for input in inputs {
        digest.update(Sha256::digest(input));
    }
    format!("{:x}", digest.finalize())
}

#[derive(Debug)]
pub(crate) struct VerifiedUpdate {
    version: Version,
    channel: UpdateChannel,
    notes: String,
    installer: Artifact,
    application: Option<Artifact>,
    #[cfg(any(windows, test))]
    runtime_resources_bound: bool,
    #[cfg(all(windows, any(test, not(debug_assertions))))]
    signed: Option<(Vec<u8>, String)>,
}

impl VerifiedUpdate {
    pub(crate) fn version(&self) -> &Version {
        &self.version
    }

    pub(crate) fn channel(&self) -> UpdateChannel {
        self.channel
    }

    pub(crate) fn notes(&self) -> &str {
        &self.notes
    }

    pub(crate) fn installer_size(&self) -> u64 {
        self.installer.size_bytes
    }

    pub(crate) fn installer_name(&self) -> String {
        let kind = if self.application.is_some() {
            "update"
        } else {
            "setup"
        };
        format!("Headroom_{}_x64-{kind}.exe", self.version)
    }

    pub(crate) fn verify_installer(&self, installer: impl Read) -> Result<(), UpdateError> {
        self.installer.verify(installer)
    }

    #[cfg(any(windows, test))]
    pub(super) fn application_size(&self) -> Option<u64> {
        self.application.as_ref().map(|file| file.size_bytes)
    }

    #[cfg(any(windows, test))]
    pub(super) fn allows_release_download(&self) -> bool {
        self.application.is_none() || self.runtime_resources_bound
    }

    #[cfg(windows)]
    pub(super) fn application_sha256(&self) -> Option<&str> {
        self.application.as_ref().map(|file| file.sha256.as_str())
    }

    #[cfg(any(windows, test))]
    pub(super) fn verify_application(&self, application: impl Read) -> Result<(), UpdateError> {
        self.application
            .as_ref()
            .ok_or(UpdateError::UnsupportedSchema)?
            .verify(application)
    }

    #[cfg(all(windows, any(test, not(debug_assertions))))]
    pub(super) fn signed(&self) -> Option<(&[u8], &str)> {
        self.signed
            .as_ref()
            .map(|(bytes, sig)| (bytes.as_slice(), sig.as_str()))
    }
}

impl Artifact {
    fn valid(&self, limit: u64) -> bool {
        (1..=limit).contains(&self.size_bytes)
            && self.sha256.len() == 64
            && self
                .sha256
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    }

    fn verify(&self, bytes: impl Read) -> Result<(), UpdateError> {
        let mut hasher = Sha256::new();
        let mut bounded = bytes.take(self.size_bytes + 1);
        let size = io::copy(&mut bounded, &mut hasher).map_err(|_| UpdateError::InstallerRead)?;
        if size != self.size_bytes {
            return Err(UpdateError::InstallerSizeMismatch);
        }
        if format!("{:x}", hasher.finalize()) != self.sha256 {
            return Err(UpdateError::InstallerHashMismatch);
        }
        Ok(())
    }
}

pub(crate) fn verify_manifest(
    bytes: &[u8],
    tauri_signature: &str,
    policy: &UpdatePolicy<'_>,
) -> Result<VerifiedUpdate, UpdateError> {
    if bytes.len() > MANIFEST_LIMIT {
        return Err(UpdateError::ManifestTooLarge);
    }
    let key = decode_tauri_text(policy.trusted_tauri_public_key, PUBLIC_KEY_LIMIT)
        .and_then(|text| PublicKey::decode(&text).ok())
        .ok_or(UpdateError::InvalidTrustKey)?;
    let signature = decode_tauri_text(tauri_signature, SIGNATURE_LIMIT)
        .and_then(|text| Signature::decode(&text).ok())
        .ok_or(UpdateError::InvalidSignature)?;
    key.verify(bytes, &signature, false)
        .map_err(|_| UpdateError::InvalidSignature)?;
    let verified = parse_manifest(bytes, policy)?;
    #[cfg(all(windows, any(test, not(debug_assertions))))]
    let verified = VerifiedUpdate {
        signed: Some((bytes.to_vec(), tauri_signature.to_owned())),
        ..verified
    };
    Ok(verified)
}

fn decode_tauri_text(text: &str, limit: usize) -> Option<String> {
    if text.len() > limit {
        return None;
    }
    let decoded = STANDARD.decode(text.trim()).ok()?;
    String::from_utf8(decoded).ok()
}

fn parse_manifest(bytes: &[u8], policy: &UpdatePolicy<'_>) -> Result<VerifiedUpdate, UpdateError> {
    let manifest: Manifest =
        serde_json::from_slice(bytes).map_err(|_| UpdateError::InvalidManifest)?;
    if manifest.app_identifier != policy.app_identifier || manifest.target != WINDOWS_TARGET {
        return Err(UpdateError::WrongAppOrTarget);
    }
    if manifest.channel != policy.channel {
        return Err(UpdateError::WrongChannel);
    }
    if manifest.version.len() > VERSION_LIMIT
        || manifest.notes.len() > NOTES_LIMIT
        || manifest
            .notes
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\t' | '\n' | '\r'))
        || !manifest.installer.valid(INSTALLER_LIMIT)
    {
        return Err(UpdateError::InvalidManifest);
    }
    let version = Version::parse(&manifest.version).map_err(|_| UpdateError::InvalidManifest)?;
    if !version.build.is_empty() {
        return Err(UpdateError::InvalidManifest);
    }
    if version.pre.is_empty() != (manifest.channel == UpdateChannel::Stable) {
        return Err(UpdateError::WrongChannel);
    }
    let precedence = version.cmp_precedence(policy.installed_version);
    // A signed older release can require the engine resources that shipped with it.
    let older = precedence.is_lt();
    let application = match (manifest.schema_version, manifest.application) {
        (1, None) => None,
        (2 | 3, Some(application)) => {
            match (
                manifest.schema_version,
                application.runtime_resources_sha256.as_deref(),
            ) {
                (2, None) => {}
                (3, Some(pin)) if older || pin == windows_runtime_resources_sha256() => {}
                (3, Some(_)) => return Err(UpdateError::RuntimeResourcesMismatch),
                _ => return Err(UpdateError::InvalidManifest),
            }
            if !older && application.state_format != UPDATE_STATE_FORMAT {
                return Err(UpdateError::StateFormatMismatch);
            }
            let artifact = Artifact {
                size_bytes: application.size_bytes,
                sha256: application.sha256,
            };
            if !artifact.valid(APPLICATION_LIMIT) {
                return Err(UpdateError::InvalidManifest);
            }
            Some(artifact)
        }
        (1..=3, _) => return Err(UpdateError::InvalidManifest),
        _ => return Err(UpdateError::UnsupportedSchema),
    };
    if !older
        && (manifest.engine.version != EXPECTED_ENGINE_VERSION
            || manifest.engine.runtime_manifest_sha256 != EXPECTED_RUNTIME_MANIFEST_SHA256)
    {
        return Err(UpdateError::EnginePinMismatch);
    }
    if !precedence.is_gt() {
        return Err(UpdateError::VersionNotNewer);
    }
    Ok(VerifiedUpdate {
        version,
        channel: manifest.channel,
        notes: manifest.notes,
        installer: manifest.installer,
        application,
        #[cfg(any(windows, test))]
        runtime_resources_bound: manifest.schema_version == 3,
        #[cfg(all(windows, any(test, not(debug_assertions))))]
        signed: None,
    })
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
