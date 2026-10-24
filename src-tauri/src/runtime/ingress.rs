//! Stable, loopback-only ingress ownership primitives for H040.
//!
//! This module owns the stable listener and endpoint record. The independent
//! generation-pinned relay is in `ingress_relay`; RuntimeManager owns their
//! lifecycle integration.

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const ENDPOINT_SCHEMA_VERSION: u8 = 1;
const MAX_ENDPOINT_BYTES: u64 = 1024;
const LISTEN_BACKLOG: i32 = 128;
const MAX_TCP_TABLE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IngressError {
    InvalidRoot,
    StateUnavailable,
    InvalidEndpointState,
    EndpointConflict,
    SocketUnavailable,
    PeerOwnerUnknown,
    PeerOwnerTableUnavailable,
    PeerOwnerMissing,
    PeerOwnerMismatch,
    PeerOwnerAmbiguous,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct EndpointRecord {
    schema_version: u8,
    bind_address: String,
    port: u16,
    endpoint_id: String,
    revision: u64,
}

/// A bound stable endpoint. The listener field intentionally precedes the
/// Winsock guard so it closes before the matching WSACleanup call.
pub struct StableIngress {
    listener: TcpListener,
    _winsock: WinsockRuntime,
    record: EndpointRecord,
}

impl StableIngress {
    /// Open the saved loopback port, or bind port zero and publish its record
    /// without replacing an existing endpoint file.
    pub fn open_or_create(trusted_runtime_root: &Path) -> Result<Self, IngressError> {
        let root = validate_runtime_root(trusted_runtime_root)?;
        let _winsock = WinsockRuntime::start()?;
        let endpoint_path = root.join("connection-endpoint.json");

        match safe_regular_file(&root, &endpoint_path)? {
            true => {
                let record = read_endpoint(&root, &endpoint_path)?;
                let listener = bind_exclusive_loopback(record.port)?;
                Ok(Self {
                    listener,
                    _winsock,
                    record,
                })
            }
            false => {
                let listener = bind_exclusive_loopback(0)?;
                let port = match listener
                    .local_addr()
                    .map_err(|_| IngressError::SocketUnavailable)?
                {
                    SocketAddr::V4(address) if address.ip() == &Ipv4Addr::LOCALHOST => {
                        address.port()
                    }
                    _ => return Err(IngressError::SocketUnavailable),
                };
                let record = EndpointRecord {
                    schema_version: ENDPOINT_SCHEMA_VERSION,
                    bind_address: Ipv4Addr::LOCALHOST.to_string(),
                    port,
                    endpoint_id: Uuid::new_v4().to_string(),
                    revision: 1,
                };
                publish_endpoint(&root, &endpoint_path, &record)?;
                Ok(Self {
                    listener,
                    _winsock,
                    record,
                })
            }
        }
    }

    pub fn listener(&self) -> &TcpListener {
        &self.listener
    }

    pub fn port(&self) -> u16 {
        self.record.port
    }

    pub fn endpoint_id(&self) -> &str {
        &self.record.endpoint_id
    }
}

#[derive(Debug)]
struct WinsockRuntime;

impl WinsockRuntime {
    fn start() -> Result<Self, IngressError> {
        use windows_sys::Win32::Networking::WinSock::{WSAStartup, WSADATA};
        let mut data = WSADATA::default();
        // SAFETY: WSAStartup writes the supplied WSADATA and acquires one
        // process-local Winsock reference, balanced by Drop.
        if unsafe { WSAStartup(0x0202, &mut data) } == 0 {
            Ok(Self)
        } else {
            Err(IngressError::SocketUnavailable)
        }
    }
}

impl Drop for WinsockRuntime {
    fn drop(&mut self) {
        use windows_sys::Win32::Networking::WinSock::WSACleanup;
        // SAFETY: this guard exists only after its successful WSAStartup.
        let _ = unsafe { WSACleanup() };
    }
}

#[derive(Debug)]
struct WinsockSocket(usize);

impl WinsockSocket {
    fn new() -> Result<Self, IngressError> {
        use windows_sys::Win32::Networking::WinSock::{
            WSASocketW, AF_INET, INVALID_SOCKET, IPPROTO_TCP, SOCK_STREAM,
            WSA_FLAG_NO_HANDLE_INHERIT, WSA_FLAG_OVERLAPPED,
        };
        // SAFETY: this creates a fresh overlapped IPv4/TCP socket. No protocol
        // info is inherited and handle inheritance is explicitly disabled.
        let socket = unsafe {
            WSASocketW(
                AF_INET as i32,
                SOCK_STREAM,
                IPPROTO_TCP,
                std::ptr::null(),
                0,
                WSA_FLAG_OVERLAPPED | WSA_FLAG_NO_HANDLE_INHERIT,
            )
        };
        if socket == INVALID_SOCKET {
            Err(IngressError::SocketUnavailable)
        } else {
            Ok(Self(socket))
        }
    }
}

impl Drop for WinsockSocket {
    fn drop(&mut self) {
        use windows_sys::Win32::Networking::WinSock::closesocket;
        // SAFETY: this wrapper uniquely owns the WSASocketW handle.
        let _ = unsafe { closesocket(self.0) };
    }
}

fn bind_exclusive_loopback(port: u16) -> Result<TcpListener, IngressError> {
    use std::os::windows::io::{FromRawSocket, RawSocket};
    use windows_sys::Win32::Networking::WinSock::{
        bind, listen, setsockopt, SOCKADDR, SOCKADDR_IN, SOL_SOCKET, SO_EXCLUSIVEADDRUSE,
    };

    let socket = WinsockSocket::new()?;
    let enabled: i32 = 1;
    // SO_EXCLUSIVEADDRUSE must be applied before bind. This prevents a local
    // SO_REUSEADDR socket from claiming the same address while this listener
    // owns it.
    // SAFETY: optval references one valid i32 for the exact option length.
    let option_result = unsafe {
        setsockopt(
            socket.0,
            SOL_SOCKET,
            SO_EXCLUSIVEADDRUSE,
            (&enabled as *const i32).cast(),
            std::mem::size_of::<i32>() as i32,
        )
    };
    if option_result != 0 {
        return Err(IngressError::SocketUnavailable);
    }

    let address = sockaddr_loopback(port);
    // SAFETY: sockaddr_loopback returns a correctly sized IPv4 sockaddr.
    if unsafe {
        bind(
            socket.0,
            (&address as *const SOCKADDR_IN).cast::<SOCKADDR>(),
            std::mem::size_of::<SOCKADDR_IN>() as i32,
        )
    } != 0
    {
        return Err(if port == 0 {
            IngressError::SocketUnavailable
        } else {
            IngressError::EndpointConflict
        });
    }
    // SAFETY: the socket is a successfully bound IPv4/TCP socket.
    if unsafe { listen(socket.0, LISTEN_BACKLOG) } != 0 {
        return Err(IngressError::SocketUnavailable);
    }

    let raw = socket.0;
    std::mem::forget(socket);
    // SAFETY: raw is uniquely owned and converted into TcpListener exactly once.
    Ok(unsafe { TcpListener::from_raw_socket(raw as RawSocket) })
}

fn sockaddr_loopback(port: u16) -> windows_sys::Win32::Networking::WinSock::SOCKADDR_IN {
    use windows_sys::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};
    let mut address = SOCKADDR_IN::default();
    address.sin_family = AF_INET;
    address.sin_port = port.to_be();
    address.sin_addr.S_un.S_addr = u32::from_ne_bytes(Ipv4Addr::LOCALHOST.octets());
    address
}

