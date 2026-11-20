use super::uninstall::{app_local_data, open_update_gate};
use super::update::{
    verify_manifest, windows_runtime_resources_sha256, UpdateChannel, UpdatePolicy, VerifiedUpdate,
    MANIFEST_LIMIT, SIGNATURE_LIMIT, UPDATE_STATE_FORMAT,
};
use super::update_commit::{copy_verified, create_bytes, read_owned, read_small, FileDigest, Swap};
use super::update_flow::{matches_configured_release, Recovery};
use crate::integrations::config_io::{pin_existing_directory_chain, PinnedDirectoryChain};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io::{self, Seek, SeekFrom};
use std::mem::size_of_val;
use std::os::windows::{ffi::OsStrExt, process::CommandExt};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;
use windows_sys::Win32::Foundation::{ERROR_SHARING_VIOLATION, ERROR_SUCCESS};
use windows_sys::Win32::System::{Registry::*, Threading::CREATE_NO_WINDOW};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES, MB_ICONERROR, MB_ICONQUESTION, MB_OK, MB_YESNO,
};

const WORKER: &str = "Headroom Recovery.exe";
const RECORD: &str = "recovery.json";
const APP: &str = "application.exe";
const MANIFEST: &str = "manifest.json";
const SIGNATURE: &str = "manifest.json.sig";
const MAX_RECORDS: usize = 32;

#[derive(Clone)]
pub(crate) struct Installation {
    identifier: String,
    registry_key: Vec<u16>,
    version: Version,
    app_data: PathBuf,
}

struct Registered {
    exe: PathBuf,
    version: Version,
    pins: PinnedDirectoryChain,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema_version: u8,
    exe: PathBuf,
    previous_version: String,
    next_version: String,
    previous: FileDigest,
    next: FileDigest,
    channel: UpdateChannel,
}

pub(crate) struct Launch {
    path: PathBuf,
    worker: Option<File>,
    pins: Option<PinnedDirectoryChain>,
    restoring: bool,
    own_pending: bool,
}

fn invalid(code: &'static str) -> io::Error {
    io::Error::other(code)
}

fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}

fn pin(path: &Path) -> io::Result<PinnedDirectoryChain> {
    pin_existing_directory_chain(path).map_err(|_| invalid("update_directory"))
}

fn same_path(first: &Path, second: &Path) -> io::Result<bool> {
    Ok(fs::canonicalize(first)? == fs::canonicalize(second)?)
}

fn as_version(value: &str) -> io::Result<Version> {
    Version::parse(value).map_err(|_| invalid("update_version"))
}

impl Installation {
    pub(crate) fn new(config: &tauri::Config, version: Version) -> Option<Self> {
        let name = config.product_name.as_deref()?;
        if config.app.app_directories_override.is_some()
            || name.is_empty()
            || name.contains(['\\', '/', '\0'])
        {
            return None;
        }
        let identifier = config.identifier.clone();
        let app_data = app_local_data(&identifier)?;
        Some(Self {
            identifier,
            registry_key: wide(format!(
                r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{name}"
            )),
            version,
            app_data,
        })
    }

