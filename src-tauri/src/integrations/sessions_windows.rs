use super::protocol;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use socket2::{Domain, SockAddr, Socket, Type};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::os::windows::{
    ffi::{OsStrExt, OsStringExt},
    io::AsRawSocket,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tungstenite::{client::client_with_config, protocol::WebSocketConfig, Message, WebSocket};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_TIMEOUT};
use windows_sys::Win32::Networking::WinSock::{WSAIoctl, SIO_AF_UNIX_GETPEERPID, SOCKET_ERROR};
use windows_sys::Win32::Security::{
    EqualSid, GetTokenInformation, IsValidSid, TokenElevation, TokenUser, TOKEN_ELEVATION,
    TOKEN_QUERY, TOKEN_USER,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
    WaitForSingleObject, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

const MAX_MESSAGE: usize = 1024 * 1024;
const BUDGET: Duration = Duration::from_secs(8);

struct Handle(HANDLE);
unsafe impl Send for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

struct Peer {
    pid: u32,
    process: Handle,
    executable: PathBuf,
}
impl Peer {
    fn alive(&self, pid: u32) -> bool {
        self.pid == pid && unsafe { WaitForSingleObject(self.process.0, 0) } == WAIT_TIMEOUT
    }
}

#[derive(Default)]
pub(super) struct Reader {
    peer: Option<Peer>,
    observed: Option<(PathBuf, u32)>,
}
fn profile_home() -> Result<PathBuf, &'static str> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join(".codex")))
        .ok_or("scope_unavailable")
}

// Use a neutral existing volume root. Never create a workspace or inherit a
// repository's instructions/config just to display the official task picker.
fn neutral_root(home: &Path) -> Result<PathBuf, &'static str> {
    let root = home
        .ancestors()
        .last()
        .filter(|p| p.has_root())
        .ok_or("unsafe_scope")?;
    super::super::config_io::pin_existing_directory_chain(root).map_err(|_| "unsafe_scope")?;
    for path in [
        root.join(".codex"),
        root.join("AGENTS.md"),
        root.join("AGENTS.override.md"),
    ] {
        match std::fs::symlink_metadata(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            _ => return Err("unsafe_scope"),
        }
    }
    Ok(root.into())
}

impl Reader {
    pub fn read(&mut self) -> Result<protocol::Snapshot, &'static str> {
        self.observed = None;
        let home = profile_home()?;
        self.read_home(&home)
    }
    fn read_home(&mut self, home: &Path) -> Result<protocol::Snapshot, &'static str> {
        self.observed = None;
        if !home.is_absolute() {
            return Err("unsafe_scope");
        }
        let directory = home.join("app-server-control");
        let pins = super::super::config_io::pin_existing_directory_chain(&directory)
            .map_err(|_| "endpoint_unavailable")?;
        let scope = std::fs::canonicalize(&home).map_err(|_| "unsafe_scope")?;
        let io = DeadlineIo::connect(&directory.join("app-server-control.sock"))?;
        let pid = peer_pid(&io.socket)?;
        if !self.peer.as_ref().is_some_and(|p| p.alive(pid)) {
            self.peer = None;
            self.peer = Some(verify_peer(pid)?);
        }
        if pins.validate().is_err() {
            return Err("unsafe_scope");
        }
        // The connected kernel peer is pinned by the process handle; no directory or
        // executable handle is kept between refreshes, so updates remain possible.
        drop(pins);
        io.remaining().map_err(|_| "timeout")?;
        let namespace = {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(scope.to_string_lossy().as_bytes()))
        };
        let value = read_metadata(io, &namespace)?;
        self.observed = Some((scope, pid));
        Ok(value)
    }
    pub fn can_open_center(&self) -> bool {
        self.observed
            .as_ref()
            .is_some_and(|(_, pid)| self.peer.as_ref().is_some_and(|p| p.alive(*pid)))
            && profile_home().is_ok_and(|home| neutral_root(&home).is_ok())
    }
    pub fn open_center(&mut self) -> Result<OpenedCenter, &'static str> {
        let (scope, expected_pid) = self.observed.take().ok_or("stale_snapshot")?;
        let home = profile_home().map_err(|_| "unsafe_scope")?;
        let root = neutral_root(&home)?;
        let pins =
            super::super::config_io::pin_existing_directory_chain(&home.join("app-server-control"))
                .map_err(|_| "unsafe_scope")?;
        if std::fs::canonicalize(&home).map_err(|_| "unsafe_scope")? != scope {
            return Err("stale_snapshot");
        }
        let io = DeadlineIo::connect(&home.join("app-server-control/app-server-control.sock"))
            .map_err(|_| "stale_snapshot")?;
        let pid = peer_pid(&io.socket)?;
        let peer = self
            .peer
            .as_ref()
            .filter(|p| p.alive(pid) && pid == expected_pid)
            .ok_or("stale_snapshot")?;
        let parents = super::super::config_io::pin_existing_directory_chain(
            peer.executable.parent().ok_or("untrusted_endpoint")?,
        )
        .map_err(|_| "untrusted_endpoint")?;
        let _file = super::super::discovery::pin_verified_client_cli(
            "codex",
            &peer.executable,
            protocol::VERSION,
        )
        .ok_or("untrusted_endpoint")?;
        if pins.validate().is_err()
            || parents.validate().is_err()
            || neutral_root(&home)? != root
            || !peer.alive(pid)
        {
            return Err("unsafe_scope");
        }
        // Fixed arguments: no prompt, thread id, auth, model, provider or reasoning.
        OpenedCenter::launch(&peer.executable, &root)
    }
}

