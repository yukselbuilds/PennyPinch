pub(crate) mod autostart;
pub(crate) mod lifecycle;
pub(crate) mod settings;
pub(crate) mod tray;
#[cfg(all(windows, any(test, not(debug_assertions))))]
pub(crate) mod uninstall;
#[allow(
    dead_code,
    reason = "H042-b wires the verified update contract to the app"
)]
pub(crate) mod update;
