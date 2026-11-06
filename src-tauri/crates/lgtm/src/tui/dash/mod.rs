//! DASH mode: what the workspace's agent is doing while it works. The files glowing as
//! they're written, a running log of each file's changes since the dashboard was last
//! cleared, the newest turn's todo list, and its churn.

mod churn;
mod diff;
mod draw;

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

/// The most lines the log keeps; older entries drop off the top.
const MAX_LINES: usize = 5000;

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
    ("f", "follow the bottom of the log"),
    ("c", "clear the diff"),
    ("< >", "resize the files"),
    ("j k", "move"),
    ("C-d C-u", "page"),
    ("g G", "top, bottom"),
    ("Enter", "open in review, or open the file"),
    ("C-o", "back"),
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
    /// The commit the next diff starts from: the last one diffed to, or after a clear the
    /// start of the newest turn.
    since: Option<String>,
    /// The log: each load's changed files, oldest first, and its lines.
    diffs: Vec<FileDiff>,
    lines: Vec<Line>,
    /// Where the latest load's entries start in `diffs`.
    fresh: usize,
    /// Whether the log stays scrolled to its bottom.
    following: bool,
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
            fresh: 0,
            following: true,
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
    /// changed. A new turn adds to the log; only clearing starts it again.
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

    /// Empties the log and starts it again from the newest turn's start. Returns the commit
    /// to diff from.
    pub fn clear(&mut self) -> Option<String> {
        self.diffs.clear();
        self.lines.clear();
        self.fresh = 0;
        self.top = 0;
        self.since = self.activity.as_ref().map(|a| a.start.clone());
        self.since.clone()
    }

    /// The commit the next diff starts from.
    pub fn start(&self) -> Option<&str> {
        self.since.as_deref()
    }

    /// Adds what changed from `start` to the log, and diffs from where it ended next. A
    /// stale load is dropped, and returns the commit to diff from again when it saw more
    /// than the log has.
    pub fn diff_loaded(
        &mut self,
        start: &str,
        result: anyhow::Result<(Vec<FileDiff>, String)>,
    ) -> Option<String> {
        let (diffs, to) = match result {
            Ok(loaded) => loaded,
            Err(e) => {
                if self.start() == Some(start) {
                    self.notice = Some(format!("{e:#}"));
                }
                return None;
            }
        };
        let since = self.since.clone()?;
        if since != start {
            return (since != to).then_some(since);
        }
        self.since = Some(to);
        let diffs: Vec<FileDiff> = diffs.into_iter().filter(|d| !d.hunks.is_empty()).collect();
        if diffs.is_empty() {
            return None;
        }
        self.fresh = self.diffs.len();
        self.diffs.extend(diffs);
        let all = diff::layout(&self.diffs).len();
        let mut lines = diff::layout(&self.diffs);
        let mut dropped = 0;
        while lines.len() > MAX_LINES && self.diffs.len() - dropped > 1 {
            dropped += 1;
            lines = diff::layout(&self.diffs[dropped..]);
        }
        self.diffs.drain(..dropped);
        self.fresh = self.fresh.saturating_sub(dropped);
        // Lines that dropped off the top take the view down with them.
        self.top = self.top.saturating_sub(all - lines.len());
        self.lines = lines;
        None
    }

    /// The lowest the log scrolls.
    fn bottom(&self) -> usize {
        self.lines.len().saturating_sub(self.height.max(1))
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
            KeyCode::Char('f') => self.following = !self.following,
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

    /// Scrolls the log by `n` lines. Leaving the bottom stops following it, and reaching
    /// the bottom follows it again.
    pub fn scroll(&mut self, n: isize) {
        let top = if self.following {
            self.bottom()
        } else {
            self.top
        };
        let max = self.bottom();
        self.top = (top as isize).saturating_add(n).clamp(0, max as isize) as usize;
        self.following = self.top == max && n > 0;
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

    fn loaded(d: &mut Dash, start: &str, files: &[(&str, &[&str])], to: &str) -> Option<String> {
        let diffs = files
            .iter()
            .map(|(p, rows)| diff::tests::file(p, rows))
            .collect();
        d.diff_loaded(start, Ok((diffs, to.into())))
    }

    fn paths(d: &Dash) -> Vec<&str> {
        d.diffs.iter().map(|f| f.path.as_str()).collect()
    }

    #[test]
    fn each_load_appends_its_files_and_the_next_diffs_from_where_it_ended() {
        let mut d = Dash::default();
        d.set_activity(turn("a", 1));
        loaded(&mut d, "a", &[("x.rs", &["+1"])], "b");
        loaded(&mut d, "b", &[("y.rs", &["+2"]), ("x.rs", &["+3"])], "c");
        assert_eq!(paths(&d), ["x.rs", "y.rs", "x.rs"]);
        assert_eq!(d.start(), Some("c"));
        assert_eq!(d.fresh, 1);
        // Nothing changed: nothing added.
        loaded(&mut d, "c", &[], "d");
        assert_eq!(paths(&d).len(), 3);
    }

    #[test]
    fn a_stale_load_is_dropped_and_asks_again_when_it_saw_further() {
        let mut d = Dash::default();
        d.set_activity(turn("a", 1));
        loaded(&mut d, "a", &[("x.rs", &["+1"])], "b");
        assert_eq!(
            loaded(&mut d, "a", &[("x.rs", &["+1", "+2"])], "c").as_deref(),
            Some("b")
        );
        assert_eq!(loaded(&mut d, "a", &[("x.rs", &["+1"])], "b"), None);
        assert_eq!(paths(&d), ["x.rs"]);
    }

    #[test]
    fn the_log_drops_its_oldest_entries_past_the_cap() {
        let mut d = Dash::default();
        d.set_activity(turn("a", 1));
        let big: Vec<&str> = vec!["+x"; MAX_LINES / 3];
        loaded(&mut d, "a", &[("old.rs", &big)], "b");
        loaded(&mut d, "b", &[("mid.rs", &big)], "c");
        loaded(&mut d, "c", &[("new.rs", &big)], "d");
        assert_eq!(paths(&d), ["mid.rs", "new.rs"]);
        assert!(d.lines.len() <= MAX_LINES);
    }

    #[test]
    fn scrolling_up_stops_following_and_scrolling_back_to_the_bottom_follows_again() {
        let mut d = Dash::default();
        let mut tree = FileTree::default();
        d.set_activity(turn("a", 1));
        loaded(&mut d, "a", &[("x.rs", &["+x"; 30])], "b");
        d.height = 10;
        d.key(KeyEvent::from(KeyCode::Char('k')), &mut tree);
        assert!(!d.following);
        assert_eq!(d.top, d.bottom() - 1);
        d.key(KeyEvent::from(KeyCode::Char('j')), &mut tree);
        assert!(d.following);
        d.key(KeyEvent::from(KeyCode::Char('f')), &mut tree);
        assert!(!d.following);
    }
}
