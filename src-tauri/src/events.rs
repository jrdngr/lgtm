//! Events the backend sends to a repo window.

use serde::Serialize;
use specta::Type;
use tauri_specta::Event;

/// A jj operation landed in the window's repo, so reviews may be out of date.
#[derive(Clone, Serialize, Type, Event)]
pub struct RepoChanged {}

/// The working copy became stale (`message` is jj's error) or stopped being stale (`null`).
#[derive(Clone, Serialize, Type, Event)]
pub struct WorkingCopyStale {
    pub message: Option<String>,
}
