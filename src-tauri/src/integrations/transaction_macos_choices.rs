use super::*;

const TARGET: &CStr = c"selections.json";

pub(crate) struct SelectionFile {
    directory: Directory,
    _lock: Option<File>,
    before: Option<Snapshot>,
    limit: usize,
    write: bool,
}

impl SelectionFile {
    pub(crate) fn open(root: &Path, limit: usize, write: bool) -> Result<Self, TransactionFsError> {
        let directory = Directory::open(root, true)?;
        let lock = lock(&directory, write)?;
        let before = directory.snapshot(TARGET, limit)?;
        if before
            .as_ref()
            .is_some_and(|file| file.version.mode & 0o7777 != 0o600)
        {
            return Err(TransactionFsError::UnsafeFile);
        }
        Ok(Self {
            directory,
            _lock: lock,
            before,
            limit,
            write,
        })
    }

    pub(crate) fn bytes(&self) -> Option<&[u8]> {
        self.before.as_ref().map(|file| file.bytes.as_slice())
    }

    pub(crate) fn publish(self, bytes: &[u8]) -> Result<(), TransactionFsError> {
        self.publish_with(bytes, || {}, || {})
    }

    fn publish_with(
        self,
        bytes: &[u8],
        before_rename: impl FnOnce(),
        after_rename: impl FnOnce(),
    ) -> Result<(), TransactionFsError> {
        if !self.write {
            return Err(TransactionFsError::Unsupported);
        }
        if bytes.len() > self.limit {
            return Err(TransactionFsError::TooLarge);
        }
        let directory = &self.directory;
        let temporary = name(&Uuid::new_v4().to_string(), "selection")?;
        let staged = directory.create(&temporary, bytes, None)?;
        let before = version(&self.before);
        if version(&directory.snapshot(TARGET, self.limit)?) != before
            || version(&directory.snapshot(&temporary, self.limit)?) != Some(&staged.version)
        {
            self.discard(&temporary, &staged.version);
            return Err(TransactionFsError::Conflict);
        }

        before_rename();
        if let Err(error) = directory.publish(&temporary, TARGET, before.is_some()) {
            if error == TransactionFsError::Conflict {
                self.discard(&temporary, &staged.version);
            }
            return Err(error);
        }
        after_rename();

        let current = self.published_snapshot(TARGET)?;
        let displaced = self.published_snapshot(&temporary)?;
        if version(&current) != Some(&staged.version) {
            return Err(TransactionFsError::CommitFailed);
        }
        if version(&displaced) == before {
            if let Some(previous) = before {
                self.discard(&temporary, previous);
            }
            return Ok(());
        }

        // A late writer replaced the old file. Put its version back if both names still match.
        if displaced.is_some()
            && version(&self.published_snapshot(TARGET)?) == Some(&staged.version)
            && version(&self.published_snapshot(&temporary)?) == version(&displaced)
        {
            directory
                .publish(&temporary, TARGET, true)
                .map_err(|_| TransactionFsError::CommitFailed)?;
            if version(&self.published_snapshot(TARGET)?) == version(&displaced)
                && version(&self.published_snapshot(&temporary)?) == Some(&staged.version)
            {
                self.discard(&temporary, &staged.version);
                return Err(TransactionFsError::Conflict);
            }
        }
        Err(TransactionFsError::CommitFailed)
    }

    fn published_snapshot(&self, name: &CStr) -> Result<Option<Snapshot>, TransactionFsError> {
        self.directory
            .snapshot(name, self.limit)
            .map_err(|_| TransactionFsError::CommitFailed)
    }

    fn discard(&self, name: &CStr, expected: &Version) {
        if self
            .directory
            .snapshot(name, self.limit)
            .ok()
            .flatten()
            .is_some_and(|file| file.version == *expected)
        {
            // The held private directory and the matching temporary version are app-owned.
            if unsafe { libc::unlinkat(self.directory.file.as_raw_fd(), name.as_ptr(), 0) } == 0 {
                let _ = self.directory.file.sync_all();
            }
        }
    }
}

#[cfg(test)]
#[path = "transaction_macos_choices_tests.rs"]
mod tests;
