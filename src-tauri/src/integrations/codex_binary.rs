//! Read-only inspection of signed Codex binaries. Recognition is not a traffic proof.
#[cfg(windows)]
pub(super) fn registered_desktop_roots() -> Vec<std::path::PathBuf> {
    use windows_sys::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
    use windows_sys::Win32::Storage::Packaging::Appx::{
        GetPackagePathByFullName, GetPackagesByPackageFamily,
    };
    let family = windows_sys::core::w!("OpenAI.Codex_2p2nqsd0c76g0");
    let mut count = 0;
    let mut length = 0;
    let status = unsafe {
        GetPackagesByPackageFamily(
            family,
            &mut count,
            std::ptr::null_mut(),
            &mut length,
            std::ptr::null_mut(),
        )
    };
    #[cfg(debug_assertions)]
    eprintln!("headroom_codex_packages status={status} count={count} chars={length}");
    if status != ERROR_INSUFFICIENT_BUFFER || count == 0 || count > 16 || length > 65536 {
        return Vec::new();
    }
    let mut names = vec![std::ptr::null_mut(); count as usize];
    let mut buffer = vec![0u16; length as usize];
    if unsafe {
        GetPackagesByPackageFamily(
            family,
            &mut count,
            names.as_mut_ptr(),
            &mut length,
            buffer.as_mut_ptr(),
        )
    } != 0
        || count as usize > names.len()
        || length as usize > buffer.len()
    {
        return Vec::new();
    }
    let mut roots = Vec::new();
    for pointer in names.into_iter().take(count as usize) {
        let start = pointer as usize;
        let base = buffer.as_ptr() as usize;
        let Some(offset) = start
            .checked_sub(base)
            .filter(|n| n % 2 == 0)
            .map(|n| n / 2)
        else {
            continue;
        };
        let Some(tail) = buffer.get(offset..) else {
            continue;
        };
        let Some(end) = tail.iter().position(|&c| c == 0) else {
            continue;
        };
        let name = &tail[..=end];
        let mut size = 0;
        if unsafe { GetPackagePathByFullName(name.as_ptr(), &mut size, std::ptr::null_mut()) }
            != ERROR_INSUFFICIENT_BUFFER
            || size == 0
            || size > 32768
        {
            continue;
        }
        let mut path = vec![0u16; size as usize];
        if unsafe { GetPackagePathByFullName(name.as_ptr(), &mut size, path.as_mut_ptr()) } != 0
            || size == 0
            || size as usize > path.len()
            || path[size as usize - 1] != 0
        {
            continue;
        }
        if let Ok(path) = String::from_utf16(&path[..size as usize - 1]) {
            roots.push(path.into());
        }
    }
    roots
}

#[cfg(not(windows))]
pub(super) fn registered_desktop_roots() -> Vec<std::path::PathBuf> {
    Vec::new()
}

pub(super) fn version_parts(version: &str) -> Option<(u32, u32, u32)> {
    if version.len() > 64 || !version.is_ascii() {
        return None;
    }
    let (core, suffix) = version.split_once('-').unwrap_or((version, ""));
    if version.contains('-')
        && (suffix.is_empty()
            || suffix.split('.').any(|part| {
                part.is_empty() || !part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
            }))
    {
        return None;
    }
    let numbers = core
        .split('.')
        .map(|part| {
            if part.is_empty()
                || part.len() > 5
                || (part.len() > 1 && part.starts_with('0'))
                || !part.bytes().all(|c| c.is_ascii_digit())
            {
                None
            } else {
                part.parse::<u32>().ok()
            }
        })
        .collect::<Option<Vec<_>>>()?;
    (numbers.len() == 3).then(|| (numbers[0], numbers[1], numbers[2]))
}

pub(super) fn cli_config_supported(version: &str) -> bool {
    matches!(version_parts(version), Some((0, minor, _)) if minor >= 163)
}

pub(super) fn desktop_version_supported(version: &str) -> bool {
    version_parts(version).is_some()
        || (version.len() <= 64
            && version.split('.').count() == 4
            && version.split('.').all(|part| {
                !part.is_empty()
                    && part.len() <= 5
                    && (part == "0" || !part.starts_with('0'))
                    && part.bytes().all(|c| c.is_ascii_digit())
            }))
}

pub(super) fn config_supported(surface: &str, version: Option<&str>) -> bool {
    match (surface, version) {
        ("cli", Some(version)) => cli_config_supported(version),
        // Desktop's app version is independent of its backend's CLI version.
        // macOS config/GUI traffic evidence is scoped to this signed app release.
        ("desktop", Some("26.1106.51428")) if cfg!(target_os = "macos") => true,
        ("desktop", Some(version)) if !cfg!(target_os = "macos") => {
            desktop_version_supported(version)
        }
        _ => false,
    }
}

#[cfg(windows)]
pub(super) fn signed_version(
    path: &std::path::Path,
    file: &std::fs::File,
    digest: &str,
) -> Option<String> {
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<VecDeque<(String, String)>>> = OnceLock::new();
    if !openai_signature(path, file) {
        return None;
    }
    let cache = CACHE.get_or_init(|| Mutex::new(VecDeque::new()));
    if let Some(version) = cache
        .lock()
        .ok()?
        .iter()
        .find(|(hash, _)| hash == digest)
        .map(|(_, v)| v.clone())
    {
        return Some(version);
    }
    let version = probe_version(path)?;
    let mut entries = cache.lock().ok()?;
    if entries.len() == 16 {
        entries.pop_front();
    }
    entries.push_back((digest.to_owned(), version.clone()));
    Some(version)
}