    fn registry_text(&self, name: &str) -> io::Result<String> {
        let mut value = [0u16; 1024];
        let mut size = size_of_val(&value) as u32;
        let code = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                self.registry_key.as_ptr(),
                wide(name).as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                value.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if code != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(code as i32));
        }
        let value = value[..size as usize / 2]
            .strip_suffix(&[0])
            .ok_or_else(|| invalid("update_registration"))?;
        String::from_utf16(value)
            .ok()
            .filter(|value| !value.contains('\0'))
            .ok_or_else(|| invalid("update_registration"))
    }

    fn set_version(&self, version: &str) -> io::Result<()> {
        let mut key = std::ptr::null_mut();
        let code = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                self.registry_key.as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut key,
            )
        };
        if code != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(code as i32));
        }
        let version = wide(version);
        let code = unsafe {
            RegSetValueExW(
                key,
                wide("DisplayVersion").as_ptr(),
                0,
                REG_SZ,
                version.as_ptr().cast(),
                size_of_val(version.as_slice()) as u32,
            )
        };
        unsafe { RegCloseKey(key) };
        if code != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(code as i32));
        }
        Ok(())
    }

    fn registered(&self) -> io::Result<Registered> {
        let path = self.registry_text("InstallLocation")?;
        let root = PathBuf::from(
            path.strip_prefix('"')
                .and_then(|path| path.strip_suffix('"'))
                .unwrap_or(&path),
        );
        let pins = pin(&root)?;
        let file = self.registry_text("MainBinaryName")?;
        let mut parts = Path::new(&file).components();
        if !matches!(parts.next(), Some(Component::Normal(_)))
            || parts.next().is_some()
            || !Path::new(&file)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
            || file.contains(':')
        {
            return Err(invalid("update_registered_file"));
        }
        read_owned(&root.join("uninstall.exe"))?;
        let exe = root.join(file);
        read_owned(&exe)?;
        let version = as_version(&self.registry_text("DisplayVersion")?)?;
        Ok(Registered { exe, version, pins })
    }

    fn current(&self) -> io::Result<Registered> {
        let installation = self.registered()?;
        if !same_path(&installation.exe, &std::env::current_exe()?)? {
            return Err(invalid("update_install_identity"));
        }
        Ok(installation)
    }

    pub(crate) fn can_apply(&self) -> bool {
        self.current()
            .is_ok_and(|installation| installation.version == self.version)
    }

    fn recovery_root(&self) -> PathBuf {
        self.app_data.join("update-recovery")
    }

    fn recovery_dir(&self, id: Uuid) -> PathBuf {
        self.recovery_root().join(id.to_string())
    }

    pub(crate) fn recovery(&self) -> Option<Recovery> {
        let current = self.current().ok()?;
        let root = self.recovery_root();
        let _pins = pin(&root).ok()?;
        let mut digest = None;
        let mut found: Option<(FileDigest, Recovery)> = None;
        for entry in fs::read_dir(root).ok()?.take(MAX_RECORDS + 1).enumerate() {
            let (index, entry) = entry;
            if index == MAX_RECORDS {
                return None;
            }
            let entry = entry.ok()?;
            let Some(id) = entry
                .file_name()
                .to_str()
                .and_then(|name| Uuid::parse_str(name).ok())
            else {
                continue;
            };
            let Ok((_pins, record)) = read_record(&self.recovery_dir(id)) else {
                continue;
            };
            if record.exe != current.exe || record.next_version != self.version.to_string() {
                continue;
            }
            if digest.is_none() {
                digest = Some(FileDigest::from_file(&mut read_owned(&current.exe).ok()?).ok()?);
            }
            if digest.as_ref() != Some(&record.next) {
                continue;
            }
            if record
                .previous
                .check(&mut read_owned(&self.recovery_dir(id).join(WORKER)).ok()?)
                .is_err()
            {
                continue;
            }
            if let Some((previous, recovery)) = &found {
                if previous != &record.previous || recovery.version != record.previous_version {
                    return None;
                }
                continue;
            }
            found = Some((
                record.previous,
                Recovery {
                    id: id.to_string(),
                    version: record.previous_version,
                    can_restore: false,
                },
            ));
        }
        found.map(|(_, recovery)| recovery)
    }

    pub(crate) fn prepare(&self, update: &VerifiedUpdate, staged: &mut File) -> io::Result<Launch> {
        let installed = self.current()?;
        if installed.version != self.version {
            return Err(invalid("update_registered_version"));
        }
        let (manifest, signature) = update.signed().ok_or_else(|| invalid("update_unsigned"))?;
        staged.seek(SeekFrom::Start(0))?;
        update
            .verify_application(&mut *staged)
            .map_err(|_| invalid("update_application"))?;
        let next = FileDigest::from_file(staged)?;
        let mut live = read_owned(&installed.exe)?;
        let previous = FileDigest::from_file(&mut live)?;
        let _data_pins = pin(&self.app_data)?;
        let root = self.recovery_root();
        match fs::create_dir(&root) {
            Ok(()) => (),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(error),
        }
        let _root_pins = pin(&root)?;
        self.prune_history(&installed.exe, &previous)?;
        if fs::read_dir(&root)?.take(MAX_RECORDS).count() >= MAX_RECORDS {
            return Err(invalid("update_recovery_limit"));
        }
        let directory = self.recovery_dir(Uuid::new_v4());
        fs::create_dir(&directory)?;
        let result = (|| {
            let pins = pin(&directory)?;
            let path = directory.join(WORKER);
            copy_verified(&path, &mut live, &previous)?;
            copy_verified(&directory.join(APP), staged, &next)?;
            create_bytes(&directory.join(MANIFEST), manifest)?;
            create_bytes(&directory.join(SIGNATURE), signature.as_bytes())?;
            let record = Record {
                schema_version: 1,
                exe: installed.exe,
                previous_version: self.version.to_string(),
                next_version: update.version().to_string(),
                previous,
                next,
                channel: update.channel(),
            };
            let bytes = serde_json::to_vec(&record)?;
            create_bytes(&directory.join(RECORD), &bytes)?;
            let worker = read_owned(&path)?;
            Ok(Launch {
                path,
                worker: Some(worker),
                pins: Some(pins),
                restoring: false,
                own_pending: true,
            })
        })();
        if result.is_err() {
            remove_record(&directory);
        }
        result
    }

    pub(crate) fn prepare_restore(&self, id: Uuid) -> io::Result<Launch> {
        self.recovery()
            .filter(|recovery| recovery.id == id.to_string())
            .ok_or_else(|| invalid("update_recovery_changed"))?;
        let directory = self.recovery_dir(id);
        let (pins, record) = read_record(&directory)?;
        let path = directory.join(WORKER);
        let mut worker = read_owned(&path)?;
        record.previous.check(&mut worker)?;
        Ok(Launch {
            path,
            worker: Some(worker),
            pins: Some(pins),
            restoring: true,
            own_pending: false,
        })
    }

    fn exclusive(&self) -> io::Result<File> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match open_update_gate(&self.app_data) {
                Err(error)
                    if error.raw_os_error() == Some(ERROR_SHARING_VIOLATION as i32)
                        && Instant::now() < deadline =>
                {
                    thread::sleep(Duration::from_millis(50));
                }
                result => return result,
            }
        }
    }

    fn signed_record(&self, directory: &Path, record: &Record) -> io::Result<VerifiedUpdate> {
        let public_key = option_env!("HEADROOM_DESKTOP_UPDATE_PUBLIC_KEY")
            .ok_or_else(|| invalid("update_source_unavailable"))?;
        let manifest = read_small(&directory.join(MANIFEST), MANIFEST_LIMIT as u64)?;
        let signature = read_small(&directory.join(SIGNATURE), SIGNATURE_LIMIT as u64)?;
        let signature =
            std::str::from_utf8(&signature).map_err(|_| invalid("update_signed_record"))?;
        let previous_version = as_version(&record.previous_version)?;
        let verified = verify_manifest(
            &manifest,
            signature,
            &UpdatePolicy {
                app_identifier: &self.identifier,
                installed_version: &previous_version,
                channel: record.channel,
                trusted_tauri_public_key: public_key,
            },
        )
        .map_err(|_| invalid("update_signed_record"))?;
        if verified.version().to_string() != record.next_version
            || verified.application_size().is_none()
        {
            return Err(invalid("update_signed_record"));
        }
        Ok(verified)
    }

    fn prune_history(&self, live: &Path, current: &FileDigest) -> io::Result<()> {
        let root = self.recovery_root();
        let _root_pins = pin(&root)?;
        let mut entries = fs::read_dir(&root)?
            .take(MAX_RECORDS + 1)
            .collect::<io::Result<Vec<_>>>()?;
        if entries.len() > MAX_RECORDS {
            return Err(invalid("update_recovery_limit"));
        }
        entries.sort_by_key(|entry| entry.file_name());
        let mut kept = HashSet::new();
        for entry in entries {
            let name = entry.file_name();
            let Some(id) = name.to_str().and_then(|name| Uuid::parse_str(name).ok()) else {
                continue;
            };
            if id.is_nil() || name != OsStr::new(&id.to_string()) {
                continue;
            }
            let directory = self.recovery_dir(id);
            let Ok(pins) = pin(&directory) else {
                continue;
            };
            let Ok(files) =
                fs::read_dir(&directory).and_then(|files| files.collect::<io::Result<Vec<_>>>())
            else {
                continue;
            };
            if files.iter().any(|file| {
                ![APP, MANIFEST, SIGNATURE, RECORD, WORKER]
                    .iter()
                    .any(|name| file.file_name() == OsStr::new(name))
            }) {
                continue;
            }
            let snapshots = files
                .iter()
                .map(|entry| {
                    let path = entry.path();
                    let mut lease = read_owned(&path)?;
                    let digest = FileDigest::from_file(&mut lease)?;
                    Ok((path, lease, digest))
                })
                .collect::<io::Result<Vec<_>>>();
            let Ok(snapshots) = snapshots else { continue };
            let Ok(record) = read_record(&directory).map(|(_, record)| record) else {
                continue;
            };
            if record.exe != live {
                continue;
            }
            let Ok(signed) = self.signed_record(&directory, &record) else {
                continue;
            };
            if signed.application_size() != Some(record.next.size)
                || signed.application_sha256() != Some(record.next.sha256.as_str())
            {
                continue;
            }
            let Ok(mut backup) = read_owned(&directory.join(WORKER)) else {
                continue;
            };
            if record.previous.check(&mut backup).is_err() {
                continue;
            }
            drop(backup);
            if record.next == *current
                && kept.insert((
                    record.previous.size,
                    record.previous.sha256.clone(),
                    record.previous_version.clone(),
                ))
            {
                continue;
            }
            if directory.join(APP).exists() {
                let Ok(mut application) = read_owned(&directory.join(APP)) else {
                    continue;
                };
                if record.next.check(&mut application).is_err() {
                    continue;
                }
            }
            let expected: Vec<_> = snapshots
                .into_iter()
                .map(|(path, lease, digest)| {
                    drop(lease);
                    (path, digest)
                })
                .collect();
            if remove_history_files(&pins, &expected).is_ok() {
                drop(pins);
                let _ = fs::remove_dir(&directory);
            }
        }
        Ok(())
    }

    fn apply(&self, directory: &Path, record: &Record) -> io::Result<()> {
        let gate = self.exclusive()?;
        let installed = self.registered()?;
        if installed.exe != record.exe
            || installed.version != self.version
            || record.previous_version != self.version.to_string()
        {
            return Err(invalid("update_install_changed"));
        }
        let verified = self.signed_record(directory, record)?;
        let mut staged = read_owned(&directory.join(APP))?;
        verified
            .verify_application(&mut staged)
            .map_err(|_| invalid("update_application"))?;
        record.next.check(&mut staged)?;
        let swap = Swap::stage(&installed.exe, &mut staged, &record.next)?;
        {
            let _sealed = swap.seal()?;
            probe(swap.path(), &self.identifier, &record.next_version)?;
        }
        installed
            .pins
            .validate()
            .map_err(|_| invalid("update_install_changed"))?;
        swap.publish(&record.previous)?;
        if let Err(error) = self.set_version(&record.next_version) {
            self.restore_locked(directory, record, &installed)?;
            drop(gate);
            let _ = Command::new(&installed.exe).spawn();
            return Err(error);
        }
        drop(staged);
        let _ = fs::remove_file(directory.join(APP));
        drop(gate);
        if let Err(error) = restart(&installed.exe) {
            self.restore(directory, record)?;
            return Err(error);
        }
        Ok(())
    }

    fn restore_locked(
        &self,
        directory: &Path,
        record: &Record,
        installed: &Registered,
    ) -> io::Result<()> {
        if installed.exe != record.exe || record.previous_version != self.version.to_string() {
            return Err(invalid("update_recovery_changed"));
        }
        let mut backup = read_owned(&directory.join(WORKER))?;
        record.previous.check(&mut backup)?;
        Swap::stage(&installed.exe, &mut backup, &record.previous)?.publish(&record.next)?;
        self.set_version(&record.previous_version)
    }

    fn restore(&self, directory: &Path, record: &Record) -> io::Result<()> {
        let gate = self.exclusive()?;
        let installed = self.registered()?;
        self.signed_record(directory, record)?
            .verify_application(&mut read_owned(&installed.exe)?)
            .map_err(|_| invalid("update_recovery_changed"))?;
        self.restore_locked(directory, record, &installed)?;
        drop(gate);
        Command::new(&installed.exe).spawn()?;
        Ok(())
    }
}

