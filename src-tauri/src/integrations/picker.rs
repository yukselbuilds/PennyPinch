use std::path::PathBuf;

#[cfg(windows)]
mod windows_picker {
    use super::PathBuf;
    use std::ffi::{c_void, OsString};
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows::core::{HRESULT, PCWSTR, PWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FileOpenDialog, FileSaveDialog, IFileOpenDialog, IFileSaveDialog, IShellItem,
        SHCreateItemFromParsingName, FOS_DONTADDTORECENT, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM,
        FOS_NOCHANGEDIR, FOS_NODEREFERENCELINKS, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS,
        FOS_STRICTFILETYPES, SIGDN_FILESYSPATH,
    };

    const MAX_PATH_UNITS: usize = 32_767;
    const HRESULT_CANCELLED: HRESULT = HRESULT(0x8007_04C7u32 as i32);

    struct ComApartment;

    impl ComApartment {
        fn initialize() -> Result<Self, ()> {
            let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
            if result.0 >= 0 {
                Ok(Self)
            } else {
                Err(())
            }
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            unsafe { CoUninitialize() };
        }
    }

    struct TaskMemory(PWSTR);

    impl Drop for TaskMemory {
        fn drop(&mut self) {
            if !self.0 .0.is_null() {
                unsafe { CoTaskMemFree(Some(self.0 .0.cast::<c_void>())) };
            }
        }
    }

    fn filesystem_path(item: IShellItem) -> Result<PathBuf, ()> {
        let display_name = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH).map_err(|_| ())? };
        let memory = TaskMemory(display_name);
        let raw = memory.0 .0;
        if raw.is_null() {
            return Err(());
        }
        let length = (0..=MAX_PATH_UNITS)
            .find(|&index| unsafe { raw.add(index).read() == 0 })
            .ok_or(())?;
        if length == 0 {
            return Err(());
        }
        let units = unsafe { std::slice::from_raw_parts(raw, length) };
        Ok(PathBuf::from(OsString::from_wide(units)))
    }

    pub(super) fn save_report(
        owner_hwnd: usize,
        name: &str,
        extension: &str,
        turkish: bool,
    ) -> Result<Option<PathBuf>, ()> {
        if owner_hwnd == 0 || !matches!(extension, "json" | "csv") {
            return Err(());
        }
        let _apartment = ComApartment::initialize()?;
        let dialog: IFileSaveDialog = unsafe {
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER).map_err(|_| ())?
        };
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let title = wide(if turkish {
            "Raporu kaydet"
        } else {
            "Save report"
        });
        let name = wide(name);
        let extension = wide(extension);
        let pattern = wide(if extension[0] == b'j' as u16 {
            "*.json"
        } else {
            "*.csv"
        });
        let label = wide(if extension[0] == b'j' as u16 {
            "JSON"
        } else {
            "CSV (UTF-8)"
        });
        let filters = [COMDLG_FILTERSPEC {
            pszName: PCWSTR(label.as_ptr()),
            pszSpec: PCWSTR(pattern.as_ptr()),
        }];
        unsafe {
            dialog
                .SetOptions(
                    FOS_FORCEFILESYSTEM
                        | FOS_PATHMUSTEXIST
                        | FOS_NOCHANGEDIR
                        | FOS_NODEREFERENCELINKS
                        | FOS_DONTADDTORECENT
                        | FOS_STRICTFILETYPES,
                )
                .map_err(|_| ())?;
            dialog.SetFileTypes(&filters).map_err(|_| ())?;
            dialog
                .SetDefaultExtension(PCWSTR(extension.as_ptr()))
                .map_err(|_| ())?;
            dialog.SetFileName(PCWSTR(name.as_ptr())).map_err(|_| ())?;
            dialog.SetTitle(PCWSTR(title.as_ptr())).map_err(|_| ())?;
            if let Err(error) = dialog.Show(Some(HWND(owner_hwnd as *mut c_void))) {
                return if error.code() == HRESULT_CANCELLED {
                    Ok(None)
                } else {
                    Err(())
                };
            }
            filesystem_path(dialog.GetResult().map_err(|_| ())?).map(Some)
        }
    }

    pub(super) fn select_file(owner_hwnd: usize, client_kind: &str) -> Result<Option<PathBuf>, ()> {
        if owner_hwnd == 0 {
            return Err(());
        }
        let (title, label, pattern) = match client_kind {
            "codex" => ("Select Codex command", "Codex command", "*.exe;*.cmd;*.bat"),
            "claude_code" => (
                "Select Claude Code command",
                "Claude Code command",
                "*.exe;*.cmd;*.bat",
            ),
            "opencode" => (
                "Select OpenCode command",
                "OpenCode command",
                "*.exe;*.cmd;*.bat",
            ),
            _ => return Err(()),
        };

        let _apartment = ComApartment::initialize()?;
        let dialog: IFileOpenDialog = unsafe {
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).map_err(|_| ())?
        };
        let title: Vec<u16> = title.encode_utf16().chain(Some(0)).collect();
        let label: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
        let pattern: Vec<u16> = pattern.encode_utf16().chain(Some(0)).collect();
        let filters = [COMDLG_FILTERSPEC {
            pszName: PCWSTR(label.as_ptr()),
            pszSpec: PCWSTR(pattern.as_ptr()),
        }];
        let options = FOS_FORCEFILESYSTEM
            | FOS_FILEMUSTEXIST
            | FOS_PATHMUSTEXIST
            | FOS_NOCHANGEDIR
            | FOS_NODEREFERENCELINKS
            | FOS_DONTADDTORECENT;

        unsafe {
            dialog.SetFileTypes(&filters).map_err(|_| ())?;
            dialog.SetFileTypeIndex(1).map_err(|_| ())?;
            dialog.SetOptions(options).map_err(|_| ())?;
            dialog.SetTitle(PCWSTR(title.as_ptr())).map_err(|_| ())?;
            let owner = HWND(owner_hwnd as *mut c_void);
            if let Err(error) = dialog.Show(Some(owner)) {
                return if error.code() == HRESULT_CANCELLED {
                    Ok(None)
                } else {
                    Err(())
                };
            }

            let item = dialog.GetResult().map_err(|_| ())?;
            filesystem_path(item).map(Some)
        }
    }

    pub(super) fn select_project_directory(
        owner_hwnd: usize,
        initial_directory: Option<PathBuf>,
    ) -> Result<Option<PathBuf>, ()> {
        if owner_hwnd == 0 {
            return Err(());
        }
        let _apartment = ComApartment::initialize()?;
        let dialog: IFileOpenDialog = unsafe {
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).map_err(|_| ())?
        };
        let title: Vec<u16> = "Select Claude Code project folder"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let options = FOS_FORCEFILESYSTEM
            | FOS_PATHMUSTEXIST
            | FOS_NOCHANGEDIR
            | FOS_NODEREFERENCELINKS
            | FOS_DONTADDTORECENT
            | FOS_PICKFOLDERS;

        unsafe {
            dialog.SetOptions(options).map_err(|_| ())?;
            dialog.SetTitle(PCWSTR(title.as_ptr())).map_err(|_| ())?;
            let initial_directory = initial_directory.ok_or(())?;
            let folder_wide: Vec<u16> = initial_directory
                .as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect();
            if folder_wide.len() > MAX_PATH_UNITS + 1 {
                return Err(());
            }
            let folder =
                SHCreateItemFromParsingName::<_, _, IShellItem>(PCWSTR(folder_wide.as_ptr()), None)
                    .map_err(|_| ())?;
            dialog.SetFolder(&folder).map_err(|_| ())?;
            let owner = HWND(owner_hwnd as *mut c_void);
            if let Err(error) = dialog.Show(Some(owner)) {
                return if error.code() == HRESULT_CANCELLED {
                    Ok(None)
                } else {
                    Err(())
                };
            }

            let item = dialog.GetResult().map_err(|_| ())?;
            filesystem_path(item).map(Some)
        }
    }
}

