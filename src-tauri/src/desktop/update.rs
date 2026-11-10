//! Validate signed update metadata and installer bytes before any apply step.

use crate::runtime::manager::{EXPECTED_ENGINE_VERSION, EXPECTED_RUNTIME_MANIFEST_SHA256};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use minisign_verify::{PublicKey, Signature};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{self, Read};

const MANIFEST_LIMIT: usize = 32 * 1024;
const SIGNATURE_LIMIT: usize = 4 * 1024;
const PUBLIC_KEY_LIMIT: usize = 1024;
const NOTES_LIMIT: usize = 4 * 1024;
const VERSION_LIMIT: usize = 64;
const INSTALLER_LIMIT: u64 = 1024 * 1024 * 1024;
const WINDOWS_TARGET: &str = "windows-x86_64";

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
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
    installer: Installer,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnginePin {
    version: String,
    runtime_manifest_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Installer {
    size_bytes: u64,
    sha256: String,
}

#[derive(Debug)]
pub(crate) struct VerifiedUpdate {
    version: Version,
    channel: UpdateChannel,
    notes: String,
    installer: Installer,
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
        format!("Headroom_{}_x64-setup.exe", self.version)
    }

    pub(crate) fn verify_installer(&self, installer: impl Read) -> Result<(), UpdateError> {
        let mut hasher = Sha256::new();
        let mut bounded = installer.take(self.installer.size_bytes + 1);
        let size = io::copy(&mut bounded, &mut hasher).map_err(|_| UpdateError::InstallerRead)?;
        if size != self.installer.size_bytes {
            return Err(UpdateError::InstallerSizeMismatch);
        }
        if format!("{:x}", hasher.finalize()) != self.installer.sha256 {
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
    parse_manifest(bytes, policy)
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
    if manifest.schema_version != 1 {
        return Err(UpdateError::UnsupportedSchema);
    }
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
        || !(1..=INSTALLER_LIMIT).contains(&manifest.installer.size_bytes)
        || manifest.installer.sha256.len() != 64
        || !manifest
            .installer
            .sha256
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
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
    if !version.cmp_precedence(policy.installed_version).is_gt() {
        return Err(UpdateError::VersionNotNewer);
    }
    if manifest.engine.version != EXPECTED_ENGINE_VERSION
        || manifest.engine.runtime_manifest_sha256 != EXPECTED_RUNTIME_MANIFEST_SHA256
    {
        return Err(UpdateError::EnginePinMismatch);
    }
    Ok(VerifiedUpdate {
        version,
        channel: manifest.channel,
        notes: manifest.notes,
        installer: manifest.installer,
    })
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
