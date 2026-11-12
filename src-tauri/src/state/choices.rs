//! Explicit, non-secret selections that can be rebound to a fresh discovery scan.

use crate::integrations::config_io::{pin_existing_directory_chain, PinnedDirectoryChain};
#[cfg(target_os = "macos")]
use crate::integrations::transaction_fs::MacSelectionFile;
use crate::integrations::transaction_fs::{FileIdentity, TransactionFsError};
#[cfg(not(target_os = "macos"))]
use crate::integrations::transaction_fs::{FileSnapshot, FileToken, FileTransaction};
use crate::state::ownership::{OwnershipError, OwnershipStore};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

const SCHEMA_VERSION: u8 = 1;
const MAX_CHOICES_BYTES: usize = 16 * 1024;
const MAX_INSTALLATIONS: usize = 3;
const MAX_PATH_BYTES: usize = 32 * 1024;
const MAX_VERSION_BYTES: usize = 128;
const MAX_INSTALLATION_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_SAFE_REVISION: u64 = 9_007_199_254_740_991;
#[cfg(any(test, not(target_os = "macos")))]
const CHOICES_FILE: &str = "selections.json";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ChoiceClient {
    Codex,
    ClaudeCode,
    OpenCode,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ChoiceSurface {
    Cli,
    Desktop,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ChoiceOrigin {
    ManualFile,
    Path,
    Scoop,
    UserLocalNative,
    UserLocalNpm,
    WindowsAppPackage,
    MacosAppBundle,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ClaudeChoiceScope {
    UserConfig,
    ProjectLocalConfig,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChoiceAvailability {
    Available,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct InstallationChoice {
    pub(crate) client: ChoiceClient,
    pub(crate) surface: ChoiceSurface,
    pub(crate) origin: ChoiceOrigin,
    pub(crate) executable_path: PathBuf,
    pub(crate) identity: FileIdentity,
    pub(crate) size_bytes: u64,
    pub(crate) version: Option<String>,
    pub(crate) availability: ChoiceAvailability,
    #[cfg(target_os = "macos")]
    pub(crate) macos_change_time: Option<MacChangeTime>,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MacChangeTime {
    seconds: i64,
    nanos: i64,
}

#[cfg(target_os = "macos")]
impl MacChangeTime {
    pub(crate) fn from_metadata(info: &fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;
        Self {
            seconds: info.ctime(),
            nanos: info.ctime_nsec(),
        }
    }

    fn valid(self) -> bool {
        (0..1_000_000_000).contains(&self.nanos)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClaudeChoice {
    pub(crate) scope: ClaudeChoiceScope,
    pub(crate) project_root: Option<PathBuf>,
    pub(crate) project_identity: Option<FileIdentity>,
    pub(crate) availability: ChoiceAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ChoicesSnapshot {
    pub(crate) revision: u64,
    pub(crate) installations: Vec<InstallationChoice>,
    pub(crate) claude: Option<ClaudeChoice>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ChoiceError {
    Invalid,
    UnsafePath,
    Conflict,
    TooLarge,
    Unsupported,
    Unavailable,
}

#[derive(Clone)]
pub(crate) struct ChoiceStore {
    inner: Arc<StoreInner>,
}

struct StoreInner {
    location: StoreLocation,
    gate: Mutex<()>,
}

#[derive(Clone)]
struct StoreLocation {
    boundary: PathBuf,
    boundary_identity: FileIdentity,
    root: PathBuf,
    owner: OwnershipStore,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredChoices {
    schema_version: u8,
    revision: u64,
    installations: Vec<StoredInstallationChoice>,
    claude: Option<StoredClaudeChoice>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredIdentity {
    volume_serial: u32,
    file_index: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredInstallationChoice {
    client: ChoiceClient,
    surface: ChoiceSurface,
    origin: ChoiceOrigin,
    executable_path: String,
    identity: StoredIdentity,
    size_bytes: u64,
    version: Option<String>,
    #[cfg(target_os = "macos")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    macos_change_time: Option<MacChangeTime>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredClaudeChoice {
    scope: ClaudeChoiceScope,
    project_root: Option<String>,
    project_identity: Option<StoredIdentity>,
}

#[cfg(not(target_os = "macos"))]
struct LoadedChoices {
    document: StoredChoices,
    token: Option<FileToken>,
}

struct InspectedPath {
    canonical_path: String,
    identity: FileIdentity,
    size_bytes: u64,
    #[cfg(target_os = "macos")]
    macos_change_time: Option<MacChangeTime>,
}

impl StoredChoices {
    fn empty() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            revision: 0,
            installations: Vec::new(),
            claude: None,
        }
    }

    fn validate_structure(&self) -> Result<(), ChoiceError> {
        if self.schema_version != SCHEMA_VERSION
            || self.revision > MAX_SAFE_REVISION
            || self.installations.len() > MAX_INSTALLATIONS
        {
            return Err(ChoiceError::Invalid);
        }

        for (index, choice) in self.installations.iter().enumerate() {
            if self.installations[..index]
                .iter()
                .any(|previous| previous.client == choice.client)
            {
                return Err(ChoiceError::Invalid);
            }
            validate_stored_path(&choice.executable_path)?;
            if choice.size_bytes == 0
                || choice.size_bytes > MAX_INSTALLATION_BYTES
                || choice.identity.file_index == 0
                || !valid_version(choice.version.as_deref())
            {
                return Err(ChoiceError::Invalid);
            }
            #[cfg(target_os = "macos")]
            if choice.macos_change_time.is_some_and(|time| !time.valid()) {
                return Err(ChoiceError::Invalid);
            }
        }

        if let Some(claude) = &self.claude {
            match (claude.scope, &claude.project_root, claude.project_identity) {
                (ClaudeChoiceScope::UserConfig, None, None) => {}
                (ClaudeChoiceScope::ProjectLocalConfig, Some(path), Some(identity)) => {
                    validate_stored_path(path)?;
                    if identity.file_index == 0 {
                        return Err(ChoiceError::Invalid);
                    }
                }
                _ => return Err(ChoiceError::Invalid),
            }
        }
        Ok(())
    }
}

impl StoreLocation {
    fn new(
        boundary: impl Into<PathBuf>,
        relative_root: impl Into<PathBuf>,
    ) -> Result<Self, ChoiceError> {
        let raw_boundary = boundary.into();
        let relative_root = normalize_relative_root(&relative_root.into())?;
        let owner = OwnershipStore::new(raw_boundary.clone(), relative_root.clone())
            .map_err(map_ownership_error)?;
        let boundary = fs::canonicalize(raw_boundary).map_err(|_| ChoiceError::UnsafePath)?;
        let boundary = PathBuf::from(normalized_canonical_string(&boundary)?);
        let inspected_boundary = inspect_path(&boundary, true)?;
        let boundary = PathBuf::from(inspected_boundary.canonical_path);
        let mut root = boundary.clone();
        for component in relative_root.components() {
            if let Component::Normal(name) = component {
                root.push(name);
            } else {
                return Err(ChoiceError::UnsafePath);
            }
        }
        Ok(Self {
            boundary,
            boundary_identity: inspected_boundary.identity,
            root,
            owner,
        })
    }

    fn verify_boundary(&self) -> Result<PinnedDirectoryChain, ChoiceError> {
        let pin = pin_existing_directory_chain(&self.boundary).map_err(map_config_io_error)?;
        let current_boundary = inspect_path(&self.boundary, true)?;
        if current_boundary.identity != self.boundary_identity
            || current_boundary.canonical_path != normalized_canonical_string(&self.boundary)?
        {
            return Err(ChoiceError::UnsafePath);
        }
        pin.validate().map_err(map_config_io_error)?;
        Ok(pin)
    }

    fn verify_root_under_boundary_pin(
        &self,
        boundary_pin: &PinnedDirectoryChain,
    ) -> Result<Option<PinnedDirectoryChain>, ChoiceError> {
        let metadata = match fs::symlink_metadata(&self.root) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                boundary_pin.validate().map_err(map_config_io_error)?;
                return Ok(None);
            }
            Err(_) => return Err(ChoiceError::UnsafePath),
        };
        if !metadata.is_dir() || metadata_is_reparse(&metadata) {
            return Err(ChoiceError::UnsafePath);
        }
        let pin = pin_existing_directory_chain(&self.root).map_err(map_config_io_error)?;
        pin.validate().map_err(map_config_io_error)?;
        boundary_pin.validate().map_err(map_config_io_error)?;
        Ok(Some(pin))
    }

    fn verify_root(&self) -> Result<Option<PinnedDirectoryChain>, ChoiceError> {
        let boundary_pin = self.verify_boundary()?;
        self.verify_root_under_boundary_pin(&boundary_pin)
    }

    #[cfg(not(target_os = "macos"))]
    fn file_path(&self) -> PathBuf {
        self.root.join(CHOICES_FILE)
    }
}

impl ChoiceStore {
    /// Locate the non-secret selection store without creating state during setup.
    pub(crate) fn for_app_local_data(
        app_local_data_dir: Option<PathBuf>,
    ) -> Result<Self, ChoiceError> {
        if cfg!(debug_assertions) && !cfg!(target_os = "macos") {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let project_root = manifest_dir.parent().ok_or(ChoiceError::UnsafePath)?;
            return Self::new(
                project_root,
                PathBuf::from("work").join("app-state").join("onboarding"),
            );
        }

        let app_local_data_dir = app_local_data_dir.ok_or(ChoiceError::Unavailable)?;
        #[cfg(target_os = "macos")]
        {
            crate::integrations::config_io::validate_lexical_path(&app_local_data_dir)
                .map_err(map_config_io_error)?;
            let (boundary, relative) =
                super::store::canonical_existing_ancestor(&app_local_data_dir)
                    .map_err(|_| ChoiceError::UnsafePath)?;
            Self::new(boundary, relative.join("onboarding"))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let parent = app_local_data_dir.parent().ok_or(ChoiceError::UnsafePath)?;
            let app_dir_name = app_local_data_dir
                .file_name()
                .ok_or(ChoiceError::UnsafePath)?;
            Self::new(parent, PathBuf::from(app_dir_name).join("onboarding"))
        }
    }

    /// Construct without creating the store directory or file.
    pub(crate) fn new(
        boundary: impl Into<PathBuf>,
        relative_root: impl Into<PathBuf>,
    ) -> Result<Self, ChoiceError> {
        let location = StoreLocation::new(boundary, relative_root)?;
        Ok(Self {
            inner: Arc::new(StoreInner {
                location,
                gate: Mutex::new(()),
            }),
        })
    }

    /// Read saved preferences without creating application or client files.
    pub(crate) fn read(&self) -> Result<ChoicesSnapshot, ChoiceError> {
        let _guard = self
            .inner
            .gate
            .lock()
            .map_err(|_| ChoiceError::Unavailable)?;
        let Some(root_pin) = self.inner.location.verify_root()? else {
            return Ok(snapshot_from_document(StoredChoices::empty()));
        };
        #[cfg(target_os = "macos")]
        let document = MacSelectionFile::open(&self.inner.location.root, MAX_CHOICES_BYTES, false)
            .map_err(map_transaction_error)?
            .bytes()
            .map(decode_document)
            .transpose()?
            .unwrap_or_else(StoredChoices::empty);

        #[cfg(not(target_os = "macos"))]
        let document = {
            let path = self.inner.location.file_path();
            match fs::symlink_metadata(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    root_pin.validate().map_err(map_config_io_error)?;
                    return Ok(snapshot_from_document(StoredChoices::empty()));
                }
                Err(_) => return Err(ChoiceError::UnsafePath),
                Ok(metadata) if !metadata.is_file() || metadata_is_reparse(&metadata) => {
                    return Err(ChoiceError::UnsafePath);
                }
                Ok(_) => {}
            }

            let mut transaction = FileTransaction::begin().map_err(map_transaction_error)?;
            let (token, snapshot) = transaction
                .open_existing(&path, MAX_CHOICES_BYTES)
                .map_err(map_transaction_error)?;
            let document = decode_document(&snapshot.bytes)?;
            root_pin.validate().map_err(map_config_io_error)?;
            transaction.rollback().map_err(map_transaction_error)?;
            let _ = token;
            document
        };
        root_pin.validate().map_err(map_config_io_error)?;
        Ok(snapshot_from_document(document))
    }

    /// Persist the user's explicit installation selection for one client.
    pub(crate) fn save_installation(
        &self,
        choice: InstallationChoice,
    ) -> Result<ChoicesSnapshot, ChoiceError> {
        #[cfg(not(target_os = "macos"))]
        let stored = validate_new_installation(&choice)?;
        #[cfg(target_os = "macos")]
        validate_new_installation(&choice)?;
        self.update(|document| {
            #[cfg(target_os = "macos")]
            let stored = validate_new_installation(&choice)?;
            let existing = document
                .installations
                .iter_mut()
                .find(|entry| entry.client == stored.client);
            if let Some(existing) = existing {
                if *existing == stored {
                    return Ok(false);
                }
                *existing = stored.clone();
            } else {
                document.installations.push(stored.clone());
                document
                    .installations
                    .sort_by_key(|entry| client_order(entry.client));
            }
            Ok(true)
        })
    }

    pub(crate) fn capture_installation(
        client: ChoiceClient,
        surface: ChoiceSurface,
        origin: ChoiceOrigin,
        executable_path: PathBuf,
        version: Option<String>,
    ) -> Result<InstallationChoice, ChoiceError> {
        if !valid_version(version.as_deref()) {
            return Err(ChoiceError::Invalid);
        }
        let inspected = inspect_path(&executable_path, false)?;
        Ok(InstallationChoice {
            client,
            surface,
            origin,
            executable_path: PathBuf::from(inspected.canonical_path),
            identity: inspected.identity,
            size_bytes: inspected.size_bytes,
            version,
            availability: ChoiceAvailability::Available,
            #[cfg(target_os = "macos")]
            macos_change_time: inspected.macos_change_time,
        })
    }

    /// Persist the user's explicit Claude user/project scope selection.
    pub(crate) fn save_claude(&self, choice: ClaudeChoice) -> Result<ChoicesSnapshot, ChoiceError> {
        let stored = validate_new_claude_choice(&choice)?;
        self.update(|document| {
            if document.claude.as_ref() == Some(&stored) {
                return Ok(false);
            }
            document.claude = Some(stored.clone());
            Ok(true)
        })
    }

    fn update(
        &self,
        change: impl FnOnce(&mut StoredChoices) -> Result<bool, ChoiceError>,
    ) -> Result<ChoicesSnapshot, ChoiceError> {
        let _guard = self
            .inner
            .gate
            .lock()
            .map_err(|_| ChoiceError::Unavailable)?;
        let boundary_pin = self.inner.location.verify_boundary()?;
        self.inner
            .location
            .owner
            .prepare_for_apply()
            .map_err(map_ownership_error)?;
        let root_pin = self
            .inner
            .location
            .verify_root_under_boundary_pin(&boundary_pin)?
            .ok_or(ChoiceError::Unavailable)?;
        #[cfg(target_os = "macos")]
        {
            let file = MacSelectionFile::open(&self.inner.location.root, MAX_CHOICES_BYTES, true)
                .map_err(map_transaction_error)?;
            let mut document = file
                .bytes()
                .map(decode_document)
                .transpose()?
                .unwrap_or_else(StoredChoices::empty);
            let next = change_document(&mut document, change)?;
            root_pin.validate().map_err(map_config_io_error)?;
            boundary_pin.validate().map_err(map_config_io_error)?;
            if let Some(bytes) = next {
                file.publish(&bytes).map_err(map_transaction_error)?;
            }
            Ok(snapshot_from_document(document))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let path = self.inner.location.file_path();
            let mut transaction = FileTransaction::begin().map_err(map_transaction_error)?;
            let loaded = read_document_in_transaction(&mut transaction, &path)?;
            let mut document = loaded
                .as_ref()
                .map(|loaded| loaded.document.clone())
                .unwrap_or_else(StoredChoices::empty);
            let Some(bytes) = change_document(&mut document, change)? else {
                transaction.rollback().map_err(map_transaction_error)?;
                root_pin.validate().map_err(map_config_io_error)?;
                return Ok(snapshot_from_document(document));
            };
            let token = match loaded.and_then(|loaded| loaded.token) {
                Some(token) => token,
                None => {
                    transaction
                        .create_missing(&path, MAX_CHOICES_BYTES)
                        .map_err(map_transaction_error)?
                        .0
                }
            };
            transaction
                .replace_contents(token, &bytes)
                .map_err(map_transaction_error)?;
            transaction.seal().map_err(map_transaction_error)?;
            root_pin.validate().map_err(map_config_io_error)?;
            boundary_pin.validate().map_err(map_config_io_error)?;
            transaction.commit().map_err(map_transaction_error)?;
            Ok(snapshot_from_document(document))
        }
    }

    #[cfg(test)]
    pub(super) fn installation_for_test(
        client: ChoiceClient,
        path: PathBuf,
    ) -> Result<InstallationChoice, ChoiceError> {
        let inspected = inspect_path(&path, false)?;
        Ok(InstallationChoice {
            client,
            surface: ChoiceSurface::Cli,
            origin: ChoiceOrigin::ManualFile,
            executable_path: PathBuf::from(inspected.canonical_path),
            identity: inspected.identity,
            size_bytes: inspected.size_bytes,
            version: None,
            availability: ChoiceAvailability::Available,
            #[cfg(target_os = "macos")]
            macos_change_time: inspected.macos_change_time,
        })
    }

    #[cfg(test)]
    pub(super) fn claude_project_for_test(path: PathBuf) -> Result<ClaudeChoice, ChoiceError> {
        let inspected = inspect_path(&path, true)?;
        Ok(ClaudeChoice {
            scope: ClaudeChoiceScope::ProjectLocalConfig,
            project_root: Some(PathBuf::from(inspected.canonical_path)),
            project_identity: Some(inspected.identity),
            availability: ChoiceAvailability::Available,
        })
    }
}

fn change_document(
    document: &mut StoredChoices,
    change: impl FnOnce(&mut StoredChoices) -> Result<bool, ChoiceError>,
) -> Result<Option<Vec<u8>>, ChoiceError> {
    if !change(document)? {
        return Ok(None);
    }
    document.revision = document
        .revision
        .checked_add(1)
        .filter(|revision| *revision <= MAX_SAFE_REVISION)
        .ok_or(ChoiceError::Conflict)?;
    document.validate_structure()?;
    let bytes = serde_json::to_vec(document).map_err(|_| ChoiceError::Invalid)?;
    if bytes.len() > MAX_CHOICES_BYTES {
        return Err(ChoiceError::TooLarge);
    }
    Ok(Some(bytes))
}

#[cfg(not(target_os = "macos"))]
fn read_document_in_transaction(
    transaction: &mut FileTransaction,
    path: &Path,
) -> Result<Option<LoadedChoices>, ChoiceError> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(ChoiceError::UnsafePath),
        Ok(metadata) if !metadata.is_file() || metadata_is_reparse(&metadata) => {
            Err(ChoiceError::UnsafePath)
        }
        Ok(_) => {
            let (token, FileSnapshot { bytes, .. }) = transaction
                .open_existing(path, MAX_CHOICES_BYTES)
                .map_err(map_transaction_error)?;
            Ok(Some(LoadedChoices {
                document: decode_document(&bytes)?,
                token: Some(token),
            }))
        }
    }
}

fn decode_document(bytes: &[u8]) -> Result<StoredChoices, ChoiceError> {
    if bytes.len() > MAX_CHOICES_BYTES {
        return Err(ChoiceError::TooLarge);
    }
    let document: StoredChoices =
        serde_json::from_slice(bytes).map_err(|_| ChoiceError::Invalid)?;
    document.validate_structure()?;
    Ok(document)
}

fn snapshot_from_document(document: StoredChoices) -> ChoicesSnapshot {
    let installations = document
        .installations
        .into_iter()
        .map(|stored| {
            let inspected = inspect_path(Path::new(&stored.executable_path), false);
            let availability = match inspected {
                Ok(value)
                    if value.canonical_path == stored.executable_path
                        && value.identity == to_identity(stored.identity)
                        && value.size_bytes == stored.size_bytes
                        && {
                            #[cfg(target_os = "macos")]
                            {
                                stored.macos_change_time.is_some()
                                    && stored.macos_change_time == value.macos_change_time
                            }
                            #[cfg(not(target_os = "macos"))]
                            {
                                true
                            }
                        } =>
                {
                    ChoiceAvailability::Available
                }
                _ => ChoiceAvailability::Unavailable,
            };
            InstallationChoice {
                client: stored.client,
                surface: stored.surface,
                origin: stored.origin,
                executable_path: PathBuf::from(stored.executable_path),
                identity: to_identity(stored.identity),
                size_bytes: stored.size_bytes,
                version: stored.version,
                availability,
                #[cfg(target_os = "macos")]
                macos_change_time: stored.macos_change_time,
            }
        })
        .collect();
    let claude = document.claude.map(|stored| {
        let (project_root, project_identity, availability) =
            match (stored.project_root, stored.project_identity) {
                (Some(path), Some(identity)) => {
                    let available = inspect_path(Path::new(&path), true)
                        .map(|inspected| {
                            inspected.canonical_path == path
                                && inspected.identity == to_identity(identity)
                        })
                        .unwrap_or(false);
                    (
                        Some(PathBuf::from(path)),
                        Some(to_identity(identity)),
                        if available {
                            ChoiceAvailability::Available
                        } else {
                            ChoiceAvailability::Unavailable
                        },
                    )
                }
                _ => (None, None, ChoiceAvailability::Available),
            };
        ClaudeChoice {
            scope: stored.scope,
            project_root,
            project_identity,
            availability,
        }
    });
    ChoicesSnapshot {
        revision: document.revision,
        installations,
        claude,
    }
}

fn validate_new_installation(
    choice: &InstallationChoice,
) -> Result<StoredInstallationChoice, ChoiceError> {
    if choice.availability != ChoiceAvailability::Available
        || !valid_version(choice.version.as_deref())
    {
        return Err(ChoiceError::Invalid);
    }
    let inspected = inspect_path(&choice.executable_path, false)?;
    if inspected.identity != choice.identity
        || inspected.size_bytes != choice.size_bytes
        || inspected.size_bytes > MAX_INSTALLATION_BYTES
    {
        return Err(ChoiceError::Conflict);
    }
    #[cfg(target_os = "macos")]
    if choice.macos_change_time.is_none() || choice.macos_change_time != inspected.macos_change_time
    {
        return Err(ChoiceError::Conflict);
    }
    Ok(StoredInstallationChoice {
        client: choice.client,
        surface: choice.surface,
        origin: choice.origin,
        executable_path: inspected.canonical_path,
        identity: from_identity(inspected.identity),
        size_bytes: inspected.size_bytes,
        version: choice.version.clone(),
        #[cfg(target_os = "macos")]
        macos_change_time: inspected.macos_change_time,
    })
}

fn validate_new_claude_choice(choice: &ClaudeChoice) -> Result<StoredClaudeChoice, ChoiceError> {
    match (choice.scope, &choice.project_root, choice.project_identity) {
        (ClaudeChoiceScope::UserConfig, None, None) => Ok(StoredClaudeChoice {
            scope: choice.scope,
            project_root: None,
            project_identity: None,
        }),
        (ClaudeChoiceScope::ProjectLocalConfig, Some(path), Some(identity)) => {
            if choice.availability != ChoiceAvailability::Available {
                return Err(ChoiceError::Invalid);
            }
            let inspected = inspect_path(path, true)?;
            if inspected.identity != identity {
                return Err(ChoiceError::Conflict);
            }
            Ok(StoredClaudeChoice {
                scope: choice.scope,
                project_root: Some(inspected.canonical_path),
                project_identity: Some(from_identity(inspected.identity)),
            })
        }
        _ => Err(ChoiceError::Invalid),
    }
}

fn inspect_path(path: &Path, directory: bool) -> Result<InspectedPath, ChoiceError> {
    validate_path_shape(path)?;
    let parent = path.parent().ok_or(ChoiceError::UnsafePath)?;
    let parent_pin = pin_existing_directory_chain(parent).map_err(map_config_io_error)?;
    let before = native_identity(path, directory)?;
    let canonical = fs::canonicalize(path).map_err(|_| ChoiceError::Unavailable)?;
    let canonical_string = normalized_canonical_string(&canonical)?;
    let canonical_path = Path::new(&canonical_string);
    let after = native_identity(canonical_path, directory)?;
    parent_pin.validate().map_err(map_config_io_error)?;
    if before.identity != after.identity || before.size_bytes != after.size_bytes {
        return Err(ChoiceError::Conflict);
    }
    #[cfg(target_os = "macos")]
    if before.macos_change_time != after.macos_change_time {
        return Err(ChoiceError::Conflict);
    }
    Ok(InspectedPath {
        canonical_path: canonical_string,
        identity: after.identity,
        size_bytes: after.size_bytes,
        #[cfg(target_os = "macos")]
        macos_change_time: after.macos_change_time,
    })
}

#[cfg(windows)]
fn native_identity(path: &Path, directory: bool) -> Result<NativeIdentity, ChoiceError> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{
        GetLastError, GENERIC_READ, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
        FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING, SYNCHRONIZE,
    };

    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    if wide.len() > 32_768 {
        return Err(ChoiceError::UnsafePath);
    }
    let access = if directory {
        FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | SYNCHRONIZE
    } else {
        GENERIC_READ | FILE_READ_ATTRIBUTES
    };
    let flags = FILE_FLAG_OPEN_REPARSE_POINT
        | if directory {
            FILE_FLAG_BACKUP_SEMANTICS
        } else {
            0
        };
    let raw = unsafe {
        CreateFileW(
            wide.as_ptr(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            null(),
            OPEN_EXISTING,
            flags,
            null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        let code = unsafe { GetLastError() };
        return Err(match code {
            2 | 3 => ChoiceError::Unavailable,
            32 | 33 | 6800 => ChoiceError::Conflict,
            _ => ChoiceError::UnsafePath,
        });
    }
    let handle = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(handle.as_raw_handle() as HANDLE, &mut info) } == 0 {
        return Err(ChoiceError::Unavailable);
    }
    let is_directory = info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0;
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || is_directory != directory
        || (!directory && info.nNumberOfLinks != 1)
    {
        return Err(ChoiceError::UnsafePath);
    }
    Ok(NativeIdentity {
        identity: FileIdentity {
            volume_serial: info.dwVolumeSerialNumber,
            file_index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        },
        size_bytes: (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow),
    })
}

#[cfg(target_os = "macos")]
fn native_identity(path: &Path, directory: bool) -> Result<NativeIdentity, ChoiceError> {
    use crate::integrations::config_io::ensure_fixed_volume;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    if directory {
        ensure_fixed_volume(&path.join(".headroom-choice-volume"))
    } else {
        ensure_fixed_volume(path)
    }
    .map_err(map_config_io_error)?;
    let flags = libc::O_NOFOLLOW_ANY
        | libc::O_NONBLOCK
        | libc::O_CLOEXEC
        | if directory { libc::O_DIRECTORY } else { 0 };
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(flags)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ChoiceError::Unavailable
            } else {
                ChoiceError::UnsafePath
            }
        })?;
    let info = file.metadata().map_err(|_| ChoiceError::Unavailable)?;
    let uid = unsafe { libc::geteuid() };
    let safe = if directory {
        info.is_dir() && info.uid() == uid && info.mode() & 0o022 == 0
    } else {
        info.is_file()
            && info.nlink() == 1
            && info.mode() & 0o7022 == 0
            && (info.uid() == uid || info.uid() == 0)
    };
    if !safe {
        return Err(ChoiceError::UnsafePath);
    }
    Ok(NativeIdentity {
        identity: FileIdentity {
            volume_serial: info.dev().try_into().map_err(|_| ChoiceError::UnsafePath)?,
            file_index: info.ino(),
        },
        size_bytes: if directory { 0 } else { info.len() },
        macos_change_time: (!directory).then(|| MacChangeTime::from_metadata(&info)),
    })
}

#[cfg(not(any(windows, target_os = "macos")))]
fn native_identity(_path: &Path, _directory: bool) -> Result<NativeIdentity, ChoiceError> {
    Err(ChoiceError::Unsupported)
}

struct NativeIdentity {
    identity: FileIdentity,
    size_bytes: u64,
    #[cfg(target_os = "macos")]
    macos_change_time: Option<MacChangeTime>,
}

fn validate_path_shape(path: &Path) -> Result<(), ChoiceError> {
    let text = path.to_str().ok_or(ChoiceError::UnsafePath)?;
    if text.is_empty()
        || text.len() > MAX_PATH_BYTES
        || text.chars().any(char::is_control)
        || !path.is_absolute()
    {
        return Err(ChoiceError::UnsafePath);
    }
    #[cfg(target_os = "macos")]
    crate::integrations::config_io::validate_lexical_path(path).map_err(map_config_io_error)?;
    for component in path.components() {
        match component {
            Component::ParentDir | Component::CurDir => return Err(ChoiceError::UnsafePath),
            #[cfg(windows)]
            Component::Normal(name) if name.to_string_lossy().contains(':') => {
                return Err(ChoiceError::UnsafePath)
            }
            _ => {}
        }
    }
    #[cfg(windows)]
    {
        use std::path::Prefix;
        match path.components().next() {
            Some(Component::Prefix(prefix))
                if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_)) => {}
            _ => return Err(ChoiceError::UnsafePath),
        }
    }
    Ok(())
}

fn validate_stored_path(path: &str) -> Result<(), ChoiceError> {
    let path = Path::new(path);
    validate_path_shape(path)
}

fn normalized_canonical_string(path: &Path) -> Result<String, ChoiceError> {
    let text = path.to_str().ok_or(ChoiceError::UnsafePath)?;
    #[cfg(windows)]
    let text = text.strip_prefix(r"\\?\").unwrap_or(text);
    #[cfg(windows)]
    let normalized = text.replace('/', "\\");
    #[cfg(not(windows))]
    let normalized = text.to_owned();
    validate_stored_path(&normalized)?;
    Ok(normalized)
}

fn normalize_relative_root(path: &Path) -> Result<PathBuf, ChoiceError> {
    if path.as_os_str().is_empty() {
        return Err(ChoiceError::UnsafePath);
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            _ => return Err(ChoiceError::UnsafePath),
        }
    }
    if normalized.as_os_str().is_empty() {
        Err(ChoiceError::UnsafePath)
    } else {
        Ok(normalized)
    }
}

fn valid_version(version: Option<&str>) -> bool {
    version.is_none_or(|value| {
        !value.is_empty()
            && value.len() <= MAX_VERSION_BYTES
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b".-+_".contains(&byte))
    })
}

