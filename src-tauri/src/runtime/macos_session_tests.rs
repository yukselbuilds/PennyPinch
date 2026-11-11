use super::*;
use std::io::{BufRead, BufReader, Read};
use std::os::fd::{FromRawFd, OwnedFd};
use std::os::unix::fs::PermissionsExt;
use std::sync::mpsc;

fn state() -> PathBuf {
    let base =
        PathBuf::from(std::env::var_os("HEADROOM_MACOS_PROOF_ROOT").expect("private proof root"));
    let path = base.join(format!("s-{}", &Uuid::new_v4().simple().to_string()[..8]));
    DirBuilder::new().mode(0o700).create(&path).unwrap();
    private_directory(&path).unwrap();
    path
}

fn runtime() -> MacRuntime {
    let path =
        PathBuf::from(std::env::var_os("HEADROOM_MACOS_RUNTIME_ROOT").expect("staged runtime"));
    MacRuntime::open(&path).expect("compiled manifest and complete payload must match")
}

#[test]
#[ignore = "requires the pinned native macOS resource tree"]
fn managed_engine_lifecycle() {
    let runtime = runtime();
    for name in ["manifest.json", "cache_control_shim.py"] {
        let path = runtime.root.join(name);
        let original = fs::read(&path).unwrap();
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"\n# fixture tamper\n")
            .unwrap();
        let rejected = MacRuntime::open(&runtime.root).err();
        fs::write(&path, original).unwrap();
        assert_eq!(rejected, Some(MacRuntimeError::Integrity));
    }
    let injected = runtime.root.join("unexpected.fixture");
    fs::write(&injected, b"untracked overlay").unwrap();
    let rejected = MacRuntime::open(&runtime.root).err();
    fs::remove_file(injected).unwrap();
    assert_eq!(rejected, Some(MacRuntimeError::Integrity));
    runtime.revalidate().unwrap();
    let state = state();
    let tools = vec!["exec_command".to_owned(), "headroom_retrieve".to_owned()];
    let mut session =
        MacEngineSession::start(&runtime, &state, &tools).expect("owned engine ready");
    assert!(session.is_alive());
    assert_eq!(state_lock(&state).err(), Some(MacRuntimeError::Busy));
    let first_generation = session.generation().to_owned();
    let metrics = session
        .capture_metrics()
        .expect("private metrics are available");
    assert_eq!(metrics.generation, first_generation);
    assert!(!metrics.projection.aggregate_only);

    for path in ["/health", "/stats?cached=1"] {
        let secret = std::mem::replace(&mut session.secret, "f".repeat(32));
        let redacted = session.get_json(path).expect("public aggregate response");
        session.secret = secret;
        assert!(redacted.get("config").is_none());
        assert!(redacted.get("recent_requests").is_none());
        assert!(redacted.get("request_logs").is_none());
    }

    let paused = session
        .optimization(ControlOperation::Set {
            expected_revision: 0,
            enabled: false,
        })
        .unwrap();
    assert!(!paused.enabled && paused.revision == 1);
    assert!(session
        .optimization(ControlOperation::Set {
            expected_revision: 0,
            enabled: true
        })
        .is_err());
    assert!(
        !session
            .optimization(ControlOperation::Read)
            .unwrap()
            .enabled
    );
    let resumed = session
        .optimization(ControlOperation::Set {
            expected_revision: 1,
            enabled: true,
        })
        .unwrap();
    assert!(resumed.enabled && resumed.revision == 2);
    let marker = state.join("cache/keep.fixture");
    fs::write(&marker, b"persistent local cache").unwrap();
    let endpoint = session.endpoint_root.clone();
    assert!(session.stop().expect("graceful owned stop"));
    assert!(session.stop().expect("idempotent stop"));
    assert!(!session.is_alive() && !endpoint.exists());
    drop(session);
    let mut restarted = MacEngineSession::start(&runtime, &state, &tools).unwrap();
    assert_ne!(restarted.generation(), first_generation);
    assert_eq!(fs::read(&marker).unwrap(), b"persistent local cache");
    assert!(restarted.stop().unwrap());
    assert!(state_lock(&state).is_ok());
}

