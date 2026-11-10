use super::*;
use std::time::{Duration, Instant};

fn root() -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work/proofs/update/swap")
        .join(Uuid::new_v4().to_string());
    fs::create_dir_all(&path).unwrap();
    path
}

fn file(path: &Path, contents: &[u8]) -> FileDigest {
    fs::write(path, contents).unwrap();
    FileDigest::from_file(&mut read_owned(path).unwrap()).unwrap()
}

#[test]
fn publishes_an_entire_file_and_restores_the_previous_file() {
    let root = root();
    let live = root.join("headroom.exe");
    let incoming = root.join("candidate.exe");
    let old = file(&live, b"previous working application");
    let next = file(&incoming, b"next complete application");
    let backup = root.join("previous.exe");
    copy_verified(&backup, &mut read_owned(&live).unwrap(), &old).unwrap();
    Swap::stage(&live, &mut read_owned(&incoming).unwrap(), &next)
        .unwrap()
        .publish(&old)
        .unwrap();
    next.check(&mut read_owned(&live).unwrap()).unwrap();
    Swap::stage(&live, &mut read_owned(&backup).unwrap(), &old)
        .unwrap()
        .publish(&next)
        .unwrap();
    old.check(&mut read_owned(&live).unwrap()).unwrap();
}

#[test]
fn staging_write_failure_preserves_the_current_application() {
    let root = root();
    let live = root.join("headroom.exe");
    let old = file(&live, b"working");
    let source = root.join("source.exe");
    file(&source, b"incoming");
    let mut readonly = read_owned(&source).unwrap();
    let bytes = FileDigest::from_file(&mut readonly).unwrap();
    let target = root.join("occupied.exe");
    fs::write(&target, b"other owner").unwrap();
    assert!(copy_verified(&target, &mut readonly, &bytes).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"other owner");
    old.check(&mut read_owned(&live).unwrap()).unwrap();
}

#[test]
fn interrupted_staging_changed_sources_and_locked_targets_preserve_live_bytes() {
    let root = root();
    let live = root.join("headroom.exe");
    let source = root.join("source.exe");
    let old = file(&live, b"working");
    let next = file(&source, b"candidate");
    let swap = Swap::stage(&live, &mut read_owned(&source).unwrap(), &next).unwrap();
    let staged = swap.path().to_owned();
    drop(swap);
    assert!(!staged.exists());
    old.check(&mut read_owned(&live).unwrap()).unwrap();
    let swap = Swap::stage(&live, &mut read_owned(&source).unwrap(), &next).unwrap();
    fs::write(swap.path(), b"modified candidate").unwrap();
    assert!(swap.publish(&old).is_err());
    old.check(&mut read_owned(&live).unwrap()).unwrap();
    let swap = Swap::stage(&live, &mut read_owned(&source).unwrap(), &next).unwrap();
    let locked = read_owned(&live).unwrap();
    assert!(swap.publish(&old).is_err());
    drop(locked);
    old.check(&mut read_owned(&live).unwrap()).unwrap();
    let swap = Swap::stage(&live, &mut read_owned(&source).unwrap(), &next).unwrap();
    fs::write(&live, b"external change").unwrap();
    assert!(swap.publish(&old).is_err());
    assert_eq!(fs::read(&live).unwrap(), b"external change");
}

#[test]
fn disk_full_during_copy_removes_the_partial_file_and_preserves_live_bytes() {
    let root = root();
    let live = root.join("headroom.exe");
    let old = file(&live, b"working application");
    let source = root.join("source.exe");
    let next = file(&source, b"new application");
    fault::disk_full_next_copy();
    let error = Swap::stage(&live, &mut read_owned(&source).unwrap(), &next)
        .err()
        .unwrap();
    assert_eq!(
        error.raw_os_error(),
        Some(windows_sys::Win32::Foundation::ERROR_DISK_FULL as i32)
    );
    old.check(&mut read_owned(&live).unwrap()).unwrap();
    assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
}

#[test]
#[ignore = "owned child fixture used by the publication interruption test"]
fn publication_child() {
    let root = PathBuf::from(std::env::var_os("HEADROOM_TEST_SWAP_ROOT").unwrap());
    let boundary = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("work/proofs/update/swap");
    assert!(fs::canonicalize(&root)
        .unwrap()
        .starts_with(fs::canonicalize(boundary).unwrap()));
    let mut source = read_owned(&root.join("source.exe")).unwrap();
    let next = FileDigest::from_file(&mut source).unwrap();
    let old = FileDigest::from_file(&mut read_owned(&root.join("headroom.exe")).unwrap()).unwrap();
    Swap::stage(&root.join("headroom.exe"), &mut source, &next)
        .unwrap()
        .publish(&old)
        .unwrap();
    panic!("the parent must terminate this test at the checkpoint");
}

#[test]
fn terminated_publication_process_leaves_a_complete_version_and_a_restorable_backup() {
    use std::process::{Command, Stdio};
    for point in ["before_publish", "after_publish"] {
        let root = root();
        let live = root.join("headroom.exe");
        let source = root.join("source.exe");
        let backup = root.join("previous.exe");
        let old = file(&live, b"complete previous application");
        let next = file(&source, b"complete next application");
        copy_verified(&backup, &mut read_owned(&live).unwrap(), &old).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "desktop::update_commit::tests::publication_child",
                "--exact",
                "--ignored",
            ])
            .env("HEADROOM_TEST_SWAP_ROOT", &root)
            .env("HEADROOM_TEST_SWAP_POINT", point)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while !root.join("checkpoint").exists() && Instant::now() < deadline {
            assert!(
                child.try_wait().unwrap().is_none(),
                "child exited before publication checkpoint"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let checkpoint = root.join("checkpoint").is_file();
        let _ = child.kill();
        child.wait().unwrap();
        assert!(checkpoint, "publication checkpoint was not reached");
        old.check(&mut read_owned(&backup).unwrap()).unwrap();
        if point == "before_publish" {
            old.check(&mut read_owned(&live).unwrap()).unwrap();
        } else {
            next.check(&mut read_owned(&live).unwrap()).unwrap();
            Swap::stage(&live, &mut read_owned(&backup).unwrap(), &old)
                .unwrap()
                .publish(&next)
                .unwrap();
            old.check(&mut read_owned(&live).unwrap()).unwrap();
        }
    }
}
