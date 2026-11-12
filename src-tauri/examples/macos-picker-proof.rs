#[cfg(target_os = "macos")]
#[path = "../src/integrations/picker_macos.rs"]
mod picker;

#[cfg(target_os = "macos")]
mod proof {
    use super::picker;
    use block2::RcBlock;
    use core_foundation::base::{CFType, CFTypeRef, TCFType};
    use objc2::{rc::Retained, MainThreadMarker};
    use objc2_app_kit::{
        NSApplication, NSApplicationActivationPolicy, NSEventModifierFlags,
        NSModalPanelRunLoopMode, NSOpenPanel, NSPasteboard, NSPasteboardTypeString,
    };
    use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSRunLoop, NSString, NSTimer};
    use sha2::{Digest, Sha256};
    use std::cell::Cell;
    use std::os::unix::fs::MetadataExt;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightPostEventAccess() -> bool;
        fn CGEventCreateKeyboardEvent(source: CFTypeRef, key: u16, key_down: bool) -> CFTypeRef;
        fn CGEventSetFlags(event: CFTypeRef, flags: u64);
        fn CGEventPostToPid(pid: i32, event: CFTypeRef);
    }

    const KEY_G: u16 = 5;
    const KEY_V: u16 = 9;
    const KEY_RETURN: u16 = 36;

    struct Driver {
        timer: Retained<NSTimer>,
        timed_out: Rc<Cell<bool>>,
        saw_panel: Rc<Cell<bool>>,
    }

    impl Driver {
        fn new(mtm: MainThreadMarker, selection: Option<PathBuf>) -> Self {
            let start = Instant::now();
            let timed_out = Rc::new(Cell::new(false));
            let saw_panel = Rc::new(Cell::new(false));
            let phase = Cell::new(0);
            let timeout_result = timed_out.clone();
            let panel_result = saw_panel.clone();
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
                // Send user-equivalent keys to this process's own panel.
                // The selected URL is read only by the production picker.
                let sent = match phase.get() {
                    0 => key(
                        &app,
                        KEY_G,
                        NSEventModifierFlags::Command | NSEventModifierFlags::Shift,
                    ),
                    2 => {
                        let pasteboard = NSPasteboard::generalPasteboard();
                        pasteboard.clearContents();
                        pasteboard.setString_forType(
                            &NSString::from_str(&path.to_string_lossy()),
                            unsafe { NSPasteboardTypeString },
                        ) && key(&app, KEY_V, NSEventModifierFlags::Command)
                    }
                    4 | 6 | 8 => key(&app, KEY_RETURN, NSEventModifierFlags::empty()),
                    _ => true,
                };
                if !sent {
                    timeout_result.set(true);
                    unsafe { panel.cancel(None) };
                }
                if matches!(phase.get(), 0 | 2 | 4 | 6 | 8) {
                    println!("headroom-picker-event={}", phase.get());
                }
                phase.set(phase.get() + 1);
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
            }
        }

        fn verify(self) {
            self.timer.invalidate();
            assert!(self.saw_panel.get(), "native_panel_not_observed");
            assert!(!self.timed_out.get(), "native_panel_input_did_not_complete");
        }
    }

    fn key(app: &NSApplication, code: u16, flags: NSEventModifierFlags) -> bool {
        if app.keyWindow().is_none() || !unsafe { CGPreflightPostEventAccess() } {
            return false;
        }
        let modifiers = [
            (NSEventModifierFlags::Command, 55),
            (NSEventModifierFlags::Shift, 56),
        ];
        let mut pressed = NSEventModifierFlags::empty();
        let mut sent = true;
        for (modifier, key) in modifiers {
            if flags.contains(modifier) {
                pressed.insert(modifier);
                sent &= post_key(key, true, pressed);
            }
        }
        for down in [true, false] {
            sent &= post_key(code, down, pressed);
        }
        for (modifier, key) in modifiers.into_iter().rev() {
            if flags.contains(modifier) {
                pressed.remove(modifier);
                sent &= post_key(key, false, pressed);
            }
        }
        sent
    }

    fn post_key(code: u16, down: bool, flags: NSEventModifierFlags) -> bool {
        // CoreGraphics owns the new event. Route it only to this test process.
        let raw = unsafe { CGEventCreateKeyboardEvent(std::ptr::null(), code, down) };
        if raw.is_null() {
            return false;
        }
        let event = unsafe { CFType::wrap_under_create_rule(raw) };
        unsafe {
            CGEventSetFlags(event.as_CFTypeRef(), flags.bits() as u64);
            CGEventPostToPid(std::process::id() as i32, event.as_CFTypeRef());
        }
        true
    }

    pub fn run() {
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_secs(60));
            eprintln!("headroom-picker-timeout");
            std::process::exit(70);
        });
        let mtm = MainThreadMarker::new().expect("picker proof must start on main thread");
        let root = PathBuf::from(std::env::var_os("HEADROOM_MACOS_PICKER_PROOF_ROOT").unwrap());
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
        println!("headroom-picker-post-event-access={}", unsafe {
            CGPreflightPostEventAccess()
        });
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
        assert_eq!(picker::select_file(1, "codex"), Ok(Some(command.clone())));
        select.verify();
        let after = Sha256::digest(std::fs::read(&command).unwrap());
        assert_eq!(before, after);
        println!("headroom-picker-input=cli before={before:x} after={after:x}");
        println!("headroom-picker-case=file status=pass");

        let project = root.join("Claude 项目");
        std::fs::create_dir(&project).unwrap();
        let select = Driver::new(mtm, Some(project.clone()));
        let initial = Some(root.clone());
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
        assert_eq!(selected, Ok(Some(project.clone())));
        select.verify();
        assert!(!project.join(".claude").exists());
        println!("headroom-picker-case=folder_background_dispatch status=pass");
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    proof::run();
    #[cfg(not(target_os = "macos"))]
    eprintln!("macOS-only native picker proof");
}
