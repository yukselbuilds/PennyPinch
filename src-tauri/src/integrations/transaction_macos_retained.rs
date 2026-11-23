use super::*;

#[cfg(target_os = "macos")]
const ARCHIVE_PREFIX: &str = ".apfs-archive-";
#[cfg(target_os = "linux")]
const ARCHIVE_PREFIX: &str = ".pennypinch-archive-";

#[derive(Debug)]
pub(crate) struct RetainedReview {
    pub(crate) review_fingerprint: String,
    pub(crate) active_records: usize,
    pub(crate) selected_records: usize,
    pub(crate) unresolved_records: usize,
    pub(crate) changed_versions: usize,
    pub(crate) completed_archives: usize,
    pub(crate) can_archive: bool,
}

pub(crate) fn inspect(
    state: &Path,
    config: &Path,
    owner: &str,
) -> Result<RetainedReview, TransactionFsError> {
    let state = Directory::open(state, true)?;
    let _lock = lock(&state, false)?;
    Ok(observe(&state, config, owner)?.public)
}

pub(crate) fn archive(
    state: &Path,
    config: &Path,
    owner: &str,
    expected: &str,
) -> Result<RetainedReview, TransactionFsError> {
    archive_inner(state, config, owner, expected, |_| {})
}

pub(super) const MAX_ARCHIVES: usize = 64;
const MAX_MANIFEST_BYTES: usize = 256 * 1024;
pub(super) const MAX_ROOT_ENTRIES: usize =
    MAX_JOURNALS * (MAX_ARCHIVES + 1) * 5 + MAX_ARCHIVES + 16;
const SUFFIXES: [&str; 5] = ["json", "done", "owner", "intent", "mark"];

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArchivedFile {
    filename: String,
    version: Version,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u8,
    id: String,
    state: Object,
    binding: String,
    owner_filename: String,
    review_fingerprint: String,
    journal_ids: Vec<String>,
    files: Vec<ArchivedFile>,
}

