use crate::integrations::plan::{ClientKind, ConfigPresence, ConfigScope, ConfigSource};
use crate::integrations::transaction::{OpenCodeRouteReceipt, PatchReceipt, PatchReceiptMetadata};
use crate::integrations::transaction_fs::{
    FileSnapshot, FileToken, FileTransaction, TransactionFsError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

const SCHEMA_VERSION: u8 = 1;
const OWNER_TAG: &str = "headroom.connection_ownership.v1";
const SCOPE: &str = "user_config";
const MAX_RECORD_BYTES: usize = 16 * 1024;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnershipState {
    Active,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnershipError {
    Invalid,
    Conflict,
    UnsafePath,
    TooLarge,
    Unsupported,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredReceipt {
    client_kind: String,
    config_source: String,
    endpoint_port: u16,
    created_ancestor_mask: u8,
    config_was_missing: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    opencode_routes: Option<Vec<OpenCodeRouteReceipt>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredOwnership {
    schema_version: u8,
    owner_tag: String,
    state: String,
    owner_id: String,
    endpoint_id: String,
    client_kind: String,
    scope: String,
    config_source: String,
    config_presence: String,
    config_path_binding_sha256: String,
    endpoint_host: String,
    endpoint_port: u16,
    revision: u64,
    updated_at_unix_seconds: u64,
    before_config_sha256: String,
    after_config_sha256: String,
    patch_receipt: StoredReceipt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnershipRecord {
    stored: StoredOwnership,
}

impl OwnershipRecord {
    pub(crate) fn active_from_patch(
        owner_id: &str,
        endpoint_id: &str,
        config_path: &Path,
        revision: u64,
        updated_at_unix_seconds: u64,
        before_config: &[u8],
        after_config: &[u8],
        receipt: &PatchReceipt,
    ) -> Result<Self, OwnershipError> {
        Self::active_from_scoped_patch(
            owner_id,
            endpoint_id,
            config_path,
            revision,
            updated_at_unix_seconds,
            before_config,
            after_config,
            receipt,
            ConfigScope::UserConfig,
        )
    }

    pub(crate) fn active_from_scoped_patch(
        owner_id: &str,
        endpoint_id: &str,
        config_path: &Path,
        revision: u64,
        updated_at_unix_seconds: u64,
        before_config: &[u8],
        after_config: &[u8],
        receipt: &PatchReceipt,
        scope: ConfigScope,
    ) -> Result<Self, OwnershipError> {
        let binding = config_path_binding(config_path)?;
        Self::active_from_scoped_patch_with_binding(
            owner_id,
            endpoint_id,
            revision,
            updated_at_unix_seconds,
            before_config,
            after_config,
            receipt,
            scope,
            &binding,
        )
    }

    pub(crate) fn active_from_scoped_patch_with_binding(
        owner_id: &str,
        endpoint_id: &str,
        revision: u64,
        updated_at_unix_seconds: u64,
        before_config: &[u8],
        after_config: &[u8],
        receipt: &PatchReceipt,
        scope: ConfigScope,
        binding: &str,
    ) -> Result<Self, OwnershipError> {
        let metadata = receipt.metadata();
        if !scope_matches_client(metadata.client_kind, scope) || !valid_hash(binding) {
            return Err(OwnershipError::Invalid);
        }
        let stored = StoredOwnership {
            schema_version: SCHEMA_VERSION,
            owner_tag: OWNER_TAG.to_owned(),
            state: "active".to_owned(),
            owner_id: owner_id.to_owned(),
            endpoint_id: endpoint_id.to_owned(),
            client_kind: client_kind_code(metadata.client_kind).to_owned(),
            scope: scope_code(scope).to_owned(),
            config_source: config_source_code(metadata.config_source).to_owned(),
            config_presence: if metadata.config_was_missing {
                "missing".to_owned()
            } else {
                "present".to_owned()
            },
            config_path_binding_sha256: binding.to_owned(),
            endpoint_host: "127.0.0.1".to_owned(),
            endpoint_port: metadata.endpoint_port,
            revision,
            updated_at_unix_seconds,
            before_config_sha256: content_hash(before_config),
            after_config_sha256: content_hash(after_config),
            patch_receipt: StoredReceipt::from_metadata(metadata),
        };
        let record = Self { stored };
        record.validate()?;
        Ok(record)
    }

    pub(crate) fn released(&self, updated_at_unix_seconds: u64) -> Result<Self, OwnershipError> {
        if self.state() != OwnershipState::Active
            || self.stored.revision >= MAX_SAFE_INTEGER
            || updated_at_unix_seconds > MAX_SAFE_INTEGER
            || updated_at_unix_seconds < self.stored.updated_at_unix_seconds
        {
            return Err(OwnershipError::Conflict);
        }
        let mut stored = self.stored.clone();
        stored.state = "released".to_owned();
        stored.revision += 1;
        stored.updated_at_unix_seconds = updated_at_unix_seconds;
        let record = Self { stored };
        record.validate()?;
        Ok(record)
    }

    pub(crate) fn state(&self) -> OwnershipState {
        if self.stored.state == "active" {
            OwnershipState::Active
        } else {
            OwnershipState::Released
        }
    }

    pub(crate) fn owner_id(&self) -> &str {
        &self.stored.owner_id
    }

    pub(crate) fn endpoint_id(&self) -> &str {
        &self.stored.endpoint_id
    }

    pub(crate) fn client_kind(&self) -> Option<ClientKind> {
        parse_client_kind(&self.stored.client_kind)
    }

    pub(crate) fn config_source(&self) -> Option<ConfigSource> {
        parse_config_source(&self.stored.config_source)
    }

    pub(crate) fn config_scope(&self) -> Option<ConfigScope> {
        parse_scope(&self.stored.scope)
    }

    pub(crate) fn endpoint_port(&self) -> u16 {
        self.stored.endpoint_port
    }

    pub(crate) fn revision(&self) -> u64 {
        self.stored.revision
    }

    pub(crate) fn config_path_binding_sha256(&self) -> &str {
        &self.stored.config_path_binding_sha256
    }

    pub(crate) fn config_was_missing(&self) -> bool {
        self.stored.patch_receipt.config_was_missing
    }

    pub(crate) fn patch_receipt(&self) -> Result<PatchReceipt, OwnershipError> {
        self.stored.patch_receipt.to_receipt()
    }

    pub(crate) fn encoded_fingerprint(&self) -> Result<String, OwnershipError> {
        Ok(content_hash(&self.encode()?))
    }

    fn encode(&self) -> Result<Vec<u8>, OwnershipError> {
        self.validate()?;
        let bytes = serde_json::to_vec(&self.stored).map_err(|_| OwnershipError::Invalid)?;
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(OwnershipError::TooLarge);
        }
        Ok(bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self, OwnershipError> {
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(OwnershipError::TooLarge);
        }
        let stored: StoredOwnership =
            serde_json::from_slice(bytes).map_err(|_| OwnershipError::Invalid)?;
        let record = Self { stored };
        record.validate()?;
        Ok(record)
    }

    fn validate(&self) -> Result<(), OwnershipError> {
        let stored = &self.stored;
        let kind = parse_client_kind(&stored.client_kind).ok_or(OwnershipError::Invalid)?;
        let scope = parse_scope(&stored.scope).ok_or(OwnershipError::Invalid)?;
        let source = parse_config_source(&stored.config_source).ok_or(OwnershipError::Invalid)?;
        let presence = parse_presence(&stored.config_presence).ok_or(OwnershipError::Invalid)?;
        let state = match stored.state.as_str() {
            "active" => OwnershipState::Active,
            "released" => OwnershipState::Released,
            _ => return Err(OwnershipError::Invalid),
        };
        let receipt = stored.patch_receipt.to_receipt()?;
        let receipt_metadata = receipt.metadata();
        if stored.schema_version != SCHEMA_VERSION
            || stored.owner_tag != OWNER_TAG
            || !canonical_uuid(&stored.owner_id)
            || !canonical_uuid(&stored.endpoint_id)
            || !scope_matches_client(kind, scope)
            || !source_matches(kind, source, presence)
            || stored.endpoint_host != "127.0.0.1"
            || stored.endpoint_port == 0
            || stored.revision == 0
            || stored.revision > MAX_SAFE_INTEGER
            || (state == OwnershipState::Released && stored.revision < 2)
            || stored.updated_at_unix_seconds > MAX_SAFE_INTEGER
            || !valid_hash(&stored.config_path_binding_sha256)
            || !valid_hash(&stored.before_config_sha256)
            || !valid_hash(&stored.after_config_sha256)
            || receipt_metadata.client_kind != kind
            || receipt_metadata.config_source != source
            || receipt_metadata.endpoint_port != stored.endpoint_port
            || receipt_metadata.config_was_missing != (presence == ConfigPresence::Missing)
        {
            return Err(OwnershipError::Invalid);
        }
        Ok(())
    }
}

impl StoredReceipt {
    fn from_metadata(metadata: PatchReceiptMetadata) -> Self {
        Self {
            client_kind: client_kind_code(metadata.client_kind).to_owned(),
            config_source: config_source_code(metadata.config_source).to_owned(),
            endpoint_port: metadata.endpoint_port,
            created_ancestor_mask: metadata.created_ancestor_mask,
            config_was_missing: metadata.config_was_missing,
            opencode_routes: metadata.opencode_routes,
        }
    }

    fn to_receipt(&self) -> Result<PatchReceipt, OwnershipError> {
        let client_kind = parse_client_kind(&self.client_kind).ok_or(OwnershipError::Invalid)?;
        let config_source =
            parse_config_source(&self.config_source).ok_or(OwnershipError::Invalid)?;
        PatchReceipt::from_metadata(PatchReceiptMetadata {
            client_kind,
            config_source,
            endpoint_port: self.endpoint_port,
            created_ancestor_mask: self.created_ancestor_mask,
            config_was_missing: self.config_was_missing,
            opencode_routes: self.opencode_routes.clone(),
        })
        .map_err(|_| OwnershipError::Invalid)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct OwnershipRead {
    kind: ClientKind,
    binding: String,
    slot_path: PathBuf,
    current: Option<OwnershipRecord>,
    token: Option<FileToken>,
    record_fingerprint: Option<String>,
}

impl OwnershipRead {
    pub(crate) fn current(&self) -> Option<&OwnershipRecord> {
        self.current.as_ref()
    }

    pub(crate) fn record_fingerprint(&self) -> Option<&str> {
        self.record_fingerprint.as_deref()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct OwnershipSnapshot {
    pub(crate) record: OwnershipRecord,
    pub(crate) record_fingerprint: String,
}

#[derive(Clone, Debug)]
pub(crate) struct OwnershipStore {
    boundary: PathBuf,
    relative_root: PathBuf,
}

impl OwnershipStore {
    #[cfg(target_os = "macos")]
    pub(crate) fn retained_versions_root(&self) -> Result<Option<PathBuf>, OwnershipError> {
        self.verify_root(false)
    }
    /// Construct without creating directories or files. `boundary` must exist;
    /// only `relative_root` may be created later by `prepare_for_apply`.
    pub(crate) fn new(
        boundary: impl Into<PathBuf>,
        relative_root: impl Into<PathBuf>,
    ) -> Result<Self, OwnershipError> {
        let boundary = validate_boundary(&boundary.into())?;
        let relative_root = normalize_relative_root(&relative_root.into())?;
        let store = Self {
            boundary,
            relative_root,
        };
        store.verify_root(false)?;
        Ok(store)
    }

    /// Create only the bounded application-owned relative root under the
    /// trusted state boundary. Call only after a real patch is planned, before
    /// opening the client/ownership FileTransaction; a same-endpoint no-op
    /// must not call it.
    pub(crate) fn prepare_for_apply(&self) -> Result<(), OwnershipError> {
        self.prepare_for_apply_inner(|_| {})
    }

    #[cfg(test)]
    pub(super) fn prepare_for_apply_with_hook(
        &self,
        before_create: impl FnMut(&Path),
    ) -> Result<(), OwnershipError> {
        self.prepare_for_apply_inner(before_create)
    }

    fn prepare_for_apply_inner(
        &self,
        mut before_create: impl FnMut(&Path),
    ) -> Result<(), OwnershipError> {
        self.verify_boundary()?;
        let mut pinned = vec![
            crate::integrations::config_io::pin_existing_directory_chain(&self.boundary)
                .map_err(map_config_io_error)?,
        ];
        let mut current = self.boundary.clone();
        for component in self.relative_root.components() {
            let Component::Normal(name) = component else {
                return Err(OwnershipError::UnsafePath);
            };
            current.push(name);
            match fs::symlink_metadata(&current) {
                Ok(metadata) => verify_directory(&current, &metadata, &self.boundary)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    for chain in &pinned {
                        chain.validate().map_err(map_config_io_error)?;
                    }
                    self.verify_boundary()?;
                    self.verify_existing_parent(&current)?;
                    before_create(&current);
                    for chain in &pinned {
                        chain.validate().map_err(map_config_io_error)?;
                    }
                    self.verify_boundary()?;
                    #[cfg(target_os = "macos")]
                    let creation = {
                        use std::os::unix::fs::DirBuilderExt;
                        fs::DirBuilder::new().mode(0o700).create(&current)
                    };
                    #[cfg(not(target_os = "macos"))]
                    let creation = fs::create_dir(&current);
                    match creation {
                        Ok(()) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                        Err(_) => return Err(OwnershipError::Unavailable),
                    }
                    let metadata =
                        fs::symlink_metadata(&current).map_err(|_| OwnershipError::UnsafePath)?;
                    verify_directory(&current, &metadata, &self.boundary)?;
                }
                Err(_) => return Err(OwnershipError::UnsafePath),
            }
            pinned.push(
                crate::integrations::config_io::pin_existing_directory_chain(&current)
                    .map_err(map_config_io_error)?,
            );
            for chain in &pinned {
                chain.validate().map_err(map_config_io_error)?;
            }
        }
        if self.verify_root(false)?.is_none() {
            return Err(OwnershipError::Unavailable);
        }
        Ok(())
    }

    /// Read a fixed client slot through the caller's held transaction. A
    /// missing store/slot is passive and never creates a directory or file.
    pub(crate) fn read_in_transaction(
        &self,
        transaction: &mut FileTransaction,
        kind: ClientKind,
        config_path_binding_sha256: &str,
    ) -> Result<OwnershipRead, OwnershipError> {
        if !valid_hash(config_path_binding_sha256) {
            return Err(OwnershipError::Invalid);
        }
        let binding = config_path_binding_sha256.to_owned();
        let root = self.verify_root(false)?;
        let slot_path = root
            .unwrap_or_else(|| self.boundary.join(&self.relative_root))
            .join(slot_filename(kind));
        #[cfg(target_os = "macos")]
        transaction
            .bind_ownership_path(&slot_path, Some(config_path_binding_sha256))
            .map_err(map_transaction_error)?;
        let metadata = match fs::symlink_metadata(&slot_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(OwnershipRead {
                    kind,
                    binding,
                    slot_path,
                    current: None,
                    token: None,
                    record_fingerprint: None,
                })
            }
            Err(_) => return Err(OwnershipError::UnsafePath),
        };
        if !metadata.is_file() || is_reparse(&metadata) {
            return Err(OwnershipError::UnsafePath);
        }
        #[cfg(any(windows, target_os = "macos"))]
        {
            let (token, FileSnapshot { bytes, .. }) = transaction
                .open_existing(&slot_path, MAX_RECORD_BYTES)
                .map_err(map_transaction_error)?;
            let current = OwnershipRecord::decode(&bytes)?;
            if current.client_kind() != Some(kind) {
                return Err(OwnershipError::Invalid);
            }
            if current.state() == OwnershipState::Active
                && current.config_path_binding_sha256() != binding
            {
                return Err(OwnershipError::Conflict);
            }
            Ok(OwnershipRead {
                kind,
                binding,
                slot_path,
                current: Some(current),
                token: Some(token),
                record_fingerprint: Some(content_hash(&bytes)),
            })
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = transaction;
            Err(OwnershipError::Unsupported)
        }
    }

    /// Read the bounded, fixed client slot without creating the app-owned root.
    /// The caller owns the transaction and must roll it back after this snapshot.
    pub(crate) fn read_snapshot_in_transaction(
        &self,
        transaction: &mut FileTransaction,
        kind: ClientKind,
    ) -> Result<Option<OwnershipSnapshot>, OwnershipError> {
        let Some(root) = self.verify_root(false)? else {
            return Ok(None);
        };
        let slot_path = root.join(slot_filename(kind));
        #[cfg(target_os = "macos")]
        transaction
            .bind_ownership_path(&slot_path, None)
            .map_err(map_transaction_error)?;
        let metadata = match fs::symlink_metadata(&slot_path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(OwnershipError::UnsafePath),
        };
        if !metadata.is_file() || is_reparse(&metadata) {
            return Err(OwnershipError::UnsafePath);
        }
        #[cfg(any(windows, target_os = "macos"))]
        {
            let (_, FileSnapshot { bytes, .. }) = transaction
                .open_existing(&slot_path, MAX_RECORD_BYTES)
                .map_err(map_transaction_error)?;
            let record = OwnershipRecord::decode(&bytes)?;
            if record.client_kind() != Some(kind) {
                return Err(OwnershipError::Invalid);
            }
            Ok(Some(OwnershipSnapshot {
                record,
                record_fingerprint: content_hash(&bytes),
            }))
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = transaction;
            Err(OwnershipError::Unsupported)
        }
    }

    /// Stage the ownership record in the same transaction as its config file.
    /// The caller retains responsibility for sealing/committing/rolling back.
    pub(crate) fn stage_in_transaction(
        &self,
        transaction: &mut FileTransaction,
        current: OwnershipRead,
        next: &OwnershipRecord,
    ) -> Result<(), OwnershipError> {
        next.validate()?;
        if current.kind != next.client_kind().ok_or(OwnershipError::Invalid)? {
            return Err(OwnershipError::Invalid);
        }
        if current.binding != next.config_path_binding_sha256() {
            return Err(OwnershipError::Conflict);
        }
        if !same_path(&current.slot_path, &self.slot_path(current.kind)) {
            return Err(OwnershipError::UnsafePath);
        }
        if self.verify_root(false)?.is_none() {
            return Err(OwnershipError::Unavailable);
        }
        let bytes = next.encode()?;
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(OwnershipError::TooLarge);
        }
        let token = match current.current.as_ref() {
            None => {
                if next.state() != OwnershipState::Active || next.revision() != 1 {
                    return Err(OwnershipError::Conflict);
                }
                #[cfg(any(windows, target_os = "macos"))]
                {
                    let (token, _) = transaction
                        .create_missing(&current.slot_path, MAX_RECORD_BYTES)
                        .map_err(map_transaction_error)?;
                    token
                }
                #[cfg(not(any(windows, target_os = "macos")))]
                return Err(OwnershipError::Unsupported);
            }
            Some(previous) => {
                if !valid_transition(previous, next) {
                    return Err(OwnershipError::Conflict);
                }
                current.token.ok_or(OwnershipError::Conflict)?
            }
        };
        #[cfg(any(windows, target_os = "macos"))]
        {
            transaction
                .replace_contents(token, &bytes)
                .map_err(map_transaction_error)
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = (transaction, token, bytes);
            Err(OwnershipError::Unsupported)
        }
    }

    fn verify_root(&self, create: bool) -> Result<Option<PathBuf>, OwnershipError> {
        self.verify_boundary()?;
        let mut current = self.boundary.clone();
        for component in self.relative_root.components() {
            let Component::Normal(name) = component else {
                return Err(OwnershipError::UnsafePath);
            };
            current.push(name);
            match fs::symlink_metadata(&current) {
                Ok(metadata) => verify_directory(&current, &metadata, &self.boundary)?,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound && !create => {
                    return Ok(None)
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    return Err(OwnershipError::Unavailable)
                }
                Err(_) => return Err(OwnershipError::UnsafePath),
            }
        }
        Ok(Some(current))
    }

    fn verify_boundary(&self) -> Result<(), OwnershipError> {
        let current = validate_boundary(&self.boundary)?;
        if !same_path(&current, &self.boundary) {
            return Err(OwnershipError::UnsafePath);
        }
        Ok(())
    }

    fn verify_existing_parent(&self, child: &Path) -> Result<(), OwnershipError> {
        let parent = child.parent().ok_or(OwnershipError::UnsafePath)?;
        let metadata = fs::symlink_metadata(parent).map_err(|_| OwnershipError::UnsafePath)?;
        verify_directory(parent, &metadata, &self.boundary)
    }

    fn slot_path(&self, kind: ClientKind) -> PathBuf {
        self.boundary
            .join(&self.relative_root)
            .join(slot_filename(kind))
    }

    #[cfg(test)]
    pub(super) fn file_path_for_test(&self, kind: ClientKind) -> PathBuf {
        self.slot_path(kind)
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn validate_macos_publication(
    config_path: &Path,
    owner_filename: &str,
    previous_owner: Option<&[u8]>,
    next_owner: &[u8],
    before_config: Option<&[u8]>,
    after_config: &[u8],
) -> Result<(), OwnershipError> {
    let next = OwnershipRecord::decode(next_owner)?;
    let previous = previous_owner.map(OwnershipRecord::decode).transpose()?;
    let kind = validate_macos_owner_target(config_path, owner_filename, &next)?;
    if !previous.as_ref().map_or(
        next.state() == OwnershipState::Active && next.revision() == 1,
        |previous| valid_transition(previous, &next),
    ) {
        return Err(OwnershipError::Conflict);
    }
    let empty: &[u8] = if kind == ClientKind::Codex {
        b""
    } else {
        b"{}"
    };
    let before = before_config.unwrap_or(empty);
    match next.state() {
        OwnershipState::Active => {
            if next.config_was_missing() != before_config.is_none()
                || next.stored.before_config_sha256 != content_hash(before)
                || next.stored.after_config_sha256 != content_hash(after_config)
                || crate::integrations::transaction::undo_owned_endpoint(
                    &next.patch_receipt()?,
                    after_config,
                )
                .is_err()
            {
                return Err(OwnershipError::Conflict);
            }
        }
        OwnershipState::Released => {
            if before_config.is_none()
                || !crate::integrations::transaction::undo_owned_endpoint(
                    &next.patch_receipt()?,
                    before,
                )
                .is_ok_and(|bytes| bytes == after_config)
            {
                return Err(OwnershipError::Conflict);
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn validate_macos_owner_target(
    config_path: &Path,
    owner_filename: &str,
    next: &OwnershipRecord,
) -> Result<ClientKind, OwnershipError> {
    let kind = next.client_kind().ok_or(OwnershipError::Invalid)?;
    let expected_leaf = match (next.config_source(), next.config_scope()) {
        (Some(ConfigSource::CodexToml), Some(ConfigScope::UserConfig)) => "config.toml",
        (Some(ConfigSource::ClaudeJson), Some(ConfigScope::UserConfig)) => "settings.json",
        (Some(ConfigSource::ClaudeJson), Some(ConfigScope::ProjectLocalConfig)) => {
            "settings.local.json"
        }
        (Some(ConfigSource::OpenCodeJson), Some(ConfigScope::UserConfig)) => "opencode.json",
        (Some(ConfigSource::OpenCodeJsonc), Some(ConfigScope::UserConfig)) => "opencode.jsonc",
        _ => return Err(OwnershipError::Invalid),
    };
    let binding = if next.config_source() == Some(ConfigSource::ClaudeJson)
        && next.config_scope() == Some(ConfigScope::ProjectLocalConfig)
    {
        config_path_binding_for_claude_project_target(
            config_path
                .parent()
                .and_then(Path::parent)
                .ok_or(OwnershipError::UnsafePath)?,
            config_path,
        )?
    } else {
        config_path_binding(config_path)?
    };
    if owner_filename != slot_filename(kind)
        || config_path.file_name().and_then(|value| value.to_str()) != Some(expected_leaf)
        || binding != next.config_path_binding_sha256()
    {
        return Err(OwnershipError::Conflict);
    }
    Ok(kind)
}

#[cfg(target_os = "macos")]
pub(crate) fn validate_macos_ownership_bytes(bytes: &[u8]) -> Result<(), OwnershipError> {
    OwnershipRecord::decode(bytes).map(|_| ())
}

#[cfg(target_os = "macos")]
pub(crate) fn validate_macos_retained_adoption(
    config_path: &Path,
    owner_filename: &str,
    previous_owner: Option<&[u8]>,
    next_owner: &[u8],
    current: &[u8],
) -> Result<(), OwnershipError> {
    let next = OwnershipRecord::decode(next_owner)?;
    let previous = previous_owner.map(OwnershipRecord::decode).transpose()?;
    let kind = validate_macos_owner_target(config_path, owner_filename, &next)?;
    if !previous.as_ref().map_or(
        next.state() == OwnershipState::Active && next.revision() == 1,
        |previous| previous_owner == Some(next_owner) || valid_transition(previous, &next),
    ) {
        return Err(OwnershipError::Conflict);
    }
    crate::integrations::config_document::parse_document(kind, current)
        .map_err(|_| OwnershipError::Conflict)?;
    match next.state() {
        OwnershipState::Active => {
            if crate::integrations::transaction::undo_owned_endpoint(
                &next.patch_receipt()?,
                current,
            )
            .is_err()
            {
                return Err(OwnershipError::Conflict);
            }
        }
        OwnershipState::Released => {}
    }
    Ok(())
}

fn valid_transition(previous: &OwnershipRecord, next: &OwnershipRecord) -> bool {
    let old = &previous.stored;
    let new = &next.stored;
    if old.client_kind != new.client_kind
        || new.revision != old.revision.saturating_add(1)
        || old.revision >= MAX_SAFE_INTEGER
    {
        return false;
    }
    match (previous.state(), next.state()) {
        (OwnershipState::Active, OwnershipState::Active) => same_owner_and_target(old, new),
        (OwnershipState::Active, OwnershipState::Released) => {
            same_owner_and_target(old, new)
                && old.patch_receipt == new.patch_receipt
                && old.before_config_sha256 == new.before_config_sha256
                && old.after_config_sha256 == new.after_config_sha256
        }
        (OwnershipState::Released, OwnershipState::Active) => true,
        (OwnershipState::Released, OwnershipState::Released) => false,
    }
}

fn same_owner_and_target(old: &StoredOwnership, new: &StoredOwnership) -> bool {
    old.owner_id == new.owner_id
        && old.endpoint_id == new.endpoint_id
        && old.config_path_binding_sha256 == new.config_path_binding_sha256
        && old.endpoint_host == new.endpoint_host
        && old.endpoint_port == new.endpoint_port
        && old.config_source == new.config_source
        && old.scope == new.scope
}

fn scope_code(scope: ConfigScope) -> &'static str {
    match scope {
        ConfigScope::UserConfig => SCOPE,
        ConfigScope::ProjectLocalConfig => "project_local_config",
    }
}

fn parse_scope(value: &str) -> Option<ConfigScope> {
    match value {
        SCOPE => Some(ConfigScope::UserConfig),
        "project_local_config" => Some(ConfigScope::ProjectLocalConfig),
        _ => None,
    }
}

fn scope_matches_client(kind: ClientKind, scope: ConfigScope) -> bool {
    matches!(
        (kind, scope),
        (ClientKind::Codex, ConfigScope::UserConfig)
            | (ClientKind::ClaudeCode, ConfigScope::UserConfig)
            | (ClientKind::ClaudeCode, ConfigScope::ProjectLocalConfig)
            | (ClientKind::OpenCodeV1, ConfigScope::UserConfig)
    )
}

fn source_matches(kind: ClientKind, source: ConfigSource, presence: ConfigPresence) -> bool {
    match (kind, source, presence) {
        (ClientKind::Codex, ConfigSource::CodexToml, _) => true,
        (ClientKind::ClaudeCode, ConfigSource::ClaudeJson, _) => true,
        (ClientKind::OpenCodeV1, ConfigSource::OpenCodeMissing, ConfigPresence::Missing) => true,
        (ClientKind::OpenCodeV1, ConfigSource::OpenCodeJson, ConfigPresence::Present)
        | (ClientKind::OpenCodeV1, ConfigSource::OpenCodeJsonc, ConfigPresence::Present) => true,
        _ => false,
    }
}

fn client_kind_code(kind: ClientKind) -> &'static str {
    match kind {
        ClientKind::Codex => "codex",
        ClientKind::ClaudeCode => "claude_code",
        ClientKind::OpenCodeV1 => "opencode_v1",
    }
}

fn parse_client_kind(value: &str) -> Option<ClientKind> {
    match value {
        "codex" => Some(ClientKind::Codex),
        "claude_code" => Some(ClientKind::ClaudeCode),
        "opencode_v1" => Some(ClientKind::OpenCodeV1),
        _ => None,
    }
}

fn config_source_code(source: ConfigSource) -> &'static str {
    match source {
        ConfigSource::CodexToml => "codex_toml",
        ConfigSource::ClaudeJson => "claude_json",
        ConfigSource::OpenCodeJson => "opencode_json",
        ConfigSource::OpenCodeJsonc => "opencode_jsonc",
        ConfigSource::OpenCodeMissing => "opencode_missing",
    }
}

fn parse_config_source(value: &str) -> Option<ConfigSource> {
    match value {
        "codex_toml" => Some(ConfigSource::CodexToml),
        "claude_json" => Some(ConfigSource::ClaudeJson),
        "opencode_json" => Some(ConfigSource::OpenCodeJson),
        "opencode_jsonc" => Some(ConfigSource::OpenCodeJsonc),
        "opencode_missing" => Some(ConfigSource::OpenCodeMissing),
        _ => None,
    }
}

fn parse_presence(value: &str) -> Option<ConfigPresence> {
    match value {
        "present" => Some(ConfigPresence::Present),
        "missing" => Some(ConfigPresence::Missing),
        _ => None,
    }
}

fn canonical_uuid(value: &str) -> bool {
    Uuid::parse_str(value)
        .map(|uuid| uuid.hyphenated().to_string() == value)
        .unwrap_or(false)
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn content_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn config_path_binding(path: &Path) -> Result<String, OwnershipError> {
    let absolute = make_absolute(path)?;
    validate_absolute_local_path(&absolute)?;
    let parent = absolute.parent().ok_or(OwnershipError::UnsafePath)?;
    validate_existing_directory_chain(parent)?;
    let filename = absolute.file_name().ok_or(OwnershipError::UnsafePath)?;
    if filename.is_empty() || filename.to_string_lossy().contains(':') {
        return Err(OwnershipError::UnsafePath);
    }
    let binding_path = match fs::symlink_metadata(&absolute) {
        Ok(metadata) => {
            if !metadata.is_file() || is_reparse(&metadata) {
                return Err(OwnershipError::UnsafePath);
            }
            let canonical = normalize_path(
                &fs::canonicalize(&absolute).map_err(|_| OwnershipError::UnsafePath)?,
            );
            if !same_path(&canonical, &absolute) {
                return Err(OwnershipError::UnsafePath);
            }
            canonical
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => absolute,
        Err(_) => return Err(OwnershipError::UnsafePath),
    };
    hash_config_path_binding(&binding_path)
}

pub(crate) fn config_path_binding_for_claude_project_target(
    project_root: &Path,
    target_path: &Path,
) -> Result<String, OwnershipError> {
    let absolute_root = make_absolute(project_root)?;
    validate_absolute_local_path(&absolute_root)?;
    validate_existing_directory_chain(&absolute_root)?;
    let canonical_root =
        normalize_path(&fs::canonicalize(&absolute_root).map_err(|_| OwnershipError::UnsafePath)?);
    if !same_path(&canonical_root, &absolute_root) {
        return Err(OwnershipError::UnsafePath);
    }

    let expected_target = canonical_root.join(".claude").join("settings.local.json");
    let absolute_target = make_absolute(target_path)?;
    if !same_path(&absolute_target, &expected_target) {
        return Err(OwnershipError::UnsafePath);
    }

    let claude_directory = canonical_root.join(".claude");
    match fs::symlink_metadata(&claude_directory) {
        Ok(metadata) => {
            if !metadata.is_dir() || is_reparse(&metadata) {
                return Err(OwnershipError::UnsafePath);
            }
            config_path_binding(&expected_target)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            hash_config_path_binding(&expected_target)
        }
        Err(_) => Err(OwnershipError::UnsafePath),
    }
}

fn hash_config_path_binding(binding_path: &Path) -> Result<String, OwnershipError> {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStrExt;
        let bytes = binding_path.as_os_str().as_bytes();
        let mut digest = Sha256::new();
        digest.update(b"headroom-ownership-config-path/posix-v1\0");
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
        return Ok(digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect());
    }
    #[cfg(not(target_os = "macos"))]
    {
        let units: Vec<u16> = binding_path.as_os_str().encode_wide_compat();
        let mut digest = Sha256::new();
        digest.update(b"headroom-ownership-config-path/v1\0");
        digest.update((units.len() as u64).to_le_bytes());
        for unit in units {
            digest.update(unit.to_le_bytes());
        }
        Ok(digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }
}

trait EncodePathUnits {
    fn encode_wide_compat(&self) -> Vec<u16>;
}

#[cfg(windows)]
impl EncodePathUnits for std::ffi::OsStr {
    fn encode_wide_compat(&self) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        self.encode_wide().collect()
    }
}

#[cfg(not(windows))]
impl EncodePathUnits for std::ffi::OsStr {
    fn encode_wide_compat(&self) -> Vec<u16> {
        self.to_string_lossy().encode_utf16().collect()
    }
}

fn validate_boundary(path: &Path) -> Result<PathBuf, OwnershipError> {
    let absolute = make_absolute(path)?;
    validate_absolute_local_path(&absolute)?;
    validate_existing_directory_chain(&absolute)?;
    let canonical =
        normalize_path(&fs::canonicalize(&absolute).map_err(|_| OwnershipError::UnsafePath)?);
    if !same_path(&canonical, &absolute) {
        return Err(OwnershipError::UnsafePath);
    }
    Ok(canonical)
}

fn normalize_relative_root(path: &Path) -> Result<PathBuf, OwnershipError> {
    let mut normalized = PathBuf::new();
    let mut count = 0usize;
    for component in path.components() {
        let Component::Normal(part) = component else {
            return Err(OwnershipError::UnsafePath);
        };
        count += 1;
        if part.is_empty() || part.to_string_lossy().contains(':') || count > 16 {
            return Err(OwnershipError::UnsafePath);
        }
        normalized.push(part);
    }
    if count == 0 {
        return Err(OwnershipError::UnsafePath);
    }
    Ok(normalized)
}

fn make_absolute(path: &Path) -> Result<PathBuf, OwnershipError> {
    if path.is_absolute() {
        Ok(normalize_path(path))
    } else {
        Err(OwnershipError::UnsafePath)
    }
}

#[cfg(target_os = "macos")]
fn validate_absolute_local_path(path: &Path) -> Result<(), OwnershipError> {
    crate::integrations::config_io::validate_lexical_path(path).map_err(map_config_io_error)
}

#[cfg(not(target_os = "macos"))]
fn validate_absolute_local_path(path: &Path) -> Result<(), OwnershipError> {
    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err(OwnershipError::UnsafePath);
    };
    #[cfg(windows)]
    let local_prefix = matches!(
        prefix.kind(),
        std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)
    );
    #[cfg(not(windows))]
    let local_prefix = !matches!(prefix.kind(), std::path::Prefix::UNC(_, _));
    if !local_prefix || !matches!(components.next(), Some(Component::RootDir)) {
        return Err(OwnershipError::UnsafePath);
    }
    for component in components {
        match component {
            Component::Normal(part) if !part.to_string_lossy().contains(':') => {}
            _ => return Err(OwnershipError::UnsafePath),
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn validate_existing_directory_chain(path: &Path) -> Result<(), OwnershipError> {
    crate::integrations::config_io::pin_existing_directory_chain(path)
        .and_then(|chain| chain.validate())
        .map_err(map_config_io_error)
}

#[cfg(not(target_os = "macos"))]
fn validate_existing_directory_chain(path: &Path) -> Result<(), OwnershipError> {
    validate_absolute_local_path(path)?;
    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return Err(OwnershipError::UnsafePath);
    };
    let mut current = PathBuf::from(prefix.as_os_str());
    current.push("\\");
    if let Ok(metadata) = fs::symlink_metadata(&current) {
        verify_directory(&current, &metadata, &current)?;
    }
    for component in components {
        let Component::Normal(name) = component else {
            continue;
        };
        current.push(name);
        let metadata = fs::symlink_metadata(&current).map_err(|_| OwnershipError::UnsafePath)?;
        if is_reparse(&metadata) || !metadata.is_dir() {
            return Err(OwnershipError::UnsafePath);
        }
        let canonical =
            normalize_path(&fs::canonicalize(&current).map_err(|_| OwnershipError::UnsafePath)?);
        if !same_path(&canonical, &current) {
            return Err(OwnershipError::UnsafePath);
        }
    }
    Ok(())
}

fn verify_directory(
    path: &Path,
    metadata: &fs::Metadata,
    boundary: &Path,
) -> Result<(), OwnershipError> {
    if !metadata.is_dir() || is_reparse(metadata) {
        return Err(OwnershipError::UnsafePath);
    }
    let canonical =
        normalize_path(&fs::canonicalize(path).map_err(|_| OwnershipError::UnsafePath)?);
    if !same_path(path, &canonical) || !within_boundary(&canonical, boundary) {
        return Err(OwnershipError::UnsafePath);
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn within_boundary(path: &Path, boundary: &Path) -> bool {
    let child = path_key(path);
    let parent = path_key(boundary);
    child == parent || child.starts_with(&(parent + "\\"))
}

#[cfg(target_os = "macos")]
fn within_boundary(path: &Path, boundary: &Path) -> bool {
    path.starts_with(boundary)
}

#[cfg(target_os = "macos")]
fn same_path(left: &Path, right: &Path) -> bool {
    left == right
}

#[cfg(not(target_os = "macos"))]
fn same_path(left: &Path, right: &Path) -> bool {
    path_key(left).eq_ignore_ascii_case(&path_key(right))
}

fn path_key(path: &Path) -> String {
    let value = path.to_string_lossy();
    let normalized = value.strip_prefix(r"\\?\").unwrap_or(&value);
    normalized.trim_end_matches(['\\', '/']).to_owned()
}

fn normalize_path(path: &Path) -> PathBuf {
    let value = path.to_string_lossy();
    match value.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}

fn is_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn slot_filename(kind: ClientKind) -> &'static str {
    match kind {
        ClientKind::Codex => "codex.json",
        ClientKind::ClaudeCode => "claude_code.json",
        ClientKind::OpenCodeV1 => "opencode_v1.json",
    }
}

fn map_transaction_error(error: TransactionFsError) -> OwnershipError {
    match error {
        TransactionFsError::Conflict => OwnershipError::Conflict,
        TransactionFsError::UnsafePath | TransactionFsError::UnsafeFile => {
            OwnershipError::UnsafePath
        }
        TransactionFsError::TooLarge
        | TransactionFsError::TooManyFiles
        | TransactionFsError::RetainedVersionsLimit => OwnershipError::TooLarge,
        TransactionFsError::Unsupported => OwnershipError::Unsupported,
        TransactionFsError::Expired
        | TransactionFsError::Io
        | TransactionFsError::CommitFailed
        | TransactionFsError::RollbackFailed => OwnershipError::Unavailable,
    }
}

fn map_config_io_error(error: crate::integrations::config_io::ConfigIoError) -> OwnershipError {
    match error {
        crate::integrations::config_io::ConfigIoError::InvalidRoot
        | crate::integrations::config_io::ConfigIoError::UnsafePath => OwnershipError::UnsafePath,
        crate::integrations::config_io::ConfigIoError::TooLarge => OwnershipError::TooLarge,
        crate::integrations::config_io::ConfigIoError::Ambiguous => OwnershipError::Conflict,
        crate::integrations::config_io::ConfigIoError::Unavailable => OwnershipError::Unavailable,
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "ownership_macos_recovery.rs"]
mod macos_recovery;

#[cfg(test)]
mod codec_tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_path_bindings_preserve_native_bytes_and_directory_boundaries() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        let first = PathBuf::from(OsString::from_vec(
            b"/Users/fixture/\xff/config.toml".to_vec(),
        ));
        let second = PathBuf::from(OsString::from_vec(
            b"/Users/fixture/\xfe/config.toml".to_vec(),
        ));
        assert!(validate_absolute_local_path(&first).is_ok());
        assert_ne!(
            hash_config_path_binding(&first).unwrap(),
            hash_config_path_binding(&second).unwrap()
        );
        assert!(within_boundary(
            Path::new("/Users/fixture/.codex/config.toml"),
            Path::new("/Users/fixture")
        ));
        assert!(!within_boundary(
            Path::new("/Users/fixture-other/config.toml"),
            Path::new("/Users/fixture")
        ));
        assert!(!same_path(
            Path::new("/Users/fixture/A"),
            Path::new("/Users/fixture/a")
        ));
        assert!(validate_absolute_local_path(Path::new("/Users/fixture/../other")).is_err());
    }

    #[test]
    fn legacy_opencode_receipt_without_optional_route_rows_decodes() {
        let legacy = r#"{"client_kind":"opencode_v1","config_source":"opencode_jsonc","endpoint_port":45231,"created_ancestor_mask":0,"config_was_missing":false}"#;
        let stored: StoredReceipt = serde_json::from_str(legacy).unwrap();
        let metadata = stored.to_receipt().unwrap().metadata();
        assert_eq!(metadata.client_kind, ClientKind::OpenCodeV1);
        assert!(metadata.opencode_routes.is_none());
        let encoded = serde_json::to_value(stored).unwrap();
        assert!(encoded.get("opencode_routes").is_none());
    }

    #[test]
    fn patch_receipt_metadata_rejects_opencode_missing_present_combination() {
        assert!(PatchReceipt::from_metadata(PatchReceiptMetadata {
            client_kind: ClientKind::OpenCodeV1,
            config_source: ConfigSource::OpenCodeMissing,
            endpoint_port: 45231,
            created_ancestor_mask: 7,
            config_was_missing: false,
            opencode_routes: None,
        })
        .is_err());
    }
}
