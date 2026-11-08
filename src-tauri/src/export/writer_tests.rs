use super::*;
use std::fs;

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let path = root
            .join("work")
            .join(format!(".export-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let root = fs::canonicalize(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("work"),
        )
        .unwrap();
        let path = fs::canonicalize(&self.0).unwrap();
        assert!(path.starts_with(&root) && path != root);
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn writes_complete_unicode_filename_and_keeps_existing_bytes() {
    let dir = Directory::new();
    let target = dir.0.join("ölçüm raporu.JSON");
    let bytes = "{\"önce\":1200,\"sonra\":900}".as_bytes();
    save_new(&target, "json", bytes).unwrap();
    assert_eq!(fs::read(&target).unwrap(), bytes);
    assert_eq!(
        save_new(&target, "json", b"replacement"),
        Err(Error::FileExists)
    );
    assert_eq!(fs::read(&target).unwrap(), bytes);
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
}

#[test]
fn unsafe_or_wrong_extension_and_empty_reports_never_create_files() {
    let dir = Directory::new();
    for name in [
        "NUL.json",
        "data.json:stream.json",
        "wrong.txt",
        "last.json.",
    ] {
        assert_eq!(
            save_new(&dir.0.join(name), "json", b"data"),
            Err(Error::UnsafePath)
        );
    }
    assert_eq!(
        save_new(&dir.0.join("empty.json"), "json", b""),
        Err(Error::InvalidInput)
    );
    assert_eq!(
        save_new(
            &dir.0.join("large.json"),
            "json",
            &vec![b'x'; super::super::report::MAX_REPORT_BYTES + 1]
        ),
        Err(Error::InvalidInput)
    );
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 0);
}

#[test]
fn held_file_rename_preserves_racing_destination_and_cleans_staging() {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Foundation::GENERIC_WRITE;
    use windows_sys::Win32::Storage::FileSystem::{DELETE, FILE_SHARE_READ};
    let dir = Directory::new();
    let temporary = dir.0.join("staging.tmp");
    let destination = dir.0.join("report.json");
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .access_mode(GENERIC_WRITE | DELETE)
        .share_mode(FILE_SHARE_READ)
        .open(&temporary)
        .unwrap();
    let mut staging = Staging {
        file,
        published: false,
    };
    fs::write(&destination, b"foreign destination").unwrap();
    assert_eq!(staging.publish(&destination), Err(Error::FileExists));
    drop(staging);
    assert_eq!(fs::read(&destination).unwrap(), b"foreign destination");
    assert!(!temporary.exists());
}
