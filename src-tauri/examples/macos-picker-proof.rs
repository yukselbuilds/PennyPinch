#[cfg(target_os = "macos")]
#[path = "../src/integrations/picker_macos.rs"]
mod picker;

#[cfg(target_os = "macos")]
mod proof {
    use super::picker;
    use block2::RcBlock;
    use objc2::{rc::Retained, MainThreadMarker};
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSModalPanelRunLoopMode, NSOpenPanel,
        NSRunningApplication, NSWorkspace,
    };
    use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSRunLoop, NSString, NSTimer, NSURL};
    use sha2::{Digest, Sha256};
    use std::cell::{Cell, RefCell};
    use std::os::unix::fs::MetadataExt;
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::rc::Rc;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    struct Driver {
        timer: Retained<NSTimer>,
        timed_out: Rc<Cell<bool>>,
        saw_panel: Rc<Cell<bool>>,
        script: Rc<RefCell<Option<Child>>>,
        script_ok: Rc<Cell<bool>>,
    }

    impl Driver {
        fn new(mtm: MainThreadMarker, selection: Option<PathBuf>) -> Self {
            let start = Instant::now();
            let timed_out = Rc::new(Cell::new(false));
            let saw_panel = Rc::new(Cell::new(false));
            let script: Rc<RefCell<Option<Child>>> = Rc::new(RefCell::new(None));
            let script_ok = Rc::new(Cell::new(selection.is_none()));
            let script_started = Cell::new(false);
            let directory_set = Cell::new(false);
            let timeout_result = timed_out.clone();
            let panel_result = saw_panel.clone();
            let child = script.clone();
            let child_result = script_ok.clone();
            let proof_app = NSRunningApplication::currentApplication();
            let block = RcBlock::new(move |_: std::ptr::NonNull<NSTimer>| {
                let app = NSApplication::sharedApplication(mtm);
                let Some(window) = app.modalWindow() else {
                    return;
                };
                let Some(panel) = window.downcast_ref::<NSOpenPanel>() else {
                    return;
                };
                panel_result.set(true);
                if start.elapsed() > Duration::from_secs(20) {
                    timeout_result.set(true);
                    unsafe { panel.cancel(None) };
                    return;
                }
                let Some(path) = selection.as_ref() else {
                    unsafe { panel.cancel(None) };
                    return;
                };
                if script_started.get() {
                    let finished = child.borrow_mut().as_mut().is_some_and(|process| {
                        process.try_wait().is_ok_and(|status| status.is_some())
                    });
                    if finished {
                        let ok = script_result(child.borrow_mut().take().unwrap());
                        child_result.set(ok);
                        if !ok {
                            unsafe { panel.cancel(None) };
                        }
                    }
                    return;
                }
                if !app.isActive()
                    || !window.isVisible()
                    || app.keyWindow().is_none()
                    || !NSWorkspace::sharedWorkspace()
                        .frontmostApplication()
                        .is_some_and(|front| front == proof_app)
                {
                    return;
                }
                let directory = path.parent().unwrap().to_str().unwrap();
                if !directory_set.replace(true) {
                    let url =
                        NSURL::fileURLWithPath_isDirectory(&NSString::from_str(directory), true);
                    panel.setDirectoryURL(Some(&url));
                    return;
                }
                let observed_directory = panel.directoryURL().and_then(|url| url.path());
                if !observed_directory
                    .is_some_and(|observed| observed.to_string().trim_end_matches('/') == directory)
                {
                    return;
                }
                println!("headroom-picker-fixture-directory=visible");
                script_started.set(true);
                let input_script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("examples/macos-picker-input.applescript");
                match Command::new("/usr/bin/osascript")
                    .arg(input_script)
                    .arg(std::process::id().to_string())
                    .arg(path.file_name().unwrap())
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                {
                    Ok(process) => *child.borrow_mut() = Some(process),
                    Err(_) => {
                        println!("headroom-picker-system-events=spawn_failed");
                        unsafe { panel.cancel(None) };
                    }
                }
            });
            let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(0.5, true, &block) };
            let run_loop = NSRunLoop::mainRunLoop();
            unsafe {
                run_loop.addTimer_forMode(&timer, NSModalPanelRunLoopMode);
                run_loop.addTimer_forMode(&timer, NSDefaultRunLoopMode);
            }
            Self {
                timer,
                timed_out,
                saw_panel,
                script,
                script_ok,
            }
        }

        fn verify(self) {
            self.timer.invalidate();
            if let Some(mut child) = self.script.borrow_mut().take() {
                if self.timed_out.get() {
                    let _ = child.kill();
                }
                self.script_ok.set(script_result(child));
            }
            assert!(self.saw_panel.get(), "native_panel_not_observed");
            assert!(!self.timed_out.get(), "native_panel_input_did_not_complete");
            assert!(self.script_ok.get(), "native_panel_input_failed");
        }
    }

    fn script_result(child: Child) -> bool {
        let Ok(output) = child.wait_with_output() else {
            println!("headroom-picker-system-events=wait_failed");
            return false;
        };
        let result = String::from_utf8_lossy(&output.stdout);
        let result = result.trim();
        let known = result.len() <= 48
            && !result.is_empty()
            && result.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
            });
        if known {
            println!("headroom-picker-system-events={result}");
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let code = stderr
                .trim()
                .rsplit_once(" (")
                .and_then(|(_, code)| code.strip_suffix(')'))
                .and_then(|code| code.parse::<i32>().ok());
            println!(
                "headroom-picker-system-events=execution_failed code={}",
                code.unwrap_or_default()
            );
        }
        output.status.success() && result == "clicked_confirmation"
    }

    pub fn run() {
        assert_eq!(std::env::var("GITHUB_ACTIONS").as_deref(), Ok("true"));
        assert_eq!(std::env::var("RUNNER_OS").as_deref(), Ok("macOS"));
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_secs(60));
            eprintln!("headroom-picker-timeout");
            std::process::exit(70);
        });
        let mtm = MainThreadMarker::new().expect("picker proof must start on main thread");
        let root = PathBuf::from(std::env::args_os().nth(1).expect("picker proof directory"));
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .canonicalize()
            .unwrap();
        assert_eq!(root.canonicalize().unwrap(), root);
        assert!(root.starts_with(workspace.join("work")) && root != workspace.join("work"));
        let info = std::fs::symlink_metadata(&root).unwrap();
        assert_eq!(info.uid(), unsafe { libc::geteuid() });
        assert_eq!(info.mode() & 0o7777, 0o700);
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        app.finishLaunching();
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        assert!(picker::select_file(0, "codex").is_err());
        assert!(picker::select_file(1, "unknown").is_err());
        let cancel = Driver::new(mtm, None);
        assert_eq!(picker::select_file(1, "codex"), Ok(None));
        cancel.verify();
        println!("headroom-picker-case=cancel status=pass");

        let command = root.join("CLI 工具/codex");
        std::fs::create_dir(command.parent().unwrap()).unwrap();
        std::fs::write(&command, b"owned nonexecuted picker fixture").unwrap();
        let before = Sha256::digest(std::fs::read(&command).unwrap());
        let select = Driver::new(mtm, Some(command.clone()));
        let selected = picker::select_file(1, "codex");
        select.verify();
        assert_eq!(selected, Ok(Some(command.clone())));
        let after = Sha256::digest(std::fs::read(&command).unwrap());
        assert_eq!(before, after);
        println!("headroom-picker-input=cli before={before:x} after={after:x}");
        println!("headroom-picker-case=file status=pass");

        let project = root.join("Project folders/Claude 项目");
        std::fs::create_dir_all(&project).unwrap();
        let select = Driver::new(mtm, Some(project.clone()));
        let initial = project.parent().map(std::path::Path::to_owned);
        let (send, receive) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _ = send.send(picker::select_project_directory(1, initial));
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        let selected = loop {
            if let Ok(value) = receive.try_recv() {
                break value;
            }
            assert!(Instant::now() < deadline, "main_thread_dispatch_timeout");
            NSRunLoop::mainRunLoop().runMode_beforeDate(
                unsafe { NSDefaultRunLoopMode },
                &NSDate::dateWithTimeIntervalSinceNow(0.05),
            );
        };
        worker.join().unwrap();
        select.verify();
        assert_eq!(selected, Ok(Some(project.clone())));
        assert!(!project.join(".claude").exists());
        println!("headroom-picker-case=folder_background_dispatch status=pass");
        println!("headroom-picker-proof=pass");
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    proof::run();
    #[cfg(not(target_os = "macos"))]
    eprintln!("macOS-only native picker proof");
}
