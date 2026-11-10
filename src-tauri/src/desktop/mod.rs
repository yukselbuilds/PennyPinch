pub(crate) mod autostart;
pub(crate) mod lifecycle;
pub(crate) mod settings;
pub(crate) mod tray;
#[cfg(all(windows, any(test, not(debug_assertions))))]
pub(crate) mod uninstall;
pub(crate) mod update;
#[cfg(all(windows, any(test, not(debug_assertions))))]
mod update_commit;
pub(crate) mod update_flow;
#[cfg(all(windows, not(debug_assertions)))]
pub(crate) mod update_install;
