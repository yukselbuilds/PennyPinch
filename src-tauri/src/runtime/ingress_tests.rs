#![cfg(windows)]

use super::{
    bind_exclusive_loopback, publish_endpoint, verify_backend_peer, EndpointRecord, IngressError,
    StableIngress, WinsockRuntime, ENDPOINT_SCHEMA_VERSION,
};
use std::fs;
use std::mem::size_of;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};

use windows_sys::Win32::Networking::WinSock::{
    bind, closesocket, getsockname, listen, setsockopt, WSACleanup, WSASocketW, WSAStartup,
    AF_INET, INVALID_SOCKET, IPPROTO_TCP, SOCKADDR, SOCKADDR_IN, SOCK_STREAM, SOL_SOCKET,
    SO_REUSEADDR, WSADATA, WSA_FLAG_NO_HANDLE_INHERIT, WSA_FLAG_OVERLAPPED,
};

struct Winsock;

impl Winsock {
    fn start() -> Self {
        let mut data = WSADATA::default();
        // SAFETY: WSAStartup initializes the calling process Winsock refcount and writes WSADATA.
        let result = unsafe { WSAStartup(0x0202, &mut data) };
        assert_eq!(result, 0, "WSAStartup failed with code {result}");
        Self
    }
}

impl Drop for Winsock {
    fn drop(&mut self) {
        // SAFETY: balanced with the successful WSAStartup in start().
        let _ = unsafe { WSACleanup() };
    }
}

struct RawSocket(usize);

impl RawSocket {
    fn new() -> Self {
        // SAFETY: valid IPv4/TCP parameters; no protocol-info structure is supplied.
        let socket = unsafe {
            WSASocketW(
                AF_INET as i32,
                SOCK_STREAM,
                IPPROTO_TCP,
                std::ptr::null(),
                0,
                WSA_FLAG_NO_HANDLE_INHERIT | WSA_FLAG_OVERLAPPED,
            )
        };
        assert_ne!(socket, INVALID_SOCKET, "WSASocketW failed");
        Self(socket)
    }

    fn set_reuse(&self) {
        let enabled: i32 = 1;
        // SAFETY: enabled points to a valid i32 and optlen is exactly its size.
        let set_result = unsafe {
            setsockopt(
                self.0,
                SOL_SOCKET,
                SO_REUSEADDR,
                (&enabled as *const i32).cast(),
                size_of::<i32>() as i32,
            )
        };
        assert_eq!(set_result, 0, "setsockopt(SO_REUSEADDR) failed");
    }

    fn bind(&self, address: SocketAddrV4) -> i32 {
        let sockaddr = sockaddr_in(address);
        // SAFETY: sockaddr has the required IPv4 layout and valid lifetime for this call.
        unsafe {
            bind(
                self.0,
                (&sockaddr as *const SOCKADDR_IN).cast::<SOCKADDR>(),
                size_of::<SOCKADDR_IN>() as i32,
            )
        }
    }

    fn local_addr(&self) -> SocketAddrV4 {
        let mut sockaddr = SOCKADDR_IN::default();
        let mut length = size_of::<SOCKADDR_IN>() as i32;
        // SAFETY: sockaddr is writable and length matches its allocation.
        let result = unsafe {
            getsockname(
                self.0,
                (&mut sockaddr as *mut SOCKADDR_IN).cast::<SOCKADDR>(),
                &mut length,
            )
        };
        assert_eq!(result, 0, "getsockname failed");
        SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), u16::from_be(sockaddr.sin_port))
    }

    fn listen(&self) {
        // SAFETY: self is a valid bound TCP socket.
        assert_eq!(unsafe { listen(self.0, 16) }, 0, "listen failed");
    }
}

impl Drop for RawSocket {
    fn drop(&mut self) {
        // SAFETY: this wrapper uniquely owns the WSASocketW result.
        let _ = unsafe { closesocket(self.0) };
    }
}

fn sockaddr_in(address: SocketAddrV4) -> SOCKADDR_IN {
    let mut result = SOCKADDR_IN::default();
    result.sin_family = AF_INET;
    result.sin_port = address.port().to_be();
    result.sin_addr.S_un.S_addr = u32::from_ne_bytes(address.ip().octets());
    result
}