fn validate_runtime_root(root: &Path) -> Result<PathBuf, IngressError> {
    if !root.is_absolute() {
        return Err(IngressError::InvalidRoot);
    }
    reject_reparse_components(root).map_err(|_| IngressError::InvalidRoot)?;
    let metadata = fs::symlink_metadata(root).map_err(|_| IngressError::InvalidRoot)?;
    if !metadata.is_dir() || is_reparse_point(&metadata) {
        return Err(IngressError::InvalidRoot);
    }
    let canonical = fs::canonicalize(root).map_err(|_| IngressError::InvalidRoot)?;
    reject_reparse_components(&canonical).map_err(|_| IngressError::InvalidRoot)?;
    let canonical_metadata =
        fs::symlink_metadata(&canonical).map_err(|_| IngressError::InvalidRoot)?;
    if !canonical_metadata.is_dir() || is_reparse_point(&canonical_metadata) {
        return Err(IngressError::InvalidRoot);
    }
    Ok(canonical)
}

fn reject_reparse_components(path: &Path) -> Result<(), IngressError> {
    for component in path.ancestors() {
        let metadata =
            fs::symlink_metadata(component).map_err(|_| IngressError::StateUnavailable)?;
        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(IngressError::InvalidRoot);
        }
    }
    Ok(())
}

