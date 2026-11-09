//! Only this account's single, exactly matching application Run value is writable.
use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Serialize)]
pub(crate) struct Snapshot {
    pub status: &'static str,
    pub fingerprint: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Error {
    Unavailable,
    Conflict,
    Busy,
    Invalid,
}

pub(crate) struct Autostart {
    command: Option<Vec<u16>>,
    key: String,
}

impl Autostart {
    pub(crate) fn new() -> Self {
        let command = std::env::current_exe().ok().and_then(|path| {
            let path = path.canonicalize().ok()?;
            let text = path
                .to_str()?
                .strip_prefix(r"\\?\")
                .unwrap_or(path.to_str()?);
            if !path.is_file()
                || !text.to_ascii_lowercase().ends_with(".exe")
                || text.contains(['"', '\r', '\n', '\0'])
            {
                return None;
            }
            Some(
                format!("\"{text}\"")
                    .encode_utf16()
                    .chain(Some(0))
                    .collect(),
            )
        });
        Self {
            command,
            key: r"Software\Microsoft\Windows\CurrentVersion\Run".into(),
        }
    }

    pub(crate) fn read(&self) -> Snapshot {
        self.try_read().unwrap_or(Snapshot {
            status: "unavailable",
            fingerprint: None,
        })
    }

    #[cfg(all(windows, any(test, not(debug_assertions))))]
    pub(crate) fn remove_if_owned(&self) -> Result<(), Error> {
        let snapshot = self.try_read()?;
        if snapshot.status == "registered" {
            self.set(false, snapshot.fingerprint.as_deref())?;
        }
        Ok(())
    }

    #[cfg(windows)]
    fn try_read(&self) -> Result<Snapshot, Error> {
        let _lock = native::Lock::acquire(&self.key)?;
        self.snapshot(native::read(&self.key)?)
    }
    #[cfg(not(windows))]
    fn try_read(&self) -> Result<Snapshot, Error> {
        Err(Error::Unavailable)
    }

    fn snapshot(&self, value: Option<Vec<u8>>) -> Result<Snapshot, Error> {
        let command = self.command.as_ref().ok_or(Error::Unavailable)?;
        Ok(Snapshot {
            status: match &value {
                None => "disabled",
                Some(bytes) if *bytes == encoded(command) => "registered",
                Some(_) => "conflict",
            },
            fingerprint: value.map(|v| format!("{:x}", Sha256::digest(v))),
        })
    }

    pub(crate) fn set(&self, enabled: bool, expected: Option<&str>) -> Result<Snapshot, Error> {
        if expected.is_some_and(|value| {
            value.len() != 64
                || !value
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        }) {
            return Err(Error::Invalid);
        }
        self.set_native(enabled, expected)
    }
    #[cfg(windows)]
    fn set_native(&self, enabled: bool, expected: Option<&str>) -> Result<Snapshot, Error> {
        let _lock = native::Lock::acquire(&self.key)?;
        let current = native::read(&self.key)?;
        let snapshot = self.snapshot(current.clone())?;
        if snapshot.status == "conflict" || snapshot.fingerprint.as_deref() != expected {
            return Err(Error::Conflict);
        }
        let desired = enabled.then(|| encoded(self.command.as_ref().unwrap()));
        if current == desired {
            return Ok(snapshot);
        }
        // Registry has no compare-and-swap. Serialize our writers and recheck;
        // an unrelated concurrent writer after this check cannot be locked out.
        if native::read(&self.key)? != current {
            return Err(Error::Conflict);
        }
        native::write(&self.key, desired.as_deref())?;
        let actual = native::read(&self.key)?;
        if actual != desired {
            return Err(Error::Conflict);
        }
        self.snapshot(actual)
    }
    #[cfg(not(windows))]
    fn set_native(&self, _: bool, _: Option<&str>) -> Result<Snapshot, Error> {
        Err(Error::Unavailable)
    }
}

fn encoded(value: &[u16]) -> Vec<u8> {
    let mut bytes = 1_u32.to_le_bytes().to_vec(); // REG_SZ included in ownership fingerprint.
    bytes.extend(value.iter().flat_map(|c| c.to_le_bytes()));
    bytes
}

#[cfg(windows)]
mod native {
    use super::Error;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::{
        Foundation::*,
        System::{Registry::*, Threading::*},
    };
    const NAME: &str = "HeadroomDesktop";
    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
    struct Key(HKEY);
    impl Drop for Key {
        fn drop(&mut self) {
            unsafe {
                RegCloseKey(self.0);
            }
        }
    }
    fn open(path: &str, write: bool) -> Result<Option<Key>, Error> {
        let mut key = null_mut();
        let code = unsafe {
            if write {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    wide(path).as_ptr(),
                    0,
                    null(),
                    REG_OPTION_NON_VOLATILE,
                    KEY_QUERY_VALUE | KEY_SET_VALUE,
                    null(),
                    &mut key,
                    null_mut(),
                )
            } else {
                RegOpenKeyExW(
                    HKEY_CURRENT_USER,
                    wide(path).as_ptr(),
                    0,
                    KEY_QUERY_VALUE,
                    &mut key,
                )
            }
        };
        match code {
            ERROR_SUCCESS => Ok(Some(Key(key))),
            ERROR_FILE_NOT_FOUND => Ok(None),
            _ => Err(Error::Unavailable),
        }
    }
    pub(super) fn read(path: &str) -> Result<Option<Vec<u8>>, Error> {
        let Some(key) = open(path, false)? else {
            return Ok(None);
        };
        let mut kind = 0;
        let mut size = 0;
        let code = unsafe {
            RegQueryValueExW(
                key.0,
                wide(NAME).as_ptr(),
                null(),
                &mut kind,
                null_mut(),
                &mut size,
            )
        };
        if code == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if code != ERROR_SUCCESS || size > 8192 {
            return Err(Error::Unavailable);
        }
        let mut bytes = vec![0; size as usize];
        if unsafe {
            RegQueryValueExW(
                key.0,
                wide(NAME).as_ptr(),
                null(),
                &mut kind,
                bytes.as_mut_ptr(),
                &mut size,
            )
        } != ERROR_SUCCESS
        {
            return Err(Error::Unavailable);
        }
        bytes.truncate(size as usize);
        let mut result = kind.to_le_bytes().to_vec();
        result.extend(bytes);
        Ok(Some(result))
    }
    pub(super) fn write(path: &str, bytes: Option<&[u8]>) -> Result<(), Error> {
        let key = open(path, true)?.ok_or(Error::Unavailable)?;
        let code = unsafe {
            match bytes {
                Some(bytes) => RegSetValueExW(
                    key.0,
                    wide(NAME).as_ptr(),
                    0,
                    REG_SZ,
                    bytes[4..].as_ptr(),
                    (bytes.len() - 4) as u32,
                ),
                None => RegDeleteValueW(key.0, wide(NAME).as_ptr()),
            }
        };
        if code == ERROR_SUCCESS || bytes.is_none() && code == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(Error::Unavailable)
        }
    }
    pub(super) struct Lock(HANDLE);
    impl Lock {
        pub(super) fn acquire(key: &str) -> Result<Self, Error> {
            let name = format!(
                "Local\\HeadroomAutostart-{:x}",
                <sha2::Sha256 as sha2::Digest>::digest(key.as_bytes())
            );
            let handle = unsafe { CreateMutexW(null(), 0, wide(&name).as_ptr()) };
            if handle.is_null() {
                return Err(Error::Unavailable);
            }
            let result = unsafe { WaitForSingleObject(handle, 0) };
            if result == WAIT_OBJECT_0 || result == WAIT_ABANDONED {
                Ok(Self(handle))
            } else {
                unsafe { CloseHandle(handle) };
                Err(Error::Busy)
            }
        }
    }
    impl Drop for Lock {
        fn drop(&mut self) {
            unsafe {
                ReleaseMutex(self.0);
                CloseHandle(self.0);
            }
        }
    }

