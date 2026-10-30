//! Events the backend sends to a repo window.

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

/// A jj operation landed in the window's repo, so reviews may be out of date. `snapshot`
/// when it's the app's own background snapshot, which only rewrites a working copy.
#[derive(Clone, Serialize, Type, Event)]
pub struct RepoChanged {
    pub snapshot: bool,
}

/// Review `review` shows older commits than its revset has now, and an agent is waiting on
/// it, so it reloads even while its workspace is hidden.
#[derive(Clone, Serialize, Type, Event)]
pub struct ReviewOutdated {
    pub review: u32,
}

/// Something other than the app, like a terminal or an agent, moved a workspace's working
/// copy. `workspace` is `null` for the main one.
#[derive(Clone, Serialize, Type, Event)]
pub struct WorkingCopyMoved {
    pub workspace: Option<String>,
}

/// File > Add Repository to this window's project.
#[derive(Clone, Serialize, Type, Event)]
pub struct AddRepo {}

/// The window shows the repo with `label`, asked for from outside the window.
#[derive(Clone, Serialize, Type, Event)]
pub struct ShowRepo {
    pub label: String,
}

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

/// A write in a workspace has waited on a jj lock for a while. `holders` lists the processes
/// that have the lock open, one per line: PID, elapsed time, command. `null` once it stops
/// waiting.
#[derive(Clone, Serialize, Type, Event)]
pub struct JjBlocked {
    pub workspace: Option<String>,
    pub holders: Option<String>,
}

/// A workspace's background snapshots paused after giving up on a jj lock twice, or started
/// again once the user acted there.
#[derive(Clone, Serialize, Type, Event)]
pub struct JjPaused {
    pub workspace: Option<String>,
    pub paused: bool,
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

/// The workspace's revisions log has a new revset.
#[derive(Clone, Serialize, Type, Event)]
pub struct LogRevsetChanged {
    pub workspace: Option<String>,
}

/// An agent changed review `review`'s comments or threads.
#[derive(Clone, Serialize, Type, Event)]
pub struct ReviewStateChanged {
    pub review: u32,
    pub state: crate::review::StateDto,
}

/// A terminal agent assessed review `review`'s architecture.
#[derive(Clone, Serialize, Type, Event)]
pub struct ArchChanged {
    pub workspace: Option<String>,
    pub review: u32,
}

/// Symbol Information was asked for, written or removed.
#[derive(Clone, Serialize, Type, Event)]
pub struct SymbolInfoChanged;

/// An agent's turn in the workspace started, changed files or ended, or the user saved a file.
#[derive(Clone, Serialize, Type, Event)]
pub struct TurnsChanged {
    pub workspace: Option<String>,
}

/// A terminal agent wrote review `review`'s guided review.
#[derive(Clone, Serialize, Type, Event)]
pub struct GuidedChanged {
    pub workspace: Option<String>,
    pub review: u32,
}

/// A terminal agent made a design or wrote its map.
#[derive(Clone, Serialize, Type, Event)]
pub struct DesignChanged {
    pub id: String,
    pub workspace: Option<String>,
}

/// A terminal agent made or changed a drawing.
#[derive(Clone, Serialize, Type, Event)]
pub struct DrawingChanged {
    pub id: String,
}

/// A note or folder was made, changed, moved or deleted, in any window or by an agent. `id`
/// names the note when one changed.
#[derive(Clone, Serialize, Type, Event)]
pub struct NotesChanged {
    pub id: Option<String>,
}

/// A terminal agent described the system map.
#[derive(Clone, Serialize, Type, Event)]
pub struct SystemChanged {
    pub workspace: Option<String>,
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
    pub state: AgentHookState,
    /// The prompt the user submitted, when this is a prompt.
    pub prompt: Option<String>,
    /// The agent's last message, when it stopped.
    pub reply: Option<String>,
    /// Claude Code's session id.
    pub session: Option<String>,
    /// The user ended the session.
    pub ended: bool,
    /// The name the user gave the session in Claude Code, when the event looked for it.
    pub title: Option<String>,
}

/// How a terminal's agent stands after a hook event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AgentHookState {
    Working,
    Input,
    Done,
    Failed,
    Idle,
    Gone,
}