#[test]
fn reuseaddr_pair_is_a_positive_sharing_control() {
    let _winsock = Winsock::start();
    let owner = RawSocket::new();
    owner.set_reuse();
    assert_eq!(
        owner.bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
        0,
        "fixture bind failed"
    );
    owner.listen();

    let address = owner.local_addr();
    let contender = RawSocket::new();
    contender.set_reuse();
    let result = contender.bind(address);
    assert_eq!(result, 0, "positive control did not share the endpoint");
    contender.listen();
}

#[test]
fn exclusive_listener_rejects_reuseaddr_contender() {
    let _winsock = WinsockRuntime::start().expect("Winsock start");
    let listener = bind_exclusive_loopback(0).expect("exclusive loopback listener");
    let address = match listener.local_addr().expect("listener address") {
        std::net::SocketAddr::V4(address) => address,
        std::net::SocketAddr::V6(_) => panic!("expected IPv4 ingress"),
    };
    let contender = RawSocket::new();
    contender.set_reuse();
    assert_ne!(
        contender.bind(address),
        0,
        "SO_REUSEADDR contender shared the exclusive listener endpoint"
    );
}

#[test]
fn endpoint_file_reopens_same_port_and_conflict_never_replaces_it() {
    let fixture = FixtureRoot::new();
    let endpoint = StableIngress::open_or_create(&fixture.path).expect("create endpoint");
    let port = endpoint.port();
    let id = endpoint.endpoint_id().to_owned();
    let path = fixture.path.join("connection-endpoint.json");
    let original = fs::read(&path).expect("endpoint record");
    assert!(port != 0);
    drop(endpoint);

    let reopened = StableIngress::open_or_create(&fixture.path).expect("reopen endpoint");
    assert_eq!(reopened.port(), port);
    assert_eq!(reopened.endpoint_id(), id);
    drop(reopened);

    let foreign = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).expect("owned conflict fixture");
    assert!(matches!(
        StableIngress::open_or_create(&fixture.path),
        Err(IngressError::EndpointConflict)
    ));
    assert_eq!(fs::read(&path).expect("record after conflict"), original);
    assert_eq!(
        foreign
            .local_addr()
            .expect("foreign fixture address")
            .port(),
        port
    );
    drop(foreign);

    let reopened_after_conflict =
        StableIngress::open_or_create(&fixture.path).expect("reopen after fixture release");
    assert_eq!(reopened_after_conflict.port(), port);
    drop(reopened_after_conflict);
    fixture.cleanup();
}

#[test]
fn corrupt_endpoint_state_is_preserved_and_never_bound() {
    let fixture = FixtureRoot::new();
    let endpoint_path = fixture.path.join("connection-endpoint.json");
    let bytes = br#"{"schema_version":1,"bind_address":"127.0.0.1","port":41234,"endpoint_id":"8e86751c-2871-45bc-a6e5-3b80fd5c2f65","revision":1,"pid":777}"#;
    fs::write(&endpoint_path, bytes).expect("write owned corrupt-state fixture");

    assert_eq!(
        StableIngress::open_or_create(&fixture.path).err(),
        Some(IngressError::InvalidEndpointState)
    );
    assert_eq!(
        fs::read(&endpoint_path).expect("read preserved fixture"),
        bytes
    );
    fixture.cleanup();
}

#[test]
fn first_endpoint_publish_conflict_does_not_replace_existing_file() {
    let fixture = FixtureRoot::new();
    let endpoint_path = fixture.path.join("connection-endpoint.json");
    let existing = b"preserve-existing-bytes";
    fs::write(&endpoint_path, existing).expect("create owned publication conflict fixture");
    let candidate = EndpointRecord {
        schema_version: ENDPOINT_SCHEMA_VERSION,
        bind_address: "127.0.0.1".to_owned(),
        port: 43123,
        endpoint_id: uuid::Uuid::new_v4().to_string(),
        revision: 1,
    };

    assert_eq!(
        publish_endpoint(&fixture.path, &endpoint_path, &candidate),
        Err(IngressError::EndpointConflict)
    );
    assert_eq!(
        fs::read(&endpoint_path).expect("read preserved target"),
        existing
    );
    fixture.cleanup();
}

