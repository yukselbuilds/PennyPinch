use super::{failure, Flow, InstallationSummary, Operation, Reply, Request};
use crate::integrations::config_io::{create_client_workspace, PinnedDirectoryChain};
use crate::integrations::process::OwnedJob;
use std::fs::{self, File};
use std::io::Read;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, TerminateProcess, WaitForSingleObject,
};

struct Workspace {
    path: PathBuf,
    pins: Option<PinnedDirectoryChain>,
}
impl Workspace {
    fn new(root: &Path) -> Result<Self, ()> {
        let (path, pins) = create_client_workspace(root)?;
        Ok(Self {
            path,
            pins: Some(pins),
        })
    }
    fn valid(&self) -> bool {
        self.pins.as_ref().is_some_and(|p| p.validate().is_ok())
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        if self.valid() {
            drop(self.pins.take());
            let _ = fs::remove_dir(&self.path);
        }
    }
}

pub(super) fn read(
    root: &Path,
    executable: &Path,
    i: &InstallationSummary,
) -> Result<(i32, Vec<u8>, Vec<u8>), &'static str> {
    if i.client_kind != "codex" {
        return Err("read_not_passive");
    }
    let workspace = Workspace::new(root).map_err(|_| "unavailable")?;
    let mut command = Command::new(executable);
    match i.client_kind {
        "codex" => {
            // Workload identity may perform token exchange; it is outside this passive local probe.
            if [
                "OPENAI_IDENTITY_TOKEN_FILE",
                "OPENAI_WORKLOAD_IDENTITY_CONFIG",
            ]
            .iter()
            .any(|key| std::env::var_os(key).is_some())
            {
                return Err("environment_auth");
            }
            command.args(["--no-daemon", "login", "status"]);
        }
        _ => return Err("unsupported"),
    }
    if !workspace.valid() {
        return Err("unavailable");
    }
    command
        .current_dir(&workspace.path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(0x0800_0000);
    let job = OwnedJob::create().map_err(|_| "unavailable")?;
    let mut child = command.spawn().map_err(|_| "read_failed")?;
    if job.assign(&child).is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return Err("read_failed");
    }
    let (tx, rx) = mpsc::sync_channel(2);
    for (stream, mut input) in [
        (
            0,
            child
                .stdout
                .take()
                .map(|p| Box::new(p) as Box<dyn Read + Send>),
        ),
        (
            1,
            child
                .stderr
                .take()
                .map(|p| Box::new(p) as Box<dyn Read + Send>),
        ),
    ] {
        let Some(mut input) = input.take() else {
            return Err("read_failed");
        };
        let sender = tx.clone();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = input.by_ref().take(65537).read_to_end(&mut bytes);
            let _ = sender.send((
                stream,
                result.ok().filter(|_| bytes.len() <= 65536).map(|_| bytes),
            ));
        });
    }
    drop(tx);
    let deadline = Instant::now() + Duration::from_secs(10);
    let result = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status.code().unwrap_or(-1)),
            Err(_) => break Err("read_failed"),
            Ok(None) if Instant::now() >= deadline => break Err("timeout"),
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    if !job.terminate_and_wait_empty(Duration::from_secs(2)) {
        return Err("cleanup_failed");
    }
    let _ = child.wait();
    let code = result?;
    let mut output = [None, None];
    for _ in 0..2 {
        let (stream, bytes) = rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| "read_failed")?;
        output[stream] = bytes;
    }
    if !workspace.valid() {
        return Err("unavailable");
    }
    Ok((
        code,
        output[0].take().ok_or("read_failed")?,
        output[1].take().ok_or("read_failed")?,
    ))
}

// Only the CLI root is owned. The user's browser must never join a kill-on-close job.
pub(super) struct NativeProcess(HANDLE);
unsafe impl Send for NativeProcess {}
impl NativeProcess {
    fn launch(executable: &Path, args: &[&str], cwd: &Path) -> Result<Self, ()> {
        Self::launch_with_show(executable, args, cwd, 1)
    }

