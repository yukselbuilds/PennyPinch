use super::*;

fn root() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work/proofs/update/install-faults")
        .join(Uuid::new_v4().to_string());
    fs::create_dir_all(&root).unwrap();
    root
}

fn copy(path: &Path, source: &Path) -> FileDigest {
    fs::copy(source, path).unwrap();
    FileDigest::from_file(&mut read_owned(path).unwrap()).unwrap()
}

fn fixtures() -> PathBuf {
    let path = PathBuf::from(std::env::var_os("HEADROOM_H042_FAULT_FIXTURES").unwrap());
    let boundary = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work/proofs/update");
    assert!(fs::canonicalize(&path)
        .unwrap()
        .starts_with(fs::canonicalize(boundary).unwrap()));
    path
}

struct RegistryGuard(Vec<u16>);
impl Drop for RegistryGuard {
    fn drop(&mut self) {
        unsafe {
            RegDeleteTreeW(HKEY_CURRENT_USER, self.0.as_ptr());
        }
    }
}

fn registry(installation: &Installation, exe: &Path) -> RegistryGuard {
    let mut key = std::ptr::null_mut();
    let mut disposition = 0;
    assert_eq!(
        unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                installation.registry_key.as_ptr(),
                0,
                std::ptr::null(),
                0,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut key,
                &mut disposition,
            )
        },
        ERROR_SUCCESS
    );
    assert_eq!(disposition, REG_CREATED_NEW_KEY);
    for (name, value) in [
        (
            "InstallLocation",
            exe.parent().unwrap().to_string_lossy().into_owned(),
        ),
        ("MainBinaryName", "headroom.exe".to_owned()),
        ("DisplayVersion", "0.1.0".to_owned()),
    ] {
        let bytes = wide(value);
        assert_eq!(
            unsafe {
                RegSetValueExW(
                    key,
                    wide(name).as_ptr(),
                    0,
                    REG_SZ,
                    bytes.as_ptr().cast(),
                    size_of_val(bytes.as_slice()) as u32,
                )
            },
            ERROR_SUCCESS
        );
    }
    unsafe { RegCloseKey(key) };
    RegistryGuard(installation.registry_key.clone())
}

#[test]
#[ignore = "requires signed local H042 fault fixtures"]
fn early_startup_failure_restores_the_previous_executable_registry_and_user_data() {
    let fixture = fixtures();
    let root = root();
    let live = root.join("headroom.exe");
    let previous = copy(&live, &fixture.join("old.exe"));
    fs::write(root.join("uninstall.exe"), b"owned uninstaller fixture").unwrap();
    let state = root.join("state");
    fs::create_dir(&state).unwrap();
    fs::write(state.with_file_name("state.desktop-use"), b"").unwrap();
    fs::write(state.join("user-data"), b"preserve current user data").unwrap();
    let installation = Installation {
        identifier: "dev.headroom.desktop.h042-fault".into(),
        registry_key: wide(format!(
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Headroom H042 Fault {}",
            Uuid::new_v4()
        )),
        version: Version::new(0, 1, 0),
        app_data: state.clone(),
    };
    let _registration = registry(&installation, &live);
    let directory = installation.recovery_dir(Uuid::new_v4());
    fs::create_dir_all(&directory).unwrap();
    copy(&directory.join(WORKER), &fixture.join("old.exe"));
    let next = copy(&directory.join(APP), &fixture.join("fails-startup.exe"));
    fs::copy(fixture.join("startup.json"), directory.join(MANIFEST)).unwrap();
    fs::copy(fixture.join("startup.json.sig"), directory.join(SIGNATURE)).unwrap();
    let record = Record {
        schema_version: 1,
        exe: live.clone(),
        previous_version: "0.1.0".into(),
        next_version: "0.1.1".into(),
        previous: previous.clone(),
        next,
        channel: UpdateChannel::Stable,
    };
    create_bytes(
        &directory.join(RECORD),
        &serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
    let error = installation.apply(&directory, &record).unwrap_err();
    assert_eq!(error.to_string(), "update_restart");
    previous.check(&mut read_owned(&live).unwrap()).unwrap();
    assert_eq!(
        installation.registry_text("DisplayVersion").unwrap(),
        "0.1.0"
    );
    assert_eq!(
        fs::read(state.join("user-data")).unwrap(),
        b"preserve current user data"
    );
    assert!(fs::read_dir(root).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".headroom-next-")));
}

