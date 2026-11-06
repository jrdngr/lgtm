//! DASH mode: what the workspace's agent is doing while it works. The files glowing as
//! they're written, the diff since the dashboard was last cleared following each new hunk,
//! the newest turn's todo list, and its churn.

mod churn;
mod diff;
mod draw;

use std::collections::HashSet;

use lgtm_core::diff::RowKind;
use lgtm_core::jj::FileDiff;
use lgtm_core::store::AgentActivity;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::files::{self, FileTree};
use super::keybar::Hint;
use diff::Line;

pub enum Outcome {
    None,
    Home,
    Quit,
    /// Open the file in the editor.
    Edit(String),
    /// Open the working copy's review on a path's line, with that line's text.
    Review(String, u32, String),
    /// Start the diff again from the newest turn.
    Clear,
}

/// The files' narrowest width.
const FILES_MIN: u16 = 16;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Files,
    Diff,
}

/// The keys, for `?`.
pub const KEYS: &[(&str, &str)] = &[
    ("Tab", "files, diff"),
    ("f", "follow the newest change"),
    ("c", "clear the diff"),
    ("< >", "resize the files"),
    ("j k", "move"),
    ("C-d C-u", "page"),
    ("g G", "top, bottom"),
    ("Enter", "open in review, or open the file"),
    (":h", "home"),
    (":q", "quit"),
];

/// The `:` commands, for the menu over the command line.
const COMMANDS: &[(&str, &str)] = &[
    ("h", "home"),
    ("clear", "clear the diff"),
    ("heat", "heat on the files"),
    ("side", "files on the other side"),
    ("q", "quit"),
];

pub struct Dash {
    /// The workspace's newest agent turn.
    pub activity: Option<AgentActivity>,
    /// The commit the diff starts from: the start of the first turn since the dashboard
    /// was cleared.
    since: Option<String>,
    /// The diff so far, and its lines.
    diffs: Vec<FileDiff>,
    lines: Vec<Line>,
    /// Whether the diff loaded since it was cleared.
    loaded: bool,
    /// Rows the latest load added.
    fresh: HashSet<(usize, usize, usize)>,
    /// Whether the diff scrolls to each load's new rows.
    following: bool,
    /// Whether to scroll to the fresh rows when next drawn.
    jump: bool,
    top: usize,
    /// The diff's height as last drawn.
    height: usize,
    focus: Focus,
    /// The `:` line being typed.
    cmd: Option<String>,
    pub notice: Option<String>,
    /// The files' width once `<` or `>` set it, and as last drawn.
    pub files_width: Option<u16>,
    files_drawn: u16,
}

impl Default for Dash {
    fn default() -> Self {
        Self {
            activity: None,
            since: None,
            diffs: vec![],
            lines: vec![],
            loaded: false,
            fresh: HashSet::new(),
            following: true,
            jump: false,
            top: 0,
            height: 0,
            focus: Focus::Diff,
            cmd: None,
            notice: None,
            files_width: None,
            files_drawn: 0,
        }
    }
}

impl Dash {
    /// Takes the newest turn. Returns the commit to diff from when the turn or its edits
    /// changed. A new turn adds to the diff; only clearing starts it again.
    pub fn set_activity(&mut self, activity: Option<AgentActivity>) -> Option<String> {
        let edits =
            |a: &Option<AgentActivity>| a.as_ref().map(|a| (a.start.clone(), a.edits.len()));
        let changed = edits(&self.activity) != edits(&activity);
        self.activity = activity;
        if self.since.is_none() {
            self.since = self.activity.as_ref().map(|a| a.start.clone());
        }
        self.since.clone().filter(|_| changed)
    }

    /// Starts the diff again from the newest turn's start. Returns the commit to diff from.
    pub fn clear(&mut self) -> Option<String> {
        self.diffs.clear();
        self.lines.clear();
        self.loaded = false;
        self.fresh.clear();
        self.top = 0;
        self.since = self.activity.as_ref().map(|a| a.start.clone());
        self.since.clone()
    }

    /// The commit the diff starts from.
    pub fn start(&self) -> Option<&str> {
        self.since.as_deref()
    }