fn read_record(directory: &Path) -> io::Result<(PinnedDirectoryChain, Record)> {
    let pins = pin(directory)?;
    let record: Record = serde_json::from_slice(&read_small(&directory.join(RECORD), 8192)?)?;
    if record.schema_version != 1
        || as_version(&record.next_version)? <= as_version(&record.previous_version)?
    {
        return Err(invalid("update_recovery_record"));
    }
    Ok((pins, record))
}

fn remove_record(directory: &Path) {
    let Ok(pins) = pin(directory) else { return };
    for name in [APP, MANIFEST, SIGNATURE, RECORD, WORKER] {
        let _ = fs::remove_file(directory.join(name));
    }
    drop(pins);
    let _ = fs::remove_dir(directory);
}

fn remove_history_files(
    pins: &PinnedDirectoryChain,
    entries: &[(PathBuf, FileDigest)],
) -> io::Result<()> {
    use std::fs::OpenOptions;
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Foundation::GENERIC_READ;
    use windows_sys::Win32::Storage::FileSystem::{
        FileDispositionInfo, SetFileInformationByHandle, DELETE, FILE_DISPOSITION_INFO,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
    };
    let mut files = Vec::new();
    for (path, expected) in entries {
        let mut file = OpenOptions::new()
            .read(true)
            .access_mode(GENERIC_READ | DELETE)
            .share_mode(FILE_SHARE_READ)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;
        expected.check(&mut file)?;
        files.push(file);
    }
    pins.validate()
        .map_err(|_| invalid("update_recovery_path"))?;
    for file in &files {
        let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
        if unsafe {
            SetFileInformationByHandle(
                file.as_raw_handle(),
                FileDispositionInfo,
                std::ptr::addr_of!(disposition).cast(),
                size_of_val(&disposition) as u32,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    drop(files);
    Ok(())
}

impl Launch {
    pub(crate) fn spawn(mut self) -> io::Result<()> {
        let result = quiet(&self.path)
            .arg(if self.restoring {
                "--headroom-restore-update"
            } else {
                "--headroom-apply-update"
            })
            .spawn();
        if result.is_ok() {
            self.own_pending = false;
        }
        result.map(|_| ())
    }
}

impl Drop for Launch {
    fn drop(&mut self) {
        drop(self.worker.take());
        drop(self.pins.take());
        if self.own_pending {
            if let Some(directory) = self.path.parent() {
                remove_record(directory);
            }
        }
    }
}

fn quiet(exe: &Path) -> Command {
    let mut command = Command::new(exe);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);
    command
}

fn early_status(
    child: &mut Child,
    timeout: Duration,
) -> io::Result<Option<std::process::ExitStatus>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn probe(exe: &Path, identifier: &str, version: &str) -> io::Result<()> {
    let mut child = quiet(exe)
        .args([
            "--headroom-update-probe",
            identifier,
            version,
            &UPDATE_STATE_FORMAT.to_string(),
        ])
        .spawn()?;
    match early_status(&mut child, Duration::from_secs(8)) {
        Ok(Some(status)) if status.success() => Ok(()),
        Ok(Some(_)) => Err(invalid("update_probe")),
        _ => {
            let _ = child.kill();
            let _ = child.wait();
            Err(invalid("update_probe"))
        }
    }
}

fn restart(exe: &Path) -> io::Result<()> {
    let mut child = Command::new(exe).spawn()?;
    if early_status(&mut child, Duration::from_secs(5))?.is_some_and(|status| !status.success()) {
        return Err(invalid("update_restart"));
    }
    Ok(())
}

fn message(text: &str, flags: u32) -> i32 {
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide(text).as_ptr(),
            wide("Headroom").as_ptr(),
            flags,
        )
    }
}

