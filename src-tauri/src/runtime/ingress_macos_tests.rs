use super::{IngressError, StableIngress};
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

const RECORD_NAME: &str = "connection-endpoint.json";

struct FixtureRoot {
    base: PathBuf,
    path: PathBuf,
    identity: (u64, u64),
}

impl FixtureRoot {
    fn new() -> Self {
        let base = PathBuf::from(
            std::env::var_os("HEADROOM_MACOS_PROOF_ROOT")
                .expect("CI must provide HEADROOM_MACOS_PROOF_ROOT"),
        );
        assert!(base.is_absolute());
        let base_info = fs::symlink_metadata(&base).expect("proof root exists");
        assert!(base_info.is_dir() && !base_info.file_type().is_symlink());
        assert_eq!(base_info.uid(), unsafe { libc::geteuid() });
        assert_eq!(base_info.mode() & 0o7777, 0o700);
        assert_eq!(fs::canonicalize(&base).expect("canonical proof root"), base);

        let random = Uuid::new_v4().simple().to_string();
        let path = base.join(format!("ig-{}", &random[..8]));
        fs::create_dir(&path).expect("create isolated ingress fixture root");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .expect("restrict ingress fixture root");
        assert_eq!(
            fs::canonicalize(&path).expect("canonical fixture root"),
            path
        );
        let info = fs::symlink_metadata(&path).unwrap();
        assert_eq!(info.mode() & 0o7777, 0o700);
        Self {
            base,
            path,
            identity: (info.dev(), info.ino()),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn endpoint_record(&self) -> PathBuf {
        self.path.join(RECORD_NAME)
    }
}

impl Drop for FixtureRoot {
    fn drop(&mut self) {
        // This directory is unique to the test; remove only its direct entries.
        let Ok(current) = fs::symlink_metadata(&self.path) else {
            return;
        };
        if !current.is_dir()
            || current.file_type().is_symlink()
            || (current.dev(), current.ino()) != self.identity
            || fs::canonicalize(&self.path).ok().as_deref() != Some(self.path.as_path())
        {
            return;
        }
        if let Ok(entries) = fs::read_dir(&self.path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(kind) = entry.file_type() {
                    if kind.is_file() || kind.is_symlink() || kind.is_socket() {
                        let _ = fs::remove_file(path);
                    }
                }
            }
        }
        let _ = fs::remove_dir(&self.path);
    }
}

#[test]
fn reopen_preserves_the_owned_loopback_port_and_endpoint_id() {
    let root = FixtureRoot::new();
    let first = StableIngress::open_or_create(root.path()).expect("create stable endpoint");
    let port = first.port();
    let endpoint_id = first.endpoint_id().to_owned();
    let record_before = fs::read(root.endpoint_record()).expect("read endpoint record");
    let record_info = fs::symlink_metadata(root.endpoint_record()).unwrap();
    assert!(record_info.is_file() && !record_info.file_type().is_symlink());
    assert_eq!(record_info.uid(), unsafe { libc::geteuid() });
    assert_eq!(record_info.nlink(), 1);
    assert_eq!(record_info.mode() & 0o7777, 0o600);
    assert_ne!(port, 0);
    assert!(matches!(
        first.listener().local_addr().unwrap().ip(),
        std::net::IpAddr::V4(ip) if ip == Ipv4Addr::LOCALHOST
    ));
    assert!(first.is_owned());
    let parsed_id = Uuid::parse_str(&endpoint_id).expect("endpoint identity UUID");
    assert!(!parsed_id.is_nil());
    assert_eq!(parsed_id.to_string(), endpoint_id);
    let value: Value = serde_json::from_slice(&record_before).expect("endpoint JSON");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["bind_address"], "127.0.0.1");
    assert_eq!(value["port"], port);
    assert_eq!(value["endpoint_id"], endpoint_id);
    assert_eq!(value["revision"], 1);

    // Leave the server side in TIME_WAIT before reopening the same stable port.
    let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
    let mut client =
        TcpStream::connect_timeout(&address, Duration::from_millis(500)).expect("short probe");
    client
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    let (mut accepted, _) = first.listener().accept().expect("accept short probe");
    accepted.shutdown(Shutdown::Write).unwrap();
    let mut eof = [0u8; 1];
    assert_eq!(client.read(&mut eof).unwrap(), 0);
    client.shutdown(Shutdown::Write).unwrap();
    accepted
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    assert_eq!(accepted.read(&mut eof).unwrap(), 0);
    drop(accepted);
    drop(client);
    drop(first);

