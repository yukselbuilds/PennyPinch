//! Own the saved loopback endpoint and its private, exclusively locked metadata.

use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use uuid::Uuid;

const ENDPOINT: &str = "connection-endpoint.json";
const LOCK: &str = ".ingress.lock";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IngressError {
    InvalidRoot,
    StateUnavailable,
    InvalidEndpointState,
    EndpointConflict,
    SocketUnavailable,
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

#[derive(Clone, Copy, Eq, PartialEq)]
struct FileStamp(u64, u64, u64, i64, i64, i64, i64);

fn stamp(info: &fs::Metadata) -> FileStamp {
    FileStamp(
        info.dev(),
        info.ino(),
        info.len(),
        info.mtime(),
        info.mtime_nsec(),
        info.ctime(),
        info.ctime_nsec(),
    )
}

struct PrivateRoot {
    path: PathBuf,
    directory: File,
    identity: (u64, u64),
}

impl PrivateRoot {
    fn open(path: &Path) -> Result<Self, IngressError> {
        if !path.is_absolute() || fs::canonicalize(path).ok().as_deref() != Some(path) {
            return Err(IngressError::InvalidRoot);
        }
        let info = fs::symlink_metadata(path).map_err(|_| IngressError::InvalidRoot)?;
        if !info.is_dir()
            || info.uid() != unsafe { libc::geteuid() }
            || info.mode() & 0o7777 != 0o700
        {
            return Err(IngressError::InvalidRoot);
        }
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)
            .map_err(|_| IngressError::InvalidRoot)?;
        let opened = directory
            .metadata()
            .map_err(|_| IngressError::InvalidRoot)?;
        let root = Self {
            path: path.to_owned(),
            directory,
            identity: (info.dev(), info.ino()),
        };
        if (opened.dev(), opened.ino()) != root.identity || !root.is_current() {
            return Err(IngressError::InvalidRoot);
        }
        Ok(root)
    }

    fn is_current(&self) -> bool {
        fs::symlink_metadata(&self.path).is_ok_and(|info| {
            info.is_dir()
                && (info.dev(), info.ino()) == self.identity
                && info.uid() == unsafe { libc::geteuid() }
                && info.mode() & 0o7777 == 0o700
        }) && fs::canonicalize(&self.path).ok().as_deref() == Some(self.path.as_path())
    }

    fn file(&self, name: &str, flags: i32) -> io::Result<File> {
        let name = CString::new(name).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        // All names are internal single components; the held directory cannot be redirected.
        let fd = unsafe {
            libc::openat(
                self.directory.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

fn private_file(info: &fs::Metadata) -> bool {
    info.is_file()
        && info.uid() == unsafe { libc::geteuid() }
        && info.nlink() == 1
        && info.mode() & 0o7777 == 0o600
}

fn read_endpoint(root: &PrivateRoot) -> Result<Option<(EndpointRecord, FileStamp)>, IngressError> {
    let mut file = match root.file(ENDPOINT, libc::O_RDONLY | libc::O_NONBLOCK) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(IngressError::InvalidEndpointState),
    };
    let before = file
        .metadata()
        .map_err(|_| IngressError::InvalidEndpointState)?;
    if !private_file(&before) || before.len() > 1024 {
        return Err(IngressError::InvalidEndpointState);
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(1025)
        .read_to_end(&mut bytes)
        .map_err(|_| IngressError::InvalidEndpointState)?;
    let after = file
        .metadata()
        .map_err(|_| IngressError::InvalidEndpointState)?;
    if bytes.len() > 1024 || stamp(&before) != stamp(&after) || !root.is_current() {
        return Err(IngressError::InvalidEndpointState);
    }
    let named = root
        .file(ENDPOINT, libc::O_RDONLY | libc::O_NONBLOCK)
        .and_then(|value| value.metadata())
        .map_err(|_| IngressError::InvalidEndpointState)?;
    if stamp(&named) != stamp(&before) || !private_file(&named) {
        return Err(IngressError::InvalidEndpointState);
    }
    let record: EndpointRecord =
        serde_json::from_slice(&bytes).map_err(|_| IngressError::InvalidEndpointState)?;
    let valid_id = Uuid::parse_str(&record.endpoint_id)
        .ok()
        .is_some_and(|id| !id.is_nil() && id.to_string() == record.endpoint_id);
    if record.schema_version != 1
        || record.bind_address != "127.0.0.1"
        || record.port == 0
        || record.revision != 1
        || !valid_id
    {
        return Err(IngressError::InvalidEndpointState);
    }
    Ok(Some((record, stamp(&after))))
}

fn publish(root: &PrivateRoot, record: &EndpointRecord) -> Result<(), IngressError> {
    let name = format!(".endpoint-{}.tmp", Uuid::new_v4().simple());
    let mut file = root
        .file(&name, libc::O_RDWR | libc::O_CREAT | libc::O_EXCL)
        .map_err(|_| IngressError::StateUnavailable)?;
    let bytes = serde_json::to_vec(record).map_err(|_| IngressError::StateUnavailable)?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| IngressError::StateUnavailable)?;
    let before = file
        .metadata()
        .map_err(|_| IngressError::StateUnavailable)?;
    let named = root
        .file(&name, libc::O_RDONLY)
        .and_then(|value| value.metadata())
        .map_err(|_| IngressError::StateUnavailable)?;
    if !private_file(&before) || stamp(&before) != stamp(&named) || !root.is_current() {
        return Err(IngressError::StateUnavailable);
    }
    let name = CString::new(name).map_err(|_| IngressError::StateUnavailable)?;
    let destination = CString::new(ENDPOINT).unwrap();
    // Exclusive rename publishes one complete record without replacing any concurrent file.
    if unsafe {
        libc::renameatx_np(
            root.directory.as_raw_fd(),
            name.as_ptr(),
            root.directory.as_raw_fd(),
            destination.as_ptr(),
            libc::RENAME_EXCL,
        )
    } != 0
    {
        return Err(
            if io::Error::last_os_error().kind() == io::ErrorKind::AlreadyExists {
                IngressError::EndpointConflict
            } else {
                IngressError::StateUnavailable
            },
        );
    }
    root.directory
        .sync_all()
        .map_err(|_| IngressError::StateUnavailable)?;
    let actual = read_endpoint(root)?.ok_or(IngressError::InvalidEndpointState)?;
    if actual.0 != *record {
        return Err(IngressError::InvalidEndpointState);
    }
    Ok(())
}

pub struct StableIngress {
    listener: TcpListener,
    root: PrivateRoot,
    lock: File,
    lock_identity: (u64, u64),
    record: EndpointRecord,
    record_stamp: FileStamp,
}

impl StableIngress {
    pub fn open_or_create(path: &Path) -> Result<Self, IngressError> {
        let root = PrivateRoot::open(path)?;
        let lock = root
            .file(LOCK, libc::O_RDWR | libc::O_CREAT | libc::O_NONBLOCK)
            .map_err(|_| IngressError::StateUnavailable)?;
        let info = lock
            .metadata()
            .map_err(|_| IngressError::StateUnavailable)?;
        if !private_file(&info) {
            return Err(IngressError::InvalidEndpointState);
        }
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(
                if io::Error::last_os_error().kind() == io::ErrorKind::WouldBlock {
                    IngressError::EndpointConflict
                } else {
                    IngressError::StateUnavailable
                },
            );
        }
        let previous = read_endpoint(&root)?;
        let port = previous.as_ref().map_or(0, |value| value.0.port);
        if port != 0 && super::macos_ports::observe_port(port)?.wildcard {
            return Err(IngressError::EndpointConflict);
        }
        let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))
            .map_err(|_| IngressError::SocketUnavailable)?;
        // Reuse a drained saved port after TIME_WAIT; SO_REUSEPORT remains disabled.
        socket
            .set_reuse_address(true)
            .map_err(|_| IngressError::SocketUnavailable)?;
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        socket.bind(&address.into()).map_err(|error| {
            if error.kind() == io::ErrorKind::AddrInUse {
                IngressError::EndpointConflict
            } else {
                IngressError::SocketUnavailable
            }
        })?;
        socket
            .listen(128)
            .map_err(|_| IngressError::SocketUnavailable)?;
        let listener = TcpListener::from(socket);
        let bound_port = listener
            .local_addr()
            .map_err(|_| IngressError::SocketUnavailable)?
            .port();
        let listeners = super::macos_ports::observe_port(bound_port)?;
        if listeners.wildcard {
            return Err(IngressError::EndpointConflict);
        }
        if !listeners.loopback {
            return Err(IngressError::StateUnavailable);
        }
        let record = match previous {
            Some((record, _)) => record,
            None => {
                let record = EndpointRecord {
                    schema_version: 1,
                    bind_address: "127.0.0.1".to_owned(),
                    port: listener
                        .local_addr()
                        .map_err(|_| IngressError::SocketUnavailable)?
                        .port(),
                    endpoint_id: Uuid::new_v4().to_string(),
                    revision: 1,
                };
                publish(&root, &record)?;
                record
            }
        };
        let observed = read_endpoint(&root)?.ok_or(IngressError::InvalidEndpointState)?;
        if observed.0 != record {
            return Err(IngressError::InvalidEndpointState);
        }
        let result = Self {
            listener,
            root,
            lock,
            lock_identity: (info.dev(), info.ino()),
            record,
            record_stamp: observed.1,
        };
        if !result.is_owned() {
            return Err(IngressError::InvalidEndpointState);
        }
        Ok(result)
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

    pub fn is_owned(&self) -> bool {
        if !self.root.is_current() {
            return false;
        }
        let lock = self
            .root
            .file(LOCK, libc::O_RDONLY | libc::O_NONBLOCK)
            .and_then(|value| value.metadata());
        let held = self.lock.metadata();
        if !matches!((lock, held), (Ok(named), Ok(held)) if private_file(&named)
            && private_file(&held) && (named.dev(), named.ino()) == self.lock_identity
            && (held.dev(), held.ino()) == self.lock_identity)
        {
            return false;
        }
        self.listener.local_addr().is_ok_and(|address| {
            address == SocketAddr::from((Ipv4Addr::LOCALHOST, self.record.port))
        }) && matches!(read_endpoint(&self.root), Ok(Some((record, stamp))) if record == self.record && stamp == self.record_stamp)
    }
}

#[cfg(test)]
#[path = "ingress_macos_tests.rs"]
mod tests;
