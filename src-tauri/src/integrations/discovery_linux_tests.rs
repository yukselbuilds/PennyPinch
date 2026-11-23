use super::*;
use std::fs::{FileTimes, Permissions};
use std::io::Write;
use std::os::unix::fs::{symlink, DirBuilderExt, MetadataExt, PermissionsExt};
use std::process::Command;

fn proof_root() -> PathBuf {
    let work = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work")
        .canonicalize()
        .unwrap();
    let root = PathBuf::from(env::var_os("HEADROOM_LINUX_PROOF_ROOT").unwrap());
    assert_eq!(root.canonicalize().unwrap(), root);
    assert!(root != work && root.starts_with(work));
    let info = fs::symlink_metadata(&root).unwrap();
    assert!(info.is_dir() && info.uid() == unsafe { libc::geteuid() });
    assert_eq!(info.mode() & 0o7777, 0o700);
    root
}

fn private_dir(path: &Path) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
}

fn snapshot(manager: &DiscoveryManager) -> DiscoverySnapshot {
    let DiscoveryReply::Snapshot(scan) = manager.scan(Uuid::new_v4().to_string()) else {
        panic!("Discovery did not return a snapshot");
    };
    scan
}

#[test]
#[ignore = "requires isolated Ubuntu ext4 root and verified native CLI fixtures"]
fn native_linux_discovery_preserves_choices_and_does_not_run_clients() {
    let root = proof_root();
    let clients = root.join("client-fixtures/bin");
    let home = root.join("discovery-home");
    let path_bin = root.join("discovery-path");
    let local_bin = home.join(".local/bin");
    let opencode_bin = home.join(".opencode/bin");
    let decoys = root.join("discovery-decoys");
    let unsafe_bin = root.join("discovery-untrusted");
    for directory in [
        &home,
        &home.join(".local"),
        &local_bin,
        &home.join(".opencode"),
        &opencode_bin,
        &path_bin,
        &decoys,
        &unsafe_bin,
    ] {
        private_dir(directory);
    }
    symlink(clients.join("codex"), path_bin.join("codex")).unwrap();
    symlink(
        "../../client-fixtures/bin/claude",
        home.join(".local/claude"),
    )
    .unwrap();
    symlink("../claude", local_bin.join("claude")).unwrap();
    symlink(clients.join("opencode"), opencode_bin.join("opencode")).unwrap();
    symlink(clients.join("codex"), decoys.join("codex")).unwrap();
    symlink(clients.join("opencode"), unsafe_bin.join("claude")).unwrap();
    fs::set_permissions(&unsafe_bin, Permissions::from_mode(0o777)).unwrap();

    let marker_script = decoys.join("opencode");
    fs::write(&marker_script, b"#!/bin/sh\n: > \"$HOME/ran-a-client\"\n").unwrap();
    OpenOptions::new()
        .write(true)
        .open(&marker_script)
        .unwrap()
        .set_len(fs::metadata(clients.join("opencode")).unwrap().len())
        .unwrap();
    fs::set_permissions(&marker_script, Permissions::from_mode(0o700)).unwrap();
    fs::write(decoys.join("claude"), b"group-writable decoy").unwrap();
    fs::set_permissions(decoys.join("claude"), Permissions::from_mode(0o770)).unwrap();

    let result = Command::new(env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "integrations::discovery::linux_tests::isolated_discovery_process",
        ])
        .env_clear()
        .env("HOME", &home)
        .env(
            "PATH",
            env::join_paths([path_bin, decoys, unsafe_bin, PathBuf::from(".")]).unwrap(),
        )
        .env("HEADROOM_LINUX_PROOF_ROOT", &root)
        .env("HEADROOM_LINUX_DISCOVERY_CHILD", "1")
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(0),
        "Isolated discovery failed\n{}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("1 passed"));
    assert!(!home.join("ran-a-client").exists());
}

#[test]
#[ignore = "run only as the isolated discovery child"]
fn isolated_discovery_process() {
    assert_eq!(
        env::var("HEADROOM_LINUX_DISCOVERY_CHILD").as_deref(),
        Ok("1")
    );
    let root = proof_root();
    let app_data = root.join("discovery-app-data");
    let open_manager = || {
        DiscoveryManager::with_choice_store(Some(
            ChoiceStore::for_app_local_data(Some(app_data.clone())).unwrap(),
        ))
    };
    let manager = open_manager();
    let scan = snapshot(&manager);
    assert_eq!(scan.installations.len(), 4);
    assert!(!serde_json::to_string(&scan)
        .unwrap()
        .contains(root.to_str().unwrap()));
    assert!(!app_data.exists(), "Automatic discovery must be read-only");
    let expected = [
        ("codex", "0.163.0-alpha.11.1", "path"),
        ("claude_code", "2.1.158", "user_local_native"),
        ("opencode", "1.19.3", "user_local_native"),
    ];
    for (client, version, source) in expected {
        let selected = scan
            .installations
            .iter()
            .find(|row| row.client_kind == client && row.version.as_deref() == Some(version))
            .expect("Missing verified CLI");
        assert_eq!(selected.source, source);
        assert_eq!(selected.connection_status, "not_verified");
        let SavedInstallationChoiceReply::Snapshot(saved) =
            manager.remember_installation_choice(RememberInstallationChoiceRequest {
                schema_version: 1,
                request_id: Uuid::new_v4().to_string(),
                installation_id: selected.installation_id.clone(),
                scan_generation: scan.scan_generation,
            })
        else {
            panic!("Could not save installation");
        };
        assert_eq!(saved.status, "saved");
    }

    let reopened = open_manager();
    let fresh = snapshot(&reopened);
    for (client, version, _) in expected {
        let SavedInstallationChoiceReply::Snapshot(saved) =
            reopened.saved_installation_choice(SavedInstallationChoiceRequest {
                schema_version: 1,
                request_id: Uuid::new_v4().to_string(),
                client_kind: client.to_owned(),
            })
        else {
            panic!("Could not read saved installation");
        };
        assert_eq!(saved.status, "available");
        let (installation, _) = reopened
            .select_installation_with_path(&saved.installation_id.unwrap(), fresh.scan_generation)
            .unwrap();
        assert_eq!(installation.version.as_deref(), Some(version));
    }

    let unverified = fresh
        .installations
        .iter()
        .find(|row| row.version.is_none())
        .unwrap();
    let script = root.join("discovery-decoys/opencode");
    let file = OpenOptions::new().write(true).open(script).unwrap();
    let pre_edit = file.metadata().unwrap().modified().unwrap();
    let mut handle = &file;
    handle.write_all(b"#?").unwrap();
    file.set_times(FileTimes::new().set_modified(pre_edit))
        .unwrap();
    assert!(matches!(
        reopened.select_installation_with_path(&unverified.installation_id, fresh.scan_generation),
        Err(InstallationSelectionError::StaleScanGeneration)
    ));
}