pub(crate) fn save_report(
    owner: usize,
    artifact: &crate::export::report::Artifact,
) -> Result<Option<PathBuf>, ()> {
    #[cfg(windows)]
    {
        let name = artifact.suggested_name.clone();
        let extension = artifact.extension;
        let turkish = artifact.language == crate::export::report::Language::Tr;
        return std::thread::Builder::new()
            .name("headroom-export-picker-sta".into())
            .spawn(move || windows_picker::save_report(owner, &name, extension, turkish))
            .map_err(|_| ())?
            .join()
            .map_err(|_| ())?;
    }
    #[cfg(not(windows))]
    {
        let _ = (owner, artifact);
        Err(())
    }
}

#[cfg(windows)]
pub fn select_file(owner_hwnd: usize, client_kind: &str) -> Result<Option<PathBuf>, ()> {
    windows_picker::select_file(owner_hwnd, client_kind)
}

#[cfg(windows)]
pub fn select_project_directory(
    owner_hwnd: usize,
    initial_directory: Option<PathBuf>,
) -> Result<Option<PathBuf>, ()> {
    windows_picker::select_project_directory(owner_hwnd, initial_directory)
}

#[cfg(not(windows))]
pub fn select_file(_owner_hwnd: usize, _client_kind: &str) -> Result<Option<PathBuf>, ()> {
    Err(())
}

#[cfg(not(windows))]
pub fn select_project_directory(
    _owner_hwnd: usize,
    _initial_directory: Option<PathBuf>,
) -> Result<Option<PathBuf>, ()> {
    Err(())
}
