use super::*;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::{symlink, DirBuilderExt, PermissionsExt};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn proof_root() -> PathBuf {
    let work = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work")
        .canonicalize()
        .unwrap();
    let root = PathBuf::from(
        env::var_os("HEADROOM_MACOS_CLIENT_PROOF_ROOT").expect("isolated client root"),
    );
    assert_eq!(root.canonicalize().unwrap(), root);
    assert!(root != work && root.starts_with(work));
    let metadata = fs::symlink_metadata(&root).unwrap();
    assert!(metadata.is_dir() && metadata.uid() == unsafe { libc::geteuid() });
    assert_eq!(metadata.mode() & 0o7777, 0o700);
    root
}

fn private_dir(path: &Path) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
}

#[test]
fn manual_cli_selection_keeps_a_saved_choice_across_fresh_scans() {
    let root = proof_root().join("manual-selection");
    private_dir(&root);
    let executable = root.join("codex");
    let versioned = root.join("0.163.0-alpha.11.1");
    fs::copy(proof_root().join("clients/bin/codex"), &versioned).unwrap();
    symlink(&versioned, &executable).unwrap();
    let app_data = root.join("app-data");
    let store = ChoiceStore::for_app_local_data(Some(app_data.clone())).unwrap();
    let manager = DiscoveryManager::with_choice_store(Some(store.clone()));
    let request_id = Uuid::new_v4().to_string();
    let DiscoveryReply::Cancelled(cancelled) =
        manager.select_client_file_with(request_id.clone(), "codex".into(), |_| Ok(None))
    else {
        panic!("picker cancellation was not correlated");
    };
    assert_eq!(cancelled.request_id, request_id);
    assert!(!app_data.exists());
    let DiscoveryReply::Snapshot(selected) =
        manager.select_client_file_with(Uuid::new_v4().to_string(), "codex".into(), |_| {
            Ok(Some(executable.clone()))
        })
    else {
        panic!("native CLI picker result was rejected");
    };
    let row = &selected.installations[0];
    assert_eq!(row.version.as_deref(), Some("0.163.0-alpha.11.1"));
    assert_eq!(row.source, "manual_file");
    assert!(matches!(
        manager.remember_installation_choice(RememberInstallationChoiceRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            scan_generation: selected.scan_generation,
            installation_id: row.installation_id.clone(),
        }),
        SavedInstallationChoiceReply::Snapshot(_)
    ));
    drop(manager);
    let reopened = DiscoveryManager::with_choice_store(Some(store));
    let DiscoveryReply::Snapshot(refreshed) =
        reopened.scan_with_inputs(Uuid::new_v4().to_string(), DiscoveryInputs::default())
    else {
        panic!("manual selection rescan failed");
    };
    assert_eq!(refreshed.installations.len(), 1);
    assert_eq!(refreshed.installations[0].source, "manual_file");
    assert_ne!(
        refreshed.installations[0].installation_id,
        row.installation_id
    );
    let DiscoveryReply::Snapshot(cancelled_scan) =
        reopened.list_snapshot(Uuid::new_v4().to_string())
    else {
        panic!("saved scan unavailable");
    };
    assert!(matches!(
        reopened.select_client_file_with(Uuid::new_v4().to_string(), "codex".into(), |_| Ok(None)),
        DiscoveryReply::Cancelled(_)
    ));
    let DiscoveryReply::Snapshot(after_cancel) = reopened.list_snapshot(Uuid::new_v4().to_string())
    else {
        panic!("scan after cancellation unavailable");
    };
    assert_eq!(after_cancel.scan_generation, cancelled_scan.scan_generation);
    assert!(matches!(
        reopened.select_client_file_with(Uuid::new_v4().to_string(), "claude_code".into(), |_| Ok(
            Some(executable.clone())
        )),
        DiscoveryReply::Error(_)
    ));
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o600)).unwrap();
    let DiscoveryReply::Snapshot(changed) =
        reopened.scan_with_inputs(Uuid::new_v4().to_string(), DiscoveryInputs::default())
    else {
        panic!("changed CLI rescan failed");
    };
    assert!(changed.installations.is_empty());
}

