use serde::Serialize;
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use uuid::Uuid;

pub(crate) const CHANGED: &str = "headroom:desktop-change";

#[derive(Default)]
pub(crate) struct Lifecycle(Mutex<State>);

#[derive(Default)]
struct State {
    tray_available: bool,
    confirm_on_close: bool,
    confirmation: Option<String>,
    exiting: bool,
}

#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum Reply {
    Snapshot {
        schema_version: u8,
        request_id: String,
        status: &'static str,
        tray_available: bool,
        close_to_tray: bool,
        exit_state: &'static str,
        exit_token: Option<String>,
    },
    Error {
        schema_version: u8,
        request_id: String,
        status: &'static str,
        error: Error,
    },
}

#[derive(Serialize)]
pub(crate) struct Error {
    code: &'static str,
    retryable: bool,
    message_key: String,
    diagnostic_id: Option<String>,
}

fn valid_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|id| !id.is_nil() && id.to_string() == value)
}

fn error(request_id: &str, code: &'static str) -> Reply {
    Reply::Error {
        schema_version: 1,
        request_id: if valid_uuid(request_id) {
            request_id.to_owned()
        } else {
            Uuid::nil().to_string()
        },
        status: "error",
        error: Error {
            code,
            retryable: code != "validation_failed",
            message_key: format!("desktop.{code}"),
            diagnostic_id: None,
        },
    }
}

impl State {
    fn reply(&self, request_id: &str) -> Reply {
        Reply::Snapshot {
            schema_version: 1,
            request_id: request_id.to_owned(),
            status: "snapshot",
            tray_available: self.tray_available,
            close_to_tray: !self.confirm_on_close,
            exit_state: if self.exiting {
                "shutting_down"
            } else if self.confirmation.is_some() {
                "confirmation_required"
            } else {
                "idle"
            },
            exit_token: self.confirmation.clone(),
        }
    }
}

impl Lifecycle {
    #[cfg(all(windows, any(test, not(debug_assertions))))]
    pub(crate) fn begin_update(&self, hold_actions: impl FnOnce() -> bool) -> bool {
        let mut state = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if state.exiting || state.confirmation.is_some() || !hold_actions() {
            return false;
        }
        state.exiting = true;
        true
    }

    pub(crate) fn set_close_to_tray(&self, enabled: bool) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .confirm_on_close = !enabled;
    }
    pub(crate) fn set_tray_available(&self, available: bool) {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .tray_available = available;
    }
    pub(crate) fn is_exiting(&self) -> bool {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).exiting
    }
    pub(crate) fn snapshot(&self, id: &str) -> Reply {
        if !valid_uuid(id) {
            return error(id, "validation_failed");
        }
        self.0.lock().unwrap_or_else(|e| e.into_inner()).reply(id)
    }
    pub(crate) fn request(&self, id: &str) -> Reply {
        if !valid_uuid(id) {
            return error(id, "validation_failed");
        }
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if !state.exiting && state.confirmation.is_none() {
            state.confirmation = Some(Uuid::new_v4().to_string());
        }
        state.reply(id)
    }
    pub(crate) fn cancel(&self, id: &str, token: &str) -> Reply {
        if !valid_uuid(id) || !valid_uuid(token) {
            return error(id, "validation_failed");
        }
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.exiting || state.confirmation.as_deref() != Some(token) {
            return error(id, "state_conflict");
        }
        state.confirmation = None;
        state.reply(id)
    }
    pub(crate) fn confirm(
        &self,
        id: &str,
        token: &str,
        hold_config_actions: impl FnOnce() -> bool,
    ) -> (Reply, bool) {
        if !valid_uuid(id) || !valid_uuid(token) {
            return (error(id, "validation_failed"), false);
        }
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.exiting || state.confirmation.as_deref() != Some(token) {
            return (error(id, "state_conflict"), false);
        }
        if !hold_config_actions() {
            return (error(id, "busy"), false);
        }
        state.confirmation = None;
        state.exiting = true;
        (state.reply(id), true)
    }
    pub(crate) fn hide_on_close(&self) -> bool {
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if state.exiting || !state.tray_available || state.confirm_on_close {
            return false;
        }
        state.confirmation = None;
        true
    }
}

