use super::*;
use std::fs;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::{Arc, Barrier};

struct Directory(PathBuf);

impl Directory {
    fn new() -> Self {
        let work = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("work")
            .canonicalize()
            .unwrap();
        let path = work.join(format!(".export-macos-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let work = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("work")
            .canonicalize()
            .unwrap();
        assert_eq!(self.0.parent(), Some(work.as_path()));
        assert_eq!(self.0.canonicalize().unwrap(), self.0);
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn unicode_report_is_complete_private_and_existing_target_is_preserved() {
    let directory = Directory::new();
    let target = directory.0.join("ölçüm 项目.JSON");
    let bytes = "{\"önce\":1200,\"sonra\":900}".as_bytes();
    assert_eq!(save_new(&target, "json", bytes), Ok(()));
    assert_eq!(fs::read(&target).unwrap(), bytes);
    let metadata = fs::symlink_metadata(&target).unwrap();
    assert_eq!(metadata.mode() & 0o7777, 0o600);
    assert_eq!(metadata.nlink(), 1);
    assert_eq!(
        save_new(&target, "json", b"replacement"),
        Err(Error::FileExists)
    );
    assert_eq!(fs::read(&target).unwrap(), bytes);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn invalid_paths_and_size_limits_create_no_files() {
    let directory = Directory::new();
    for target in [
        PathBuf::from("relative.json"),
        directory.0.join("wrong.txt"),
        directory.0.join("line\nbreak.json"),
        PathBuf::from(format!("{}/./alias.json", directory.0.display())),
        directory.0.join("missing/report.json"),
    ] {
        assert_eq!(save_new(&target, "json", b"data"), Err(Error::UnsafePath));
    }
    let target = directory.0.join("report.json");
    assert_eq!(save_new(&target, "json", b""), Err(Error::InvalidInput));
    assert_eq!(
        save_new(
            &target,
            "json",
            &vec![b'x'; super::super::report::MAX_REPORT_BYTES + 1]
        ),
        Err(Error::InvalidInput)
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 0);
}

#[test]
fn parent_and_destination_symlinks_never_redirect_a_report() {
    let directory = Directory::new();
    let actual = directory.0.join("actual");
    fs::create_dir(&actual).unwrap();
    symlink(&actual, directory.0.join("alias")).unwrap();
    assert_eq!(
        save_new(&directory.0.join("alias/report.json"), "json", b"data"),
        Err(Error::UnsafePath)
    );
    let original = actual.join("original.json");
    fs::write(&original, b"foreign bytes").unwrap();
    let link = directory.0.join("report.json");
    symlink(&original, &link).unwrap();
    assert_eq!(
        save_new(&link, "json", b"replacement"),
        Err(Error::FileExists)
    );
    assert_eq!(fs::read(&original).unwrap(), b"foreign bytes");
    assert_eq!(fs::read_dir(&actual).unwrap().count(), 1);
}

#[test]
fn concurrent_exports_publish_one_complete_file_without_overwrite_or_staging_leaks() {
    let directory = Directory::new();
    let target = directory.0.join("report.csv");
    let barrier = Arc::new(Barrier::new(6));
    let workers = (0..6)
        .map(|index| {
            let barrier = barrier.clone();
            let target = target.clone();
            std::thread::spawn(move || {
                let bytes = vec![b'a' + index; super::super::report::MAX_REPORT_BYTES];
                barrier.wait();
                (save_new(&target, "csv", &bytes), bytes)
            })
        })
        .collect::<Vec<_>>();
    let outcomes = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    let winner = outcomes
        .iter()
        .filter(|(result, _)| result.is_ok())
        .collect::<Vec<_>>();
    assert_eq!(winner.len(), 1);
    assert_eq!(fs::read(&target).unwrap(), winner[0].1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|(result, _)| *result == Err(Error::FileExists))
            .count(),
        5
    );
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
}