    /// Takes the diff from `start`. A stale one, or one that failed, is dropped.
    pub fn diff_loaded(&mut self, start: &str, result: anyhow::Result<Vec<FileDiff>>) {
        if self.start() != Some(start) {
            return;
        }
        let diffs = match result {
            Ok(d) => d,
            Err(e) => {
                self.notice = Some(format!("{e:#}"));
                return;
            }
        };
        // The first load of a turn shows it from the top, with nothing new yet.
        let first = !std::mem::replace(&mut self.loaded, true);
        let fresh = diff::fresh(&self.diffs, &diffs);
        self.lines = diff::layout(&diffs);
        self.diffs = diffs;
        self.top = self.top.min(self.lines.len().saturating_sub(1));
        if first || fresh.is_empty() {
            return;
        }
        self.fresh = fresh;
        if self.following {
            self.follow();
        }
    }

    /// Scrolls to the first fresh row, a third of the way down, once the diff's height is
    /// known.
    fn follow(&mut self) {
        self.jump = true;
    }

    fn jump_to_fresh(&mut self) {
        self.jump = false;
        let first = self.lines.iter().position(|l| match *l {
            Line::Row { file, hunk, row } => self.fresh.contains(&(file, hunk, row)),
            _ => false,
        });
        if let Some(at) = first {
            self.top = at.saturating_sub(self.height / 3);
        }
    }

    /// Whether keys are going into the `:` line.
    pub fn typing(&self) -> bool {
        self.cmd.is_some()
    }

    pub fn prompt(&self) -> Option<String> {
        self.cmd.as_ref().map(|c| format!(":{c}"))
    }

    pub fn command_menu(&self) -> Vec<(String, &'static str)> {
        let Some(typed) = &self.cmd else {
            return vec![];
        };
        COMMANDS
            .iter()
            .filter(|(name, _)| name.starts_with(typed.as_str()))
            .map(|(name, label)| (format!(":{name}"), *label))
            .collect()
    }

    fn command_key(&mut self, key: KeyEvent, tree: &mut FileTree) -> Outcome {
        let Some(typed) = self.cmd.as_mut() else {
            return Outcome::None;
        };
        match key.code {
            KeyCode::Esc => self.cmd = None,
            KeyCode::Backspace if typed.is_empty() => self.cmd = None,
            KeyCode::Backspace => {
                typed.pop();
            }
            KeyCode::Enter => {
                let line = self.cmd.take().unwrap_or_default();
                match line.trim() {
                    "h" | "home" => return Outcome::Home,
                    "clear" => return Outcome::Clear,
                    "heat" => tree.heat_on = !tree.heat_on,
                    "side" => crate::tui::side::toggle(),
                    "q" | "q!" | "quit" => return Outcome::Quit,
                    "" => {}
                    other => self.notice = Some(format!("no command {other}")),
                }
            }
            KeyCode::Char(c) => typed.push(c),
            _ => {}
        }
        Outcome::None
    }

