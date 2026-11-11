use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const SCHEMA_VERSION: u8 = 1;
const MAX_STATE_BYTES: u64 = 4096;
const MAX_SAFE_REVISION: u64 = 9_007_199_254_740_991;
const ENGINE_VERSION: &str = "0.41.0";
const RUNTIME_MANIFEST_SHA256: &str =
    "8e4edf5f1328a7a6323ecf435604feaa48fd80c9648288f05b449584924b7908";
const MODE: &str = "cache";
const SAVINGS_PROFILE: &str = "coding";
const CCR_TTL_SECONDS: u64 = 7200;

#[derive(Clone)]
pub struct PreferenceStore {
    inner: Arc<StoreInner>,
}

struct StoreInner {
    location: Result<StoreLocation, StoreError>,
    operation: Mutex<()>,
    #[cfg(test)]
    fail_replace_once: AtomicBool,
}

#[derive(Clone)]
struct StoreLocation {
    boundary: PathBuf,
    relative_root: PathBuf,
}

struct LoadedDocument {
    document: StoredDocument,
    fingerprint: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum StoreError {
    ValidationFailed,
    UnsupportedSchema,
    EnvironmentBlocked,
    Busy,
    ConfigConflict,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum PreferencesReply {
    Snapshot(PreferencesSnapshot),
    Error(PreferencesError),
}

#[derive(Serialize)]
pub struct PreferencesSnapshot {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub revision: u64,
    pub state_fingerprint: Option<String>,
    pub preferences: UserPreferences,
    pub runtime_manifest: RuntimeManifestReference,
}

#[derive(Serialize)]
pub struct PreferencesError {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub error: PreferencesErrorDetail,
}

#[derive(Serialize)]
pub struct PreferencesErrorDetail {
    pub code: &'static str,
    pub retryable: bool,
    pub message_key: &'static str,
    pub diagnostic_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserPreferences {
    pub language: String,
    pub theme: String,
    #[serde(default)]
    pub operating: OperatingPreferences,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub show_recommendations: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatingPreferences {
    pub close_to_tray: bool,
    pub retention_days: u16,
    pub protected_tools: Vec<String>,
}

impl Default for OperatingPreferences {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            retention_days: 30,
            protected_tools: Vec::new(),
        }
    }
}

impl OperatingPreferences {
    pub(crate) fn is_valid(&self) -> bool {
        matches!(self.retention_days, 7 | 30 | 90)
            && self.protected_tools.len() <= 32
            && self.protected_tools.iter().all(|name| {
                !name.is_empty()
                    && name.len() <= 96
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-.:/".contains(&b))
            })
            && self
                .protected_tools
                .windows(2)
                .all(|pair| pair[0] < pair[1])
    }
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            language: "tr".to_owned(),
            theme: "system".to_owned(),
            operating: OperatingPreferences::default(),
            show_recommendations: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeManifestReference {
    pub engine_version: String,
    pub manifest_sha256: String,
    pub mode: String,
    pub savings_profile: String,
    pub ccr_ttl_seconds: u64,
}

impl RuntimeManifestReference {
    fn pinned() -> Self {
        Self {
            engine_version: ENGINE_VERSION.to_owned(),
            manifest_sha256: RUNTIME_MANIFEST_SHA256.to_owned(),
            mode: MODE.to_owned(),
            savings_profile: SAVINGS_PROFILE.to_owned(),
            ccr_ttl_seconds: CCR_TTL_SECONDS,
        }
    }

    fn matches_pin(&self) -> bool {
        self == &Self::pinned()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredDocument {
    schema_version: u8,
    revision: u64,
    preferences: UserPreferences,
    runtime_manifest: RuntimeManifestReference,
}

impl StoredDocument {
    fn defaults() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            revision: 0,
            preferences: UserPreferences::default(),
            runtime_manifest: RuntimeManifestReference::pinned(),
        }
    }

    fn validate(&self) -> Result<(), StoreError> {
        if self.schema_version != SCHEMA_VERSION
            || self.revision > MAX_SAFE_REVISION
            || !valid_preferences(&self.preferences)
            || !self.runtime_manifest.matches_pin()
        {
            return Err(StoreError::UnsupportedSchema);
        }
        Ok(())
    }
}

impl PreferenceStore {
    /// Create a store without touching disk. Invalid or unavailable roots are
    /// retained as a fixed error so app setup and the runtime manager survive.
    pub fn new(app_data_dir: Option<PathBuf>) -> Self {
        let location = if cfg!(debug_assertions) && !cfg!(target_os = "macos") {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            manifest_dir
                .parent()
                .ok_or(StoreError::EnvironmentBlocked)
                .and_then(|root| {
                    StoreLocation::new(
                        root.to_path_buf(),
                        PathBuf::from("work/app-state/preferences"),
                    )
                })
        } else {
            app_data_dir
                .ok_or(StoreError::EnvironmentBlocked)
                .and_then(StoreLocation::release)
        };
        Self::from_location(location)
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        project_root: PathBuf,
        relative_root: PathBuf,
    ) -> Result<Self, &'static str> {
        StoreLocation::new(project_root, relative_root)
            .map(|location| Self::from_location(Ok(location)))
            .map_err(|_| "unsafe test root")
    }

    fn from_location(location: Result<StoreLocation, StoreError>) -> Self {
        Self {
            inner: Arc::new(StoreInner {
                location,
                operation: Mutex::new(()),
                #[cfg(test)]
                fail_replace_once: AtomicBool::new(false),
            }),
        }
    }

    pub fn get_preferences(&self, request_id: String) -> PreferencesReply {
        if !valid_request_id(&request_id) {
            return error_reply(&request_id, StoreError::ValidationFailed);
        }
        let location = match &self.inner.location {
            Ok(location) => location,
            Err(error) => return error_reply(&request_id, *error),
        };
        // Startup readers share this store; serialize them before the cross-process lock.
        let _operation = match self.inner.operation.lock() {
            Ok(guard) => guard,
            Err(_) => return error_reply(&request_id, StoreError::EnvironmentBlocked),
        };
        let _lock = match StoreLock::acquire(location, false) {
            Ok(lock) => lock,
            Err(error) => return error_reply(&request_id, error),
        };
        match load_document(location) {
            Ok(loaded) => snapshot_reply(request_id, loaded.document, loaded.fingerprint),
            Err(error) => error_reply(&request_id, error),
        }
    }

    pub fn save_preferences(
        &self,
        request_id: String,
        language: String,
        theme: String,
        expected_revision: u64,
        expected_fingerprint: Option<String>,
    ) -> PreferencesReply {
        self.save_with_operating(
            request_id,
            language,
            theme,
            None,
            None,
            expected_revision,
            expected_fingerprint,
        )
    }

    pub fn save_with_operating(
        &self,
        request_id: String,
        language: String,
        theme: String,
        operating: Option<OperatingPreferences>,
        show_recommendations: Option<bool>,
        expected_revision: u64,
        expected_fingerprint: Option<String>,
    ) -> PreferencesReply {
        if !valid_request_id(&request_id)
            || !valid_preferences(&UserPreferences {
                language: language.clone(),
                theme: theme.clone(),
                operating: operating.clone().unwrap_or_default(),
                show_recommendations: show_recommendations.unwrap_or_default(),
            })
            || expected_fingerprint
                .as_deref()
                .is_some_and(|value| !valid_fingerprint(value))
        {
            return error_reply(&request_id, StoreError::ValidationFailed);
        }
        let location = match &self.inner.location {
            Ok(location) => location,
            Err(error) => return error_reply(&request_id, *error),
        };
        let _operation = match self.inner.operation.lock() {
            Ok(guard) => guard,
            Err(_) => return error_reply(&request_id, StoreError::EnvironmentBlocked),
        };
        let _lock = match StoreLock::acquire(location, true) {
            Ok(lock) => lock,
            Err(error) => return error_reply(&request_id, error),
        };
        let current = match load_document(location) {
            Ok(loaded) => loaded,
            Err(error) => return error_reply(&request_id, error),
        };
        if current.document.revision != expected_revision
            || current.fingerprint != expected_fingerprint
        {
            return error_reply(&request_id, StoreError::ConfigConflict);
        }
        let Some(revision) = current.document.revision.checked_add(1) else {
            return error_reply(&request_id, StoreError::ConfigConflict);
        };
        if revision > MAX_SAFE_REVISION {
            return error_reply(&request_id, StoreError::ConfigConflict);
        }
        let next = StoredDocument {
            schema_version: SCHEMA_VERSION,
            revision,
            preferences: UserPreferences {
                language,
                theme,
                operating: operating.unwrap_or(current.document.preferences.operating),
                show_recommendations: show_recommendations
                    .unwrap_or(current.document.preferences.show_recommendations),
            },
            runtime_manifest: RuntimeManifestReference::pinned(),
        };
        match save_document(location, &next, current.fingerprint.as_deref(), &self.inner) {
            Ok(fingerprint) => snapshot_reply(request_id, next, Some(fingerprint)),
            Err(error) => error_reply(&request_id, error),
        }
    }

    pub(crate) fn operating(&self) -> Result<OperatingPreferences, ()> {
        match self.get_preferences(Uuid::new_v4().to_string()) {
            PreferencesReply::Snapshot(snapshot) => Ok(snapshot.preferences.operating),
            _ => Err(()),
        }
    }

    #[cfg(test)]
    pub(super) fn file_path_for_test(&self) -> PathBuf {
        let location = self
            .inner
            .location
            .as_ref()
            .expect("valid fixture location");
        location.root_path().join("preferences.json")
    }

    #[cfg(test)]
    pub(super) fn fail_before_next_replace(&self) {
        self.inner.fail_replace_once.store(true, Ordering::SeqCst);
    }
}

impl StoreLocation {
    fn new(boundary: PathBuf, relative_root: PathBuf) -> Result<Self, StoreError> {
        if !relative_path_is_safe(&relative_root) {
            return Err(StoreError::EnvironmentBlocked);
        }
        let boundary = fs::canonicalize(boundary).map_err(|_| StoreError::EnvironmentBlocked)?;
        verify_directory(&boundary, None)?;
        let location = Self {
            boundary,
            relative_root,
        };
        location.verify_root(false)?;
        Ok(location)
    }

    fn release(app_data_dir: PathBuf) -> Result<Self, StoreError> {
        let (boundary, suffix) = canonical_existing_ancestor(&app_data_dir)?;
        let mut relative_root = suffix;
        relative_root.push("preferences");
        Self::new(boundary, relative_root)
    }

    fn root_path(&self) -> PathBuf {
        self.boundary.join(&self.relative_root)
    }

    fn verify_root(&self, create: bool) -> Result<Option<PathBuf>, StoreError> {
        verify_directory(&self.boundary, None)?;
        if fs::canonicalize(&self.boundary).map_err(|_| StoreError::EnvironmentBlocked)?
            != self.boundary
        {
            return Err(StoreError::EnvironmentBlocked);
        }
        let mut current = self.boundary.clone();
        for component in self.relative_root.components() {
            let Component::Normal(name) = component else {
                return Err(StoreError::EnvironmentBlocked);
            };
            current.push(name);
            match fs::symlink_metadata(&current) {
                Ok(metadata) => {
                    verify_directory(&current, Some(&metadata))?;
                    let canonical =
                        fs::canonicalize(&current).map_err(|_| StoreError::EnvironmentBlocked)?;
                    if !canonical.starts_with(&self.boundary) {
                        return Err(StoreError::EnvironmentBlocked);
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if !create {
                        return Ok(None);
                    }
                    #[cfg(target_os = "macos")]
                    {
                        use std::os::unix::fs::DirBuilderExt;
                        match fs::DirBuilder::new().mode(0o700).create(&current) {
                            Ok(()) => {}
                            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                            Err(_) => return Err(StoreError::EnvironmentBlocked),
                        }
                    }
                    #[cfg(not(target_os = "macos"))]
                    fs::create_dir(&current).map_err(|_| StoreError::EnvironmentBlocked)?;
                    let metadata = fs::symlink_metadata(&current)
                        .map_err(|_| StoreError::EnvironmentBlocked)?;
                    verify_directory(&current, Some(&metadata))?;
                    let canonical =
                        fs::canonicalize(&current).map_err(|_| StoreError::EnvironmentBlocked)?;
                    if !canonical.starts_with(&self.boundary) {
                        return Err(StoreError::EnvironmentBlocked);
                    }
                }
                Err(_) => return Err(StoreError::EnvironmentBlocked),
            }
        }
        Ok(Some(current))
    }
}

pub(crate) fn canonical_existing_ancestor(path: &Path) -> Result<(PathBuf, PathBuf), StoreError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| StoreError::EnvironmentBlocked)?
            .join(path)
    };
    if absolute
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(StoreError::EnvironmentBlocked);
    }
    verify_existing_path_components(&absolute)?;
    let mut ancestor = absolute.as_path();
    let mut suffix = Vec::new();
    while !ancestor.exists() {
        let name = ancestor.file_name().ok_or(StoreError::EnvironmentBlocked)?;
        suffix.push(name.to_os_string());
        ancestor = ancestor.parent().ok_or(StoreError::EnvironmentBlocked)?;
    }
    let canonical = fs::canonicalize(ancestor).map_err(|_| StoreError::EnvironmentBlocked)?;
    verify_directory(
        &canonical,
        Some(&fs::symlink_metadata(&canonical).map_err(|_| StoreError::EnvironmentBlocked)?),
    )?;
    let mut relative = PathBuf::new();
    for part in suffix.into_iter().rev() {
        relative.push(part);
    }
    Ok((canonical, relative))
}

