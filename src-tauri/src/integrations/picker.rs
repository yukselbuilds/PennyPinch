use std::path::PathBuf;

#[cfg(windows)]
mod windows_picker {
    use super::PathBuf;
    use std::ffi::{c_void, OsString};
    use std::os::windows::ffi::OsStringExt;
    use windows::core::{HRESULT, PCWSTR, PWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, FOS_DONTADDTORECENT, FOS_FILEMUSTEXIST,
        FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR, FOS_NODEREFERENCELINKS, FOS_PATHMUSTEXIST,
        FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
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
            let display_name = item.GetDisplayName(SIGDN_FILESYSPATH).map_err(|_| ())?;
            let memory = TaskMemory(display_name);
            let raw = memory.0 .0;
            if raw.is_null() {
                return Err(());
            }
            let mut length = None;
            for index in 0..=MAX_PATH_UNITS {
                if raw.add(index).read() == 0 {
                    length = Some(index);
                    break;
                }
            }
            let Some(length) = length else {
                return Err(());
            };
            if length == 0 {
                return Err(());
            }
            let units = std::slice::from_raw_parts(raw, length);
            let path = OsString::from_wide(units);
            if path.is_empty() {
                return Err(());
            }
            Ok(Some(PathBuf::from(path)))
        }
    }

    pub(super) fn select_project_directory(owner_hwnd: usize) -> Result<Option<PathBuf>, ()> {
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
            let owner = HWND(owner_hwnd as *mut c_void);
            if let Err(error) = dialog.Show(Some(owner)) {
                return if error.code() == HRESULT_CANCELLED {
                    Ok(None)
                } else {
                    Err(())
                };
            }

            let item = dialog.GetResult().map_err(|_| ())?;
            let display_name = item.GetDisplayName(SIGDN_FILESYSPATH).map_err(|_| ())?;
            let memory = TaskMemory(display_name);
            let raw = memory.0 .0;
            if raw.is_null() {
                return Err(());
            }
            let mut length = None;
            for index in 0..=MAX_PATH_UNITS {
                if raw.add(index).read() == 0 {
                    length = Some(index);
                    break;
                }
            }
            let Some(length) = length else {
                return Err(());
            };
            if length == 0 {
                return Err(());
            }
            let path = OsString::from_wide(std::slice::from_raw_parts(raw, length));
            if path.is_empty() {
                return Err(());
            }
            Ok(Some(PathBuf::from(path)))
        }
    }
}

#[cfg(windows)]
pub fn select_file(owner_hwnd: usize, client_kind: &str) -> Result<Option<PathBuf>, ()> {
    windows_picker::select_file(owner_hwnd, client_kind)
}

#[cfg(windows)]
pub fn select_project_directory(owner_hwnd: usize) -> Result<Option<PathBuf>, ()> {
    windows_picker::select_project_directory(owner_hwnd)
}

#[cfg(not(windows))]
pub fn select_file(_owner_hwnd: usize, _client_kind: &str) -> Result<Option<PathBuf>, ()> {
    Err(())
}

#[cfg(not(windows))]
pub fn select_project_directory(_owner_hwnd: usize) -> Result<Option<PathBuf>, ()> {
    Err(())
}
