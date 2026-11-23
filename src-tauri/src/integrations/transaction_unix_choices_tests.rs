use super::*;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::process::{Command, Stdio};
use std::thread;

const OLD: &[u8] = br#"{"schema_version":1,"revision":1,"installations":[],"claude":null}"#;
const NEXT: &[u8] = br#"{"schema_version":1,"revision":2,"installations":[],"claude":null}"#;
const OTHER: &[u8] = br#"{"schema_version":1,"revision":3,"installations":[],"claude":null}"#;
const LIMIT: usize = 16 * 1024;
#[cfg(target_os = "macos")]
const PROOF_ROOT_ENV: &str = "HEADROOM_MACOS_PROOF_ROOT";
#[cfg(target_os = "linux")]
const PROOF_ROOT_ENV: &str = "HEADROOM_LINUX_PROOF_ROOT";

fn proof_root() -> PathBuf {
    let work = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work")
        .canonicalize()
        .unwrap();
    let root = PathBuf::from(std::env::var_os(PROOF_ROOT_ENV).unwrap());
    assert_eq!(root.canonicalize().unwrap(), root);
    assert!(root != work && root.starts_with(work));
    Directory::open(&root, true).unwrap();
    root
}

fn fixture() -> PathBuf {
    let root = proof_root().join(format!("selection-{}", Uuid::new_v4()));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    root
}

fn snapshot(root: &Path) -> Option<Vec<u8>> {
    SelectionFile::open(root, LIMIT, false)
        .unwrap()
        .bytes()
        .map(<[u8]>::to_vec)
}

#[test]
#[cfg_attr(target_os = "linux", ignore = "requires isolated Ubuntu ext4 root")]
fn late_selection_replacement_is_preserved_and_reported_as_conflict() {
    for initial in [None, Some(OLD)] {
        let root = fixture();
        assert_eq!(snapshot(&root), None);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
        if let Some(bytes) = initial {
            SelectionFile::open(&root, LIMIT, true)
                .unwrap()
                .publish(bytes)
                .unwrap();
        }
        let competing = root.join("competing.json");
        fs::write(&competing, OTHER).unwrap();
        fs::set_permissions(&competing, fs::Permissions::from_mode(0o600)).unwrap();
        let competing_identity = fs::metadata(&competing).unwrap().ino();
        let result = SelectionFile::open(&root, LIMIT, true)
            .unwrap()
            .publish_with(
                NEXT,
                || {
                    fs::rename(&competing, root.join("selections.json")).unwrap();
                },
                || {},
            );
        assert_eq!(result, Err(TransactionFsError::Conflict));
        assert_eq!(snapshot(&root).as_deref(), Some(OTHER));
        assert_eq!(
            fs::metadata(root.join("selections.json")).unwrap().ino(),
            competing_identity
        );
        assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
    }
}

fn run_owned_child(root: &Path, phase: &str, expected: i32) {
    let module = module_path!()
        .strip_prefix(&format!("{}::", env!("CARGO_CRATE_NAME")))
        .unwrap_or(module_path!());
    let filter = format!("{module}::owned_selection_file_child");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &filter, "--ignored", "--test-threads=1"])
        .env_clear()
        .env(PROOF_ROOT_ENV, proof_root())
        .env("HEADROOM_TEST_SELECTION_ROOT", root)
        .env("HEADROOM_TEST_SELECTION_PHASE", phase)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match child.try_wait().unwrap() {
            Some(status) => {
                assert_eq!(
                    status.code(),
                    Some(expected),
                    "owned selection fixture phase {phase}"
                );
                break;
            }
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("owned selection child deadline");
            }
        }
    }
}

#[test]
#[cfg_attr(target_os = "linux", ignore = "requires isolated Ubuntu ext4 root")]
fn selection_file_serializes_writers_and_reopens_after_process_exit() {
    let root = fixture();
    SelectionFile::open(&root, LIMIT, true)
        .unwrap()
        .publish(OLD)
        .unwrap();
    let held = SelectionFile::open(&root, LIMIT, true).unwrap();
    run_owned_child(&root, "locked", 50);
    drop(held);
    run_owned_child(&root, "before", 51);
    assert_eq!(snapshot(&root).as_deref(), Some(OLD));
    run_owned_child(&root, "after", 52);
    assert_eq!(snapshot(&root).as_deref(), Some(NEXT));
    SelectionFile::open(&root, LIMIT, true)
        .unwrap()
        .publish(OTHER)
        .unwrap();
    assert_eq!(snapshot(&root).as_deref(), Some(OTHER));
}

#[test]
#[ignore = "invoked by the private cross-process selection fixture"]
fn owned_selection_file_child() {
    let root = PathBuf::from(std::env::var_os("HEADROOM_TEST_SELECTION_ROOT").unwrap());
    assert_eq!(root.parent(), Some(proof_root().as_path()));
    let phase = std::env::var("HEADROOM_TEST_SELECTION_PHASE").unwrap();
    let opened = SelectionFile::open(&root, LIMIT, true);
    if phase == "locked" {
        assert!(matches!(opened, Err(TransactionFsError::Conflict)));
        std::process::exit(50);
    }
    opened
        .unwrap()
        .publish_with(
            NEXT,
            || {
                if phase == "before" {
                    std::process::exit(51);
                }
            },
            || {
                if phase == "after" {
                    std::process::exit(52);
                }
            },
        )
        .unwrap();
    panic!("selection process did not reach the requested phase");
}
