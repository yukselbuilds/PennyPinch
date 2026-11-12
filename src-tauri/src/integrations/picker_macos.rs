use dispatch2::DispatchQueue;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSModalResponseCancel, NSModalResponseOK, NSOpenPanel};
use objc2_foundation::{NSString, NSURL};
use std::ffi::{CStr, CString, OsStr};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;

pub fn select_file(owner: usize, kind: &str) -> Result<Option<PathBuf>, ()> {
    let title = match kind {
        "codex" => "Select Codex command",
        "claude_code" => "Select Claude Code command",
        "opencode" => "Select OpenCode command",
        _ => return Err(()),
    };
    select(owner, title, false, None)
}

pub fn select_project_directory(
    owner: usize,
    initial_directory: Option<PathBuf>,
) -> Result<Option<PathBuf>, ()> {
    select(
        owner,
        "Select Claude Code project folder",
        true,
        initial_directory.as_deref(),
    )
}

fn select(
    owner: usize,
    title: &str,
    directory: bool,
    initial_directory: Option<&Path>,
) -> Result<Option<PathBuf>, ()> {
    if owner == 0 {
        return Err(());
    }
    let mut result = Err(());
    let mut show = || {
        result = MainThreadMarker::new()
            .ok_or(())
            .and_then(|mtm| open_panel(mtm, title, directory, initial_directory));
    };
    if MainThreadMarker::new().is_some() {
        show();
    } else {
        DispatchQueue::main().exec_sync(show);
    }
    result
}

fn open_panel(
    mtm: MainThreadMarker,
    title: &str,
    directory: bool,
    initial_directory: Option<&Path>,
) -> Result<Option<PathBuf>, ()> {
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setTitle(Some(&NSString::from_str(title)));
    panel.setCanChooseFiles(!directory);
    panel.setCanChooseDirectories(directory);
    panel.setAllowsMultipleSelection(false);
    panel.setResolvesAliases(false);
    panel.setCanCreateDirectories(false);
    panel.setTreatsFilePackagesAsDirectories(!directory);
    panel.setShowsHiddenFiles(true);
    if let Some(path) = initial_directory {
        if !path.is_absolute() {
            return Err(());
        }
        let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| ())?;
        // The C string remains alive for the complete URL constructor call.
        let url = unsafe {
            NSURL::fileURLWithFileSystemRepresentation_isDirectory_relativeToURL(
                NonNull::new(path.as_ptr().cast_mut()).ok_or(())?,
                true,
                None,
            )
        };
        panel.setDirectoryURL(Some(&url));
    }
    let response = panel.runModal();
    if response == unsafe { NSModalResponseCancel } {
        return Ok(None);
    }
    if response != unsafe { NSModalResponseOK } {
        return Err(());
    }
    let urls = panel.URLs();
    if urls.count() != 1 {
        return Err(());
    }
    let url = urls.objectAtIndex(0);
    if !url.isFileURL() {
        return Err(());
    }
    // NSURL owns a NUL-terminated file-system representation.
    let bytes = unsafe { CStr::from_ptr(url.fileSystemRepresentation().as_ptr()) }.to_bytes();
    if bytes.is_empty() || bytes.len() >= libc::PATH_MAX as usize {
        return Err(());
    }
    let path = PathBuf::from(OsStr::from_bytes(bytes));
    path.is_absolute().then_some(Some(path)).ok_or(())
}
