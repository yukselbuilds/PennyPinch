//! Authenticate a private Unix engine socket before sending HTTP.

use socket2::{Domain, SockAddr, Socket, Type};
use std::fs;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UnixBackendError {
    InvalidEndpoint,
    OwnershipLost,
    Transport,
}

#[derive(Clone, Copy)]
pub(crate) struct UnixTarget<'a> {
    pub socket_path: &'a Path,
    pub expected_pid: u32,
}

impl UnixTarget<'_> {
    pub(crate) fn connect(
        &self,
        is_alive: impl Fn() -> bool,
    ) -> Result<UnixStream, UnixBackendError> {
        self.connect_with_timeout(Duration::from_millis(300), is_alive)
    }

    pub(crate) fn connect_with_timeout(
        &self,
        timeout: Duration,
        is_alive: impl Fn() -> bool,
    ) -> Result<UnixStream, UnixBackendError> {
        if self.expected_pid == 0 || self.expected_pid > i32::MAX as u32 {
            return Err(UnixBackendError::InvalidEndpoint);
        }
        if !is_alive() {
            return Err(UnixBackendError::OwnershipLost);
        }
        let before = endpoint_identity(self.socket_path)?;
        let address =
            SockAddr::unix(self.socket_path).map_err(|_| UnixBackendError::InvalidEndpoint)?;
        let socket = Socket::new(Domain::UNIX, Type::STREAM, None)
            .map_err(|_| UnixBackendError::Transport)?;
        socket
            .connect_timeout(&address, timeout.min(Duration::from_millis(300)))
            .map_err(|_| UnixBackendError::Transport)?;
        if !peer_matches(&socket, self.expected_pid)
            || endpoint_identity(self.socket_path)? != before
            || !is_alive()
        {
            return Err(UnixBackendError::OwnershipLost);
        }
        Ok(UnixStream::from(OwnedFd::from(socket)))
    }
}

#[cfg(target_os = "macos")]
fn peer_matches(socket: &Socket, expected_pid: u32) -> bool {
    let mut pid: libc::pid_t = 0;
    let mut length = std::mem::size_of_val(&pid) as libc::socklen_t;
    // SAFETY: LOCAL_PEERPID writes one pid_t to the sized, live buffer.
    let status = unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            libc::SOL_LOCAL,
            libc::LOCAL_PEERPID,
            (&mut pid as *mut libc::pid_t).cast(),
            &mut length,
        )
    };
    status == 0 && length as usize == std::mem::size_of_val(&pid) && pid as u32 == expected_pid
}

#[cfg(target_os = "linux")]
fn peer_matches(socket: &Socket, expected_pid: u32) -> bool {
    let mut peer = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut length = std::mem::size_of_val(&peer) as libc::socklen_t;
    // SAFETY: SO_PEERCRED writes one ucred to the sized, live buffer.
    let status = unsafe {
        libc::getsockopt(
            socket.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut peer as *mut libc::ucred).cast(),
            &mut length,
        )
    };
    status == 0
        && length as usize == std::mem::size_of_val(&peer)
        && peer.pid as u32 == expected_pid
        && peer.uid == unsafe { libc::geteuid() }
}

fn endpoint_identity(path: &Path) -> Result<(u64, u64, u64, u64), UnixBackendError> {
    if !path.is_absolute() || path.as_os_str().as_bytes().len() >= 104 {
        return Err(UnixBackendError::InvalidEndpoint);
    }
    let parent = path.parent().ok_or(UnixBackendError::InvalidEndpoint)?;
    let directory = fs::symlink_metadata(parent).map_err(|_| UnixBackendError::InvalidEndpoint)?;
    // SAFETY: geteuid has no arguments or memory preconditions.
    let uid = unsafe { libc::geteuid() };
    if !directory.is_dir()
        || directory.uid() != uid
        || directory.mode() & 0o077 != 0
        || fs::canonicalize(parent).ok().as_deref() != Some(parent)
    {
        return Err(UnixBackendError::InvalidEndpoint);
    }
    let endpoint = fs::symlink_metadata(path).map_err(|_| UnixBackendError::Transport)?;
    if !endpoint.file_type().is_socket() || endpoint.uid() != uid {
        return Err(UnixBackendError::InvalidEndpoint);
    }
    Ok((
        directory.dev(),
        directory.ino(),
        endpoint.dev(),
        endpoint.ino(),
    ))
}

#[cfg(test)]
#[path = "unix_backend_tests.rs"]
mod tests;
