use super::*;
use std::os::windows::fs::OpenOptionsExt;

fn fixture() -> PathBuf {
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let parent = project.join("work/proofs/windows-uninstall/unit");
    fs::create_dir_all(&parent).unwrap();
    pin_existing_directory_chain(&parent)
        .unwrap()
        .validate()
        .unwrap();
    let root = parent.join(uuid::Uuid::new_v4().to_string());
    fs::create_dir(&root).unwrap();
    assert!(root
        .canonicalize()
        .unwrap()
        .starts_with(parent.canonicalize().unwrap()));
    root
}

#[test]
fn removal_gate_allows_multiple_apps_but_excludes_uninstall_and_relaunch() {
    let fixture = fixture();
    let root = fixture.join("dev.headroom.desktop");
    let first = open_desktop_use(&root).unwrap();
    let second = open_desktop_use(&root).unwrap();
    assert!(require_removal_gate(&root).is_err());
    assert_eq!(
        open_gate(&root, 0, OPEN_ALWAYS).unwrap_err().raw_os_error(),
        Some(ERROR_SHARING_VIOLATION as i32)
    );
    drop((first, second));
    let removal = open_gate(&root, 0, OPEN_ALWAYS).unwrap();
    assert!(require_removal_gate(&root).is_ok());
    assert!(open_desktop_use(&root).is_err());
    drop(removal);
    assert!(require_removal_gate(&root).is_err());
    drop(open_desktop_use(&root).unwrap());
    fs::remove_file(gate_path(&root).unwrap()).unwrap();
    fs::remove_dir(fixture).unwrap();
}

#[test]
fn removal_gate_never_overwrites_foreign_data() {
    let fixture = fixture();
    let root = fixture.join("dev.headroom.desktop");
    let gate = gate_path(&root).unwrap();
    fs::write(&gate, b"keep").unwrap();
    assert!(open_desktop_use(&root).is_err());
    assert!(require_removal_gate(&root).is_err());
    assert_eq!(fs::read(&gate).unwrap(), b"keep");
    fs::remove_file(gate).unwrap();
    fs::remove_dir(fixture).unwrap();
}

#[test]
fn purge_fails_on_locked_state_and_retries_without_touching_neighbors() {
    let fixture = fixture();
    let root = fixture.join("dev.headroom.desktop");
    fs::create_dir(&root).unwrap();
    let history = root.join("history");
    let neighbor = fixture.join("external-config");
    fs::write(&history, b"history").unwrap();
    fs::write(&neighbor, b"preserve").unwrap();
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&history)
        .unwrap();
    assert!(purge_local_state(&root).is_err());
    assert!(root.is_dir());
    drop(locked);
    assert_eq!(fs::read(&history).unwrap(), b"history");
    purge_local_state(&root).unwrap();
    assert!(!root.exists());
    purge_local_state(&root).unwrap();
    assert_eq!(fs::read(&neighbor).unwrap(), b"preserve");
    fs::remove_file(neighbor).unwrap();
    fs::remove_dir(fixture).unwrap();
}