pub(super) struct OpenedCenter {
    _process: Handle,
}
impl OpenedCenter {
    fn launch(executable: &Path, cwd: &Path) -> Result<Self, &'static str> {
        let executable = executable.to_owned();
        let cwd = cwd.to_owned();
        std::thread::spawn(move || unsafe {
            use windows::Win32::System::Com::{
                CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
            };
            use windows_sys::Win32::UI::Shell::{
                ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS,
                SHELLEXECUTEINFOW,
            };
            if CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_err() {
                return Err("launch_failed");
            }
            let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(Some(0)).collect::<Vec<u16>>();
            let file = wide(executable.as_os_str());
            let directory = wide(cwd.as_os_str());
            let parameters = wide(std::ffi::OsStr::new("agents --remote unix://"));
            let mut info = SHELLEXECUTEINFOW {
                cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
                lpFile: file.as_ptr(),
                lpParameters: parameters.as_ptr(),
                lpDirectory: directory.as_ptr(),
                nShow: 1,
                ..Default::default()
            };
            let ok = ShellExecuteExW(&mut info) != 0;
            CoUninitialize();
            if info.hProcess.is_null() {
                return Err("launch_failed");
            }
            let opened = Self {
                _process: Handle(info.hProcess),
            };
            if ok {
                Ok(opened)
            } else {
                Err("launch_failed")
            }
        })
        .join()
        .map_err(|_| "launch_failed")?
    }
}

struct DeadlineIo {
    socket: Socket,
    deadline: Instant,
}
impl DeadlineIo {
    fn connect(path: &Path) -> Result<Self, &'static str> {
        let address = SockAddr::unix(path).map_err(|_| "unsafe_scope")?;
        let socket =
            Socket::new(Domain::UNIX, Type::STREAM, None).map_err(|_| "endpoint_unavailable")?;
        socket
            .connect_timeout(&address, Duration::from_secs(2))
            .map_err(|_| "endpoint_unavailable")?;
        Ok(Self {
            socket,
            deadline: Instant::now() + BUDGET,
        })
    }
    fn remaining(&self) -> io::Result<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|v| !v.is_zero())
            .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))
    }
}
impl Read for DeadlineIo {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.socket.set_read_timeout(Some(self.remaining()?))?;
        self.socket.read(bytes)
    }
}
impl Write for DeadlineIo {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.socket.set_write_timeout(Some(self.remaining()?))?;
        self.socket.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.socket.flush()
    }
}