fn verify_existing_path_components(path: &Path) -> Result<(), StoreError> {
    let mut ancestors = Vec::new();
    let mut current = Some(path);
    while let Some(value) = current {
        ancestors.push(value.to_path_buf());
        current = value.parent();
    }
    for ancestor in ancestors.iter().rev() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
                    return Err(StoreError::EnvironmentBlocked);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(StoreError::EnvironmentBlocked),
        }
    }
    Ok(())
}

fn verify_directory(path: &Path, metadata: Option<&fs::Metadata>) -> Result<(), StoreError> {
    let owned_metadata;
    let metadata = if let Some(value) = metadata {
        value
    } else {
        owned_metadata = fs::symlink_metadata(path).map_err(|_| StoreError::EnvironmentBlocked)?;
        &owned_metadata
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(StoreError::EnvironmentBlocked);
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

fn relative_path_is_safe(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn load_document(location: &StoreLocation) -> Result<LoadedDocument, StoreError> {
    let Some(root) = location.verify_root(false)? else {
        return Ok(LoadedDocument {
            document: StoredDocument::defaults(),
            fingerprint: None,
        });
    };
    let path = root.join("preferences.json");
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LoadedDocument {
                document: StoredDocument::defaults(),
                fingerprint: None,
            })
        }
        Err(_) => return Err(StoreError::EnvironmentBlocked),
    };
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || is_reparse_point(&metadata)
        || metadata.len() > MAX_STATE_BYTES
    {
        return Err(StoreError::UnsupportedSchema);
    }
    let canonical = fs::canonicalize(&path).map_err(|_| StoreError::EnvironmentBlocked)?;
    if !canonical.starts_with(&root) {
        return Err(StoreError::EnvironmentBlocked);
    }
    let file = File::open(&path).map_err(|_| StoreError::EnvironmentBlocked)?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_STATE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| StoreError::EnvironmentBlocked)?;
    if bytes.len() as u64 > MAX_STATE_BYTES {
        return Err(StoreError::UnsupportedSchema);
    }
    let document: StoredDocument =
        serde_json::from_slice(&bytes).map_err(|_| StoreError::UnsupportedSchema)?;
    document.validate()?;
    Ok(LoadedDocument {
        document,
        fingerprint: Some(fingerprint(&bytes)),
    })
}

