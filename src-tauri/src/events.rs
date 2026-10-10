//! Events the backend sends to a repo window.

use serde::Serialize;
use specta::Type;
use tauri_specta::Event;

/// A jj operation landed in the window's repo, so reviews may be out of date.
#[derive(Clone, Serialize, Type, Event)]
pub struct RepoChanged {}

/// A workspace's working copy became stale (`message` is jj's error) or stopped being stale
/// (`null`). `workspace` is `null` for the main one.
#[derive(Clone, Serialize, Type, Event)]
pub struct WorkingCopyStale {
    pub workspace: Option<String>,
    pub message: Option<String>,
}