fn peer_pid(socket: &Socket) -> Result<u32, &'static str> {
    let mut pid = 0u32;
    let mut returned = 0;
    let result = unsafe {
        WSAIoctl(
            socket.as_raw_socket() as _,
            SIO_AF_UNIX_GETPEERPID,
            std::ptr::null(),
            0,
            (&mut pid as *mut u32).cast(),
            4,
            &mut returned,
            std::ptr::null_mut(),
            None,
        )
    };
    if result == SOCKET_ERROR || pid == 0 {
        Err("untrusted_endpoint")
    } else {
        Ok(pid)
    }
}

fn token(process: HANDLE) -> Result<Handle, &'static str> {
    let mut raw = std::ptr::null_mut();
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut raw) } == 0 {
        Err("untrusted_endpoint")
    } else {
        Ok(Handle(raw))
    }
}
fn user(token: &Handle) -> Result<Vec<usize>, &'static str> {
    let mut size = 0;
    unsafe {
        GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut size);
    }
    if size == 0 || size > 65536 {
        return Err("untrusted_endpoint");
    }
    let mut data = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            data.as_mut_ptr().cast(),
            size,
            &mut size,
        )
    } == 0
    {
        return Err("untrusted_endpoint");
    }
    let sid = unsafe { &*(data.as_ptr().cast::<TOKEN_USER>()) }.User.Sid;
    if unsafe { IsValidSid(sid) } == 0 {
        return Err("untrusted_endpoint");
    }
    Ok(data)
}
fn not_elevated(token: &Handle) -> bool {
    let mut elevation = TOKEN_ELEVATION::default();
    let mut size = 0;
    (unsafe {
        GetTokenInformation(
            token.0,
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            std::mem::size_of_val(&elevation) as u32,
            &mut size,
        )
    }) != 0
        && elevation.TokenIsElevated == 0
}
fn verify_peer(pid: u32) -> Result<Peer, &'static str> {
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | 0x0010_0000, 0, pid) };
    if raw.is_null() {
        return Err("untrusted_endpoint");
    }
    let process = Handle(raw);
    let me = token(unsafe { GetCurrentProcess() })?;
    let other = token(process.0)?;
    let a = user(&me)?;
    let b = user(&other)?;
    let same = unsafe {
        EqualSid(
            (*(a.as_ptr().cast::<TOKEN_USER>())).User.Sid,
            (*(b.as_ptr().cast::<TOKEN_USER>())).User.Sid,
        )
    } != 0;
    if !same || !not_elevated(&me) || !not_elevated(&other) {
        return Err("untrusted_endpoint");
    }
    let mut image = vec![0u16; 32768];
    let mut length = image.len() as u32;
    if unsafe {
        QueryFullProcessImageNameW(
            process.0,
            PROCESS_NAME_WIN32,
            image.as_mut_ptr(),
            &mut length,
        )
    } == 0
        || length == 0
        || length as usize >= image.len()
    {
        return Err("untrusted_endpoint");
    }
    let path = PathBuf::from(OsString::from_wide(&image[..length as usize]));
    let parents = super::super::config_io::pin_existing_directory_chain(
        path.parent().ok_or("untrusted_endpoint")?,
    )
    .map_err(|_| "untrusted_endpoint")?;
    let version =
        super::super::discovery::verified_codex_cli_version(&path).ok_or("untrusted_endpoint")?;
    if version != protocol::VERSION {
        return Err("unsupported_version");
    }
    let pin = super::super::discovery::pin_verified_client_cli("codex", &path, &version)
        .ok_or("untrusted_endpoint")?;
    if parents.validate().is_err() || unsafe { WaitForSingleObject(process.0, 0) } != WAIT_TIMEOUT {
        return Err("untrusted_endpoint");
    }
    drop(pin);
    drop(parents);
    Ok(Peer {
        pid,
        process,
        executable: path,
    })
}

