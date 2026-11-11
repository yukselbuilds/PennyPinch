//! Own one macOS engine process, its parent-pipe lifetime and inherited state lock.

use super::control::{self, ControlOperation, OptimizationState};
use super::macos_layout::{MacRuntime, MacRuntimeError};
use super::metrics_read::{self, OwnedMetricsSnapshot};
use super::unix_backend::UnixTarget;
use serde_json::{json, Value};
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(crate) struct MacEngineSession {
    child: Mutex<Child>,
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
        runtime: &MacRuntime,
        state: &Path,
        protected_tools: &[String],
    ) -> Result<Self, MacRuntimeError> {
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
        runtime.revalidate()?;
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
        let endpoint_root = state.join(format!("e-{}", &generation[..8]));
        let endpoint = endpoint_root.join("s");
        use std::os::unix::ffi::OsStrExt;
        if endpoint.as_os_str().as_bytes().len() >= 104 {
            return Err(MacRuntimeError::UnsafeState);
        }
        DirBuilder::new()
            .mode(0o700)
            .create(&endpoint_root)
            .map_err(|_| MacRuntimeError::UnsafeState)?;
        let root_info =
            fs::symlink_metadata(&endpoint_root).map_err(|_| MacRuntimeError::UnsafeState)?;
        let endpoint_root_identity = (root_info.dev(), root_info.ino());
        let frame = serde_json::to_vec(&json!({
            "schema_version": 1,
            "generation": generation,
            "secret": secret,
            "protected_tools": protected_tools,
        }))
        .map_err(|_| MacRuntimeError::InvalidProfile)?;
        if frame.len() + 1 > 4096 {
            let _ = fs::remove_dir(&endpoint_root);
            return Err(MacRuntimeError::InvalidProfile);
        }
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
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(test)]
        command
            .stderr(Stdio::inherit())
            .env("HEADROOM_SKIP_UPSTREAM_CHECK", "1")
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
                let _ = fs::remove_dir(&endpoint_root);
                return Err(MacRuntimeError::Spawn);
            }
        };
        let stdin = child.stdin.take();
        let pid = child.id();
        let mut session = Self {
            child: Mutex::new(child),
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
        let writer = session.stdin.as_mut().ok_or(MacRuntimeError::Spawn)?;
        writer
            .write_all(&frame)
            .and_then(|_| writer.write_all(b"\n"))
            .and_then(|_| writer.flush())
            .map_err(|_| MacRuntimeError::Spawn)?;
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            if !session.is_alive() {
                return Err(MacRuntimeError::NotReady);
            }
            if session.endpoint.exists() {
                match session.get_json("/readyz") {
                    Ok(ready)
                        if ready.get("ready") == Some(&Value::Bool(true))
                            && ready.get("version").and_then(Value::as_str) == Some("0.41.0") =>
                    {
                        break
                    }
                    Err(MacRuntimeError::OwnershipLost) => {
                        return Err(MacRuntimeError::OwnershipLost)
                    }
                    _ => {}
                }
            }
            if Instant::now() >= deadline {
                return Err(MacRuntimeError::NotReady);
            }
            thread::sleep(Duration::from_millis(50));
        }
        let health = session.get_json("/health")?;
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
        let initial = session.optimization(ControlOperation::Read)?;
        if !initial.enabled || initial.revision != 0 || !initial.can_pause {
            return Err(MacRuntimeError::Control);
        }
        session.capture_metrics()?;
        Ok(session)
    }

    pub(crate) fn is_alive(&self) -> bool {
        self.child
            .lock()
            .ok()
            .and_then(|mut child| child.try_wait().ok())
            .is_some_and(|status| status.is_none())
    }

    pub(crate) fn pid(&self) -> u32 {
        self.pid
    }

    pub(crate) fn generation(&self) -> &str {
        &self.generation
    }

    pub(crate) fn target(&self) -> UnixTarget<'_> {
        UnixTarget {
            socket_path: &self.endpoint,
            expected_pid: self.pid,
        }
    }

    pub(crate) fn optimization(
        &self,
        operation: ControlOperation,
    ) -> Result<OptimizationState, MacRuntimeError> {
        control::request_unix(
            self.target(),
            &self.generation,
            &self.secret,
            &Uuid::new_v4().to_string(),
            operation,
            || self.is_alive(),
        )
        .map_err(|_| MacRuntimeError::Control)
    }

    pub(crate) fn capture_metrics(&self) -> Result<OwnedMetricsSnapshot, MacRuntimeError> {
        metrics_read::read_owned_projection_unix(
            self.target(),
            &self.generation,
            &self.secret,
            || self.is_alive(),
        )
        .map_err(|_| MacRuntimeError::Control)
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
        stream
            .set_write_timeout(Some(Duration::from_secs(1)))
            .map_err(|_| MacRuntimeError::NotReady)?;
        let request = format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Headroom-Desktop-Control: {}\r\nConnection: close\r\n\r\n", self.secret);
        stream
            .write_all(request.as_bytes())
            .map_err(|_| MacRuntimeError::NotReady)?;
        let (status, body) =
            super::manager::read_http_body_with_limit(&mut stream, deadline, 64 * 1024)
                .ok_or(MacRuntimeError::NotReady)?;
        if !self.is_alive() {
            return Err(MacRuntimeError::OwnershipLost);
        }
        if status != 200 {
            return Err(MacRuntimeError::NotReady);
        }
        serde_json::from_slice(&body).map_err(|_| MacRuntimeError::NotReady)
    }

    pub(crate) fn stop(&mut self) -> Result<bool, MacRuntimeError> {
        if self.lock.is_none() {
            return self
                .exit_status()?
                .map(|status| status.success())
                .ok_or(MacRuntimeError::Cleanup);
        }
        drop(self.stdin.take());
        let deadline = Instant::now() + Duration::from_secs(25);
        let mut status = self.exit_status()?;
        while status.is_none() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(25));
            status = self.exit_status()?;
        }
        if status.is_none() {
            self.child
                .lock()
                .map_err(|_| MacRuntimeError::Cleanup)?
                .kill()
                .map_err(|_| MacRuntimeError::Cleanup)?;
            let deadline = Instant::now() + Duration::from_secs(5);
            while status.is_none() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(25));
                status = self.exit_status()?;
            }
        }
        let status = status.ok_or(MacRuntimeError::Cleanup)?;
        let root =
            fs::symlink_metadata(&self.endpoint_root).map_err(|_| MacRuntimeError::Cleanup)?;
        if !root.is_dir() || (root.dev(), root.ino()) != self.endpoint_root_identity {
            return Err(MacRuntimeError::Cleanup);
        }
        match fs::symlink_metadata(&self.endpoint) {
            Ok(info)
                if self.socket_identity == Some((info.dev(), info.ino()))
                    && info.file_type().is_socket() =>
            {
                fs::remove_file(&self.endpoint).map_err(|_| MacRuntimeError::Cleanup)?;
            }
            Ok(_) => return Err(MacRuntimeError::Cleanup),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(MacRuntimeError::Cleanup),
        }
        fs::remove_dir(&self.endpoint_root).map_err(|_| MacRuntimeError::Cleanup)?;
        drop(self.lock.take());
        Ok(status.success())
    }

    fn exit_status(&self) -> Result<Option<ExitStatus>, MacRuntimeError> {
        self.child
            .lock()
            .map_err(|_| MacRuntimeError::Cleanup)?
            .try_wait()
            .map_err(|_| MacRuntimeError::Cleanup)
    }
}

impl Drop for MacEngineSession {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

fn private_directory(path: &Path) -> Result<(), MacRuntimeError> {
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