fn safe_regular_file(root: &Path, path: &Path) -> Result<bool, IngressError> {
    validate_runtime_root(root)?;
    if path.parent() != Some(root) || !path.starts_with(root) {
        return Err(IngressError::InvalidRoot);
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || is_reparse_point(&metadata)
            {
                Err(IngressError::InvalidEndpointState)
            } else {
                Ok(true)
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(IngressError::StateUnavailable),
    }
}

fn read_endpoint(root: &Path, path: &Path) -> Result<EndpointRecord, IngressError> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;

    if !safe_regular_file(root, path)? {
        return Err(IngressError::InvalidEndpointState);
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| IngressError::StateUnavailable)?;
    let metadata = file
        .metadata()
        .map_err(|_| IngressError::StateUnavailable)?;
    if !metadata.is_file() || is_reparse_point(&metadata) || metadata.len() > MAX_ENDPOINT_BYTES {
        return Err(IngressError::InvalidEndpointState);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_ENDPOINT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| IngressError::StateUnavailable)?;
    if bytes.len() as u64 > MAX_ENDPOINT_BYTES {
        return Err(IngressError::InvalidEndpointState);
    }
    let record: EndpointRecord =
        serde_json::from_slice(&bytes).map_err(|_| IngressError::InvalidEndpointState)?;
    validate_endpoint_record(&record)?;
    Ok(record)
}

fn validate_endpoint_record(record: &EndpointRecord) -> Result<(), IngressError> {
    let parsed =
        Uuid::parse_str(&record.endpoint_id).map_err(|_| IngressError::InvalidEndpointState)?;
    if record.schema_version != ENDPOINT_SCHEMA_VERSION
        || record.bind_address != Ipv4Addr::LOCALHOST.to_string()
        || record.port == 0
        || record.revision != 1
        || parsed.to_string() != record.endpoint_id
    {
        return Err(IngressError::InvalidEndpointState);
    }
    Ok(())
}

fn publish_endpoint(
    root: &Path,
    endpoint_path: &Path,
    record: &EndpointRecord,
) -> Result<(), IngressError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};

    validate_runtime_root(root)?;
    if endpoint_path.parent() != Some(root) || !endpoint_path.starts_with(root) {
        return Err(IngressError::InvalidRoot);
    }
    let temp_path = root.join(format!("connection-endpoint.{}.tmp", Uuid::new_v4()));
    let bytes = serde_json::to_vec(record).map_err(|_| IngressError::StateUnavailable)?;
    if bytes.len() as u64 > MAX_ENDPOINT_BYTES {
        return Err(IngressError::InvalidEndpointState);
    }

    let mut temp_owned = false;
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|_| IngressError::StateUnavailable)?;
        temp_owned = true;
        file.write_all(&bytes)
            .map_err(|_| IngressError::StateUnavailable)?;
        file.flush().map_err(|_| IngressError::StateUnavailable)?;
        file.sync_all()
            .map_err(|_| IngressError::StateUnavailable)?;
        drop(file);

        validate_runtime_root(root)?;
        if !safe_regular_file(root, &temp_path)? {
            return Err(IngressError::StateUnavailable);
        }
        let source: Vec<u16> = temp_path.as_os_str().encode_wide().chain(Some(0)).collect();
        let target: Vec<u16> = endpoint_path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        // Omitting MOVEFILE_REPLACE_EXISTING is the no-clobber commit. Both
        // paths are in the same trusted runtime directory.
        // SAFETY: both UTF-16 buffers are nul terminated and valid for this call.
        let moved =
            unsafe { MoveFileExW(source.as_ptr(), target.as_ptr(), MOVEFILE_WRITE_THROUGH) };
        if moved == 0 {
            if fs::symlink_metadata(endpoint_path).is_ok() {
                return Err(IngressError::EndpointConflict);
            }
            return Err(IngressError::StateUnavailable);
        }
        temp_owned = false;
        Ok(())
    })();

    if result.is_err() && temp_owned {
        cleanup_owned_temp(root, &temp_path);
    }
    result
}

fn cleanup_owned_temp(root: &Path, temp_path: &Path) {
    if validate_runtime_root(root).ok().as_deref() != Some(root) {
        return;
    }
    let Ok(metadata) = fs::symlink_metadata(temp_path) else {
        return;
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return;
    }
    if fs::canonicalize(temp_path)
        .map(|resolved| resolved.starts_with(root))
        .unwrap_or(false)
    {
        let _ = fs::remove_file(temp_path);
    }
}

fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Verify the server-side owner of this already-connected engine socket before
/// any client payload is read or forwarded. The argument is the Rust-side
/// client socket connected to the engine, so its peer/local tuple is reversed
/// to match the engine's server-side TCP row.
pub fn verify_backend_peer(
    backend_connection: &TcpStream,
    expected_engine_pid: u32,
) -> Result<(), IngressError> {
    let local = match backend_connection
        .peer_addr()
        .map_err(|_| IngressError::PeerOwnerUnknown)?
    {
        SocketAddr::V4(address) => address,
        SocketAddr::V6(_) => return Err(IngressError::PeerOwnerUnknown),
    };
    let remote = match backend_connection
        .local_addr()
        .map_err(|_| IngressError::PeerOwnerUnknown)?
    {
        SocketAddr::V4(address) => address,
        SocketAddr::V6(_) => return Err(IngressError::PeerOwnerUnknown),
    };
    if local.ip() != &Ipv4Addr::LOCALHOST
        || remote.ip() != &Ipv4Addr::LOCALHOST
        || expected_engine_pid == 0
    {
        return Err(IngressError::PeerOwnerUnknown);
    }
    let owner = query_established_owner(local, remote)?;
    match owner {
        Some(owner) if owner == expected_engine_pid => Ok(()),
        Some(_) => Err(IngressError::PeerOwnerMismatch),
        None => Err(IngressError::PeerOwnerMissing),
    }
}

/// Verify that a specific child process has an established IPv4 loopback
/// connection to the application-owned stable listener. This is a transport
/// ownership check only; it does not inspect HTTP or request content.
pub(crate) fn verify_cli_connection_to_owned_ingress(
    port: u16,
    cli_pid: u32,
    app_pid: u32,
) -> Result<(), IngressError> {
    use std::mem::size_of;
    use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, MIB_TCP_STATE_ESTAB, MIB_TCP_STATE_LISTEN,
        TCP_TABLE_OWNER_PID_ALL,
    };

    if port == 0 || cli_pid == 0 || app_pid == 0 || cli_pid == app_pid {
        return Err(IngressError::PeerOwnerUnknown);
    }
    const MAX_TABLE_BYTES: usize = 4 * 1024 * 1024;
    let table = (|| {
        for _ in 0..3 {
            let mut size = 0u32;
            // SAFETY: null buffer with a size pointer is the documented query.
            let first = unsafe {
                GetExtendedTcpTable(
                    std::ptr::null_mut(),
                    &mut size,
                    0,
                    2,
                    TCP_TABLE_OWNER_PID_ALL,
                    0,
                )
            };
            if first != ERROR_INSUFFICIENT_BUFFER || size as usize > MAX_TABLE_BYTES {
                return Err(IngressError::PeerOwnerTableUnavailable);
            }
            let mut buffer = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
            // SAFETY: buffer is aligned and bounded by the preceding size query.
            let result = unsafe {
                GetExtendedTcpTable(
                    buffer.as_mut_ptr().cast(),
                    &mut size,
                    0,
                    2,
                    TCP_TABLE_OWNER_PID_ALL,
                    0,
                )
            };
            if result == ERROR_INSUFFICIENT_BUFFER {
                continue;
            }
            if result != NO_ERROR || size as usize > buffer.len() * size_of::<usize>() {
                return Err(IngressError::PeerOwnerTableUnavailable);
            }
            let bytes = buffer.as_ptr().cast::<u8>();
            let count = unsafe { bytes.cast::<u32>().read_unaligned() } as usize;
            let rows_bytes = count
                .checked_mul(size_of::<MIB_TCPROW_OWNER_PID>())
                .ok_or(IngressError::PeerOwnerTableUnavailable)?;
            if size_of::<u32>() + rows_bytes > size as usize {
                return Err(IngressError::PeerOwnerTableUnavailable);
            }
            let rows = unsafe {
                std::slice::from_raw_parts(
                    bytes.add(size_of::<u32>()).cast::<MIB_TCPROW_OWNER_PID>(),
                    count,
                )
            };
            return Ok(rows.to_vec());
        }
        Err(IngressError::PeerOwnerTableUnavailable)
    })()?;

    let loopback = u32::from_ne_bytes(Ipv4Addr::LOCALHOST.octets());
    let mut listener_count = 0usize;
    let mut client_connection_found = false;
    for row in table {
        let local_port = u16::from_be(row.dwLocalPort as u16);
        let remote_port = u16::from_be(row.dwRemotePort as u16);
        if row.dwState == MIB_TCP_STATE_LISTEN as u32 && local_port == port {
            listener_count += 1;
            if row.dwLocalAddr != loopback || row.dwOwningPid != app_pid {
                return Err(IngressError::PeerOwnerMismatch);
            }
        }
        if row.dwState == MIB_TCP_STATE_ESTAB as u32
            && row.dwOwningPid == cli_pid
            && row.dwLocalAddr == loopback
            && row.dwRemoteAddr == loopback
            && remote_port == port
        {
            client_connection_found = true;
        }
    }
    if listener_count != 1 {
        return Err(if listener_count == 0 {
            IngressError::PeerOwnerMissing
        } else {
            IngressError::PeerOwnerAmbiguous
        });
    }
    if !client_connection_found {
        return Err(IngressError::PeerOwnerMissing);
    }
    Ok(())
}