#[test]
#[ignore = "invoked only by the parent-loss test"]
fn parent_helper() {
    if std::env::var("HEADROOM_MACOS_PARENT_HELPER")
        .ok()
        .as_deref()
        != Some("1")
    {
        return;
    }
    let state = PathBuf::from(std::env::var_os("HEADROOM_MACOS_HELPER_STATE").unwrap());
    let session = MacEngineSession::start(&runtime(), &state, &[]).unwrap();
    {
        let mut child = session.child.lock().unwrap();
        assert!(child.try_wait().unwrap().is_none());
        // A live unreaped owned child cannot have its PID recycled here.
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGSTOP) }, 0);
    }
    println!("macos_owned_child_pid={}", session.pid());
    std::io::stdout().flush().unwrap();
    let _ = std::io::stdin().read(&mut [0u8; 1]);
}

struct ResumeChild(i32);
impl Drop for ResumeChild {
    fn drop(&mut self) {
        if self.0 > 0 {
            // Only the test-owned, still-stopped descendant is resumed.
            unsafe {
                libc::kill(self.0, libc::SIGCONT);
            }
        }
    }
}

#[test]
#[ignore = "requires the pinned native macOS resource tree"]
fn managed_engine_parent_loss() {
    let _ = runtime();
    let state = state();
    let mut parent = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "runtime::macos_session::tests::parent_helper",
            "--ignored",
            "--nocapture",
        ])
        .env("HEADROOM_MACOS_PARENT_HELPER", "1")
        .env("HEADROOM_MACOS_HELPER_STATE", &state)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let stdout = parent.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().take(20).flatten() {
            if let Some(pid) = line
                .split_once("macos_owned_child_pid=")
                .and_then(|(_, value)| value.parse::<i32>().ok())
            {
                let _ = send.send(pid);
                return;
            }
        }
    });
    let pid = match receive.recv_timeout(Duration::from_secs(110)) {
        Ok(pid) => pid,
        Err(_) => {
            let _ = parent.kill();
            let _ = parent.wait();
            panic!("parent helper did not reach native readiness");
        }
    };
    let mut resume = ResumeChild(pid);
    // NOTE_EXIT tracks this exact process, including after its parent is killed.
    let queue = unsafe { libc::kqueue() };
    assert!(queue >= 0);
    let queue = unsafe { OwnedFd::from_raw_fd(queue) };
    let change = libc::kevent {
        ident: pid as usize,
        filter: libc::EVFILT_PROC,
        flags: libc::EV_ADD | libc::EV_ENABLE | libc::EV_ONESHOT,
        fflags: libc::NOTE_EXIT,
        data: 0,
        udata: std::ptr::null_mut(),
    };
    assert_eq!(
        unsafe {
            libc::kevent(
                queue.as_raw_fd(),
                &change,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        },
        0
    );
    parent.kill().unwrap();
    parent.wait().unwrap();
    reader.join().unwrap();
    assert_eq!(
        state_lock(&state).err(),
        Some(MacRuntimeError::Busy),
        "orphan retains the inherited lock"
    );
    assert_eq!(unsafe { libc::kill(pid, libc::SIGCONT) }, 0);
    resume.0 = 0;
    let mut event = change;
    let timeout = libc::timespec {
        tv_sec: 25,
        tv_nsec: 0,
    };
    assert_eq!(
        unsafe {
            libc::kevent(
                queue.as_raw_fd(),
                std::ptr::null(),
                0,
                &mut event,
                1,
                &timeout,
            )
        },
        1,
        "actual engine must exit after parent pipe EOF"
    );
    let observed_pid = event.ident;
    assert_eq!(observed_pid, pid as usize);
    assert_eq!(event.fflags & libc::NOTE_EXIT, libc::NOTE_EXIT);
    assert!(
        state_lock(&state).is_ok(),
        "state is available only after engine exit"
    );
}

#[test]
#[ignore = "requires a private native proof root"]
fn managed_engine_private_state() {
    let state = state();
    let lock = state_lock(&state).unwrap();
    assert_eq!(state_lock(&state).err(), Some(MacRuntimeError::Busy));
    drop(lock);
    fs::set_permissions(&state, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(private_directory(&state), Err(MacRuntimeError::UnsafeState));
    fs::set_permissions(&state, fs::Permissions::from_mode(0o700)).unwrap();
    let link = state.join(".engine.lock");
    fs::rename(&link, state.join("preserved.lock")).unwrap();
    std::os::unix::fs::symlink(state.join("preserved.lock"), &link).unwrap();
    assert_eq!(state_lock(&state).err(), Some(MacRuntimeError::UnsafeState));
}
