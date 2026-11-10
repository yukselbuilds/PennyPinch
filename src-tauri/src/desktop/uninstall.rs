//! The installed binary runs this entry point before starting Tauri or its motor.
use crate::integrations::config_io::pin_existing_directory_chain;
use std::fs::{self, File};
use std::io;
use std::os::windows::{ffi::OsStrExt, fs::MetadataExt, io::FromRawHandle};
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::{ERROR_SHARING_VIOLATION, GENERIC_READ, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
    OPEN_ALWAYS, OPEN_EXISTING,
};

#[cfg(not(debug_assertions))]
pub(crate) fn enter(config: &tauri::Config) -> File {
    let mut args = std::env::args_os().skip(1);
    let removing = args.next().is_some_and(|arg| arg == "--headroom-uninstall");
    let root = if config.app.app_directories_override.is_none() {
        app_local_data(&config.identifier)
    } else {
        None
    };
    let Some(root) = root else {
        std::process::exit(12);
    };
    if removing {
        let mode = args.next();
        if args.next().is_some() || require_removal_gate(&root).is_err() {
            std::process::exit(12);
        }
        let delete = match mode.as_deref().and_then(std::ffi::OsStr::to_str) {
            Some("keep") => false,
            Some("delete") => true,
            _ => std::process::exit(12),
        };
        std::process::exit(prepare(&root, delete));
    }
    open_desktop_use(&root).unwrap_or_else(|_| std::process::exit(12))
}

#[cfg(any(test, not(debug_assertions)))]
pub(super) fn app_local_data(identifier: &str) -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::{
        System::Com::CoTaskMemFree,
        UI::Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath, KF_FLAG_DEFAULT},
    };
    let folder =
        unsafe { SHGetKnownFolderPath(&FOLDERID_LocalAppData, KF_FLAG_DEFAULT, None) }.ok()?;
    let base = PathBuf::from(OsString::from_wide(unsafe { folder.as_wide() }));
    unsafe { CoTaskMemFree(Some(folder.0.cast())) };
    Some(base.join(identifier))
}

#[cfg(any(test, not(debug_assertions)))]
pub(super) fn open_update_gate(root: &Path) -> io::Result<File> {
    open_gate(root, 0, OPEN_EXISTING)
}

#[cfg(not(debug_assertions))]
fn prepare(root: &Path, delete: bool) -> i32 {
    use crate::integrations::config_io::{ownership_store_for_app_local_data, PlanManager};
    use crate::state::choices::ChoiceStore;

    let local_data = Some(root.to_owned());
    let Ok(ownership) = ownership_store_for_app_local_data(local_data.clone()) else {
        return 10;
    };
    let plans = PlanManager::for_uninstall()
        .with_optional_ownership_store(Some(ownership))
        .with_claude_choice_store(ChoiceStore::for_app_local_data(local_data).ok());
    if plans.undo_for_uninstall().is_err() {
        return 10;
    }
    if super::autostart::Autostart::new()
        .remove_if_owned()
        .is_err()
        || delete && purge_local_state(root).is_err()
    {
        return 11;
    }
    0
}

fn gate_path(root: &Path) -> io::Result<PathBuf> {
    let mut name = root
        .file_name()
        .ok_or_else(|| io::Error::other("uninstall_gate_path"))?
        .to_os_string();
    name.push(".desktop-use");
    Ok(root.with_file_name(name))
}

fn open_desktop_use(root: &Path) -> io::Result<File> {
    open_gate(root, FILE_SHARE_READ, OPEN_ALWAYS)
}

fn require_removal_gate(root: &Path) -> io::Result<()> {
    let path = gate_path(root)?;
    validate_gate(&fs::symlink_metadata(&path)?)?;
    match open_gate(root, FILE_SHARE_READ, OPEN_EXISTING) {
        Err(error) if error.raw_os_error() == Some(ERROR_SHARING_VIOLATION as i32) => Ok(()),
        _ => Err(io::Error::other("uninstall_gate_not_exclusive")),
    }
}

fn validate_gate(metadata: &fs::Metadata) -> io::Result<()> {
    if !metadata.is_file()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || metadata.len() != 0
    {
        return Err(io::Error::other("uninstall_gate_unsafe"));
    }
    Ok(())
}

fn open_gate(root: &Path, share: u32, disposition: u32) -> io::Result<File> {
    let path = gate_path(root)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("uninstall_gate_path"))?;
    let pins = pin_existing_directory_chain(parent)
        .map_err(|_| io::Error::other("uninstall_gate_path"))?;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ,
            share,
            std::ptr::null(),
            disposition,
            FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let file = unsafe { File::from_raw_handle(handle) };
    validate_gate(&file.metadata()?)?;
    pins.validate()
        .map_err(|_| io::Error::other("uninstall_gate_path"))?;
    Ok(file)
}

fn purge_local_state(root: &Path) -> io::Result<()> {
    let parent = root
        .parent()
        .ok_or_else(|| io::Error::other("uninstall_data_path"))?;
    let pins = pin_existing_directory_chain(parent)
        .map_err(|_| io::Error::other("uninstall_data_path"))?;
    match fs::symlink_metadata(root) {
        Ok(metadata)
            if metadata.is_dir()
                && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0 =>
        {
            pins.validate()
                .map_err(|_| io::Error::other("uninstall_data_path"))?;
            fs::remove_dir_all(root)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        _ => Err(io::Error::other("uninstall_data_path")),
    }
}

#[cfg(test)]
#[path = "uninstall_tests.rs"]
mod tests;
