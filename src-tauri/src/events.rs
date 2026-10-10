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

/// A persona review's part `part` moved on to something else, like reading a file.
#[derive(Clone, Serialize, Type, Event)]
pub struct BotRunActivity {
    pub run: u32,
    pub part: u32,
    pub activity: String,
}

/// A persona review started, stopped, or added or finished a part.
#[derive(Clone, Serialize, Type, Event)]
pub struct BotRunChanged {
    pub run: u32,
}

/// A persona review's fix `fix` streamed more of its reply.
#[derive(Clone, Serialize, Type, Event)]
pub struct BotFixText {
    pub run: u32,
    pub fix: u32,
    pub text: String,
}