struct ArchiveGroup {
    directory: Directory,
    manifest: Manifest,
    completed: bool,
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn archive_name(id: &str) -> Result<CString, TransactionFsError> {
    name(id, "json")?;
    CString::new(format!("{ARCHIVE_PREFIX}{id}")).map_err(|_| TransactionFsError::Conflict)
}

impl Manifest {
    fn validate(&self, state: &Directory) -> Result<(), TransactionFsError> {
        archive_name(&self.id)?;
        if self.schema_version != 1
            || self.state != state.object
            || !valid_hash(&self.binding)
            || !valid_hash(&self.review_fingerprint)
            || self.journal_ids.is_empty()
            || self.journal_ids.len() > MAX_JOURNALS
            || self.files.len() > MAX_JOURNALS * SUFFIXES.len()
            || !matches!(
                self.owner_filename.as_str(),
                "codex.json" | "claude_code.json" | "opencode_v1.json"
            )
        {
            return Err(TransactionFsError::Conflict);
        }
        let mut unique = std::collections::HashSet::new();
        for id in &self.journal_ids {
            name(id, "json")?;
            if !unique.insert(id) {
                return Err(TransactionFsError::Conflict);
            }
        }
        let mut filenames = std::collections::HashSet::new();
        for file in &self.files {
            let matched = self.journal_ids.iter().any(|id| {
                SUFFIXES
                    .iter()
                    .any(|suffix| file.filename == format!("{RECORD_PREFIX}{id}.{suffix}"))
            });
            if !matched
                || !filenames.insert(&file.filename)
                || !valid_hash(&file.version.sha256)
                || !valid_hash(&file.version.metadata_sha256)
                || file.version.object.inode == 0
                || file.version.birth_nanos >= 1_000_000_000
            {
                return Err(TransactionFsError::Conflict);
            }
        }
        if self
            .journal_ids
            .iter()
            .any(|id| !filenames.contains(&format!("{RECORD_PREFIX}{id}.json")))
        {
            return Err(TransactionFsError::Conflict);
        }
        Ok(())
    }
}

fn groups(state: &Directory) -> Result<Vec<ArchiveGroup>, TransactionFsError> {
    state.verify()?;
    let mut groups = Vec::new();
    for (count, entry) in fs::read_dir(&state.path)
        .map_err(|_| TransactionFsError::Io)?
        .enumerate()
    {
        if count >= MAX_ROOT_ENTRIES {
            return Err(TransactionFsError::RetainedVersionsLimit);
        }
        let entry = entry.map_err(|_| TransactionFsError::Io)?;
        let filename = entry.file_name();
        let Some(id) = filename
            .to_str()
            .and_then(|value| value.strip_prefix(ARCHIVE_PREFIX))
        else {
            continue;
        };
        archive_name(id)?;
        let directory = Directory::open(&entry.path(), true)?;
        let completed = directory.snapshot(c"manifest.json", MAX_MANIFEST_BYTES)?;
        let prepared = directory.snapshot(c"prepare.json", MAX_MANIFEST_BYTES)?;
        let Some(file) = completed.as_ref().or(prepared.as_ref()) else {
            continue;
        };
        let manifest: Manifest =
            serde_json::from_slice(&file.bytes).map_err(|_| TransactionFsError::Conflict)?;
        manifest.validate(state)?;
        if manifest.id != id {
            return Err(TransactionFsError::Conflict);
        }
        if let (Some(complete), Some(prepare)) = (&completed, &prepared) {
            if complete.bytes != prepare.bytes {
                return Err(TransactionFsError::Conflict);
            }
        }
        groups.push(ArchiveGroup {
            directory,
            manifest,
            completed: completed.is_some(),
        });
        if groups.len() > MAX_ARCHIVES {
            return Err(TransactionFsError::RetainedVersionsLimit);
        }
    }
    state.verify()?;
    Ok(groups)
}

pub(super) fn archived_ids(
    state: &Directory,
) -> Result<std::collections::HashSet<String>, TransactionFsError> {
    let mut ids = std::collections::HashSet::new();
    for group in groups(state)?.into_iter().filter(|group| group.completed) {
        for id in group.manifest.journal_ids {
            if !ids.insert(id) {
                return Err(TransactionFsError::Conflict);
            }
        }
    }
    Ok(ids)
}

fn config_snapshot(path: &Path) -> Result<(Directory, Option<Snapshot>), TransactionFsError> {
    path_binding(path)?;
    let parent = path.parent().ok_or(TransactionFsError::UnsafePath)?;
    if fs::symlink_metadata(parent).is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
        && parent.file_name() == Some(std::ffi::OsStr::new(".claude"))
    {
        return Ok((
            Directory::open(
                parent.parent().ok_or(TransactionFsError::UnsafePath)?,
                false,
            )?,
            None,
        ));
    }
    let directory = Directory::open(parent, false)?;
    let file = directory.snapshot(&component(path)?, MAX_TRANSACTION_FILE_BYTES)?;
    Ok((directory, file))
}

struct Observation {
    public: RetainedReview,
    config_version: Option<Version>,
    selected: Vec<Journal>,
}

fn observe(
    state: &Directory,
    config: &Path,
    owner: &str,
) -> Result<Observation, TransactionFsError> {
    if !matches!(
        owner,
        "codex.json" | "claude_code.json" | "opencode_v1.json"
    ) {
        return Err(TransactionFsError::UnsafePath);
    }
    let binding = path_binding(config)?;
    let (directory, current) = config_snapshot(config)?;
    let history = journals(state)?;
    let archive_groups = groups(state)?;
    let completed_archives = archive_groups
        .iter()
        .filter(|group| group.completed)
        .count();
    let mut digest = Sha256::new();
    digest.update(b"headroom-retained-review-v1\0");
    digest.update(binding.as_bytes());
    digest.update(owner.as_bytes());
    digest.update(
        serde_json::to_vec(&(
            state.object.clone(),
            directory.object.clone(),
            version(&current),
            completed_archives,
        ))
        .map_err(|_| TransactionFsError::Io)?,
    );
    let owner_file = state.snapshot(&CString::new(owner).unwrap(), MAX_TRANSACTION_FILE_BYTES)?;
    digest.update(serde_json::to_vec(&version(&owner_file)).map_err(|_| TransactionFsError::Io)?);
    let mut selected = Vec::new();
    let mut unresolved = 0;
    let mut changed = 0;
    let active_records = history.len();
    let mut history: Vec<_> = history
        .into_iter()
        .filter(|(journal, _)| journal.binding == binding && journal.owner_filename == owner)
        .collect();
    history.sort_by(|a, b| a.0.id.cmp(&b.0.id));
    for (journal, journal_hash) in history {
        digest.update(journal_hash.as_bytes());
        for suffix in SUFFIXES {
            let file = state.snapshot(&name(&journal.id, suffix)?, MAX_MANIFEST_BYTES)?;
            digest.update(suffix.as_bytes());
            digest.update(serde_json::to_vec(&version(&file)).map_err(|_| TransactionFsError::Io)?);
        }
        let retained_result = staged_config(&journal, config);
        let unsafe_retained = retained_result.is_err();
        let retained = retained_result.ok().flatten();
        digest.update(serde_json::to_vec(&version(&retained)).map_err(|_| TransactionFsError::Io)?);
        let marker = journal.marker(state, &journal_hash)?;
        let expected_retained = if marker == Some(Disposition::Aborted)
            || (marker.is_none() && version(&current) == journal.before_config.as_ref())
        {
            Some(&journal.after_config)
        } else {
            journal.before_config.as_ref()
        };
        if version(&retained) != expected_retained {
            changed += 1;
        }
        if unsafe_retained || (marker.is_none() && !can_adopt(state, &journal, config, &current)?) {
            unresolved += 1;
        }
        selected.push(journal);
    }
    directory.verify()?;
    let (_, after) = config_snapshot(config)?;
    if version(&after) != version(&current) {
        return Err(TransactionFsError::Conflict);
    }
    let selected_records = selected.len();
    Ok(Observation {
        public: RetainedReview {
            review_fingerprint: format!("{:x}", digest.finalize()),
            active_records,
            selected_records,
            unresolved_records: unresolved,
            changed_versions: changed,
            completed_archives,
            can_archive: selected_records > 0
                && unresolved == 0
                && completed_archives < MAX_ARCHIVES,
        },
        config_version: version(&current).cloned(),
        selected,
    })
}

fn can_adopt(
    state: &Directory,
    journal: &Journal,
    path: &Path,
    current: &Option<Snapshot>,
) -> Result<bool, TransactionFsError> {
    if journal.target()? != path
        || journal.owner_directory != state.object
        || path_binding(path)? != journal.binding
    {
        return Ok(false);
    }
    let owner_name = CString::new(journal.owner_filename.as_str()).unwrap();
    let owner = state.snapshot(&owner_name, MAX_TRANSACTION_FILE_BYTES)?;
    let next = state.snapshot(&name(&journal.id, "owner")?, MAX_TRANSACTION_FILE_BYTES)?;
    if version(current) == journal.before_config.as_ref()
        && version(&owner) == journal.before_owner.as_ref()
        && version(&next) == Some(&journal.after_owner)
        && version(&staged_config(journal, path)?) == Some(&journal.after_config)
    {
        return Ok(true);
    }
    let Some(current) = current else {
        return Ok(false);
    };
    let next_bytes = if version(&owner) == Some(&journal.after_owner) {
        owner.as_ref().map(|file| file.bytes.as_slice())
    } else if version(&owner) == journal.before_owner.as_ref()
        && version(&next) == Some(&journal.after_owner)
    {
        next.as_ref().map(|file| file.bytes.as_slice())
    } else {
        None
    };
    Ok(next_bytes.is_some_and(|bytes| {
        crate::state::ownership::validate_unix_retained_adoption(
            path,
            &journal.owner_filename,
            owner.as_ref().map(|file| file.bytes.as_slice()),
            bytes,
            &current.bytes,
        )
        .is_ok()
    }))
}

fn staged_config(journal: &Journal, path: &Path) -> Result<Option<Snapshot>, TransactionFsError> {
    let parent = path.parent().ok_or(TransactionFsError::UnsafePath)?;
    if journal.claude_project.is_some() {
        if fs::symlink_metadata(parent).is_ok() {
            return Ok(None);
        }
        let project = parent.parent().ok_or(TransactionFsError::UnsafePath)?;
        return Directory::open(
            &project.join(name(&journal.id, "claude")?.to_str().unwrap()),
            true,
        )?
        .snapshot(c"settings.local.json", MAX_TRANSACTION_FILE_BYTES);
    }
    Directory::open(parent, false)?
        .snapshot(&config_stage(&journal.id)?, MAX_TRANSACTION_FILE_BYTES)
}

fn adopt(state: &Directory, journal: &Journal, path: &Path) -> Result<(), TransactionFsError> {
    let (_, current) = config_snapshot(path)?;
    let journal_file = state
        .snapshot(&name(&journal.id, "json")?, MAX_MANIFEST_BYTES)?
        .ok_or(TransactionFsError::Conflict)?;
    let digest = hash(&journal_file.bytes);
    if journal.marker(state, &digest)?.is_some() {
        return Ok(());
    }
    if journal.recover(state, &digest, path).is_ok() {
        return Ok(());
    }
    if version(&current) == journal.before_config.as_ref() {
        return Err(TransactionFsError::Conflict);
    }
    if !can_adopt(state, journal, path, &current)? {
        return Err(TransactionFsError::Conflict);
    }
    let owner_name = CString::new(journal.owner_filename.as_str()).unwrap();
    let owner = state.snapshot(&owner_name, MAX_TRANSACTION_FILE_BYTES)?;
    let staged_name = name(&journal.id, "owner")?;
    let staged = state.snapshot(&staged_name, MAX_TRANSACTION_FILE_BYTES)?;
    if version(&config_snapshot(path)?.1) != version(&current)
        || (version(&owner) != Some(&journal.after_owner)
            && (version(&owner) != journal.before_owner.as_ref()
                || version(&staged) != Some(&journal.after_owner)))
    {
        return Err(TransactionFsError::Conflict);
    }
    if version(&owner) != Some(&journal.after_owner) {
        state.publish(&staged_name, &owner_name, journal.before_owner.is_some())?;
    }
    if version(&state.snapshot(&owner_name, MAX_TRANSACTION_FILE_BYTES)?)
        != Some(&journal.after_owner)
        || version(&config_snapshot(path)?.1) != version(&current)
    {
        return Err(TransactionFsError::CommitFailed);
    }
    journal.mark(state, &digest, Disposition::Committed)
}

fn collect_files(
    state: &Directory,
    selected: &[Journal],
) -> Result<Vec<ArchivedFile>, TransactionFsError> {
    let mut files = Vec::new();
    for journal in selected {
        let bytes = state
            .snapshot(&name(&journal.id, "json")?, MAX_MANIFEST_BYTES)?
            .ok_or(TransactionFsError::Conflict)?
            .bytes;
        let digest = hash(&bytes);
        let disposition = journal
            .marker(state, &digest)?
            .ok_or(TransactionFsError::Conflict)?;
        for suffix in SUFFIXES {
            let filename = name(&journal.id, suffix)?;
            if let Some(file) = state.snapshot(&filename, MAX_MANIFEST_BYTES)? {
                match suffix {
                    "json" | "intent" => {
                        if file.bytes != bytes {
                            return Err(TransactionFsError::Conflict);
                        }
                    }
                    "done" | "mark" => {
                        let marker: Marker = serde_json::from_slice(&file.bytes)
                            .map_err(|_| TransactionFsError::Conflict)?;
                        if marker.schema_version != 1
                            || marker.journal_sha256 != digest
                            || marker.disposition != disposition
                        {
                            return Err(TransactionFsError::Conflict);
                        }
                    }
                    "owner" => {
                        crate::state::ownership::validate_unix_ownership_bytes(&file.bytes)
                            .map_err(|_| TransactionFsError::Conflict)?;
                    }
                    _ => return Err(TransactionFsError::Conflict),
                }
                files.push(ArchivedFile {
                    filename: filename.to_str().unwrap().to_owned(),
                    version: file.version,
                });
            }
        }
    }
    Ok(files)
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ArchivePhase {
    CopiesReady,
    ManifestPublished,
}

pub(super) fn archive_inner(
    state_path: &Path,
    config: &Path,
    owner: &str,
    expected: &str,
    mut hook: impl FnMut(ArchivePhase),
) -> Result<RetainedReview, TransactionFsError> {
    let state = Directory::open(state_path, true)?;
    let _lock = lock(&state, true)?;
    let observed = observe(&state, config, owner)?;
    if observed.public.review_fingerprint != expected {
        return Err(TransactionFsError::Conflict);
    }
    if observed.public.completed_archives >= MAX_ARCHIVES {
        return Err(TransactionFsError::RetainedVersionsLimit);
    }
    if !observed.public.can_archive {
        return Err(TransactionFsError::Conflict);
    }
    for journal in &observed.selected {
        adopt(&state, journal, config)?;
    }
    let after_adoption = observe(&state, config, owner)?;
    if after_adoption.config_version != observed.config_version {
        return Err(TransactionFsError::Conflict);
    }
    let mut all_groups = groups(&state)?;
    let prepared = all_groups.iter().position(|group| {
        !group.completed
            && group.manifest.review_fingerprint == after_adoption.public.review_fingerprint
            && group.manifest.binding == path_binding(config).unwrap_or_default()
            && group.manifest.owner_filename == owner
    });
    let group = if let Some(index) = prepared {
        all_groups.swap_remove(index)
    } else {
        if all_groups.len() >= MAX_ARCHIVES {
            return Err(TransactionFsError::RetainedVersionsLimit);
        }
        let id = Uuid::new_v4().to_string();
        let filename = archive_name(&id)?;
        if unsafe { libc::mkdirat(state.file.as_raw_fd(), filename.as_ptr(), 0o700) } != 0 {
            return Err(TransactionFsError::Io);
        }
        let directory = Directory::open(&state.path.join(filename.to_str().unwrap()), true)?;
        let manifest = Manifest {
            schema_version: 1,
            id,
            state: state.object.clone(),
            binding: path_binding(config)?,
            owner_filename: owner.to_owned(),
            review_fingerprint: after_adoption.public.review_fingerprint.clone(),
            journal_ids: after_adoption
                .selected
                .iter()
                .map(|journal| journal.id.clone())
                .collect(),
            files: collect_files(&state, &after_adoption.selected)?,
        };
        manifest.validate(&state)?;
        let bytes = serde_json::to_vec(&manifest).map_err(|_| TransactionFsError::Io)?;
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(TransactionFsError::TooLarge);
        }
        directory.create(c"prepare.tmp", &bytes, None)?;
        directory.publish(c"prepare.tmp", c"prepare.json", false)?;
        ArchiveGroup {
            directory,
            manifest,
            completed: false,
        }
    };
    for file in &group.manifest.files {
        let name = CString::new(file.filename.as_str()).unwrap();
        let original = state
            .snapshot(&name, MAX_MANIFEST_BYTES)?
            .ok_or(TransactionFsError::Conflict)?;
        if original.version != file.version {
            return Err(TransactionFsError::Conflict);
        }
        let copy_name = CString::new(format!("{}.copy", file.filename)).unwrap();
        match group.directory.snapshot(&copy_name, MAX_MANIFEST_BYTES)? {
            Some(copy) if hash(&copy.bytes) == file.version.sha256 => {}
            Some(_) => return Err(TransactionFsError::Conflict),
            None => {
                group.directory.create(&copy_name, &original.bytes, None)?;
            }
        }
    }
    hook(ArchivePhase::CopiesReady);
    for file in &group.manifest.files {
        let copy_name = CString::new(format!("{}.copy", file.filename)).unwrap();
        if group
            .directory
            .snapshot(&copy_name, MAX_MANIFEST_BYTES)?
            .is_none_or(|copy| hash(&copy.bytes) != file.version.sha256)
        {
            return Err(TransactionFsError::Conflict);
        }
    }
    let check = observe(&state, config, owner)?;
    if check.public.review_fingerprint != after_adoption.public.review_fingerprint
        || check.config_version != observed.config_version
    {
        return Err(TransactionFsError::Conflict);
    }
    let bytes = serde_json::to_vec(&group.manifest).map_err(|_| TransactionFsError::Io)?;
    group.directory.create(c"manifest.tmp", &bytes, None)?;
    group
        .directory
        .publish(c"manifest.tmp", c"manifest.json", false)?;
    hook(ArchivePhase::ManifestPublished);
    // The committed copies are authoritative before best-effort metadata relocation.
    for file in &group.manifest.files {
        let name = CString::new(file.filename.as_str()).unwrap();
        if state
            .snapshot(&name, MAX_MANIFEST_BYTES)
            .ok()
            .flatten()
            .is_some_and(|actual| actual.version == file.version)
        {
            if rename_entry(
                state.file.as_raw_fd(),
                &name,
                group.directory.file.as_raw_fd(),
                &name,
                false,
            )
            .is_ok()
            {
                let _ = group.directory.file.sync_all();
                let _ = state.file.sync_all();
            }
        }
    }
    if config_snapshot(config)?
        .1
        .as_ref()
        .map(|file| &file.version)
        != observed.config_version.as_ref()
    {
        return Err(TransactionFsError::CommitFailed);
    }
    Ok(observe(&state, config, owner)?.public)
}
