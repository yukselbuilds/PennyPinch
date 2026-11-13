use super::discovery::{DiscoveryManager, InstallationSummary};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[cfg(windows)]
#[path = "accounts_opencode.rs"]
mod opencode;
#[cfg(windows)]
#[path = "accounts_windows.rs"]
mod platform;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Operation {
    Read,
    StartLogin,
    ActiveLogin,
    PollLogin,
    CancelLogin,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    pub schema_version: u8,
    pub request_id: String,
    pub operation: Operation,
    pub installation_id: Option<String>,
    pub scan_generation: Option<u64>,
    pub flow_id: Option<String>,
    pub confirm_profile_change: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Account {
    pub installation_id: String,
    pub client_kind: &'static str,
    pub surface: &'static str,
    pub state: &'static str,
    pub method: &'static str,
    pub source: &'static str,
    pub reason: &'static str,
    pub login_supported: bool,
    pub remote_verified: bool,
    pub providers: Vec<Provider>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Provider {
    pub provider: String,
    pub method: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Flow {
    pub flow_id: String,
    pub installation_id: String,
    pub client_kind: &'static str,
    pub state: &'static str,
}

#[derive(Serialize)]
pub(crate) struct Reply {
    pub schema_version: u8,
    pub request_id: String,
    pub status: &'static str,
    pub account: Option<Account>,
    pub flow: Option<Flow>,
    pub error: Option<&'static str>,
}

#[derive(Clone)]
pub(crate) struct AccountManager {
    inner: Arc<Mutex<Inner>>,
}

struct Inner {
    closing: bool,
    #[cfg(windows)]
    root: Option<PathBuf>,
    #[cfg(windows)]
    active: Option<platform::Login>,
}

impl AccountManager {
    pub(crate) fn new(root: Option<PathBuf>) -> Self {
        #[cfg(not(windows))]
        let _ = root;
        Self {
            inner: Arc::new(Mutex::new(Inner {
                closing: false,
                #[cfg(windows)]
                root: root.map(|p| p.join("account-commands")),
                #[cfg(windows)]
                active: None,
            })),
        }
    }

    pub(crate) fn execute(&self, request: Request, discovery: &DiscoveryManager) -> Reply {
        if !valid_request(&request) {
            return failure(&request.request_id, "validation_failed");
        }
        let Ok(mut inner) = self.inner.try_lock() else {
            return failure(&request.request_id, "busy");
        };
        if inner.closing {
            return failure(&request.request_id, "unavailable");
        }
        #[cfg(not(windows))]
        let _ = &mut inner;
        if matches!(
            request.operation,
            Operation::PollLogin | Operation::CancelLogin | Operation::ActiveLogin
        ) {
            #[cfg(windows)]
            return platform::flow_action(&mut inner.active, &request);
            #[cfg(not(windows))]
            return if matches!(request.operation, Operation::ActiveLogin) {
                Reply {
                    schema_version: 1,
                    request_id: request.request_id,
                    status: "snapshot",
                    account: None,
                    flow: None,
                    error: None,
                }
            } else {
                failure(&request.request_id, "unsupported")
            };
        }
        let id = request.installation_id.as_deref().unwrap();
        let generation = request.scan_generation.unwrap();
        let Ok((selected, path)) = discovery.select_installation_with_path(id, generation) else {
            return failure(&request.request_id, "stale_selection");
        };
        let mut account = unknown(&selected, "unsupported");
        #[cfg(windows)]
        {
            if let Some(active) = inner.active.as_mut() {
                active.update();
                if active.blocks_operations() {
                    return failure(&request.request_id, "busy");
                }
            }
            let Some(version) = selected
                .version
                .as_deref()
                .filter(|v| supported(&selected, v))
                .filter(|_| {
                    path.extension()
                        .is_some_and(|s| s.eq_ignore_ascii_case("exe"))
                })
            else {
                return if matches!(request.operation, Operation::StartLogin) {
                    failure(&request.request_id, "unsupported")
                } else {
                    account_reply(request.request_id, account)
                };
            };
            let Some(parents) = path
                .parent()
                .and_then(|p| super::config_io::pin_existing_directory_chain(p).ok())
            else {
                return failure(&request.request_id, "untrusted_installation");
            };
            if parents.validate().is_err() {
                return failure(&request.request_id, "untrusted_installation");
            }
            let Some(pin) =
                super::discovery::pin_verified_client_cli(selected.client_kind, &path, version)
            else {
                return failure(&request.request_id, "untrusted_installation");
            };
            let Some(root) = inner.root.clone() else {
                return failure(&request.request_id, "unavailable");
            };
            account.login_supported = true;
            if matches!(request.operation, Operation::StartLogin) {
                match platform::Login::start(root.as_path(), &path, selected.clone(), pin, parents)
                {
                    Ok(login) => {
                        let flow = login.snapshot.clone();
                        inner.active = Some(login);
                        let weak = Arc::downgrade(&self.inner);
                        let flow_id = flow.flow_id.clone();
                        if std::thread::Builder::new()
                            .name("headroom-account-login".into())
                            .spawn(move || loop {
                                std::thread::sleep(std::time::Duration::from_millis(250));
                                let Some(shared) = weak.upgrade() else {
                                    break;
                                };
                                let Ok(mut state) = shared.try_lock() else {
                                    continue;
                                };
                                let Some(login) = state
                                    .active
                                    .as_mut()
                                    .filter(|l| l.snapshot.flow_id == flow_id)
                                else {
                                    break;
                                };
                                login.update();
                                if login.snapshot.state != "pending" {
                                    break;
                                }
                            })
                            .is_err()
                        {
                            if let Some(login) = inner.active.as_mut() {
                                login.cancel();
                            }
                            return failure(&request.request_id, "launch_failed");
                        }
                        return Reply {
                            schema_version: 1,
                            request_id: request.request_id,
                            status: "snapshot",
                            account: None,
                            flow: Some(flow),
                            error: None,
                        };
                    }
                    Err(_) => return failure(&request.request_id, "launch_failed"),
                }
            }
            if selected.client_kind == "opencode" {
                account = opencode::read(&selected);
            } else {
                match platform::read(&root, &path, &selected) {
                    Ok((code, stdout, stderr)) => {
                        account = parse_status(&selected, code, &stdout, &stderr)
                    }
                    Err(code) => account.reason = code,
                }
            }
            if discovery
                .select_installation_with_path(id, generation)
                .ok()
                .is_none_or(|(fresh, fresh_path)| {
                    fresh_path != path || fresh.version != selected.version
                })
            {
                return failure(&request.request_id, "stale_selection");
            }
            drop(pin);
        }
        #[cfg(not(windows))]
        {
            let _ = path;
            if matches!(request.operation, Operation::StartLogin) {
                return failure(&request.request_id, "unsupported");
            }
        }
        account_reply(request.request_id, account)
    }

    pub(crate) fn shutdown(&self) -> bool {
        if let Ok(mut inner) = self.inner.lock() {
            inner.closing = true;
            #[cfg(windows)]
            if let Some(mut login) = inner.active.take() {
                if matches!(login.snapshot.state, "pending" | "cleanup_failed") {
                    login.cancel();
                }
                return login.snapshot.state != "cleanup_failed";
            }
            #[cfg(not(windows))]
            let _ = &mut inner;
            return true;
        }
        false
    }
}

fn valid_uuid(value: &str) -> bool {
    Uuid::parse_str(value).is_ok_and(|id| !id.is_nil() && id.to_string() == value)
}

fn valid_request(r: &Request) -> bool {
    if r.schema_version != 1 || !valid_uuid(&r.request_id) {
        return false;
    }
    match r.operation {
        Operation::Read | Operation::StartLogin => {
            r.installation_id.as_deref().is_some_and(valid_uuid)
                && r.scan_generation
                    .is_some_and(|n| (1..=9_007_199_254_740_991).contains(&n))
                && r.flow_id.is_none()
                && r.confirm_profile_change == matches!(r.operation, Operation::StartLogin)
        }
        Operation::PollLogin | Operation::CancelLogin => {
            r.installation_id.is_none()
                && r.scan_generation.is_none()
                && !r.confirm_profile_change
                && r.flow_id.as_deref().is_some_and(valid_uuid)
        }
        Operation::ActiveLogin => {
            r.installation_id.is_none()
                && r.scan_generation.is_none()
                && r.flow_id.is_none()
                && !r.confirm_profile_change
        }
    }
}

fn supported(i: &InstallationSummary, version: &str) -> bool {
    i.surface == "cli"
        && match i.client_kind {
            "codex" => matches!(
                version,
                "0.163.0-alpha.11.1" | "0.163.0-alpha.11.2" | "0.167.0-alpha.4.1"
            ),
            "claude_code" => version == "2.1.158",
            "opencode" => version == "1.19.3",
            _ => false,
        }
}

fn unknown(i: &InstallationSummary, reason: &'static str) -> Account {
    Account {
        installation_id: i.installation_id.clone(),
        client_kind: i.client_kind,
        surface: i.surface,
        state: "unknown",
        method: "unknown",
        source: "unavailable",
        reason,
        login_supported: false,
        remote_verified: false,
        providers: vec![],
    }
}

fn parse_status(i: &InstallationSummary, code: i32, stdout: &[u8], stderr: &[u8]) -> Account {
    let mut value = unknown(i, "read_failed");
    value.login_supported = true;
    if stdout.len() > 65536 || stderr.len() > 65536 {
        return value;
    }
    match i.client_kind {
        "codex" => {
            let Ok(text) = std::str::from_utf8(stderr) else {
                return value;
            };
            let method = match (code, text.trim()) {
                (0, "Logged in using ChatGPT") => Some("chatgpt"),
                (0, line)
                    if line.starts_with("Logged in using an API key - ")
                        && !line.contains('\n') =>
                {
                    Some("api_key")
                }
                (0, "Logged in using access token" | "Logged in using personal access token") => {
                    Some("access_token")
                }
                (1, "Not logged in") => Some("none"),
                _ => None,
            };
            if let Some(method) = method {
                value.method = method;
                value.state = if method == "none" {
                    "absent"
                } else {
                    "present"
                };
                value.source = "official_cli";
                value.reason = "none";
            }
        }
        _ => {}
    }
    value
}

fn account_reply(id: String, account: Account) -> Reply {
    Reply {
        schema_version: 1,
        request_id: id,
        status: "snapshot",
        account: Some(account),
        flow: None,
        error: None,
    }
}

pub(crate) fn failure(id: &str, code: &'static str) -> Reply {
    Reply {
        schema_version: 1,
        request_id: if valid_uuid(id) {
            id.to_owned()
        } else {
            Uuid::nil().to_string()
        },
        status: "error",
        account: None,
        flow: None,
        error: Some(code),
    }
}

#[cfg(test)]
#[path = "accounts_tests.rs"]
mod tests;