#[test]
fn installed_cli_discovery_uses_actual_environment_and_does_not_execute_candidates() {
    let root = proof_root();
    let home = root.join("home");
    let path_bin = root.join("path-bin");
    let decoy_bin = root.join("decoy-bin");
    let duplicate_bin = root.join("duplicate-bin");
    let home_local = home.join(".local");
    let home_opencode = home.join(".opencode");
    let claude_share = home_local.join("share/claude/versions");
    for dir in [
        &home,
        &path_bin,
        &decoy_bin,
        &duplicate_bin,
        &home_local,
        &home_opencode,
        &home_local.join("bin"),
        &home_opencode.join("bin"),
    ] {
        private_dir(dir);
    }
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&claude_share)
        .unwrap();
    fs::copy(
        root.join("clients/bin/claude"),
        claude_share.join("2.1.158"),
    )
    .unwrap();
    fs::copy(
        root.join("clients/bin/opencode"),
        home_opencode.join("bin/opencode"),
    )
    .unwrap();
    symlink("../clients/bin/codex", path_bin.join("codex")).unwrap();
    symlink(root.join("clients/bin/codex"), duplicate_bin.join("codex")).unwrap();
    symlink(
        "../share/claude/versions/2.1.158",
        home_local.join("bin/claude"),
    )
    .unwrap();
    fs::write(
        decoy_bin.join("opencode"),
        b"#!/bin/sh\n: > \"$HOME/unexpected-cli-execution\"\n",
    )
    .unwrap();
    fs::set_permissions(
        decoy_bin.join("opencode"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    let official_size = fs::metadata(root.join("clients/bin/opencode"))
        .unwrap()
        .len();
    OpenOptions::new()
        .write(true)
        .open(decoy_bin.join("opencode"))
        .unwrap()
        .set_len(official_size)
        .unwrap();
    fs::write(decoy_bin.join("claude"), b"fixture writable file").unwrap();
    fs::set_permissions(decoy_bin.join("claude"), fs::Permissions::from_mode(0o777)).unwrap();
    fs::write(duplicate_bin.join("claude"), b"fixture non-executable file").unwrap();
    fs::set_permissions(
        duplicate_bin.join("claude"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    symlink("codex", decoy_bin.join("codex")).unwrap();
    let path = env::join_paths([&path_bin, &duplicate_bin, &decoy_bin]).unwrap();
    let module = module_path!()
        .strip_prefix(&format!("{}::", env!("CARGO_CRATE_NAME")))
        .unwrap_or(module_path!());
    let filter = format!("{module}::isolated_installed_cli_scan_child");
    let mut child = Command::new(env::current_exe().unwrap())
        .args([
            "--exact",
            &filter,
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env_clear()
        .env("PATH", path)
        .env("HOME", &home)
        .env("HEADROOM_MACOS_CLIENT_PROOF_ROOT", &root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        match child.try_wait().unwrap() {
            Some(status) => {
                if status.code() != Some(44) {
                    let mut diagnostic = String::new();
                    if let Some(stderr) = child.stderr.take() {
                        let _ = stderr.take(4096).read_to_string(&mut diagnostic);
                    }
                    if let Some(location) = diagnostic.lines().find(|line| {
                        line.contains("panicked at src/integrations/discovery_macos_tests.rs:")
                    }) {
                        eprintln!("{location}");
                    }
                }
                assert_eq!(
                    status.code(),
                    Some(44),
                    "child completed actual environment scan"
                );
                break;
            }
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("isolated CLI scan deadline");
            }
        }
    }
    let mut names: Vec<_> = fs::read_dir(&home)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    names.sort();
    assert_eq!(names, [".local", ".opencode"].map(std::ffi::OsString::from));
}

#[test]
#[ignore = "owned child is invoked with isolated PATH and HOME"]
fn isolated_installed_cli_scan_child() {
    let root = proof_root();
    let app_data = root.join("choice-app-data");
    let open_manager = || {
        DiscoveryManager::with_choice_store(Some(
            ChoiceStore::for_app_local_data(Some(app_data.clone())).unwrap(),
        ))
    };
    let manager = open_manager();
    let DiscoveryReply::Snapshot(scan) = manager.scan(Uuid::new_v4().to_string()) else {
        panic!("CLI scan failed");
    };
    assert_eq!(scan.installations.len(), 4);
    assert_eq!(
        scan.installations
            .iter()
            .filter(|row| row.version.is_none())
            .count(),
        1
    );
    let serialized = serde_json::to_string(&scan).unwrap();
    assert!(!serialized.contains(root.to_str().unwrap()));
    assert!(!serialized.contains("clients/bin"));
    assert!(!app_data.exists(), "a scan must not create saved choices");
    let mut selected = Vec::new();
    for (client, relative_path, version, source) in [
        ("codex", "clients/bin/codex", "0.163.0-alpha.11.1", "path"),
        (
            "claude_code",
            "home/.local/share/claude/versions/2.1.158",
            "2.1.158",
            "user_local_native",
        ),
        (
            "opencode",
            "home/.opencode/bin/opencode",
            "1.19.3",
            "user_local_native",
        ),
    ] {
        let row = scan
            .installations
            .iter()
            .find(|row| row.client_kind == client && row.version.as_deref() == Some(version))
            .expect("pinned official CLI was not recognized");
        assert_eq!(row.surface, "cli");
        assert_eq!(row.status, "detected");
        assert_eq!(row.connection_status, "not_verified");
        assert_eq!(row.source, source);
        let (_, path) = manager
            .select_installation_with_path(&row.installation_id, scan.scan_generation)
            .unwrap();
        assert_eq!(path, root.join(relative_path));
        let request_id = Uuid::new_v4().to_string();
        let SavedInstallationChoiceReply::Snapshot(saved) =
            manager.remember_installation_choice(RememberInstallationChoiceRequest {
                schema_version: 1,
                request_id: request_id.clone(),
                scan_generation: scan.scan_generation,
                installation_id: row.installation_id.clone(),
            })
        else {
            panic!("explicit choice failed");
        };
        assert_eq!(saved.status, "saved");
        assert_eq!(saved.request_id, request_id);
        selected.push((client, row.installation_id.clone()));
    }
    drop(manager);
    let reopened = open_manager();
    let read = |kind: &str| {
        let reply = reopened.saved_installation_choice(SavedInstallationChoiceRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            client_kind: kind.to_owned(),
        });
        let output = serde_json::to_string(&reply).unwrap();
        assert!(!output.contains(root.to_str().unwrap()));
        let SavedInstallationChoiceReply::Snapshot(snapshot) = reply else {
            panic!("saved choice lookup failed");
        };
        snapshot
    };
    assert_eq!(read("codex").status, "not_scanned");
    let DiscoveryReply::Snapshot(next) = reopened.scan(Uuid::new_v4().to_string()) else {
        panic!("reopen scan failed");
    };
    for (kind, old_id) in selected {
        let saved = read(kind);
        assert_eq!(saved.status, "available");
        assert_eq!(saved.scan_generation, Some(next.scan_generation));
        let id = saved.installation_id.unwrap();
        assert_ne!(id, old_id);
        assert!(next
            .installations
            .iter()
            .any(|row| row.installation_id == id && row.client_kind == kind));
    }

    let before = fs::read(app_data.join("onboarding/selections.json")).unwrap();
    let opencode = read("opencode").installation_id.unwrap();
    let path = root.join("home/.opencode/bin/opencode");
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .unwrap();
    let length = file.metadata().unwrap().len();
    file.seek(SeekFrom::End(-1)).unwrap();
    let mut byte = [0];
    file.read_exact(&mut byte).unwrap();
    file.seek(SeekFrom::End(-1)).unwrap();
    file.write_all(&[byte[0] ^ 1]).unwrap();
    file.sync_all().unwrap();
    assert_eq!(file.metadata().unwrap().len(), length);
    assert!(matches!(
        reopened.select_installation_with_path(&opencode, next.scan_generation),
        Err(InstallationSelectionError::StaleScanGeneration)
    ));
    assert!(matches!(
        reopened.remember_installation_choice(RememberInstallationChoiceRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            scan_generation: next.scan_generation,
            installation_id: opencode.clone(),
        }),
        SavedInstallationChoiceReply::Error(_)
    ));
    assert_eq!(read("opencode").status, "unavailable");
    let DiscoveryReply::Snapshot(refreshed) = reopened.scan(Uuid::new_v4().to_string()) else {
        panic!("changed client scan failed");
    };
    assert!(refreshed.scan_generation > next.scan_generation);
    assert_eq!(read("opencode").status, "unavailable");
    assert_eq!(read("codex").status, "available");
    assert_eq!(
        fs::read(app_data.join("onboarding/selections.json")).unwrap(),
        before
    );
    std::process::exit(44);
}
