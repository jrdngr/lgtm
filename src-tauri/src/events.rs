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

/// A persona review's fix `fix` streamed more of its reply. `text` is the whole reply so far.
#[derive(Clone, Serialize, Type, Event)]
pub struct BotFixText {
    pub run: u32,
    pub fix: u32,
    pub text: String,
}

/// An agent asked to see `path` at `line`: in the review pane, or an editor when the review
/// doesn't change it.
#[derive(Clone, Serialize, Type, Event)]
pub struct AgentShow {
    pub workspace: Option<String>,
    pub path: String,
    pub line: Option<u32>,
}

/// An agent asked the workspace's review pane to review `revset`.
#[derive(Clone, Serialize, Type, Event)]
pub struct AgentRevset {
    pub workspace: Option<String>,
    pub revset: String,
}

/// An agent changed review `review`'s comments or hotspots.
#[derive(Clone, Serialize, Type, Event)]
pub struct ReviewStateChanged {
    pub review: u32,
    pub state: crate::review::StateDto,
}
