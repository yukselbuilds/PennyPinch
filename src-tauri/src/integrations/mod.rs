pub(crate) mod codex_binary;
pub(crate) mod config_document;
pub(crate) mod config_io;
pub mod discovery;
pub(crate) mod opencode_routes;
pub mod picker;
pub(crate) mod plan;
#[cfg(windows)]
mod process;
pub(crate) mod transaction;
pub(crate) mod transaction_fs;

#[cfg(test)]
mod discovery_tests;

#[cfg(test)]
mod plan_tests;

#[cfg(test)]
mod opencode_routes_tests;

#[cfg(test)]
mod config_io_tests;

#[cfg(test)]
mod transaction_tests;

#[cfg(test)]
mod transaction_fs_tests;

#[cfg(all(test, windows))]
mod connection_actions_tests;