#[test]
#[ignore = "requires signed local H042 fault fixtures"]
fn history_at_the_limit_keeps_the_current_rollback_and_preserves_foreign_content() {
    let fixture = fixtures();
    let root = root();
    let live = root.join("headroom.exe");
    let current = copy(&live, &fixture.join("current.exe"));
    let state = root.join("state");
    fs::create_dir(&state).unwrap();
    let installation = Installation {
        identifier: "dev.headroom.desktop.h042-fault".into(),
        registry_key: vec![],
        version: Version::new(0, 1, 2),
        app_data: state,
    };
    let history = installation.recovery_root();
    fs::create_dir(&history).unwrap();
    let mut foreign = None;
    let mut locked = None;
    let mut locked_directory = None;
    let mut untrusted = None;
    for index in 0..MAX_RECORDS {
        let directory = installation.recovery_dir(Uuid::new_v4());
        fs::create_dir(&directory).unwrap();
        let previous = copy(&directory.join(WORKER), &fixture.join("old.exe"));
        let current_record = index == 0 || index == MAX_RECORDS - 1;
        let prefix = if current_record { "current" } else { "startup" };
        let next = FileDigest::from_file(
            &mut read_owned(&fixture.join(if current_record {
                "current.exe"
            } else {
                "fails-startup.exe"
            }))
            .unwrap(),
        )
        .unwrap();
        fs::copy(
            fixture.join(format!("{prefix}.json")),
            directory.join(MANIFEST),
        )
        .unwrap();
        fs::copy(
            fixture.join(format!("{prefix}.json.sig")),
            directory.join(SIGNATURE),
        )
        .unwrap();
        let record = Record {
            schema_version: 1,
            exe: live.clone(),
            previous_version: "0.1.0".into(),
            next_version: if current_record { "0.1.2" } else { "0.1.1" }.into(),
            previous,
            next,
            channel: UpdateChannel::Stable,
        };
        create_bytes(
            &directory.join(RECORD),
            &serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
        if index == 1 {
            fs::write(
                directory.join("foreign-user-file"),
                b"preserve foreign content",
            )
            .unwrap();
            foreign = Some(directory);
        } else if index == 2 {
            locked = Some(read_owned(&directory.join(WORKER)).unwrap());
            locked_directory = Some(directory);
        } else if index == 3 {
            fs::write(directory.join(SIGNATURE), b"invalid signature").unwrap();
            untrusted = Some(directory);
        }
    }
    installation.prune_history(&live, &current).unwrap();
    assert_eq!(fs::read_dir(&history).unwrap().count(), 4);
    // Equivalent current-version backups leave one complete recovery record.
    let current_backups = fs::read_dir(&history)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| read_record(&entry.path()).is_ok_and(|(_, record)| record.next == current))
        .count();
    assert_eq!(current_backups, 1);
    assert!(locked_directory.as_ref().unwrap().join(RECORD).is_file());
    assert!(untrusted.unwrap().join(WORKER).is_file());
    drop(locked.take());
    installation.prune_history(&live, &current).unwrap();
    assert!(!locked_directory.unwrap().exists());
    assert_eq!(fs::read_dir(&history).unwrap().count(), 3);
    assert_eq!(
        fs::read(foreign.unwrap().join("foreign-user-file")).unwrap(),
        b"preserve foreign content"
    );
    current.check(&mut read_owned(&live).unwrap()).unwrap();
}