#[test]
fn accepted_connection_restart_rebind_is_measured_without_changing_port() {
    let fixture = FixtureRoot::new();
    let endpoint = StableIngress::open_or_create(&fixture.path).expect("create endpoint");
    let port = endpoint.port();
    let client = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).expect("client connect");
    let (accepted, _) = endpoint.listener().accept().expect("fixture accept");
    drop(accepted);
    drop(client);
    drop(endpoint);

    match StableIngress::open_or_create(&fixture.path) {
        Ok(reopened) => {
            assert_eq!(reopened.port(), port);
            println!("accepted_connection_rebind=available; saved_port_preserved=true");
            drop(reopened);
        }
        Err(IngressError::EndpointConflict) => {
            println!("accepted_connection_rebind=time_wait_conflict; saved_port_preserved=true");
        }
        Err(other) => panic!("unexpected bounded rebind result: {other:?}"),
    }
    fixture.cleanup();
}

#[test]
fn established_backend_peer_must_match_expected_process() {
    let _winsock = WinsockRuntime::start().expect("Winsock start");
    let backend = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("local backend");
    let address = backend.local_addr().expect("backend address");
    let client = TcpStream::connect(address).expect("backend connect");
    let (_accepted, _) = backend.accept().expect("backend accept");

    assert_eq!(verify_backend_peer(&client, std::process::id()), Ok(()));
    assert_eq!(
        verify_backend_peer(&client, std::process::id().saturating_add(1)),
        Err(IngressError::PeerOwnerMismatch)
    );
}

#[test]
fn peer_owner_table_requires_one_exact_established_row() {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        MIB_TCPROW_OWNER_PID, MIB_TCP_STATE_ESTAB,
    };

    let local = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 43120);
    let remote = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 51432);
    let make_row = |pid| MIB_TCPROW_OWNER_PID {
        dwState: MIB_TCP_STATE_ESTAB as u32,
        dwLocalAddr: u32::from_ne_bytes(local.ip().octets()),
        dwLocalPort: u32::from(local.port().to_be()),
        dwRemoteAddr: u32::from_ne_bytes(remote.ip().octets()),
        dwRemotePort: u32::from(remote.port().to_be()),
        dwOwningPid: pid,
    };

    assert_eq!(super::exact_peer_owner(&[], local, remote), Ok(None));
    assert_eq!(
        super::exact_peer_owner(&[make_row(101)], local, remote),
        Ok(Some(101))
    );
    assert_eq!(
        super::exact_peer_owner(&[make_row(101), make_row(101)], local, remote),
        Err(IngressError::PeerOwnerAmbiguous)
    );
    assert_eq!(
        super::exact_peer_owner(
            &[make_row(101)],
            local,
            SocketAddrV4::new(*remote.ip(), 51433)
        ),
        Ok(None)
    );
}

struct FixtureRoot {
    path: PathBuf,
    proof_root: PathBuf,
}

impl FixtureRoot {
    fn new() -> Self {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let project_root = manifest_dir
            .parent()
            .expect("project root")
            .canonicalize()
            .expect("canonical project root");
        let proof_root = project_root.join("work/proofs/connection-ingress");
        assert_clean_ancestors(&proof_root);
        fs::create_dir_all(&proof_root).expect("create proof fixture root");
        let proof_root = proof_root.canonicalize().expect("canonical fixture root");
        assert!(proof_root.starts_with(&project_root));
        assert_clean_ancestors(&proof_root);
        let path = proof_root.join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&path).expect("create unique fixture directory");
        let path = path.canonicalize().expect("canonical unique fixture");
        assert!(path.starts_with(&proof_root));
        assert_clean_ancestors(&path);
        Self { path, proof_root }
    }

    fn cleanup(self) {
        let resolved = self.path.canonicalize().expect("resolve owned fixture");
        assert_eq!(resolved, self.path, "fixture root changed identity");
        assert!(resolved.starts_with(&self.proof_root));
        assert_clean_tree(&resolved, &resolved);
        fs::remove_dir_all(resolved).expect("remove only this validated UUID fixture");
    }
}

fn assert_clean_ancestors(path: &Path) {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    for ancestor in path.ancestors() {
        let Ok(metadata) = fs::symlink_metadata(ancestor) else {
            continue;
        };
        assert!(!metadata.file_type().is_symlink());
        assert_eq!(metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT, 0);
    }
}

fn assert_clean_tree(root: &Path, current: &Path) {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    let metadata = fs::symlink_metadata(current).expect("fixture tree metadata");
    assert!(!metadata.file_type().is_symlink());
    assert_eq!(metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT, 0);
    assert!(current
        .canonicalize()
        .expect("fixture descendant canonical")
        .starts_with(root));
    if metadata.is_dir() {
        for entry in fs::read_dir(current).expect("fixture directory entries") {
            assert_clean_tree(root, &entry.expect("fixture entry").path());
        }
    }
}
