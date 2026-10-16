//! Events the backend sends to a repo window.

use serde::Serialize;
use specta::Type;
use tauri_specta::Event;

/// A jj operation landed in the window's repo, so reviews may be out of date.
#[derive(Clone, Serialize, Type, Event)]
pub struct RepoChanged {}

/// Something other than the app, like a terminal or an agent, moved a workspace's working
/// copy. `workspace` is `null` for the main one.
#[derive(Clone, Serialize, Type, Event)]
pub struct WorkingCopyMoved {
    pub workspace: Option<String>,
}

/// File > Open Repository in this window. The window swaps itself for the start window once
/// unsaved edits are dealt with.
#[derive(Clone, Serialize, Type, Event)]
pub struct SwitchRepo {}

/// The settings were saved.
#[derive(Clone, Serialize, Type, Event)]
pub struct SettingsChanged {
    pub settings: crate::settings::Settings,
}

/// lgtm > Settings… was chosen while this window had focus.
#[derive(Clone, Serialize, Type, Event)]
pub struct OpenSettings {}

/// Help > Keyboard Shortcuts was chosen while this window had focus.
#[derive(Clone, Serialize, Type, Event)]
pub struct OpenShortcuts {}

/// View > Word Wrap was turned on or off.
#[derive(Clone, Serialize, Type, Event)]
pub struct WordWrap {
    pub on: bool,
}

/// A workspace's working copy became stale (`message` is jj's error) or stopped being stale
/// (`null`). `workspace` is `null` for the main one.
#[derive(Clone, Serialize, Type, Event)]
pub struct WorkingCopyStale {
    pub workspace: Option<String>,
    pub message: Option<String>,
}

/// An agent published findings to report `run`, so the workspace's dock opens it.
#[derive(Clone, Serialize, Type, Event)]
pub struct BotRunChanged {
    pub run: u32,
    pub workspace: Option<String>,
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

/// An agent changed review `review`'s comments or walkthrough notes.
#[derive(Clone, Serialize, Type, Event)]
pub struct ReviewStateChanged {
    pub review: u32,
    pub state: crate::review::StateDto,
}

/// A terminal agent changed a Q&A session's questions.
#[derive(Clone, Serialize, Type, Event)]
pub struct QaChanged {
    pub workspace: Option<String>,
    pub id: String,
    /// Bring the session forward in the Q&A tab.
    pub show: bool,
}

/// Claude Code's hooks in a terminal reported how its agent stands.
#[derive(Clone, Serialize, Type, Event)]
pub struct TerminalAgent {
    /// The terminal's panel id.
    pub terminal: String,
    pub state: crate::mcp::hook::AgentHookState,
    /// The prompt the user submitted, when this is a prompt.
    pub prompt: Option<String>,
    /// The agent's last message, when it stopped.
    pub reply: Option<String>,
    /// Claude Code's session id.
    pub session: Option<String>,
    /// The user ended the session.
    pub ended: bool,
}
