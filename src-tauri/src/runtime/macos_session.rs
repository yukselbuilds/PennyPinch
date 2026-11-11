//! Own one macOS engine process, its parent-pipe lifetime and inherited state lock.

use super::control::{self, ControlError, ControlOperation, OptimizationState};
use super::ingress_relay::BackendTarget;
use super::macos_layout::{MacRuntime, MacRuntimeError};
use super::manager::EXPECTED_ENGINE_VERSION;
use super::metrics_read::{self, OwnedMetricsSnapshot, RuntimeMetricsError};
use super::unix_backend::UnixTarget;
use serde_json::{json, Value};
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

struct OwnedEngine {
    child: Mutex<Child>,
    route_revoked: AtomicBool,
}

impl OwnedEngine {
    fn is_alive(&self) -> bool {
        self.child
            .lock()
            .ok()
            .and_then(|mut child| child.try_wait().ok())
            .is_some_and(|status| status.is_none())
    }

    fn can_route(&self) -> bool {
        !self.route_revoked.load(Ordering::Acquire) && self.is_alive()
    }
}

pub(crate) struct MacEngineSession {
    engine: Arc<OwnedEngine>,
    stdin: Option<ChildStdin>,
    lock: Option<File>,
    endpoint_root: PathBuf,
    endpoint: PathBuf,
    endpoint_root_identity: (u64, u64),
    socket_identity: Option<(u64, u64)>,
    generation: String,
    secret: String,
    pid: u32,
}

