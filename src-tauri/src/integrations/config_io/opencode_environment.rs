use sha2::{Digest, Sha256};
use std::ffi::OsString;

const NAMES: [&str; 3] = [
    "OPENCODE_CONFIG",
    "OPENCODE_CONFIG_CONTENT",
    "OPENCODE_CONFIG_DIR",
];

/// A memory-only observation of Headroom's process-scoped OpenCode overrides.
/// Values are consumed into a digest and never retained in the snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Snapshot {
    pub(super) has_override: bool,
    pub(super) fingerprint: String,
}

impl Snapshot {
    pub(super) fn from_process() -> Self {
        Self::from_values(NAMES.map(std::env::var_os))
    }

    pub(super) fn from_values(values: [Option<OsString>; 3]) -> Self {
        let mut digest = Sha256::new();
        digest.update(b"headroom-opencode-process-environment/v1\0");
        let mut has_override = false;
        for (name, value) in NAMES.into_iter().zip(values) {
            digest.update((name.len() as u32).to_le_bytes());
            digest.update(name.as_bytes());
            match value {
                Some(value) => {
                    let bytes = os_value_bytes(&value);
                    has_override |= !bytes.is_empty();
                    digest.update([1]);
                    digest.update((bytes.len() as u64).to_le_bytes());
                    digest.update(bytes);
                }
                None => digest.update([0]),
            }
        }
        let fingerprint = digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Self {
            has_override,
            fingerprint,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_test_values(
        config: Option<&str>,
        content: Option<&str>,
        directory: Option<&str>,
    ) -> Self {
        Self::from_values([config, content, directory].map(|value| value.map(OsString::from)))
    }
}

#[cfg(windows)]
fn os_value_bytes(value: &std::ffi::OsStr) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    value.encode_wide().flat_map(u16::to_le_bytes).collect()
}

#[cfg(unix)]
fn os_value_bytes(value: &std::ffi::OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes().to_vec()
}

#[cfg(not(any(windows, unix)))]
fn os_value_bytes(value: &std::ffi::OsStr) -> Vec<u8> {
    value.to_string_lossy().as_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::Snapshot;

    #[test]
    fn environment_snapshot_hashes_presence_and_value_without_retaining_it() {
        let empty = Snapshot::from_test_values(None, None, None);
        let present = Snapshot::from_test_values(None, Some("private-fixture-value"), None);
        let changed = Snapshot::from_test_values(None, Some("different-private-value"), None);
        assert!(!empty.has_override);
        assert!(present.has_override);
        assert_eq!(present.fingerprint.len(), 64);
        assert_ne!(empty.fingerprint, present.fingerprint);
        assert_ne!(present.fingerprint, changed.fingerprint);
        assert!(!format!("{present:?}").contains("private-fixture-value"));
        for value in [
            Snapshot::from_test_values(Some("fixture-path"), None, None),
            Snapshot::from_test_values(None, None, Some("fixture-directory")),
        ] {
            assert!(value.has_override);
            assert_ne!(empty.fingerprint, value.fingerprint);
        }
    }
}