fn save_document(
    location: &StoreLocation,
    document: &StoredDocument,
    expected_fingerprint: Option<&str>,
    _inner: &StoreInner,
) -> Result<String, StoreError> {
    document.validate()?;
    let root = location
        .verify_root(true)?
        .ok_or(StoreError::EnvironmentBlocked)?;
    let path = root.join("preferences.json");
    match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || is_reparse_point(&metadata)
            {
                return Err(StoreError::EnvironmentBlocked);
            }
            if !fs::canonicalize(&path)
                .map_err(|_| StoreError::EnvironmentBlocked)?
                .starts_with(&root)
            {
                return Err(StoreError::EnvironmentBlocked);
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(StoreError::EnvironmentBlocked),
    }
    let bytes = serde_json::to_vec(document).map_err(|_| StoreError::EnvironmentBlocked)?;
    if bytes.len() as u64 > MAX_STATE_BYTES {
        return Err(StoreError::EnvironmentBlocked);
    }
    let temp = root.join(format!("preferences-{}.tmp", Uuid::new_v4().simple()));
    let mut temp_owned = false;
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(target_os = "macos")]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temp)
            .map_err(|_| StoreError::EnvironmentBlocked)?;
        temp_owned = true;
        file.write_all(&bytes)
            .map_err(|_| StoreError::EnvironmentBlocked)?;
        file.flush().map_err(|_| StoreError::EnvironmentBlocked)?;
        file.sync_all()
            .map_err(|_| StoreError::EnvironmentBlocked)?;
        drop(file);

        let current_root = location
            .verify_root(false)?
            .ok_or(StoreError::EnvironmentBlocked)?;
        if current_root != root {
            return Err(StoreError::EnvironmentBlocked);
        }
        let current = load_document(location)?;
        if current.fingerprint.as_deref() != expected_fingerprint {
            return Err(StoreError::ConfigConflict);
        }
        let temp_metadata =
            fs::symlink_metadata(&temp).map_err(|_| StoreError::EnvironmentBlocked)?;
        if !temp_metadata.is_file()
            || temp_metadata.file_type().is_symlink()
            || is_reparse_point(&temp_metadata)
            || !fs::canonicalize(&temp)
                .map_err(|_| StoreError::EnvironmentBlocked)?
                .starts_with(&root)
        {
            return Err(StoreError::EnvironmentBlocked);
        }
        #[cfg(test)]
        if _inner.fail_replace_once.swap(false, Ordering::SeqCst) {
            return Err(StoreError::EnvironmentBlocked);
        }
        replace_file(&temp, &path)?;
        sync_directory(&root)?;
        Ok(())
    })();
    if result.is_err() && temp_owned {
        cleanup_owned_temp(location, &root, &temp);
    }
    result.map(|()| fingerprint(&bytes))
}

