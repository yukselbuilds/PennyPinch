use super::*;
use std::os::unix::fs::{symlink, PermissionsExt};

fn proof_root() -> PathBuf {
    let work = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work")
        .canonicalize()
        .unwrap();
    let root = PathBuf::from(
        env::var_os("HEADROOM_MACOS_DESKTOP_PROOF_ROOT").expect("isolated desktop root"),
    );
    let metadata = fs::symlink_metadata(&root).unwrap();
    assert_eq!(root.canonicalize().unwrap(), root);
    assert!(root != work && root.starts_with(work));
    assert!(metadata.is_dir() && metadata.uid() == unsafe { libc::geteuid() });
    assert_eq!(metadata.mode() & 0o7777, 0o700);
    root
}

#[test]
fn signed_desktop_scan_rebinds_selection_and_rejects_untrusted_bundles() {
    let root = proof_root();
    let home = root.join("home");
    let applications = root.join("system-applications");
    let cli_bin = root.join("bin");
    let app_data = root.join("app-data");
    let bundle = home.join("Applications/ChatGPT.app");
    let launcher = bundle.join("Contents/MacOS/ChatGPT");
    let plist = bundle.join("Contents/Info.plist");
    let version = env::var("HEADROOM_MACOS_DESKTOP_VERSION").unwrap();
    let bytes = fs::read(&plist).unwrap();

    fs::create_dir(&applications).unwrap();
    fs::create_dir(&cli_bin).unwrap();
    symlink(&bundle, applications.join("ChatGPT.app")).unwrap();
    let unsigned = home.join("Applications/Codex.app/Contents");
    fs::create_dir_all(unsigned.join("MacOS")).unwrap();
    fs::write(unsigned.join("Info.plist"), &bytes).unwrap();
    for path in [unsigned.join("MacOS/ChatGPT"), cli_bin.join("codex")] {
        fs::write(&path, b"fixture executable bytes").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    let open = || {
        DiscoveryManager::with_choice_store(Some(
            ChoiceStore::for_app_local_data(Some(app_data.clone())).unwrap(),
        ))
    };
    let scan = |manager: &DiscoveryManager| {
        let DiscoveryReply::Snapshot(snapshot) = manager.scan_with_inputs(
            Uuid::new_v4().to_string(),
            DiscoveryInputs {
                macos_home: Some(home.clone()),
                macos_applications: Some(applications.clone()),
                path_entries: vec![cli_bin.clone()],
                ..Default::default()
            },
        ) else {
            panic!("desktop scan failed");
        };
        assert_eq!(
            snapshot
                .installations
                .iter()
                .filter(|row| row.client_kind == "codex" && row.surface == "cli")
                .count(),
            1
        );
        assert!(!serde_json::to_string(&snapshot)
            .unwrap()
            .contains(root.to_str().unwrap()));
        snapshot
    };
    let read = |manager: &DiscoveryManager| {
        let SavedInstallationChoiceReply::Snapshot(snapshot) =
            manager.saved_installation_choice(SavedInstallationChoiceRequest {
                schema_version: 1,
                request_id: Uuid::new_v4().to_string(),
                client_kind: "codex".to_owned(),
            })
        else {
            panic!("desktop choice lookup failed");
        };
        snapshot
    };
    let manager = open();
    let snapshot = scan(&manager);
    assert!(!app_data.exists());
    assert!(snapshot.warnings.contains(&"desktop_metadata_untrusted"));
    let desktop = desktop_row(&snapshot);
    assert_eq!(desktop.version.as_deref(), Some(version.as_str()));
    assert_eq!(desktop.status, "detected");
    assert_eq!(desktop.connection_status, "not_verified");
    assert_eq!(desktop.source, "macos_app_bundle");
    assert_eq!(desktop.location_label, "macOS application");
    assert_eq!(
        manager
            .select_installation_with_path(&desktop.installation_id, snapshot.scan_generation)
            .unwrap()
            .1,
        launcher
    );
    let SavedInstallationChoiceReply::Snapshot(saved) =
        manager.remember_installation_choice(RememberInstallationChoiceRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            scan_generation: snapshot.scan_generation,
            installation_id: desktop.installation_id.clone(),
        })
    else {
        panic!("desktop choice could not be saved");
    };
    assert_eq!(saved.status, "saved");
    drop(manager);

    let reopened = open();
    let fresh = scan(&reopened);
    let restored = read(&reopened);
    assert_eq!(restored.status, "available");
    let rebound = desktop_row(&fresh);
    assert_eq!(
        restored.installation_id.as_ref(),
        Some(&rebound.installation_id)
    );
    assert_ne!(rebound.installation_id, desktop.installation_id);

    let original = root.join("original-launcher");
    fs::rename(&launcher, &original).unwrap();
    fs::copy(&original, &launcher).unwrap();
    assert!(matches!(
        reopened.select_installation_with_path(&rebound.installation_id, fresh.scan_generation),
        Err(InstallationSelectionError::StaleScanGeneration)
    ));
    assert_eq!(
        desktop_row(&scan(&reopened)).version.as_deref(),
        Some(version.as_str())
    );
    assert_eq!(read(&reopened).status, "unavailable");

    // Preserve the plist size while invalidating the signed version metadata.
    let unchanged = scan(&reopened);
    let old_selection = desktop_row(&unchanged);
    let mut modified = bytes.clone();
    let version_start = bytes
        .windows(version.len())
        .position(|window| window == version.as_bytes())
        .unwrap();
    modified[version_start] = b'9';
    fs::write(&plist, modified).unwrap();
    assert!(matches!(
        reopened.select_installation_with_path(
            &old_selection.installation_id,
            unchanged.scan_generation
        ),
        Err(InstallationSelectionError::StaleScanGeneration)
    ));
    assert!(matches!(
        reopened.remember_installation_choice(RememberInstallationChoiceRequest {
            schema_version: 1,
            request_id: Uuid::new_v4().to_string(),
            scan_generation: unchanged.scan_generation,
            installation_id: old_selection.installation_id.clone(),
        }),
        SavedInstallationChoiceReply::Error(_)
    ));
    let altered = scan(&reopened);
    fs::write(&plist, &bytes).unwrap();
    assert!(altered
        .installations
        .iter()
        .all(|row| row.surface != "desktop"));
    assert!(altered.warnings.contains(&"desktop_metadata_untrusted"));

    let permissions = fs::metadata(&plist).unwrap().permissions();
    fs::set_permissions(&plist, fs::Permissions::from_mode(0o666)).unwrap();
    let writable = scan(&reopened);
    fs::set_permissions(&plist, permissions).unwrap();
    assert!(writable
        .installations
        .iter()
        .all(|row| row.surface != "desktop"));

    let alternate = applications.join("Codex.app");
    fs::rename(&bundle, &alternate).unwrap();
    let renamed = scan(&reopened);
    fs::rename(&alternate, &bundle).unwrap();
    assert_eq!(
        desktop_row(&renamed).version.as_deref(),
        Some(version.as_str())
    );
    assert!(!home.join(".codex").exists());
}

fn desktop_row(snapshot: &DiscoverySnapshot) -> &InstallationSummary {
    let mut rows = snapshot
        .installations
        .iter()
        .filter(|row| row.client_kind == "codex" && row.surface == "desktop");
    let row = rows
        .next()
        .expect("official Desktop installation was not detected");
    assert!(rows.next().is_none());
    row
}
