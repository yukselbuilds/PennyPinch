pub(crate) mod control;
pub(crate) mod learn;
pub mod manager;
pub(crate) mod metrics_read;
pub(crate) mod recovery;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) mod unix_backend;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) mod macos_layout;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(crate) mod macos_session;

#[cfg(target_os = "macos")]
pub(crate) mod macos_ports;

#[cfg(windows)]
pub mod ingress;

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[path = "ingress_macos.rs"]
pub mod ingress;

#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
pub mod ingress_relay;

#[cfg(all(test, target_os = "macos"))]
const ENGINE_PROOF_ENV: &str = "HEADROOM_MACOS_PROOF_ROOT";
#[cfg(all(test, target_os = "linux"))]
const ENGINE_PROOF_ENV: &str = "HEADROOM_LINUX_ENGINE_PROOF_ROOT";