fn cleanup_owned_temp(location: &StoreLocation, root: &Path, temp: &Path) {
    if location.verify_root(false).ok().flatten().as_deref() != Some(root) {
        return;
    }
    let Ok(metadata) = fs::symlink_metadata(temp) else {
        return;
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return;
    }
    if fs::canonicalize(temp)
        .map(|resolved| resolved.starts_with(root))
        .unwrap_or(false)
    {
        let _ = fs::remove_file(temp);
    }
}

#[cfg(windows)]
fn replace_file(source: &Path, target: &Path) -> Result<(), StoreError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    let ok = unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if ok == 0 {
        Err(StoreError::EnvironmentBlocked)
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn replace_file(source: &Path, target: &Path) -> Result<(), StoreError> {
    fs::rename(source, target).map_err(|_| StoreError::EnvironmentBlocked)
}

#[cfg(windows)]
fn sync_directory(_path: &Path) -> Result<(), StoreError> {
    // MoveFileExW uses MOVEFILE_WRITE_THROUGH; directory handles cannot be
    // opened portably through std on the supported Windows target.
    Ok(())
}

#[cfg(not(windows))]
fn sync_directory(path: &Path) -> Result<(), StoreError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| StoreError::EnvironmentBlocked)
}

struct StoreLock {
    #[cfg(windows)]
    handle: windows_sys::Win32::Foundation::HANDLE,
    #[cfg(target_os = "macos")]
    _file: Option<File>,
}