    pub fn key(&mut self, key: KeyEvent, tree: &mut FileTree) -> Outcome {
        self.notice = None;
        if self.cmd.is_some() {
            return self.command_key(key, tree);
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if let Some(step) = crate::tui::side::files_step(&key) {
            self.files_width = Some(crate::tui::width::moved(self.files_drawn, step, FILES_MIN));
            return Outcome::None;
        }
        match key.code {
            KeyCode::Char(':') => self.cmd = Some(String::new()),
            KeyCode::Char('c') => return Outcome::Clear,
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Files => Focus::Diff,
                    Focus::Diff => Focus::Files,
                }
            }
            KeyCode::Char('f') => {
                self.following = !self.following;
                if self.following {
                    self.follow();
                }
            }
            _ if self.focus == Focus::Files => return files_key(key, ctrl, tree),
            _ => return self.diff_key(key, ctrl),
        }
        Outcome::None
    }

    fn diff_key(&mut self, key: KeyEvent, ctrl: bool) -> Outcome {
        let page = (self.height / 2).max(1) as isize;
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.scroll(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll(-1),
            KeyCode::Char('d') if ctrl => self.scroll(page),
            KeyCode::Char('u') if ctrl => self.scroll(-page),
            KeyCode::PageDown => self.scroll(page),
            KeyCode::PageUp => self.scroll(-page),
            KeyCode::Char('g') | KeyCode::Home => self.scroll(isize::MIN / 2),
            KeyCode::Char('G') | KeyCode::End => self.scroll(isize::MAX / 2),
            KeyCode::Enter => return self.open_review(),
            _ => {}
        }
        Outcome::None
    }

    /// Scrolls the diff by `n` lines, which stops following it.
    pub fn scroll(&mut self, n: isize) {
        self.following = false;
        let max = self.lines.len().saturating_sub(self.height.max(1));
        self.top = (self.top as isize).saturating_add(n).clamp(0, max as isize) as usize;
    }

    /// The review of the working copy on the first row of the new side in view.
    fn open_review(&self) -> Outcome {
        let found = self.lines.iter().skip(self.top).find_map(|l| match *l {
            Line::Row { file, hunk, row } => {
                let d = &self.diffs[file];
                let r = &d.hunks[hunk].rows[row];
                let line = r.new_line.filter(|_| r.kind != RowKind::Removed)?;
                Some((d.path.clone(), line, r.text.clone()))
            }
            _ => None,
        });
        match found {
            Some((path, line, text)) => Outcome::Review(path, line, text),
            None => Outcome::None,
        }
    }

    pub fn hints(&self) -> Vec<Hint<'static>> {
        vec![
            ("Tab", "pane", false),
            ("f", "follow", self.following),
            ("c", "clear", false),
            ("Enter", "open in review", false),
            (":h", "home", false),
            (":q", "quit", false),
        ]
    }
}

/// Moves in the file tree, opens and closes its folders, and opens its files.
fn files_key(key: KeyEvent, ctrl: bool, tree: &mut FileTree) -> Outcome {
    let code = match key.code {
        KeyCode::Char('j') => KeyCode::Down,
        KeyCode::Char('k') => KeyCode::Up,
        KeyCode::Char('h') => KeyCode::Left,
        KeyCode::Char('l') => KeyCode::Right,
        KeyCode::Char('d') if ctrl => KeyCode::PageDown,
        KeyCode::Char('u') if ctrl => KeyCode::PageUp,
        KeyCode::Char('g') => KeyCode::Home,
        KeyCode::Char('G') => KeyCode::End,
        code @ (KeyCode::Up
        | KeyCode::Down
        | KeyCode::Left
        | KeyCode::Right
        | KeyCode::PageUp
        | KeyCode::PageDown
        | KeyCode::Home
        | KeyCode::End
        | KeyCode::Enter) => code,
        _ => return Outcome::None,
    };
    match tree.key(KeyEvent::new(code, KeyModifiers::NONE)) {
        files::Outcome::Open(path) => Outcome::Edit(path),
        files::Outcome::None => Outcome::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lgtm_core::store::TurnEdit;

    fn turn(start: &str, edits: usize) -> Option<AgentActivity> {
        let edit = TurnEdit {
            at: 0,
            added: 1,
            removed: 0,
            rewritten: 0,
            paths: vec![],
        };
        Some(AgentActivity {
            prompt: None,
            terminal: None,
            running: true,
            start: start.into(),
            started_at: 0,
            ended_at: None,
            edits: vec![edit; edits],
            todos: vec![],
        })
    }

    #[test]
    fn a_new_turn_adds_to_the_diff_from_the_first_and_clearing_starts_from_the_newest() {
        let mut d = Dash::default();
        assert_eq!(d.set_activity(turn("a", 1)).as_deref(), Some("a"));
        assert_eq!(d.set_activity(turn("a", 1)), None);
        assert_eq!(d.set_activity(turn("a", 2)).as_deref(), Some("a"));
        assert_eq!(d.set_activity(turn("b", 0)).as_deref(), Some("a"));
        assert_eq!(d.clear().as_deref(), Some("b"));
        assert_eq!(d.set_activity(turn("b", 1)).as_deref(), Some("b"));
    }

    #[test]
    fn scrolling_stops_following_and_f_starts_it_again() {
        let mut d = Dash::default();
        let mut tree = FileTree::default();
        d.key(KeyEvent::from(KeyCode::Char('j')), &mut tree);
        assert!(!d.following);
        d.key(KeyEvent::from(KeyCode::Char('f')), &mut tree);
        assert!(d.following);
    }
}