pub(crate) fn enter(config: &tauri::Config, version: &Version) {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if args
        .first()
        .is_some_and(|arg| arg == "--headroom-update-probe")
    {
        let expected = [
            OsString::from(&config.identifier),
            OsString::from(version.to_string()),
            OsString::from(UPDATE_STATE_FORMAT.to_string()),
        ];
        let valid = match args[1..].strip_prefix(expected.as_slice()) {
            Some([]) => true,
            Some([_, _, pin]) if pin != windows_runtime_resources_sha256().as_str() => false,
            Some([base, key] | [base, key, _]) => base
                .to_str()
                .zip(key.to_str())
                .is_some_and(|(base, key)| matches_configured_release(base, key)),
            _ => false,
        };
        std::process::exit(if valid { 0 } else { 2 });
    }
    let worker = std::env::current_exe()
        .ok()
        .filter(|path| path.file_name() == Some(OsStr::new(WORKER)));
    if worker.is_none()
        && !args.first().is_some_and(|arg| {
            arg == "--headroom-apply-update" || arg == "--headroom-restore-update"
        })
    {
        return;
    }
    let result = (|| {
        let worker = worker.ok_or_else(|| invalid("update_worker"))?;
        let installation = Installation::new(config, version.clone())
            .ok_or_else(|| invalid("update_installation"))?;
        let directory = worker.parent().ok_or_else(|| invalid("update_worker"))?;
        let id = directory
            .file_name()
            .and_then(OsStr::to_str)
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or_else(|| invalid("update_worker"))?;
        if !same_path(directory, &installation.recovery_dir(id))? {
            return Err(invalid("update_worker"));
        }
        let (_pins, record) = read_record(directory)?;
        record.previous.check(&mut read_owned(&worker)?)?;
        match args.as_slice() {
            [arg] if arg == "--headroom-apply-update" => installation.apply(directory, &record),
            [arg] if arg == "--headroom-restore-update" => installation.restore(directory, &record),
            [] => {
                if message(
                    "Restore the previous version?\nÖnceki sürüme dönülsün mü?",
                    MB_YESNO | MB_ICONQUESTION,
                ) == IDYES
                {
                    installation.restore(directory, &record)?;
                }
                Ok(())
            }
            _ => Err(invalid("update_worker_args")),
        }
    })();
    if result.is_err() {
        message("Headroom could not complete the update or recovery.\nGüncelleme veya kurtarma tamamlanamadı.", MB_OK | MB_ICONERROR);
    }
    std::process::exit(if result.is_ok() { 0 } else { 2 });
}

#[cfg(test)]
#[path = "update_install_tests.rs"]
mod tests;
