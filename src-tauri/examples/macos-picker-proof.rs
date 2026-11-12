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
    };
    use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSRunLoop, NSTimer};
    use sha2::{Digest, Sha256};
    use std::cell::Cell;
    use std::io::Write;
    use std::os::unix::fs::MetadataExt;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    struct CancelProbe {
        timer: Retained<NSTimer>,
        observed_modes: Rc<Cell<Option<(bool, bool, bool)>>>,
    }

    impl CancelProbe {
        fn new(mtm: MainThreadMarker) -> Self {
            let observed_modes = Rc::new(Cell::new(None));
            let observed = observed_modes.clone();
            let cancel = RcBlock::new(move |_: std::ptr::NonNull<NSTimer>| {
                let app = NSApplication::sharedApplication(mtm);
                let Some(window) = app.modalWindow() else {
                    return;
                };
                let Some(panel) = window.downcast_ref::<NSOpenPanel>() else {
                    return;
                };
                if !window.isVisible() {
                    return;
                }
                observed.set(Some((
                    panel.canChooseFiles(),
                    panel.canChooseDirectories(),
                    panel.allowsMultipleSelection(),
                )));
                unsafe { panel.cancel(None) };
            });
            let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(0.5, true, &cancel) };
            let run_loop = NSRunLoop::mainRunLoop();
            unsafe {
                run_loop.addTimer_forMode(&timer, NSModalPanelRunLoopMode);
                run_loop.addTimer_forMode(&timer, NSDefaultRunLoopMode);
            }
            Self {
                timer,
                observed_modes,
            }
        }

        fn verify(self, directory: bool) {
            self.timer.invalidate();
            assert_eq!(
                self.observed_modes.get(),
                Some((!directory, directory, false))
            );
        }
    }

    pub fn run() {
        assert_eq!(std::env::var("GITHUB_ACTIONS").as_deref(), Ok("true"));
        assert_eq!(std::env::var("RUNNER_OS").as_deref(), Ok("macOS"));
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_secs(60));
            eprintln!("headroom-picker-timeout");
            std::process::exit(70);
        });
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

        let mtm = MainThreadMarker::new().expect("picker proof must start on main thread");
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        app.finishLaunching();
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        assert!(picker::select_file(0, "codex").is_err());
        assert!(picker::select_file(1, "unknown").is_err());

        let file = CancelProbe::new(mtm);
        assert_eq!(picker::select_file(1, "codex"), Ok(None));
        file.verify(false);
        println!("headroom-picker-case=file_cancel status=pass");

        let project = root.join("Claude 项目");
        std::fs::create_dir(&project).unwrap();
        let folder = CancelProbe::new(mtm);
        let initial = Some(project.clone());
        let (send, receive) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _ = send.send(picker::select_project_directory(1, initial));
        });
        let deadline = Instant::now() + Duration::from_secs(25);
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
        assert_eq!(selected, Ok(None));
        folder.verify(true);
        assert!(std::fs::read_dir(&project).unwrap().next().is_none());
        println!("headroom-picker-case=folder_background_dispatch_cancel status=pass");

        let command = root.join("CLI 工具/codex");
        std::fs::create_dir(command.parent().unwrap()).unwrap();
        let source = PathBuf::from(std::env::args_os().nth(2).expect("verified CLI fixture"));
        assert_eq!(source.canonicalize().unwrap(), source);
        assert!(source.starts_with(workspace.join("work")));
        assert_eq!(source.file_name().unwrap(), "codex");
        std::fs::copy(&source, &command).unwrap();
        let before = Sha256::digest(std::fs::read(&command).unwrap());
        println!("headroom-picker-ready=file");
        assert_eq!(picker::select_file(1, "codex"), Ok(Some(command.clone())));
        let after = Sha256::digest(std::fs::read(&command).unwrap());
        assert_eq!(before, after);
        println!("headroom-picker-input=cli before={before:x} after={after:x}");
        println!("headroom-picker-case=file status=pass");

        println!("headroom-picker-ready=folder");
        let initial = project.parent().map(std::path::Path::to_owned);
        let (send, receive) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let _ = send.send(picker::select_project_directory(1, initial));
        });
        let deadline = Instant::now() + Duration::from_secs(25);
        let selected = loop {
            if let Ok(value) = receive.try_recv() {
                break value;
            }
            assert!(Instant::now() < deadline, "folder_selection_timeout");
            NSRunLoop::mainRunLoop().runMode_beforeDate(
                unsafe { NSDefaultRunLoopMode },
                &NSDate::dateWithTimeIntervalSinceNow(0.05),
            );
        };
        worker.join().unwrap();
        assert_eq!(selected, Ok(Some(project.clone())));
        assert!(std::fs::read_dir(&project).unwrap().next().is_none());
        println!("headroom-picker-case=folder_background_dispatch status=pass");
        let selection = serde_json::json!({"cli_file": command, "project_directory": project});
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("selections.json"))
            .unwrap();
        file.write_all(&serde_json::to_vec(&selection).unwrap())
            .unwrap();
        file.sync_all().unwrap();
        println!("headroom-picker-proof=pass");
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    proof::run();
    #[cfg(not(target_os = "macos"))]
    eprintln!("macOS-only native picker proof");
}