impl MacEngineSession {
    pub(crate) fn start(
        runtime_root: &Path,
        state: &Path,
        protected_tools: &[String],
        shutting_down: &AtomicBool,
    ) -> Result<(Self, OptimizationState), MacRuntimeError> {
        let deadline = Instant::now() + Duration::from_secs(85);
        check_startup(deadline, shutting_down)?;
        if protected_tools.len() > 32
            || protected_tools.windows(2).any(|pair| pair[0] >= pair[1])
            || protected_tools.iter().any(|name| {
                name.is_empty()
                    || name.len() > 96
                    || !name
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"_-.:/".contains(&c))
            })
        {
            return Err(MacRuntimeError::InvalidProfile);
        }
        private_directory(state)?;
        let lock = state_lock(state)?;
        let runtime = MacRuntime::open(runtime_root)?;
        for name in ["home", "temp", "workspace", "config", "cache"] {
            let path = state.join(name);
            if !path.exists() {
                DirBuilder::new()
                    .mode(0o700)
                    .create(&path)
                    .map_err(|_| MacRuntimeError::UnsafeState)?;
            }
            private_directory(&path)?;
        }
        let generation = Uuid::new_v4().simple().to_string();
        let secret = Uuid::new_v4().simple().to_string();
        let frame = serde_json::to_vec(&json!({
            "schema_version": 1,
            "generation": generation,
            "secret": secret,
            "protected_tools": protected_tools,
        }))
        .map_err(|_| MacRuntimeError::InvalidProfile)?;
        if frame.len() + 1 > 4096 {
            return Err(MacRuntimeError::InvalidProfile);
        }
        check_startup(deadline, shutting_down)?;
        let endpoint_root = create_endpoint_directory(state, &generation)?;
        let endpoint = endpoint_root.join("s");
        let root_info =
            fs::symlink_metadata(&endpoint_root).map_err(|_| MacRuntimeError::UnsafeState)?;
        let endpoint_root_identity = (root_info.dev(), root_info.ino());
        let lock_fd = lock.as_raw_fd();
        let mut command = Command::new(runtime.python());
        command
            .args(["-I", "-B", "-u"])
            .arg(runtime.root.join("macos_engine.py"))
            .arg("--runtime")
            .arg(&runtime.root)
            .arg("--state")
            .arg(state)
            .arg("--socket-dir")
            .arg(&endpoint_root)
            .arg("--state-lock-fd")
            .arg(lock_fd.to_string())
            .arg("--manifest-sha256")
            .arg(&runtime.manifest_sha256)
            .current_dir(state)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", state.join("home"))
            .env("TMPDIR", state.join("temp"))
            // Local readiness must not depend on an unrelated provider's availability.
            .env("HEADROOM_SKIP_UPSTREAM_CHECK", "1")
            // libmalloc reads these at process start, before Python embedding.
            .env("MallocAggressiveMadvise", "1")
            .env("MallocLargeCache", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(test)]
        command
            .stderr(Stdio::inherit())
            .env("HF_HUB_OFFLINE", "1")
            .env("TRANSFORMERS_OFFLINE", "1");
        // SAFETY: the post-fork closure only changes one live descriptor flag.
        // The inherited flock keeps a restarted app out until the orphan exits.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(lock_fd, libc::F_SETFD, 0) < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(_) => {
                return Err(
                    if remove_endpoint(&endpoint_root, endpoint_root_identity, None).is_ok() {
                        MacRuntimeError::Spawn
                    } else {
                        MacRuntimeError::Cleanup
                    },
                );
            }
        };
        let stdin = child.stdin.take();
        let pid = child.id();
        let mut session = Self {
            engine: Arc::new(OwnedEngine {
                child: Mutex::new(child),
                route_revoked: AtomicBool::new(false),
            }),
            stdin,
            lock: Some(lock),
            endpoint_root,
            endpoint,
            endpoint_root_identity,
            socket_identity: None,
            generation,
            secret,
            pid,
        };
        match session.confirm_startup(&frame, protected_tools, deadline, shutting_down) {
            Ok(initial) => Ok((session, initial)),
            Err(error) if session.force_stop().is_ok() => Err(error),
            Err(_) => Err(MacRuntimeError::Cleanup),
        }
    }

    fn confirm_startup(
        &mut self,
        frame: &[u8],
        protected_tools: &[String],
        deadline: Instant,
        shutting_down: &AtomicBool,
    ) -> Result<OptimizationState, MacRuntimeError> {
        let writer = self.stdin.as_mut().ok_or(MacRuntimeError::Spawn)?;
        writer
            .write_all(frame)
            .and_then(|_| writer.write_all(b"\n"))
            .and_then(|_| writer.flush())
            .map_err(|_| MacRuntimeError::Spawn)?;
        loop {
            check_startup(deadline, shutting_down)?;
            if !self.is_alive() {
                return Err(MacRuntimeError::NotReady);
            }
            if self.endpoint.exists() {
                match self.ready() {
                    Ok(()) => break,
                    Err(MacRuntimeError::OwnershipLost) => {
                        return Err(MacRuntimeError::OwnershipLost)
                    }
                    _ => {}
                }
            }
            thread::sleep(Duration::from_millis(50));
        }
        let health = self.get_json("/health")?;
        let config = health.get("config").ok_or(MacRuntimeError::NotReady)?;
        let tools = config
            .get("exclude_tools")
            .and_then(Value::as_array)
            .ok_or(MacRuntimeError::NotReady)?;
        if config.get("mode").and_then(Value::as_str) != Some("cache")
            || config.get("savings_profile").and_then(Value::as_str) != Some("coding")
            || config.get("memory") != Some(&Value::Bool(false))
            || config.get("learn") != Some(&Value::Bool(false))
            || !protected_tools
                .iter()
                .all(|tool| tools.iter().any(|value| value.as_str() == Some(tool)))
        {
            return Err(MacRuntimeError::NotReady);
        }
        let initial = self
            .optimization(&Uuid::new_v4().to_string(), ControlOperation::Read)
            .map_err(|_| MacRuntimeError::Control)?;
        if !initial.enabled || initial.revision != 0 || !initial.can_pause {
            return Err(MacRuntimeError::Control);
        }
        self.capture_metrics()
            .map_err(|_| MacRuntimeError::Control)?;
        check_startup(deadline, shutting_down)?;
        Ok(initial)
    }

    pub(crate) fn is_alive(&self) -> bool {
        self.engine.is_alive()
    }

    #[cfg(test)]
    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    pub(crate) fn generation(&self) -> &str {
        &self.generation
    }

    fn target(&self) -> UnixTarget<'_> {
        UnixTarget {
            socket_path: &self.endpoint,
            expected_pid: self.pid,
        }
    }

    pub(crate) fn optimization(
        &self,
        request_id: &str,
        operation: ControlOperation,
    ) -> Result<OptimizationState, ControlError> {
        control::request_unix(
            self.target(),
            &self.generation,
            &self.secret,
            request_id,
            operation,
            || self.is_alive(),
        )
    }

    pub(crate) fn capture_metrics(&self) -> Result<OwnedMetricsSnapshot, RuntimeMetricsError> {
        metrics_read::read_owned_projection_unix(
            self.target(),
            &self.generation,
            &self.secret,
            || self.is_alive(),
        )
    }

    pub(super) fn backend_target(&self) -> Result<BackendTarget, MacRuntimeError> {
        let generation =
            Uuid::parse_str(&self.generation).map_err(|_| MacRuntimeError::OwnershipLost)?;
        let engine = Arc::downgrade(&self.engine);
        BackendTarget::new_unix(
            self.endpoint.clone(),
            generation,
            self.pid,
            Arc::new(move || engine.upgrade().is_some_and(|engine| engine.can_route())),
        )
        .map_err(|_| MacRuntimeError::OwnershipLost)
    }

    pub(super) fn can_route(&self) -> bool {
        self.engine.can_route()
    }

    pub(super) fn revoke_route(&self) {
        self.engine.route_revoked.store(true, Ordering::Release);
    }

    pub(super) fn ready(&mut self) -> Result<(), MacRuntimeError> {
        let ready = self.get_json("/readyz")?;
        if ready.get("ready") == Some(&Value::Bool(true))
            && ready.get("version").and_then(Value::as_str) == Some(EXPECTED_ENGINE_VERSION)
        {
            Ok(())
        } else {
            Err(MacRuntimeError::NotReady)
        }
    }

    fn get_json(&mut self, path: &str) -> Result<Value, MacRuntimeError> {
        let before = fs::symlink_metadata(&self.endpoint).map_err(|_| MacRuntimeError::NotReady)?;
        let mut stream =
            self.target()
                .connect(|| self.is_alive())
                .map_err(|error| match error {
                    super::unix_backend::UnixBackendError::Transport => MacRuntimeError::NotReady,
                    _ => MacRuntimeError::OwnershipLost,
                })?;
        let after =
            fs::symlink_metadata(&self.endpoint).map_err(|_| MacRuntimeError::OwnershipLost)?;
        let identity = (after.dev(), after.ino());
        if !after.file_type().is_socket()
            || (before.dev(), before.ino()) != identity
            || self.socket_identity.is_some_and(|saved| saved != identity)
        {
            return Err(MacRuntimeError::OwnershipLost);
        }
        self.socket_identity = Some(identity);
        let deadline = Instant::now() + Duration::from_secs(2);
        let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Headroom-Desktop-Control: {}\r\nConnection: close\r\n\r\n", self.secret);
        let reply = if stream
            .set_write_timeout(Some(Duration::from_secs(1)))
            .and_then(|_| stream.write_all(request.as_bytes()))
            .is_ok()
        {
            super::manager::read_http_body_with_limit(&mut stream, deadline, 64 * 1024)
        } else {
            None
        };
        if !self.is_alive() {
            return Err(MacRuntimeError::OwnershipLost);
        }
        let (status, body) = reply.ok_or(MacRuntimeError::NotReady)?;
        if status != 200 {
            return Err(MacRuntimeError::NotReady);
        }
        serde_json::from_slice(&body).map_err(|_| MacRuntimeError::NotReady)
    }

    pub(crate) fn stop(&mut self) -> Result<bool, MacRuntimeError> {
        self.stop_owned(true)
    }

    pub(super) fn force_stop(&mut self) -> Result<(), MacRuntimeError> {
        self.stop_owned(false).map(|_| ())
    }

    fn stop_owned(&mut self, graceful: bool) -> Result<bool, MacRuntimeError> {
        self.revoke_route();
        if self.lock.is_none() {
            return self
                .exit_status()?
                .map(|status| status.success())
                .ok_or(MacRuntimeError::Cleanup);
        }
        drop(self.stdin.take());
        let mut status = if graceful {
            self.wait_for_exit(Duration::from_secs(25))?
        } else {
            self.exit_status()?
        };
        if status.is_none() {
            let kill = self
                .engine
                .child
                .lock()
                .map_err(|_| MacRuntimeError::Cleanup)?
                .kill();
            status = self.wait_for_exit(Duration::from_secs(5))?;
            if kill.is_err() && status.is_none() {
                return Err(MacRuntimeError::Cleanup);
            }
        }
        let status = status.ok_or(MacRuntimeError::Cleanup)?;
        remove_endpoint(
            &self.endpoint_root,
            self.endpoint_root_identity,
            self.socket_identity,
        )?;
        drop(self.lock.take());
        Ok(status.success())
    }

    fn exit_status(&self) -> Result<Option<ExitStatus>, MacRuntimeError> {
        self.engine
            .child
            .lock()
            .map_err(|_| MacRuntimeError::Cleanup)?
            .try_wait()
            .map_err(|_| MacRuntimeError::Cleanup)
    }

    fn wait_for_exit(&self, timeout: Duration) -> Result<Option<ExitStatus>, MacRuntimeError> {
        let deadline = Instant::now() + timeout;
        loop {
            let status = self.exit_status()?;
            if status.is_some() || Instant::now() >= deadline {
                return Ok(status);
            }
            thread::sleep(Duration::from_millis(25));
        }
    }
}

