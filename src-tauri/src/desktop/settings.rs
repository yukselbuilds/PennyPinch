use super::autostart::{Autostart, Error};
use crate::runtime::manager::RuntimeManager;
use serde_json::{json, Value};
use uuid::Uuid;

fn valid(id: &str) -> bool {
    Uuid::parse_str(id).is_ok_and(|value| !value.is_nil() && value.to_string() == id)
}
pub(crate) fn error(id: &str, code: &str) -> Value {
    json!({"schema_version":1,"request_id":if valid(id){id}else{"00000000-0000-0000-0000-000000000000"},"status":"error",
        "error":{"code":code,"retryable":matches!(code,"busy"|"config_conflict"),"message_key":format!("operating.{code}"),"diagnostic_id":null}})
}
pub(crate) fn read(id: &str, runtime: &RuntimeManager, startup: &Autostart) -> Value {
    if !valid(id) {
        return error(id, "validation_failed");
    }
    json!({"schema_version":1,"request_id":id,"status":"snapshot","autostart":startup.read(),"runtime":runtime.settings_observation()})
}
pub(crate) fn set_startup(
    id: &str,
    enabled: bool,
    fingerprint: Option<&str>,
    runtime: &RuntimeManager,
    startup: &Autostart,
) -> Value {
    if !valid(id) {
        return error(id, "validation_failed");
    }
    match startup.set(enabled, fingerprint) {
        Ok(_) => read(id, runtime, startup),
        Err(issue) => error(
            id,
            match issue {
                Error::Invalid => "validation_failed",
                Error::Busy => "busy",
                Error::Conflict => "config_conflict",
                Error::Unavailable => "environment_blocked",
            },
        ),
    }
}