impl StoreLock {
    fn acquire(location: &StoreLocation, _create: bool) -> Result<Self, StoreError> {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use std::ptr::null;
            use windows_sys::Win32::Foundation::{
                CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT,
            };
            use windows_sys::Win32::System::Threading::{CreateMutexW, WaitForSingleObject};
            let name = mutex_name(location);
            let wide: Vec<u16> = std::ffi::OsStr::new(&name)
                .encode_wide()
                .chain(Some(0))
                .collect();
            let handle = unsafe { CreateMutexW(null(), 0, wide.as_ptr()) };
            if handle.is_null() {
                return Err(StoreError::EnvironmentBlocked);
            }
            let wait = unsafe { WaitForSingleObject(handle, 0) };
            if wait == WAIT_OBJECT_0 || wait == WAIT_ABANDONED {
                return Ok(Self { handle });
            }
            unsafe { CloseHandle(handle) };
            if wait == WAIT_TIMEOUT {
                Err(StoreError::Busy)
            } else {
                Err(StoreError::EnvironmentBlocked)
            }
        }
        #[cfg(target_os = "macos")]
        {
            use std::os::fd::AsRawFd;
            use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
            let Some(root) = location.verify_root(_create)? else {
                return Ok(Self { _file: None });
            };
            let root_info =
                fs::symlink_metadata(&root).map_err(|_| StoreError::EnvironmentBlocked)?;
            // SAFETY: geteuid has no memory preconditions.
            let uid = unsafe { libc::geteuid() };
            if !root_info.is_dir() || root_info.uid() != uid || root_info.mode() & 0o022 != 0 {
                return Err(StoreError::EnvironmentBlocked);
            }
            let path = root.join(".preferences.lock");
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
                .open(&path)
                .map_err(|_| StoreError::EnvironmentBlocked)?;
            let info = file
                .metadata()
                .map_err(|_| StoreError::EnvironmentBlocked)?;
            if !info.is_file()
                || info.uid() != uid
                || info.nlink() != 1
                || info.mode() & 0o7777 != 0o600
            {
                return Err(StoreError::EnvironmentBlocked);
            }
            // SAFETY: the file owns the live descriptor through the lock lifetime.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                return Err(
                    if std::io::Error::last_os_error().kind() == std::io::ErrorKind::WouldBlock {
                        StoreError::Busy
                    } else {
                        StoreError::EnvironmentBlocked
                    },
                );
            }
            let current_root =
                fs::symlink_metadata(&root).map_err(|_| StoreError::EnvironmentBlocked)?;
            let current_lock =
                fs::symlink_metadata(&path).map_err(|_| StoreError::EnvironmentBlocked)?;
            if !current_root.is_dir()
                || (current_root.dev(), current_root.ino()) != (root_info.dev(), root_info.ino())
                || !current_lock.is_file()
                || (current_lock.dev(), current_lock.ino()) != (info.dev(), info.ino())
            {
                return Err(StoreError::EnvironmentBlocked);
            }
            Ok(Self { _file: Some(file) })
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = location;
            Err(StoreError::EnvironmentBlocked)
        }
    }
}

