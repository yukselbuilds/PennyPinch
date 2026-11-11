use super::*;
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
    let manager = DiscoveryManager::new();
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
        selected.push(row.installation_id.clone());
    }
    let DiscoveryReply::Snapshot(next) = manager.scan(Uuid::new_v4().to_string()) else {
        panic!("rescan failed");
    };
    assert!(next.scan_generation > scan.scan_generation);
    for id in selected {
        assert!(matches!(
            manager.select_installation_with_path(&id, scan.scan_generation),
            Err(InstallationSelectionError::StaleScanGeneration)
        ));
    }
    std::process::exit(44);
}
