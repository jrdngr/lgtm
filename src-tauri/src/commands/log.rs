//! The UI's logging, through the app's tracing so it lands in the same output and follows
//! `RUST_LOG`. Every UI line has target `ui`; the UI module that logged it is a field.

use serde::{Deserialize, Serialize};
use tracing::Level;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[tauri::command]
#[specta::specta]
pub(crate) fn log(level: LogLevel, module: String, message: String) {
    match level {
        LogLevel::Error => tracing::error!(target: "ui", module, "{message}"),
        LogLevel::Warn => tracing::warn!(target: "ui", module, "{message}"),
        LogLevel::Info => tracing::info!(target: "ui", module, "{message}"),
        LogLevel::Debug => tracing::debug!(target: "ui", module, "{message}"),
        LogLevel::Trace => tracing::trace!(target: "ui", module, "{message}"),
    }
}

/// The most detailed level the UI's lines are kept at, so it can skip the rest.
#[tauri::command]
#[specta::specta]
pub(crate) fn log_level() -> LogLevel {
    if tracing::enabled!(target: "ui", Level::TRACE) {
        LogLevel::Trace
    } else if tracing::enabled!(target: "ui", Level::DEBUG) {
        LogLevel::Debug
    } else if tracing::enabled!(target: "ui", Level::INFO) {
        LogLevel::Info
    } else if tracing::enabled!(target: "ui", Level::WARN) {
        LogLevel::Warn
    } else {
        LogLevel::Error
    }
}