    let reopened = StableIngress::open_or_create(root.path()).expect("reopen same endpoint");
    assert_eq!(reopened.port(), port);
    assert_eq!(reopened.endpoint_id(), endpoint_id);
    assert!(reopened.is_owned());
    assert_eq!(fs::read(root.endpoint_record()).unwrap(), record_before);
}

#[test]
fn second_owner_is_rejected_and_record_drift_revokes_ownership() {
    let root = FixtureRoot::new();
    let owner = StableIngress::open_or_create(root.path()).expect("create stable endpoint");
    let before = fs::read(root.endpoint_record()).expect("read endpoint record");
    assert!(matches!(
        StableIngress::open_or_create(root.path()),
        Err(IngressError::EndpointConflict)
    ));
    assert!(owner.is_owned());
    assert_eq!(fs::read(root.endpoint_record()).unwrap(), before);

    let mut changed: Value = serde_json::from_slice(&before).unwrap();
    changed["endpoint_id"] = serde_json::json!(Uuid::new_v4().to_string());
    let changed = serde_json::to_vec(&changed).unwrap();
    fs::write(root.endpoint_record(), &changed).unwrap();
    assert!(!owner.is_owned());
    assert_eq!(fs::read(root.endpoint_record()).unwrap(), changed);
}

#[test]
fn foreign_listener_blocks_reopen_and_preserves_saved_record() {
    let root = FixtureRoot::new();
    let ingress = StableIngress::open_or_create(root.path()).expect("create endpoint");
    let port = ingress.port();
    let before = fs::read(root.endpoint_record()).expect("read endpoint record");
    drop(ingress);

    let foreign_loopback =
        TcpListener::bind((Ipv4Addr::LOCALHOST, port)).expect("claim loopback port");
    assert!(matches!(
        StableIngress::open_or_create(root.path()),
        Err(IngressError::EndpointConflict)
    ));
    assert_eq!(fs::read(root.endpoint_record()).unwrap(), before);
    assert_foreign_listener_accepts(&foreign_loopback, port);
    drop(foreign_loopback);

    let foreign_wildcard =
        TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).expect("claim wildcard port");
    assert!(matches!(
        StableIngress::open_or_create(root.path()),
        Err(IngressError::EndpointConflict)
    ));
    assert_eq!(fs::read(root.endpoint_record()).unwrap(), before);
    assert_foreign_listener_accepts(&foreign_wildcard, port);
    drop(foreign_wildcard);
}

fn assert_foreign_listener_accepts(listener: &TcpListener, port: u16) {
    let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
    let mut client = TcpStream::connect_timeout(&address, Duration::from_millis(500))
        .expect("foreign listener remains reachable");
    let (mut accepted, _) = listener.accept().expect("foreign listener accepts");
    client.shutdown(Shutdown::Write).unwrap();
    accepted
        .set_read_timeout(Some(Duration::from_millis(500)))
        .unwrap();
    let mut byte = [0u8; 1];
    assert_eq!(accepted.read(&mut byte).unwrap(), 0);
}

#[test]
fn symlink_and_non_private_roots_are_rejected() {
    let root = FixtureRoot::new();
    let random = Uuid::new_v4().simple().to_string();
    let alias = root.base.join(format!("alias-{}", &random[..8]));
    std::os::unix::fs::symlink(root.path(), &alias).expect("create owned root alias");
    assert!(matches!(
        StableIngress::open_or_create(&alias),
        Err(IngressError::InvalidRoot)
    ));
    fs::remove_file(&alias).unwrap();

    fs::set_permissions(root.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(matches!(
        StableIngress::open_or_create(root.path()),
        Err(IngressError::InvalidRoot)
    ));
}

#[test]
fn malformed_and_symlink_endpoint_records_are_rejected_and_preserved() {
    let root = FixtureRoot::new();
    let path = root.endpoint_record();
    let malformed = br#"{"schema_version":1,"port":0,"endpoint_id":"bad"}"#;
    fs::write(&path, malformed).expect("write isolated malformed record");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();

    assert!(matches!(
        StableIngress::open_or_create(root.path()),
        Err(IngressError::InvalidEndpointState)
    ));
    assert_eq!(fs::read(path).unwrap(), malformed);

    let record = root.endpoint_record();
    fs::remove_file(&record).unwrap();
    let target = root.path().join("record-data");
    let content = b"preserve fixture target";
    fs::write(&target, content).expect("write isolated target");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    std::os::unix::fs::symlink("record-data", &record).expect("create endpoint symlink");

    assert!(matches!(
        StableIngress::open_or_create(root.path()),
        Err(IngressError::InvalidEndpointState)
    ));
    assert!(fs::symlink_metadata(record)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read(target).unwrap(), content);
}
