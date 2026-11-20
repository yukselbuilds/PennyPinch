use super::lifecycle::{self, Lifecycle};
use crate::runtime::manager::RuntimeManager;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
use uuid::Uuid;

const ID: &str = "headroom-main";
const OPEN: &str = "headroom-open";
const PRIMARY: &str = "headroom-primary";
const EXIT: &str = "headroom-exit";

#[derive(Clone, Debug, PartialEq)]
enum Action {
    Open,
    Start,
    Set {
        generation: String,
        revision: u64,
        enabled: bool,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct View {
    status: String,
    label: String,
    enabled: bool,
    action: Action,
}

fn project(value: &Value, english: bool, busy: bool) -> View {
    let engine = value["engine"]["status"].as_str().unwrap_or("unknown");
    let mode = value["compression"]["status"].as_str().unwrap_or("unknown");
    let state = match (engine, mode, english) {
        ("ready", "enabled", false) => "Sıkıştırma açık",
        ("ready", "enabled", true) => "Enabled",
        ("ready", "paused", false) => "Duraklatıldı",
        ("ready", "paused", true) => "Paused",
        ("stopped", _, false) => "Durduruldu",
        ("stopped", _, true) => "Stopped",
        ("starting", _, false) => "Başlatılıyor",
        ("starting", _, true) => "Starting",
        ("stopping", _, false) => "Durduruluyor",
        ("stopping", _, true) => "Stopping",
        ("failed", _, false) => "Hata",
        ("failed", _, true) => "Error",
        ("unavailable", _, false) => "Kullanılamıyor",
        ("unavailable", _, true) => "Unavailable",
        (_, _, false) => "Bilinmiyor",
        (_, _, true) => "Unknown",
    };
    let mut action = Action::Open;
    let mut label = if english {
        "Open PennyPinch"
    } else {
        "PennyPinch’i aç"
    };
    if matches!(engine, "stopped" | "failed") && value["capabilities"]["start"] == true {
        action = Action::Start;
        label = if english { "Start" } else { "Başlat" };
    } else if engine == "ready" && value["capabilities"]["pause"] == true {
        if let (Some(generation), Some(revision)) = (
            value["runtime_generation"].as_str(),
            value["compression"]["revision"].as_u64(),
        ) {
            if matches!(mode, "paused" | "enabled") {
                let enabled = mode == "paused";
                action = Action::Set {
                    generation: generation.to_owned(),
                    revision,
                    enabled,
                };
                label = match (enabled, english) {
                    (true, true) => "Resume",
                    (true, false) => "Devam et",
                    (false, true) => "Pause",
                    (false, false) => "Duraklat",
                };
            }
        }
    }
    View {
        status: format!("{}: {state}", if english { "Engine" } else { "Motor" }),
        label: label.to_owned(),
        enabled: !busy && !matches!(engine, "starting" | "stopping"),
        action,
    }
}

struct Items {
    open: MenuItem<tauri::Wry>,
    status: MenuItem<tauri::Wry>,
    primary: MenuItem<tauri::Wry>,
    exit: MenuItem<tauri::Wry>,
}
struct Inner {
    items: Option<Items>,
    english: AtomicBool,
    stop: AtomicBool,
    busy: AtomicBool,
    shown: Mutex<Action>,
    worker: Mutex<Option<JoinHandle<()>>>,
}
#[derive(Clone)]
pub(crate) struct TrayController(Arc<Inner>);

impl TrayController {
    pub(crate) fn install(app: &tauri::AppHandle, language: &str) -> Self {
        let built = (|| -> tauri::Result<Items> {
            let open = MenuItem::with_id(app, OPEN, "PennyPinch", true, None::<&str>)?;
            let status = MenuItem::with_id(app, "headroom-status", "Motor", false, None::<&str>)?;
            let primary = MenuItem::with_id(app, PRIMARY, "PennyPinch", false, None::<&str>)?;
            let exit = MenuItem::with_id(app, EXIT, "PennyPinch", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&open, &status, &primary, &separator, &exit])?;
            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or_else(|| std::io::Error::other("PennyPinch icon unavailable"))?;
            TrayIconBuilder::with_id(ID)
                .icon(icon)
                .tooltip("PennyPinch")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    OPEN => lifecycle::show(app),
                    EXIT => lifecycle::request_exit(app),
                    PRIMARY => {
                        if let Some(tray) = app.try_state::<TrayController>() {
                            tray.primary(app);
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if matches!(
                        event,
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                    ) {
                        lifecycle::show(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(Items {
                open,
                status,
                primary,
                exit,
            })
        })();
        let available = built.is_ok();
        let result = Self(Arc::new(Inner {
            items: built.ok(),
            english: AtomicBool::new(language == "en"),
            stop: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            shown: Mutex::new(Action::Open),
            worker: Mutex::new(None),
        }));
        app.state::<Lifecycle>().set_tray_available(available);
        if available {
            let worker_app = app.clone();
            let owned = result.clone();
            let worker = thread::Builder::new()
                .name("headroom-tray-state".to_owned())
                .spawn(move || {
                    let app = worker_app;
                    let mut previous = None;
                    while !owned.0.stop.load(Ordering::Acquire) {
                        let value = serde_json::to_value(
                            app.state::<RuntimeManager>()
                                .get_runtime_snapshot(Uuid::new_v4().to_string()),
                        )
                        .unwrap_or(Value::Null);
                        let english = owned.0.english.load(Ordering::Acquire);
                        let exiting = app.state::<Lifecycle>().is_exiting();
                        let view = project(
                            &value,
                            english,
                            exiting || owned.0.busy.load(Ordering::Acquire),
                        );
                        if previous.as_ref() != Some(&(view.clone(), english)) {
                            previous = Some((view.clone(), english));
                            let state = owned.clone();
                            let handle = app.clone();
                            let _ = app
                                .run_on_main_thread(move || state.update(&handle, view, english));
                        }
                        for _ in 0..5 {
                            if owned.0.stop.load(Ordering::Acquire) {
                                break;
                            }
                            thread::sleep(Duration::from_millis(100));
                        }
                    }
                });
            if let Ok(worker) = worker {
                *result.0.worker.lock().unwrap_or_else(|e| e.into_inner()) = Some(worker);
            } else {
                app.state::<Lifecycle>().set_tray_available(false);
                if let Some(icon) = app.tray_by_id(ID) {
                    let _ = icon.set_visible(false);
                }
            }
        }
        result
    }
    fn update(&self, app: &tauri::AppHandle, view: View, english: bool) {
        if self.0.stop.load(Ordering::Acquire) {
            return;
        }
        if let Some(items) = &self.0.items {
            let _ = items.open.set_text(if english {
                "Open PennyPinch"
            } else {
                "PennyPinch’i aç"
            });
            let _ = items.exit.set_text(if english {
                "Quit…"
            } else {
                "Tamamen çık…"
            });
            let _ = items.status.set_text(&view.status);
            let _ = items.primary.set_text(&view.label);
            let _ = items.primary.set_enabled(view.enabled);
            *self.0.shown.lock().unwrap_or_else(|e| e.into_inner()) = view.action;
            if let Some(icon) = app.tray_by_id(ID) {
                let _ = icon.set_tooltip(Some(format!("PennyPinch — {}", view.status)));
            }
        }
    }
    pub(crate) fn set_language(&self, language: &str) {
        self.0.english.store(language == "en", Ordering::Release);
    }
    fn primary(&self, app: &tauri::AppHandle) {
        if app.state::<Lifecycle>().is_exiting()
            || self
                .0
                .busy
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return;
        }
        let action = self
            .0
            .shown
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if action == Action::Open {
            self.0.busy.store(false, Ordering::Release);
            lifecycle::show(app);
            return;
        }
        let app = app.clone();
        let owned = self.clone();
        thread::spawn(move || {
            let manager = app.state::<RuntimeManager>();
            let reply = match action {
                Action::Start => manager.start_engine(Uuid::new_v4().to_string()),
                Action::Set {
                    generation,
                    revision,
                    enabled,
                } => manager.set_optimization(
                    Uuid::new_v4().to_string(),
                    generation,
                    revision,
                    enabled,
                ),
                Action::Open => unreachable!(),
            };
            owned.0.busy.store(false, Ordering::Release);
            if matches!(reply, crate::runtime::manager::RuntimeReply::Error(_))
                && !app.state::<Lifecycle>().is_exiting()
            {
                lifecycle::show(&app);
            }
        });
    }
    pub(crate) fn shutdown(&self, app: &tauri::AppHandle) -> bool {
        self.0.stop.store(true, Ordering::Release);
        let worker = self
            .0
            .worker
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let deadline = Instant::now() + Duration::from_secs(2);
        if let Some(worker) = worker {
            while !worker.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            if !worker.is_finished() {
                return false;
            }
            if worker.join().is_err() {
                return false;
            }
        }
        while self.0.busy.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        !self.0.busy.load(Ordering::Acquire)
            && app
                .tray_by_id(ID)
                .is_none_or(|icon| icon.set_visible(false).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn tray_never_calls_unknown_or_relabels_pause_as_resume_before_ack() {
        let ready = json!({"engine":{"status":"ready"},"compression":{"status":"enabled","revision":3},"runtime_generation":"generation","capabilities":{"start":false,"pause":true}});
        let view = project(&ready, false, false);
        assert_eq!(view.label, "Duraklat");
        assert_eq!(
            view.action,
            Action::Set {
                generation: "generation".into(),
                revision: 3,
                enabled: false
            }
        );
        assert!(!project(&ready, false, true).enabled);
        let mut stale_control = ready.clone();
        stale_control["capabilities"]["pause"] = Value::Bool(false);
        assert_eq!(project(&stale_control, true, false).action, Action::Open);
        stale_control["compression"]["status"] = Value::String("paused".into());
        assert_eq!(project(&stale_control, true, false).action, Action::Open);
        let unknown = json!({"engine":{"status":"unknown"},"compression":{"status":"enabled","revision":3,"can_pause":true}});
        assert_eq!(project(&unknown, true, false).action, Action::Open);
        assert!(project(&unknown, true, false).status.contains("Unknown"));
        let blocked = json!({"engine":{"status":"failed"},"capabilities":{"start":false}});
        assert_eq!(project(&blocked, false, false).action, Action::Open);
        let stopped = json!({"engine":{"status":"stopped"},"capabilities":{"start":true}});
        assert_eq!(project(&stopped, true, false).action, Action::Start);
    }
}