    #[cfg(test)]
    pub(super) fn test_neighbor(path: &str, create: bool) {
        assert!(path.starts_with(r"Software\HeadroomDesktopTests\"));
        let key = open(path, true).unwrap().unwrap();
        let name = wide("OtherApplication");
        let expected = b"untouched neighbor";
        if create {
            assert_eq!(
                unsafe {
                    RegSetValueExW(
                        key.0,
                        name.as_ptr(),
                        0,
                        REG_BINARY,
                        expected.as_ptr(),
                        expected.len() as u32,
                    )
                },
                ERROR_SUCCESS
            );
        } else {
            let mut data = [0u8; 64];
            let mut size = 64;
            let mut kind = 0;
            assert_eq!(
                unsafe {
                    RegQueryValueExW(
                        key.0,
                        name.as_ptr(),
                        null(),
                        &mut kind,
                        data.as_mut_ptr(),
                        &mut size,
                    )
                },
                ERROR_SUCCESS
            );
            assert_eq!(&data[..size as usize], expected);
            assert_eq!(kind, REG_BINARY);
            assert_eq!(
                unsafe { RegDeleteValueW(key.0, name.as_ptr()) },
                ERROR_SUCCESS
            );
        }
    }
    #[cfg(test)]
    pub(super) fn cleanup_test_key(path: &str) {
        assert!(path.starts_with(r"Software\HeadroomDesktopTests\"));
        assert_eq!(
            unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, wide(path).as_ptr()) },
            ERROR_SUCCESS
        );
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn autostart_is_passive_then_roundtrips_only_own_value_and_rejects_foreign_or_stale() {
        let key = format!(r"Software\HeadroomDesktopTests\{}", uuid::Uuid::new_v4());
        let mut app = Autostart::new();
        app.key = key.clone();
        assert_eq!(app.read().status, "disabled");
        assert!(native::read(&key).unwrap().is_none());
        native::test_neighbor(&key, true);
        let enabled = app.set(true, None).unwrap();
        assert_eq!(enabled.status, "registered");
        let again = Autostart {
            key: key.clone(),
            command: app.command.clone(),
        };
        assert_eq!(again.read().fingerprint, enabled.fingerprint);
        assert!(matches!(app.set(false, None), Err(Error::Conflict)));
        app.remove_if_owned().unwrap();
        assert_eq!(app.read().status, "disabled");
        let foreign = encoded(&"foreign.exe\0".encode_utf16().collect::<Vec<_>>());
        native::write(&key, Some(&foreign)).unwrap();
        let seen = app.read();
        assert_eq!(seen.status, "conflict");
        assert!(matches!(
            app.set(true, seen.fingerprint.as_deref()),
            Err(Error::Conflict)
        ));
        app.remove_if_owned().unwrap();
        assert_eq!(native::read(&key).unwrap(), Some(foreign));
        native::write(&key, None).unwrap();
        native::test_neighbor(&key, false);
        native::cleanup_test_key(&key);
    }
}
