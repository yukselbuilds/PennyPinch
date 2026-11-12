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
        NSModalPanelRunLoopMode, NSOpenPanel, NSPasteboard, NSPasteboardTypeString, NSWorkspace,
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
        fn CGEventPost(tap: u32, event: CFTypeRef);
    }

    const KEY_G: u16 = 5;
    const KEY_V: u16 = 9;
    const KEY_RETURN: u16 = 36;
    const CG_SESSION_EVENT_TAP: u32 = 1;

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
            let previous_focus = Cell::new(None);
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
                    println!("headroom-picker-input-timeout phase={}", phase.get());
                    unsafe { panel.cancel(None) };
                    return;
                }
                let Some(path) = selection.as_ref() else {
                    unsafe { panel.cancel(None) };
                    return;
                };
                let focus = (
                    app.isActive(),
                    NSWorkspace::sharedWorkspace()
                        .frontmostApplication()
                        .is_some_and(|front| {
                            front.processIdentifier() == std::process::id() as i32
                        }),
                    window.isVisible(),
                    app.keyWindow().is_some(),
                );
                if previous_focus.replace(Some(focus)) != Some(focus) {
                    let (active, foreground, visible, key_window) = focus;
                    println!("headroom-picker-focus active={active} owned_foreground={foreground} visible={visible} key_window={key_window}");
                }
                if focus != (true, true, true, true) {
                    return;
                }
                // The native panel runs outside our process. The foreground guard
                // confines session input to this test app's active modal picker.
                let sent = match phase.get() {
                    0 => key(
                        KEY_G,
                        NSEventModifierFlags::Command | NSEventModifierFlags::Shift,
                    ),
                    2 => {
                        let pasteboard = NSPasteboard::generalPasteboard();
                        pasteboard.clearContents();
                        pasteboard.setString_forType(
                            &NSString::from_str(&path.to_string_lossy()),
                            unsafe { NSPasteboardTypeString },
                        ) && key(KEY_V, NSEventModifierFlags::Command)
                    }
                    4 | 6 | 8 => key(KEY_RETURN, NSEventModifierFlags::empty()),
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

    fn key(code: u16, flags: NSEventModifierFlags) -> bool {
        if !unsafe { CGPreflightPostEventAccess() } {
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
        let raw = unsafe { CGEventCreateKeyboardEvent(std::ptr::null(), code, down) };
        if raw.is_null() {
            return false;
        }
        let event = unsafe { CFType::wrap_under_create_rule(raw) };
        unsafe {
            CGEventSetFlags(event.as_CFTypeRef(), flags.bits() as u64);
            CGEventPost(CG_SESSION_EVENT_TAP, event.as_CFTypeRef());
        }
        true
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
        println!("headroom-picker-proof=pass");
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    proof::run();
    #[cfg(not(target_os = "macos"))]
    eprintln!("macOS-only native picker proof");
}
