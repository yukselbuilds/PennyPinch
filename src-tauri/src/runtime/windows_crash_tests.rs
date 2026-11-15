use super::*;
use std::net::TcpListener;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use windows_sys::Win32::Foundation::{
    DuplicateHandle, GetHandleInformation, DUPLICATE_SAME_ACCESS, HANDLE, HANDLE_FLAG_INHERIT,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetExitCodeProcess, GetProcessId, WaitForSingleObject,
};

const FIXTURE_ENV: &str = "HEADROOM_H006_CRASH_RUN";
const TEST_NAME: &str =
    "runtime::manager::windows_crash_tests::packaged_controller_job_closes_after_owner_crash";
const CRASH_TIMEOUT: Duration = Duration::from_secs(5);
const STARTUP_FIXTURE_ENV: &str = "HEADROOM_H006_STARTUP_CRASH_RUN";
const STARTUP_TEST: &str =
    "runtime::manager::windows_crash_tests::packaged_controller_exits_after_owner_crash_before_job_assignment";

thread_local! {
    static STARTUP_CRASH_RUN: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

struct OwnerChild(Child);

impl Drop for OwnerChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn duplicate_from_owner(owner: &Child, raw: u64) -> OwnedHandle {
    let source = usize::try_from(raw).unwrap() as HANDLE;
    let mut duplicate = std::ptr::null_mut();
    // Only duplicate a handle from the exact child we created, never a PID lookup.
    let ok = unsafe {
        DuplicateHandle(
            owner.as_raw_handle() as HANDLE,
            source,
            GetCurrentProcess(),
            &mut duplicate,
            0,
            0,
            DUPLICATE_SAME_ACCESS,
        )
    };
    assert_ne!(ok, 0, "owned fixture handle duplication failed");
    unsafe { OwnedHandle::from_raw_handle(duplicate) }
}

pub(super) fn observe_before_job_assignment(child: &Child, job: &Mutex<OwnedJob>) {
    let Some(run) = STARTUP_CRASH_RUN.with(|slot| slot.borrow_mut().take()) else {
        return;
    };
    let child_handle = child.as_raw_handle() as HANDLE;
    assert!(!job.lock().unwrap().contains_process(child_handle));
    let receipt = serde_json::json!({
        "owner_pid": std::process::id(),
        "controller_handle": child_handle as usize as u64,
        "stdin_handle": child.stdin.as_ref().unwrap().as_raw_handle() as usize as u64,
        "stdout_handle": child.stdout.as_ref().unwrap().as_raw_handle() as usize as u64,
    });
    fs::write(run.join("spawn.tmp"), serde_json::to_vec(&receipt).unwrap()).unwrap();
    fs::rename(run.join("spawn.tmp"), run.join("spawn.json")).unwrap();
    thread::sleep(Duration::from_secs(30));
    panic!("observer did not terminate the test owner before Job assignment");
}

fn startup_fixture_processes(job: &OwnedJob) -> (u32, u32) {
    use windows_sys::Win32::System::JobObjects::{
        JobObjectBasicAccountingInformation, QueryInformationJobObject,
        JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    };
    let mut counts = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
    let ok = unsafe {
        QueryInformationJobObject(
            job.0,
            JobObjectBasicAccountingInformation,
            std::ptr::from_mut(&mut counts).cast(),
            std::mem::size_of_val(&counts) as u32,
            std::ptr::null_mut(),
        )
    };
    assert_ne!(ok, 0, "outer test Job accounting unavailable");
    (counts.TotalProcesses, counts.ActiveProcesses)
}

#[test]
#[ignore = "opt-in real Windows controller owner loss before production Job assignment"]
fn packaged_controller_exits_after_owner_crash_before_job_assignment() {
    use windows_sys::Win32::Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT};

    let project =
        fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap()).unwrap();
    let resources = project.join("work/build/windows/h010-retrieval-resources");
    let runs = project.join("work/proofs/windows-engine-crash/startup-runs");
    fs::create_dir_all(&runs).unwrap();
    let runs = fs::canonicalize(runs).unwrap();
    if let Some(run) = std::env::var_os(STARTUP_FIXTURE_ENV) {
        let run = fs::canonicalize(run).unwrap();
        assert_eq!(run.parent().and_then(Path::parent), Some(runs.as_path()));
        // The observer assigns the owner to its cleanup Job before letting it spawn.
        let mut go = [0];
        io::stdin().read_exact(&mut go).unwrap();
        assert_eq!(go, [1]);
        let layout = RuntimeLayout::from_bundle(resources, run.join("startup-state")).unwrap();
        STARTUP_CRASH_RUN.with(|slot| *slot.borrow_mut() = Some(run));
        let _ = ControllerSession::launch(
            &layout,
            &Uuid::new_v4().to_string(),
            &AtomicBool::new(false),
        );
        panic!("fixture did not reach the spawn checkpoint");
    }

    let run_root = runs.join(Uuid::new_v4().to_string());
    fs::create_dir(&run_root).unwrap();
    let mut cases = Vec::new();
    for retain_writer in [false, true] {
        let run = run_root.join(if retain_writer {
            "retained-writer"
        } else {
            "owner-only"
        });
        fs::create_dir(&run).unwrap();
        let cleanup_job = OwnedJob::create().unwrap();
        let mut owner = OwnerChild(
            Command::new(std::env::current_exe().unwrap())
                .args([STARTUP_TEST, "--exact", "--ignored", "--test-threads=1"])
                .env(STARTUP_FIXTURE_ENV, &run)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(0x0800_0000)
                .spawn()
                .unwrap(),
        );
        cleanup_job.assign(&owner.0).unwrap();
        owner.0.stdin.take().unwrap().write_all(&[1]).unwrap();
        let receipt = run.join("spawn.json");
        let deadline = Instant::now() + Duration::from_secs(105);
        while !receipt.exists() {
            assert!(
                owner.0.try_wait().unwrap().is_none(),
                "fixture exited before spawn"
            );
            assert!(Instant::now() < deadline, "controller spawn timed out");
            thread::sleep(Duration::from_millis(20));
        }
        let ready: serde_json::Value = serde_json::from_slice(&fs::read(receipt).unwrap()).unwrap();
        assert_eq!(ready["owner_pid"].as_u64(), Some(u64::from(owner.0.id())));
        let controller =
            duplicate_from_owner(&owner.0, ready["controller_handle"].as_u64().unwrap());
        let stdout = duplicate_from_owner(&owner.0, ready["stdout_handle"].as_u64().unwrap());
        let writer = retain_writer
            .then(|| duplicate_from_owner(&owner.0, ready["stdin_handle"].as_u64().unwrap()));
        let controller_handle = controller.as_raw_handle() as HANDLE;
        assert!(cleanup_job.contains_process(controller_handle));
        let controller_pid = unsafe { GetProcessId(controller_handle) };
        assert_ne!(controller_pid, 0);
        let kinds = cleanup_job
            .member_basename_kinds(&[(owner.0.id(), "test_owner"), (controller_pid, "controller")])
            .unwrap();
        assert!(kinds.contains(&"test_owner") && kinds.contains(&"controller"));
        // Windows may create console hosts even when these processes have no visible console.
        assert!(kinds
            .iter()
            .all(|kind| matches!(*kind, "test_owner" | "controller" | "conhost.exe")));
        let total_before = u32::try_from(kinds.len()).unwrap();
        assert_eq!(
            startup_fixture_processes(&cleanup_job),
            (total_before, total_before)
        );

        let owner_kill = Instant::now();
        owner.0.kill().unwrap();
        assert_eq!(
            unsafe { WaitForSingleObject(owner.0.as_raw_handle() as HANDLE, 5000) },
            WAIT_OBJECT_0
        );
        assert!(!owner.0.wait().unwrap().success());
        if retain_writer {
            assert_eq!(
                unsafe { WaitForSingleObject(controller_handle, 1000) },
                WAIT_TIMEOUT
            );
        }
        let eof = if retain_writer {
            Instant::now()
        } else {
            owner_kill
        };
        drop(writer);
        let remaining = CRASH_TIMEOUT.saturating_sub(eof.elapsed()).as_millis() as u32;
        assert_eq!(
            unsafe { WaitForSingleObject(controller_handle, remaining) },
            WAIT_OBJECT_0
        );
        let exit_after_eof = eof.elapsed();
        assert!(exit_after_eof <= CRASH_TIMEOUT);
        let mut exit_code = 0;
        assert_ne!(
            unsafe { GetExitCodeProcess(controller_handle, &mut exit_code) },
            0
        );
        assert_eq!(exit_code, 2);
        // Cumulative accounting also detects a helper that started and already exited.
        loop {
            let (total, active) = startup_fixture_processes(&cleanup_job);
            assert_eq!(
                total, total_before,
                "unexpected process started in the fixture Job"
            );
            if active == 0 {
                break;
            }
            assert!(eof.elapsed() < CRASH_TIMEOUT, "fixture Job did not empty");
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(
            File::from(stdout).read(&mut [0]).unwrap(),
            0,
            "unexpected controller frame"
        );
        cases.push(serde_json::json!({
            "retained_writer": retain_writer,
            "minimum_survival_with_writer_ms": retain_writer.then_some(1000),
            "controller_exit_observed_ms": exit_after_eof.as_millis(),
            "controller_exit_code": exit_code,
            "controller_stdout_bytes": 0,
            "outer_job_baseline_members": kinds,
            "outer_job_total_processes": total_before,
            "additional_processes_after_checkpoint": 0,
            "outer_job_remaining_processes": 0,
        }));
    }
    let result = serde_json::json!({"schema_version": 1, "passed": true, "cases": cases});
    fs::write(
        run_root.join("result.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    println!(
        "headroom_h006_startup_run={}",
        run_root.file_name().unwrap().to_string_lossy()
    );
}

fn hold_packaged_controller(run: &Path, resources: &Path) {
    let layout = RuntimeLayout::from_bundle(resources.to_owned(), run.join("Çökme state"))
        .expect("isolated staged runtime layout");
    let session = ControllerSession::launch(
        &layout,
        &Uuid::new_v4().to_string(),
        &AtomicBool::new(false),
    )
    .unwrap_or_else(|error| {
        let diagnostic = startup_diagnostic_for_test();
        fs::write(
            run.join("start-failure.json"),
            serde_json::to_vec(&serde_json::json!({
                "error_code": error.code(),
                "stage": diagnostic.stage,
            }))
            .unwrap(),
        )
        .unwrap();
        panic!(
            "packaged controller start failed: {}; stage={}",
            error.code(),
            diagnostic.stage
        )
    });
    assert!(session.backend_is_alive());
    let job = session.job.as_ref().unwrap().lock().unwrap();
    assert!(job.active_processes().is_some_and(|count| count >= 3));
    assert!(session
        .backend_lease
        .processes
        .iter()
        .all(|process| job.contains_process(process.0)));
    assert!(!job.contains_process(unsafe { GetCurrentProcess() }));
    let mut handle_flags = 0;
    assert_ne!(unsafe { GetHandleInformation(job.0, &mut handle_flags) }, 0);
    assert_eq!(handle_flags & HANDLE_FLAG_INHERIT, 0);
    let receipt = serde_json::json!({
        "schema_version": 1,
        "owner_pid": std::process::id(),
        "process_ids": [session.controller_pid, session.helper_pid, session.engine_pid],
        "engine_port": session.listener_port,
        "job_handle": job.0 as usize as u64,
        "stdin_handle": session.stdin.as_raw_handle() as usize as u64,
        "job_active_processes": job.active_processes(),
        "job_non_inheritable": true,
        "owned_chain_and_job_membership": true,
    });
    drop(job);
    fs::write(run.join("ready.tmp"), serde_json::to_vec(&receipt).unwrap()).unwrap();
    fs::rename(run.join("ready.tmp"), run.join("ready.json")).unwrap();
    // The observer kills this process without Rust Drop or a stop request.
    thread::sleep(Duration::from_secs(120));
    drop(session);
    panic!("observer did not end the isolated owner fixture");
}

#[test]
#[ignore = "opt-in Windows owner crash; real staged engine, loopback readiness only, no provider API"]
fn packaged_controller_job_closes_after_owner_crash() {
    let project =
        fs::canonicalize(PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap()).unwrap();
    let resources = project.join("work/build/windows/h006-crash-resources");
    let runs_root = project.join("work/proofs/windows-engine-crash/runs");
    fs::create_dir_all(&runs_root).unwrap();
    let runs_root = fs::canonicalize(runs_root).unwrap();
    if let Some(run) = std::env::var_os(FIXTURE_ENV) {
        let run = fs::canonicalize(run).unwrap();
        assert_eq!(
            run.parent().and_then(Path::parent),
            Some(runs_root.as_path())
        );
        assert!(run.starts_with(&runs_root));
        hold_packaged_controller(&run, &resources);
        unreachable!();
    }

    let run_root = runs_root.join(Uuid::new_v4().to_string());
    fs::create_dir(&run_root).unwrap();
    let foreign = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let foreign_address = foreign.local_addr().unwrap();
    let mut cases = Vec::new();
    for keep_job_open in [false, true] {
        let run = run_root.join(if keep_job_open {
            "retained-job"
        } else {
            "owner-only"
        });
        fs::create_dir(&run).unwrap();
        let mut owner = OwnerChild(
            Command::new(std::env::current_exe().unwrap())
                .args([TEST_NAME, "--exact", "--ignored", "--test-threads=1"])
                .env(FIXTURE_ENV, &run)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(105);
        while !run.join("ready.json").exists() {
            assert!(
                owner.0.try_wait().unwrap().is_none(),
                "owner exited before readiness"
            );
            assert!(
                Instant::now() < deadline,
                "packaged fixture readiness timed out"
            );
            thread::sleep(Duration::from_millis(20));
        }
        let ready: serde_json::Value =
            serde_json::from_slice(&fs::read(run.join("ready.json")).unwrap()).unwrap();
        assert_eq!(ready["owner_pid"].as_u64(), Some(u64::from(owner.0.id())));
        let pids: Vec<u32> = ready["process_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pid| u32::try_from(pid.as_u64().unwrap()).unwrap())
            .collect();
        assert_eq!(pids.len(), 3);
        assert!(owned_process_chain(pids[0], pids[1], pids[2]));
        let processes: Vec<_> = pids
            .iter()
            .map(|pid| OwnedProcessHandle::open(*pid).unwrap())
            .collect();
        assert!(processes.iter().all(OwnedProcessHandle::is_live));
        let port = u16::try_from(ready["engine_port"].as_u64().unwrap()).unwrap();
        assert_eq!(listener_owner(port), Some(pids[2]));
        assert!(probe_ready(port, EXPECTED_ENGINE_VERSION));

        // Holding stdin prevents EOF-driven graceful shutdown from masking a broken Job.
        let control_pipe = duplicate_from_owner(&owner.0, ready["stdin_handle"].as_u64().unwrap());
        let retained_job = keep_job_open
            .then(|| duplicate_from_owner(&owner.0, ready["job_handle"].as_u64().unwrap()));
        let owner_killed_at = Instant::now();
        owner.0.kill().expect("terminate only the owner fixture");
        let deadline = owner_killed_at + CRASH_TIMEOUT;
        let owner_exit = loop {
            if let Some(status) = owner.0.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "owner termination timed out");
            thread::sleep(Duration::from_millis(10));
        };
        assert!(!owner_exit.success());

        if keep_job_open {
            let deadline = Instant::now() + Duration::from_secs(1);
            while Instant::now() < deadline {
                assert!(processes.iter().all(OwnedProcessHandle::is_live));
                thread::sleep(Duration::from_millis(20));
            }
            assert_eq!(listener_owner(port), Some(pids[2]));
            assert!(probe_ready(port, EXPECTED_ENGINE_VERSION));
        }
        let closed_at = if keep_job_open {
            Instant::now()
        } else {
            owner_killed_at
        };
        drop(retained_job);
        let deadline = closed_at + CRASH_TIMEOUT;
        while processes.iter().any(OwnedProcessHandle::is_live) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        let all_exited = processes.iter().all(|process| !process.is_live());
        let exit_millis = closed_at.elapsed().as_millis();
        assert!(
            exit_millis <= CRASH_TIMEOUT.as_millis(),
            "process exit deadline exceeded"
        );
        let port_closed = owned_listener_closed(
            port,
            pids[2],
            deadline.saturating_duration_since(Instant::now()),
        ) && listener_owner(port).is_none()
            && TcpStream::connect_timeout(
                &SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port),
                Duration::from_millis(300),
            )
            .is_err();
        let port_closed_millis = closed_at.elapsed().as_millis();
        drop(control_pipe);
        assert!(all_exited, "owned process survived last Job handle close");
        assert!(port_closed, "owned engine listener survived the crash");
        assert!(
            port_closed_millis <= CRASH_TIMEOUT.as_millis(),
            "listener exit deadline exceeded"
        );
        let mut foreign_client = TcpStream::connect(foreign_address).unwrap();
        foreign_client.write_all(b"foreign-alive").unwrap();
        let (mut accepted, _) = foreign.accept().unwrap();
        accepted
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut bytes = [0; 13];
        accepted.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"foreign-alive");
        cases.push(serde_json::json!({
            "retained_job_control": keep_job_open,
            "retained_job_minimum_survival_ms": keep_job_open.then_some(1000),
            "owner_exit_code": owner_exit.code(),
            "core_processes_exited": all_exited,
            "trigger": if keep_job_open { "observer_closes_last_job_handle" } else { "owner_terminate_request" },
            "core_exit_observation_ms_from_trigger": exit_millis,
            "engine_port_closed": port_closed,
            "port_closed_observation_ms_from_trigger": port_closed_millis,
            "foreign_listener_preserved": true,
            "control_pipe_held_through_observation": true,
            "explicit_stop_or_terminate_job_call": false,
        }));
    }
    let result = serde_json::json!({"schema_version": 1, "passed": true, "cases": cases});
    fs::write(
        run_root.join("result.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    println!(
        "headroom_h006_crash_run={}",
        run_root.file_name().unwrap().to_string_lossy()
    );
}