#[cfg(windows)]
impl Drop for StoreLock {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::ReleaseMutex;
        unsafe {
            ReleaseMutex(self.handle);
            CloseHandle(self.handle);
        }
    }
}

#[cfg(windows)]
fn mutex_name(location: &StoreLocation) -> String {
    let path = location.root_path().to_string_lossy().to_lowercase();
    let digest = Sha256::digest(path.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    format!("Local\\HeadroomPreferences-{encoded}")
}

fn valid_preferences(preferences: &UserPreferences) -> bool {
    matches!(preferences.language.as_str(), "tr" | "en")
        && matches!(preferences.theme.as_str(), "system" | "light" | "dark")
        && preferences.operating.is_valid()
}

fn valid_fingerprint(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn fingerprint(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn valid_request_id(request_id: &str) -> bool {
    if request_id.len() != 36 || !request_id.is_ascii() {
        return false;
    }
    let bytes = request_id.as_bytes();
    if bytes[8] != b'-' || bytes[13] != b'-' || bytes[18] != b'-' || bytes[23] != b'-' {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        if [8, 13, 18, 23].contains(&index) {
            continue;
        }
        if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    matches!(bytes[14].to_ascii_lowercase(), b'1'..=b'8')
        && matches!(bytes[19].to_ascii_lowercase(), b'8' | b'9' | b'a' | b'b')
}

fn snapshot_reply(
    request_id: String,
    document: StoredDocument,
    state_fingerprint: Option<String>,
) -> PreferencesReply {
    PreferencesReply::Snapshot(PreferencesSnapshot {
        schema_version: SCHEMA_VERSION,
        request_id,
        status: "ok",
        revision: document.revision,
        state_fingerprint,
        preferences: document.preferences,
        runtime_manifest: document.runtime_manifest,
    })
}

fn error_reply(request_id: &str, error: StoreError) -> PreferencesReply {
    let (code, retryable, message_key) = match error {
        StoreError::ValidationFailed => {
            ("validation_failed", false, "preferences.validation_failed")
        }
        StoreError::UnsupportedSchema => (
            "unsupported_schema",
            false,
            "preferences.unsupported_schema",
        ),
        StoreError::EnvironmentBlocked => (
            "environment_blocked",
            false,
            "preferences.environment_blocked",
        ),
        StoreError::Busy => ("busy", true, "preferences.busy"),
        StoreError::ConfigConflict => ("config_conflict", false, "preferences.config_conflict"),
    };
    PreferencesReply::Error(PreferencesError {
        schema_version: SCHEMA_VERSION,
        request_id: if valid_request_id(request_id) {
            request_id.to_owned()
        } else {
            String::new()
        },
        status: "error",
        error: PreferencesErrorDetail {
            code,
            retryable,
            message_key,
            diagnostic_id: None,
        },
    })
}