impl Drop for MacEngineSession {
    fn drop(&mut self) {
        if self.lock.is_some() {
            let _ = self.force_stop();
        }
    }
}

fn check_startup(deadline: Instant, shutting_down: &AtomicBool) -> Result<(), MacRuntimeError> {
    if shutting_down.load(Ordering::Acquire) {
        Err(MacRuntimeError::Unavailable)
    } else if Instant::now() >= deadline {
        Err(MacRuntimeError::NotReady)
    } else {
        Ok(())
    }
}

fn create_endpoint_directory(state: &Path, generation: &str) -> Result<PathBuf, MacRuntimeError> {
    let mut directory = state.join(format!("e-{}", &generation[..8]));
    if directory.join("s").as_os_str().as_bytes().len() >= 104 {
        // Darwin's socket address limit can be shorter than the user's app data path.
        let temporary =
            fs::canonicalize(std::env::temp_dir()).map_err(|_| MacRuntimeError::UnsafeState)?;
        private_directory(&temporary)?;
        directory = temporary.join(format!("hr-{}", &generation[..16]));
        if directory.join("s").as_os_str().as_bytes().len() >= 104 {
            return Err(MacRuntimeError::UnsafeState);
        }
    }
    DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .map_err(|_| MacRuntimeError::UnsafeState)?;
    Ok(directory)
}