#[derive(Deserialize)]
struct Header {
    id: Option<u64>,
    method: Option<String>,
}
#[derive(Deserialize)]
struct Reply<T> {
    id: u64,
    result: Option<T>,
    error: Option<serde::de::IgnoredAny>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Initialize {
    user_agent: String,
}
#[derive(Deserialize)]
struct Closed {
    params: ClosedParams,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClosedParams {
    thread_id: String,
}

fn reply<T: DeserializeOwned>(
    socket: &mut WebSocket<DeadlineIo>,
    expected: u64,
    closed: &mut BTreeSet<String>,
) -> Result<T, &'static str> {
    for _ in 0..32 {
        match socket.read().map_err(|_| "read_failed")? {
            Message::Text(text) => {
                let header: Header = serde_json::from_str(&text).map_err(|_| "invalid_response")?;
                if header.id.is_some() {
                    if header.id != Some(expected) || header.method.is_some() {
                        return Err("invalid_response");
                    }
                    let value: Reply<T> =
                        serde_json::from_str(&text).map_err(|_| "invalid_response")?;
                    if value.id != expected || value.error.is_some() {
                        return Err("read_failed");
                    }
                    return value.result.ok_or("invalid_response");
                }
                match header.method.as_deref() {
                    Some("thread/closed") => {
                        let event: Closed =
                            serde_json::from_str(&text).map_err(|_| "invalid_response")?;
                        if closed.len() >= protocol::MAX_ROWS {
                            return Err("invalid_response");
                        }
                        closed.insert(event.params.thread_id);
                    }
                    Some(_) => {}
                    None => return Err("invalid_response"),
                }
            }
            Message::Ping(_) | Message::Pong(_) => {}
            _ => return Err("read_failed"),
        }
    }
    Err("invalid_response")
}

fn read_metadata(io: DeadlineIo, scope: &str) -> Result<protocol::Snapshot, &'static str> {
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE))
        .max_frame_size(Some(MAX_MESSAGE))
        .read_buffer_size(4096)
        .write_buffer_size(0)
        .max_write_buffer_size(65536);
    let (mut socket, _) =
        client_with_config("ws://localhost/", io, Some(config)).map_err(|_| "read_failed")?;
    let mut closed = BTreeSet::new();
    let requests = protocol::requests();
    let send = |index: usize, socket: &mut WebSocket<DeadlineIo>| {
        socket
            .send(Message::Text(requests[index].to_string().into()))
            .map_err(|_| "read_failed")
    };
    send(0, &mut socket)?;
    let initialized: Initialize = reply(&mut socket, 1, &mut closed)?;
    if initialized.user_agent.len() > 4096 {
        return Err("invalid_response");
    }
    send(1, &mut socket)?;
    send(2, &mut socket)?;
    let mut threads: protocol::ThreadPage = reply(&mut socket, 2, &mut closed)?;
    send(3, &mut socket)?;
    let mut loaded: protocol::LoadedPage = reply(&mut socket, 3, &mut closed)?;
    // Empty, loaded chats need not have a persisted DB row yet. Read only their
    // metadata; includeTurns=true would persist/load history in this version.
    let missing = protocol::missing_loaded(&threads, &loaded)?;
    for (offset, id) in missing.iter().enumerate() {
        let request_id = 4 + offset as u64;
        let request = protocol::metadata_request(request_id, id)?;
        socket
            .send(Message::Text(request.to_string().into()))
            .map_err(|_| "read_failed")?;
        let value = reply::<protocol::ThreadRead>(&mut socket, request_id, &mut closed)?;
        protocol::append_read(&mut threads, value, id)?;
    }
    if !missing.is_empty() {
        let request_id = 4 + missing.len() as u64;
        socket
            .send(Message::Text(
                serde_json::json!({"id":request_id,"method":"thread/loaded/list",
            "params":{"limit":protocol::PAGE_SIZE}})
                .to_string()
                .into(),
            ))
            .map_err(|_| "read_failed")?;
        loaded = reply(&mut socket, request_id, &mut closed)?;
    }
    let snapshot = protocol::normalize(scope, threads, loaded, &closed)?;
    let _ = socket.close(None);
    Ok(snapshot)
}

#[cfg(test)]
#[path = "sessions_socket_tests.rs"]
mod tests;