pub(crate) fn notify(app: &tauri::AppHandle) {
    let _ = app.emit(CHANGED, ());
}

#[cfg(all(test, windows))]
mod update_exit_tests {
    use super::*;

    #[test]
    fn update_exit_does_not_arm_a_worker_after_another_exit_was_requested() {
        let lifecycle = Lifecycle::default();
        let id = Uuid::new_v4().to_string();
        lifecycle.request(&id);
        assert!(!lifecycle.begin_update(|| panic!("unrelated exit must not arm installation")));
        assert!(!lifecycle.is_exiting());
        let empty = Lifecycle::default();
        assert!(!empty.begin_update(|| false));
        assert!(!empty.is_exiting());
        assert!(empty.begin_update(|| true));
        assert!(empty.is_exiting());
        assert!(!empty.begin_update(|| panic!("a second exit must not run")));
    }
}
pub(crate) fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    notify(app);
}
pub(crate) fn request_exit(app: &tauri::AppHandle) {
    app.state::<Lifecycle>()
        .request(&Uuid::new_v4().to_string());
    show(app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{to_value, Value};
    fn id() -> String {
        Uuid::new_v4().to_string()
    }
    fn value(reply: Reply) -> Value {
        to_value(reply).unwrap()
    }
    #[test]
    fn close_is_hide_only_with_a_tray_and_cancel_does_not_start_shutdown() {
        let life = Lifecycle::default();
        assert!(!life.hide_on_close());
        let token = value(life.request(&id()))["exit_token"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(value(life.cancel(&id(), &token))["exit_state"], "idle");
        life.set_tray_available(true);
        life.request(&id());
        assert!(life.hide_on_close());
        assert_eq!(value(life.snapshot(&id()))["exit_token"], Value::Null);
        assert!(!life.is_exiting());
    }
    #[test]
    fn stale_confirmation_and_config_busy_cannot_close_or_hold_the_writer() {
        let life = Lifecycle::default();
        let request = id();
        let pending = value(life.request(&request));
        let token = pending["exit_token"].as_str().unwrap();
        assert_eq!(value(life.request(&id()))["exit_token"], token);
        assert!(
            !life
                .confirm(&id(), &id(), || panic!("stale nonce acquired config gate"))
                .1
        );
        let (busy, started) = life.confirm(&id(), token, || false);
        assert!(!started);
        assert_eq!(value(busy)["error"]["code"], "busy");
        assert!(!life.is_exiting());
        let (reply, started) = life.confirm(&id(), token, || true);
        assert!(started);
        assert_eq!(value(reply)["exit_state"], "shutting_down");
        assert!(
            !life
                .confirm(&id(), token, || panic!("duplicate shutdown"))
                .1
        );
        assert_eq!(
            value(life.cancel(&id(), token))["error"]["code"],
            "state_conflict"
        );
        assert!(!life.hide_on_close());
    }
    #[test]
    fn invalid_id_has_fixed_error_and_no_state_change() {
        let life = Lifecycle::default();
        let reply = value(life.request("not-an-id"));
        assert_eq!(reply["request_id"], Uuid::nil().to_string());
        assert_eq!(reply["error"]["code"], "validation_failed");
        assert_eq!(value(life.snapshot(&id()))["exit_state"], "idle");
    }

    #[test]
    fn operating_close_preference_requires_confirmation_without_stopping_on_cancel() {
        let life = Lifecycle::default();
        life.set_tray_available(true);
        life.set_close_to_tray(false);
        assert!(!life.hide_on_close());
        let token = value(life.request(&id()))["exit_token"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(value(life.cancel(&id(), &token))["exit_state"], "idle");
        assert!(!life.is_exiting());
        life.set_close_to_tray(true);
        assert!(life.hide_on_close());
    }
}
