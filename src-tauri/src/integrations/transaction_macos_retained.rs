use super::*;

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
    _state: &Path,
    _config: &Path,
    _owner: &str,
) -> Result<RetainedReview, TransactionFsError> {
    Err(TransactionFsError::Unsupported)
}

pub(crate) fn archive(
    _state: &Path,
    _config: &Path,
    _owner: &str,
    _expected: &str,
) -> Result<RetainedReview, TransactionFsError> {
    Err(TransactionFsError::Unsupported)
}