#[cfg(not(windows))]
pub(super) fn signed_version(
    _path: &std::path::Path,
    _file: &std::fs::File,
    _digest: &str,
) -> Option<String> {
    None
}

#[cfg(windows)]
pub(super) fn openai_signature(path: &std::path::Path, file: &std::fs::File) -> bool {
    use std::mem::size_of;
    use std::os::windows::{ffi::OsStrExt, io::AsRawHandle};
    use windows_sys::Win32::Security::{Cryptography::*, WinTrust::*};
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut info = WINTRUST_FILE_INFO {
        cbStruct: size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: wide.as_ptr(),
        hFile: file.as_raw_handle(),
        ..Default::default()
    };
    let mut data = WINTRUST_DATA {
        cbStruct: size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 { pFile: &mut info },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL
            | WTD_REVOCATION_CHECK_CHAIN_EXCLUDE_ROOT
            | WTD_DISABLE_MD2_MD4,
        ..Default::default()
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    let status = unsafe {
        WinVerifyTrust(
            -1isize as *mut _,
            &mut action,
            &mut data as *mut _ as *mut _,
        )
    };
    let trusted = if status == 0 {
        unsafe {
            let provider = WTHelperProvDataFromStateData(data.hWVTStateData);
            let signer = if provider.is_null() {
                std::ptr::null_mut()
            } else {
                WTHelperGetProvSignerFromChain(provider, 0, 0, 0)
            };
            if signer.is_null() || (*signer).csCertChain == 0 || (*signer).pasCertChain.is_null() {
                false
            } else {
                let certificate = (*(*signer).pasCertChain).pCert;
                let name = |oid: windows_sys::core::PCSTR| -> Option<String> {
                    if certificate.is_null() {
                        return None;
                    }
                    let mut buffer = [0u16; 128];
                    let count = CertGetNameStringW(
                        certificate,
                        CERT_NAME_ATTR_TYPE,
                        0,
                        oid.cast(),
                        buffer.as_mut_ptr(),
                        buffer.len() as u32,
                    ) as usize;
                    if count <= 1 || count > buffer.len() {
                        None
                    } else {
                        String::from_utf16(&buffer[..count - 1]).ok()
                    }
                };
                name(szOID_ORGANIZATION_NAME).as_deref() == Some("OpenAI OpCo, LLC")
                    && name(szOID_COMMON_NAME).as_deref() == Some("OpenAI OpCo, LLC")
            }
        }
    } else {
        false
    };
    data.dwStateAction = WTD_STATEACTION_CLOSE;
    unsafe {
        WinVerifyTrust(
            -1isize as *mut _,
            &mut action,
            &mut data as *mut _ as *mut _,
        )
    };
    trusted
}

#[cfg(windows)]
fn probe_version(path: &std::path::Path) -> Option<String> {
    use super::process::OwnedJob;
    use std::io::Read;
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let mut command = Command::new(path);
    command
        .arg("--version")
        .env_clear()
        .current_dir(path.parent()?)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(0x0800_0000);
    for key in ["SystemRoot", "WINDIR"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    // --version exits before loading config; no user auth/config environment is inherited.
    let unused_home = std::env::current_exe()
        .ok()?
        .parent()?
        .join("headroom-version-probe-unused");
    for key in [
        "CODEX_HOME",
        "HOME",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "TEMP",
        "TMP",
    ] {
        command.env(key, &unused_home);
    }
    let job = OwnedJob::create().ok()?;
    let mut child = command.spawn().ok()?;
    if job.assign(&child).is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    let stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.take(257).read_to_end(&mut bytes).ok().map(|_| bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let success = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
        }
    };
    // Also closes any inherited output handle before joining the bounded reader.
    if !job.terminate_and_wait_empty(Duration::from_secs(1)) {
        return None;
    }
    let bytes = reader.join().ok()??;
    if !success || bytes.len() > 256 {
        return None;
    }
    let text = std::str::from_utf8(&bytes).ok()?.trim();
    let version = text.strip_prefix("codex-cli ")?;
    version_parts(version)?;
    Some(version.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn codex_current_version_parser_rejects_ambiguous_or_unbounded_output() {
        for version in ["0.166.0", "0.166.0-alpha.5.1", "26.1103.1"] {
            assert!(version_parts(version).is_some());
        }
        for version in [
            "",
            "0.166",
            "0.166.0\nextra",
            "0.166.0-",
            "0.166.0-a..b",
            "00.166.0",
            "0.999999.0",
            "0.166.0-é",
        ] {
            assert!(version_parts(version).is_none(), "{version}");
        }
        assert!(!cli_config_supported("1.0.0"));
        assert!(!cli_config_supported("0.162.0"));
    }
    #[cfg(windows)]
    #[test]
    fn codex_current_signature_rejects_an_unsigned_or_foreign_executable() {
        let path = std::env::current_exe().unwrap();
        let file = std::fs::File::open(&path).unwrap();
        assert!(!openai_signature(&path, &file));
        let path = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/notepad.exe");
        let file = std::fs::File::open(&path).unwrap();
        assert!(!openai_signature(&path, &file));
    }
}