fn remove_endpoint(
    directory: &Path,
    directory_identity: (u64, u64),
    socket_identity: Option<(u64, u64)>,
) -> Result<(), MacRuntimeError> {
    let root = fs::symlink_metadata(directory).map_err(|_| MacRuntimeError::Cleanup)?;
    if !root.is_dir() || (root.dev(), root.ino()) != directory_identity {
        return Err(MacRuntimeError::Cleanup);
    }
    let endpoint = directory.join("s");
    match fs::symlink_metadata(&endpoint) {
        Ok(info)
            if socket_identity == Some((info.dev(), info.ino()))
                && info.file_type().is_socket() =>
        {
            fs::remove_file(endpoint).map_err(|_| MacRuntimeError::Cleanup)?;
        }
        Ok(_) => return Err(MacRuntimeError::Cleanup),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(MacRuntimeError::Cleanup),
    }
    fs::remove_dir(directory).map_err(|_| MacRuntimeError::Cleanup)
}

pub(super) fn private_directory(path: &Path) -> Result<(), MacRuntimeError> {
    let info = fs::symlink_metadata(path).map_err(|_| MacRuntimeError::UnsafeState)?;
    // SAFETY: geteuid has no memory preconditions.
    let uid = unsafe { libc::geteuid() };
    if !path.is_absolute()
        || fs::canonicalize(path).ok().as_deref() != Some(path)
        || !info.is_dir()
        || info.uid() != uid
        || info.mode() & 0o7777 != 0o700
    {
        return Err(MacRuntimeError::UnsafeState);
    }
    Ok(())
}

fn state_lock(state: &Path) -> Result<File, MacRuntimeError> {
    let path = state.join(".engine.lock");
    let root = fs::symlink_metadata(state).map_err(|_| MacRuntimeError::UnsafeState)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path)
        .map_err(|_| MacRuntimeError::UnsafeState)?;
    let info = file.metadata().map_err(|_| MacRuntimeError::UnsafeState)?;
    // SAFETY: geteuid has no memory preconditions.
    let uid = unsafe { libc::geteuid() };
    if !info.is_file() || info.uid() != uid || info.nlink() != 1 || info.mode() & 0o7777 != 0o600 {
        return Err(MacRuntimeError::UnsafeState);
    }
    let after = fs::symlink_metadata(state).map_err(|_| MacRuntimeError::UnsafeState)?;
    if (root.dev(), root.ino()) != (after.dev(), after.ino()) {
        return Err(MacRuntimeError::UnsafeState);
    }
    // SAFETY: file owns this valid descriptor; flock has no borrowed buffers.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(
            if io::Error::last_os_error().kind() == io::ErrorKind::WouldBlock {
                MacRuntimeError::Busy
            } else {
                MacRuntimeError::UnsafeState
            },
        );
    }
    let named = fs::symlink_metadata(&path).map_err(|_| MacRuntimeError::UnsafeState)?;
    if !named.is_file() || (named.dev(), named.ino()) != (info.dev(), info.ino()) {
        return Err(MacRuntimeError::UnsafeState);
    }
    Ok(file)
}

#[cfg(test)]
#[path = "macos_session_tests.rs"]
mod tests;
