use super::{unknown, Account, InstallationSummary, Provider};
use serde::{de, Deserialize, Deserializer};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::Read;
use std::os::windows::{
    fs::{MetadataExt, OpenOptionsExt},
    io::AsRawHandle,
};
use std::path::Path;
use windows_sys::Win32::Storage::FileSystem::{
    GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
};

// Credential values are skipped or reduced to a boolean, never retained in the public metadata.
fn secret_present<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    struct Presence;
    impl<'de> de::Visitor<'de> for Presence {
        type Value = bool;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a credential field")
        }
        fn visit_str<E: de::Error>(self, value: &str) -> Result<bool, E> {
            Ok(!value.is_empty())
        }
    }
    d.deserialize_str(Presence)
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Api,
    Oauth,
    Wellknown,
    #[default]
    #[serde(other)]
    Unknown,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Credential {
    #[serde(rename = "type")]
    kind: Kind,
    #[serde(deserialize_with = "secret_present")]
    key: bool,
    #[serde(deserialize_with = "secret_present")]
    access: bool,
    #[serde(deserialize_with = "secret_present")]
    refresh: bool,
    #[serde(deserialize_with = "secret_present")]
    token: bool,
    expires: Option<u64>,
}

fn metadata(input: impl Read) -> Result<Vec<Provider>, ()> {
    let entries: HashMap<String, Credential> =
        serde_json::from_reader(input.take(262145)).map_err(|_| ())?;
    if entries.len() > 128 {
        return Err(());
    }
    let had_entries = !entries.is_empty();
    let mut result = vec![];
    for (name, record) in entries {
        let method = match record.kind {
            Kind::Api if record.key => "api_key",
            Kind::Oauth if record.access && record.refresh && record.expires.is_some() => "oauth",
            Kind::Wellknown if record.key && record.token => "provider_token",
            _ => continue,
        };
        let provider = match name.as_str() {
            "openai" | "anthropic" | "google" | "opencode" | "github-copilot"
            | "amazon-bedrock" | "azure" => name,
            _ => "other".into(),
        };
        result.push(Provider { provider, method });
    }
    result.sort_by(|a, b| (&a.provider, a.method).cmp(&(&b.provider, b.method)));
    result.dedup_by(|a, b| a.provider == b.provider && a.method == b.method);
    if had_entries && result.is_empty() {
        return Err(());
    }
    Ok(result)
}

fn identity(file: &File) -> Result<(u32, u32, u32), ()> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut info) } == 0 {
        return Err(());
    }
    if info.dwFileAttributes & (0x400 | 0x10) != 0 {
        return Err(());
    }
    Ok((
        info.dwVolumeSerialNumber,
        info.nFileIndexHigh,
        info.nFileIndexLow,
    ))
}

fn open(path: &Path) -> std::io::Result<File> {
    // Sharing all operations keeps the client's own token refresh/atomic replacement unblocked.
    OpenOptions::new()
        .read(true)
        .share_mode(7)
        .custom_flags(0x0020_0000)
        .open(path)
}

fn read_root(root: &Path) -> Result<Vec<Provider>, &'static str> {
    if !root.is_absolute() {
        return Err("unsafe_store");
    }
    let mut ancestor = root.to_owned();
    loop {
        match fs::symlink_metadata(&ancestor) {
            Ok(_) => break,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                ancestor = ancestor.parent().ok_or("unsafe_store")?.to_owned();
            }
            Err(_) => return Err("unsafe_store"),
        }
    }
    let pins = crate::integrations::config_io::pin_existing_directory_chain(&ancestor)
        .map_err(|_| "unsafe_store")?;
    if ancestor != root {
        return Ok(vec![]);
    }
    let path = root.join("auth.json");
    let mut file = match open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(_) => return Err("unsafe_store"),
    };
    let before = file.metadata().map_err(|_| "unsafe_store")?;
    if !before.is_file() || before.file_attributes() & 0x400 != 0 || before.len() > 262144 {
        return Err("unsafe_store");
    }
    let seen = identity(&file).map_err(|_| "unsafe_store")?;
    let result = metadata(&mut file).map_err(|_| "invalid_store")?;
    let after = file.metadata().map_err(|_| "unsafe_store")?;
    let current = open(&path).map_err(|_| "unsafe_store")?;
    if identity(&current).ok() != Some(seen)
        || after.len() != before.len()
        || after.last_write_time() != before.last_write_time()
        || pins.validate().is_err()
    {
        return Err("unsafe_store");
    }
    Ok(result)
}

pub(super) fn read(i: &InstallationSummary) -> Account {
    let mut account = unknown(i, "environment_auth");
    account.login_supported = true;
    if std::env::var_os("OPENCODE_AUTH_CONTENT").is_some() {
        return account;
    }
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(|p| std::path::PathBuf::from(p).join(".local/share"))
        });
    let Some(data) = data else {
        return account;
    };
    match read_root(&data.join("opencode")) {
        Ok(providers) => {
            account.state = if providers.is_empty() {
                "absent"
            } else {
                "present"
            };
            account.method = if providers.is_empty() {
                "none"
            } else {
                "provider_credentials"
            };
            account.source = "local_store";
            account.reason = "none";
            account.providers = providers;
        }
        Err(reason) => account.reason = reason,
    }
    account
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stored_provider_metadata_discards_secrets_and_untrusted_provider_names() {
        let input = br#"{"openai":{"type":"api","key":"secret-sentinel"},"private-sentinel":{"type":"oauth","access":"access-sentinel","refresh":"refresh-sentinel","expires":123}}"#;
        let v = metadata(input.as_slice()).unwrap();
        let encoded = serde_json::to_string(&v).unwrap();
        assert_eq!(v.len(), 2);
        assert!(!encoded.contains("sentinel"));
        assert!(
            encoded.contains("openai") && encoded.contains("other") && encoded.contains("oauth")
        );
        assert!(metadata(b"{}".as_slice()).unwrap().is_empty());
        assert!(metadata(br#"{"openai":{"type":"api","key":""}}"#.as_slice()).is_err());
        assert!(
            metadata(br#"{"openai":{"type":"oauth","access":"x","refresh":"y"}}"#.as_slice())
                .is_err()
        );
    }

    #[test]
    fn store_probe_is_passive_and_unsafe_or_oversized_inputs_stay_unknown() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("work/proofs/accounts/store-tests")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        assert!(read_root(&root.join("missing")).unwrap().is_empty());
        assert!(!root.join("missing").exists());
        let path = root.join("auth.json");
        let bytes =
            br#"{"anthropic":{"type":"oauth","access":"fixture","refresh":"fixture","expires":1}}"#;
        fs::write(&path, bytes).unwrap();
        assert_eq!(read_root(&root).unwrap().len(), 1);
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fs::write(&path, vec![b' '; 262145]).unwrap();
        assert!(read_root(&root).is_err());
        fs::remove_file(&path).unwrap();
        fs::remove_dir(&root).unwrap();
    }
}