fn client_order(client: ChoiceClient) -> u8 {
    match client {
        ChoiceClient::Codex => 0,
        ChoiceClient::ClaudeCode => 1,
        ChoiceClient::OpenCode => 2,
    }
}

fn from_identity(identity: FileIdentity) -> StoredIdentity {
    StoredIdentity {
        volume_serial: identity.volume_serial,
        file_index: identity.file_index,
    }
}

fn to_identity(identity: StoredIdentity) -> FileIdentity {
    FileIdentity {
        volume_serial: identity.volume_serial,
        file_index: identity.file_index,
    }
}

fn map_ownership_error(error: OwnershipError) -> ChoiceError {
    match error {
        OwnershipError::Invalid => ChoiceError::Invalid,
        OwnershipError::Conflict => ChoiceError::Conflict,
        OwnershipError::UnsafePath => ChoiceError::UnsafePath,
        OwnershipError::TooLarge => ChoiceError::TooLarge,
        OwnershipError::Unsupported => ChoiceError::Unsupported,
        OwnershipError::Unavailable => ChoiceError::Unavailable,
    }
}

fn map_transaction_error(error: TransactionFsError) -> ChoiceError {
    match error {
        TransactionFsError::Unsupported => ChoiceError::Unsupported,
        TransactionFsError::UnsafePath | TransactionFsError::UnsafeFile => ChoiceError::UnsafePath,
        TransactionFsError::Conflict | TransactionFsError::Expired => ChoiceError::Conflict,
        TransactionFsError::TooLarge
        | TransactionFsError::TooManyFiles
        | TransactionFsError::RetainedVersionsLimit => ChoiceError::TooLarge,
        TransactionFsError::Io
        | TransactionFsError::CommitFailed
        | TransactionFsError::RollbackFailed => ChoiceError::Unavailable,
    }
}

fn map_config_io_error(error: crate::integrations::config_io::ConfigIoError) -> ChoiceError {
    match error {
        crate::integrations::config_io::ConfigIoError::UnsafePath
        | crate::integrations::config_io::ConfigIoError::InvalidRoot => ChoiceError::UnsafePath,
        crate::integrations::config_io::ConfigIoError::TooLarge => ChoiceError::TooLarge,
        crate::integrations::config_io::ConfigIoError::Ambiguous => ChoiceError::Conflict,
        crate::integrations::config_io::ConfigIoError::Unavailable => ChoiceError::Unavailable,
    }
}

#[cfg(windows)]
fn metadata_is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn metadata_is_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(all(test, target_os = "macos"))]
#[path = "choices_macos_tests.rs"]
mod macos_tests;