#[cfg(windows)]
fn query_established_owner(
    local: SocketAddrV4,
    remote: SocketAddrV4,
) -> Result<Option<u32>, IngressError> {
    use std::mem::size_of;
    use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCPROW_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
    };

    for _ in 0..3 {
        let mut size = 0u32;
        // SAFETY: null buffer with a size pointer is the documented size query.
        let first = unsafe {
            GetExtendedTcpTable(
                std::ptr::null_mut(),
                &mut size,
                0,
                2,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        if first != ERROR_INSUFFICIENT_BUFFER || size as usize > MAX_TCP_TABLE_BYTES {
            return Err(IngressError::PeerOwnerTableUnavailable);
        }
        let mut table = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        // SAFETY: table is aligned and sized from the bounded API result.
        let result = unsafe {
            GetExtendedTcpTable(
                table.as_mut_ptr().cast(),
                &mut size,
                0,
                2,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        if result == ERROR_INSUFFICIENT_BUFFER {
            continue;
        }
        if result != NO_ERROR || size as usize > table.len() * size_of::<usize>() {
            return Err(IngressError::PeerOwnerTableUnavailable);
        }
        let bytes = table.as_ptr().cast::<u8>();
        let count = unsafe { bytes.cast::<u32>().read_unaligned() } as usize;
        let rows_bytes = count
            .checked_mul(size_of::<MIB_TCPROW_OWNER_PID>())
            .ok_or(IngressError::PeerOwnerTableUnavailable)?;
        if size_of::<u32>() + rows_bytes > size as usize {
            return Err(IngressError::PeerOwnerTableUnavailable);
        }
        let rows = unsafe {
            std::slice::from_raw_parts(
                bytes.add(size_of::<u32>()).cast::<MIB_TCPROW_OWNER_PID>(),
                count,
            )
        };
        return exact_peer_owner(rows, local, remote);
    }
    Err(IngressError::PeerOwnerTableUnavailable)
}

#[cfg(not(windows))]
fn query_established_owner(
    _local: SocketAddrV4,
    _remote: SocketAddrV4,
) -> Result<Option<u32>, IngressError> {
    Err(IngressError::PeerOwnerUnknown)
}

#[cfg(windows)]
fn exact_peer_owner(
    rows: &[windows_sys::Win32::NetworkManagement::IpHelper::MIB_TCPROW_OWNER_PID],
    local: SocketAddrV4,
    remote: SocketAddrV4,
) -> Result<Option<u32>, IngressError> {
    use windows_sys::Win32::NetworkManagement::IpHelper::MIB_TCP_STATE_ESTAB;

    let local_ip = u32::from_ne_bytes(local.ip().octets());
    let remote_ip = u32::from_ne_bytes(remote.ip().octets());
    let mut owner = None;
    for row in rows.iter().filter(|row| {
        row.dwState == MIB_TCP_STATE_ESTAB as u32
            && row.dwLocalAddr == local_ip
            && u16::from_be(row.dwLocalPort as u16) == local.port()
            && row.dwRemoteAddr == remote_ip
            && u16::from_be(row.dwRemotePort as u16) == remote.port()
    }) {
        if owner.is_some() {
            return Err(IngressError::PeerOwnerAmbiguous);
        }
        owner = Some(row.dwOwningPid);
    }
    Ok(owner)
}

#[cfg(test)]
#[path = "ingress_tests.rs"]
mod tests;

#[cfg(test)]
mod endpoint_record_tests {
    use super::*;

    #[test]
    fn endpoint_record_rejects_duplicate_and_unknown_fields() {
        let id = Uuid::new_v4();
        let duplicate = format!(
            r#"{{"schema_version":1,"bind_address":"127.0.0.1","port":41000,"endpoint_id":"{id}","revision":1,"revision":1}}"#
        );
        let unknown = format!(
            r#"{{"schema_version":1,"bind_address":"127.0.0.1","port":41000,"endpoint_id":"{id}","revision":1,"pid":123}}"#
        );
        assert!(serde_json::from_str::<EndpointRecord>(&duplicate).is_err());
        assert!(serde_json::from_str::<EndpointRecord>(&unknown).is_err());
    }
}
