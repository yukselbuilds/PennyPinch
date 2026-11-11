pub(crate) mod control;
pub mod manager;
pub(crate) mod metrics_read;
pub(crate) mod recovery;

#[cfg(target_os = "macos")]
pub(crate) mod unix_backend;

#[cfg(target_os = "macos")]
pub(crate) mod macos_layout;

#[cfg(target_os = "macos")]
pub(crate) mod macos_session;

#[cfg(windows)]
pub mod ingress;

#[cfg(target_os = "macos")]
#[path = "ingress_macos.rs"]
pub mod ingress;

#[cfg(any(windows, target_os = "macos"))]
pub mod ingress_relay;