    fn launch_with_show(
        executable: &Path,
        args: &[&str],
        cwd: &Path,
        show: i32,
    ) -> Result<Self, ()> {
        let executable = executable.to_owned();
        let cwd = cwd.to_owned();
        let args = args.join(" ");
        std::thread::spawn(move || unsafe {
            use windows::Win32::System::Com::{
                CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED,
            };
            use windows_sys::Win32::UI::Shell::{
                ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS,
                SHELLEXECUTEINFOW,
            };
            if CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_err() {
                return Err(());
            }
            let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(Some(0)).collect::<Vec<u16>>();
            let file = wide(executable.as_os_str());
            let directory = wide(cwd.as_os_str());
            let parameters = wide(std::ffi::OsStr::new(&args));
            let mut info = SHELLEXECUTEINFOW {
                cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
                lpFile: file.as_ptr(),
                lpParameters: parameters.as_ptr(),
                lpDirectory: directory.as_ptr(),
                nShow: show,
                ..Default::default()
            };
            let ok = ShellExecuteExW(&mut info) != 0 && !info.hProcess.is_null();
            CoUninitialize();
            if ok {
                Ok(Self(info.hProcess))
            } else {
                Err(())
            }
        })
        .join()
        .map_err(|_| ())?
    }

    fn exit_code(&self) -> Result<Option<u32>, ()> {
        match unsafe { WaitForSingleObject(self.0, 0) } {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let mut code = 0;
                if unsafe { GetExitCodeProcess(self.0, &mut code) } == 0 {
                    Err(())
                } else {
                    Ok(Some(code))
                }
            }
            _ => Err(()),
        }
    }

    fn stop(&self) -> bool {
        if self.exit_code().ok().flatten().is_some() {
            return true;
        }
        unsafe {
            TerminateProcess(self.0, 0xC000_013A);
            WaitForSingleObject(self.0, 2000) == WAIT_OBJECT_0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::IsTerminal;

    #[test]
    fn unresolved_login_retains_ownership_and_cancel_can_retry_after_a_failure() {
        let flow = Flow {
            flow_id: uuid::Uuid::new_v4().to_string(),
            installation_id: uuid::Uuid::new_v4().to_string(),
            client_kind: "codex",
            state: "cleanup_failed",
        };
        let mut active = Some(Login {
            snapshot: flow.clone(),
            process: None,
            started: Instant::now(),
            executable_pin: None,
            parent_pins: None,
            workspace: None,
        });
        assert!(active.as_ref().unwrap().blocks_operations());
        let mut request = Request {
            schema_version: 1,
            request_id: uuid::Uuid::new_v4().to_string(),
            operation: Operation::CancelLogin,
            installation_id: None,
            scan_generation: None,
            flow_id: Some(uuid::Uuid::new_v4().to_string()),
            confirm_profile_change: false,
        };
        assert_eq!(flow_action(&mut active, &request).error, Some("not_found"));
        assert_eq!(active.as_ref().unwrap().snapshot.state, "cleanup_failed");
        request.flow_id = Some(flow.flow_id);
        assert_eq!(
            flow_action(&mut active, &request).flow.unwrap().state,
            "cancelled"
        );
        assert!(!active.as_ref().unwrap().blocks_operations());
    }

    #[test]
    #[ignore = "invoked only by the owned parent fixture"]
    fn login_console_fixture_child() {
        assert!(std::io::stdin().is_terminal());
        fs::write("started", b"console-ready").unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while !Path::new("finish").exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(Path::new("finish").exists());
    }

    #[test]
    fn owned_login_console_can_cancel_without_stopping_another_process() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("work/proofs/accounts/console-tests")
            .join(uuid::Uuid::new_v4().to_string());
        let one = root.join("one");
        let two = root.join("two");
        fs::create_dir_all(&one).unwrap();
        fs::create_dir(&two).unwrap();
        let executable = std::env::current_exe().unwrap();
        let args = [
            "--exact",
            "integrations::accounts::platform::tests::login_console_fixture_child",
            "--ignored",
            "--nocapture",
        ];
        let a = NativeProcess::launch_with_show(&executable, &args, &one, 0).unwrap();
        let b = NativeProcess::launch_with_show(&executable, &args, &two, 0).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while (!one.join("started").exists() || !two.join("started").exists())
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            one.join("started").exists() && two.join("started").exists(),
            "console input was not available"
        );
        assert!(a.stop());
        assert!(a.exit_code().unwrap().is_some());
        assert!(b.exit_code().unwrap().is_none());
        fs::write(two.join("finish"), b"exit").unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while b.exit_code().unwrap().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(b.exit_code().unwrap(), Some(0));
        drop(a);
        drop(b);
        fs::remove_file(one.join("started")).unwrap();
        fs::remove_file(two.join("started")).unwrap();
        fs::remove_file(two.join("finish")).unwrap();
        fs::remove_dir(one).unwrap();
        fs::remove_dir(two).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
impl Drop for NativeProcess {
    fn drop(&mut self) {
        let _ = self.stop();
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub(super) struct Login {
    pub snapshot: Flow,
    process: Option<NativeProcess>,
    started: Instant,
    executable_pin: Option<File>,
    parent_pins: Option<PinnedDirectoryChain>,
    workspace: Option<Workspace>,
}
impl Login {
    pub fn blocks_operations(&self) -> bool {
        matches!(self.snapshot.state, "pending" | "cleanup_failed")
    }

    pub fn start(
        root: &Path,
        executable: &Path,
        i: InstallationSummary,
        pin: File,
        parents: PinnedDirectoryChain,
    ) -> Result<Self, ()> {
        let workspace = Workspace::new(root)?;
        let args: &[&str] = match i.client_kind {
            "codex" => &["--no-daemon", "login"],
            "claude_code" => &["auth", "login"],
            "opencode" => &["auth", "login"],
            _ => return Err(()),
        };
        if !workspace.valid() {
            return Err(());
        }
        let process = NativeProcess::launch(executable, args, &workspace.path)?;
        Ok(Self {
            snapshot: Flow {
                flow_id: uuid::Uuid::new_v4().to_string(),
                installation_id: i.installation_id,
                client_kind: i.client_kind,
                state: "pending",
            },
            process: Some(process),
            started: Instant::now(),
            executable_pin: Some(pin),
            parent_pins: Some(parents),
            workspace: Some(workspace),
        })
    }

    pub fn update(&mut self) {
        if self.snapshot.state != "pending" {
            return;
        }
        let Some(process) = self.process.as_ref() else {
            return;
        };
        self.snapshot.state = match process.exit_code() {
            Ok(Some(0)) => "finished",
            Ok(Some(_)) => "failed",
            Err(_) => {
                if process.stop() {
                    "failed"
                } else {
                    "cleanup_failed"
                }
            }
            Ok(None) if self.started.elapsed() >= Duration::from_secs(600) => {
                if process.stop() {
                    "timed_out"
                } else {
                    "cleanup_failed"
                }
            }
            Ok(None) => "pending",
        };
        self.release_finished();
    }
    pub fn cancel(&mut self) {
        self.snapshot.state = if self.process.as_ref().is_none_or(|p| p.stop()) {
            "cancelled"
        } else {
            "cleanup_failed"
        };
        self.release_finished();
    }
    fn release_finished(&mut self) {
        if !matches!(self.snapshot.state, "pending" | "cleanup_failed") {
            drop(self.process.take());
            drop(self.executable_pin.take());
            drop(self.parent_pins.take());
            drop(self.workspace.take());
        }
    }
}

pub(super) fn flow_action(active: &mut Option<Login>, request: &Request) -> Reply {
    if matches!(request.operation, Operation::ActiveLogin) {
        if let Some(login) = active.as_mut() {
            login.update();
        }
        return Reply {
            schema_version: 1,
            request_id: request.request_id.clone(),
            status: "snapshot",
            account: None,
            flow: active.as_ref().map(|l| l.snapshot.clone()),
            error: None,
        };
    }
    let Some(login) = active
        .as_mut()
        .filter(|l| Some(&l.snapshot.flow_id) == request.flow_id.as_ref())
    else {
        return failure(&request.request_id, "not_found");
    };
    login.update();
    if matches!(request.operation, Operation::CancelLogin) && login.blocks_operations() {
        login.cancel();
    }
    Reply {
        schema_version: 1,
        request_id: request.request_id.clone(),
        status: "snapshot",
        account: None,
        flow: Some(login.snapshot.clone()),
        error: None,
    }
}
