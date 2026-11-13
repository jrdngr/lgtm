//! Picking what to review from the jj log: one commit, a range picked with Space, or
//! snapshots, which `s` shows under every change.

use lgtm_core::jj::{LogCommit, LogRow};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::keybar::Hint;
use super::load::Target;
use super::motions;
use super::paint::Canvas;
use super::theme::{self, ACCENT, AMBER, BG, FAINT, FG, MUTED, PANEL, RED, SEL};
use super::versions::{self, Versions};

pub enum Outcome {
    None,
    Open(Target),
    /// Load the versions of these changes.
    Versions(Vec<String>),
    /// `jj edit` the commit, then open its review.
    EditOpen {
        rev: String,
        target: Target,
    },
}

/// A line of the picker: a row of the log, or the `j`th snapshot shown under the `k`th
/// change's row.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Item {
    Row(usize),
    Version(usize, usize),
}

#[derive(Default)]
pub struct Picker {
    rows: Vec<LogRow>,
    error: Option<String>,
    /// Index into `rows`, always a commit's row once there is one. With the cursor on a
    /// snapshot, its change's row.
    sel: usize,
    /// The other end of a range, set with Space.
    anchor: Option<usize>,
    top: usize,
    /// Every change's snapshots, under its row, while they're open.
    snapshots: Option<Vec<Versions>>,
    /// The snapshot the cursor is on, by its change and place among the shown ones.
    on: Option<(String, usize)>,
    /// Where the log was last drawn, for the mouse.
    area: Rect,
    /// The last click's line and when, to tell a double click.
    clicked: Option<(Item, std::time::Instant)>,
    /// The next log puts the cursor on the working copy, as after making a new change.
    pub to_working_copy: bool,
    /// Whether the log is git's, which keeps no snapshots and whose commits lgtm doesn't edit.
    pub git: bool,
}

/// Two clicks on a line within this make a double click.
const DOUBLE_CLICK: std::time::Duration = std::time::Duration::from_millis(400);

impl Picker {
    pub fn loaded(&mut self, result: anyhow::Result<Vec<LogRow>>) {
        match result {
            Ok(rows) => {
                let id_at = |rows: &[LogRow], i: Option<usize>| {
                    Some(rows.get(i?)?.commit.as_ref()?.change_id.clone())
                };
                let (sel, anchor) = (
                    id_at(&self.rows, Some(self.sel)),
                    id_at(&self.rows, self.anchor),
                );
                self.rows = rows;
                self.error = None;
                let find = |id: Option<String>| {
                    let id = id?;
                    self.rows
                        .iter()
                        .position(|r| r.commit.as_ref().is_some_and(|c| c.change_id == id))
                };
                let working_copy = std::mem::take(&mut self.to_working_copy)
                    .then(|| {
                        self.rows
                            .iter()
                            .position(|r| r.commit.as_ref().is_some_and(|c| c.working_copy))
                    })
                    .flatten();
                self.sel = working_copy
                    .or_else(|| find(sel))
                    .or_else(|| self.commits().first().copied())
                    .unwrap_or(0);
                self.anchor = find(anchor);
                if let Some(old) = self.snapshots.take() {
                    self.snapshots = Some(self.all_versions(old));
                }
            }
            Err(e) => self.error = Some(format!("{e:#}")),
        }
    }

    /// A change's versions for every commit row, keeping those already loaded.
    fn all_versions(&self, mut old: Vec<Versions>) -> Vec<Versions> {
        self.commits()
            .into_iter()
            .filter_map(|i| {
                let rev = self.id(i)?;
                let mut v = match old.iter().position(|v| v.rev() == rev) {
                    Some(k) => old.swap_remove(k),
                    None => Versions::new(i, rev),
                };
                v.row = i;
                Some(v)
            })
            .collect()
    }

    /// The indexes of commits' rows.
    fn commits(&self) -> Vec<usize> {
        self.rows
            .iter()
            .enumerate()
            .filter(|(_, r)| r.commit.is_some())
            .map(|(i, _)| i)
            .collect()
    }

    /// The selected rows, newest first.
    fn range(&self) -> (usize, usize) {
        let a = self.anchor.unwrap_or(self.sel);
        (a.min(self.sel), a.max(self.sel))
    }

    /// How a review names the row's commit: by its name when it has one, else as jj tells it
    /// apart.
    fn id(&self, i: usize) -> Option<String> {
        let c = self.rows.get(i)?.commit.as_ref()?;
        if let Some(name) = &c.name {
            return Some(name.clone());
        }
        Some(if c.divergent {
            c.commit_id.clone()
        } else {
            c.change_id.clone()
        })
    }

    /// The changes whose versions are open.
    pub fn versions_revs(&self) -> Vec<String> {
        self.snapshots
            .iter()
            .flatten()
            .map(|v| v.rev().to_string())
            .collect()
    }

    /// Takes the versions of the change `rev`, each with its label.
    pub fn versions_loaded(
        &mut self,
        rev: &str,
        result: anyhow::Result<Vec<(lgtm_core::jj::Version, Option<String>)>>,
    ) {
        let Some(v) = self.snapshots.iter_mut().flatten().find(|v| v.rev() == rev) else {
            return;
        };
        match result {
            Ok(list) => {
                v.error = None;
                v.loaded(list);
            }
            Err(e) => v.error = Some(format!("{e:#}")),
        }
        // The cursor's snapshot may be gone.
        if self.on.as_ref().is_some_and(|(r, _)| r == rev) {
            self.on = None;
        }
    }

    /// The revset for the selection: the commit, or the range from its oldest to its newest.
    fn revset(&self) -> Option<String> {
        let (newest, oldest) = self.range();
        let (newest, oldest) = (self.id(newest)?, self.id(oldest)?);
        Some(if newest == oldest {
            newest
        } else {
            format!("{oldest}::{newest}")
        })
    }

    /// The commit Enter moves the working copy to before reviewing it: the cursor's, when it's
    /// one commit, not a range or a snapshot, and jj may edit it and isn't already on it.
    fn to_edit(&self) -> Option<String> {
        if self.on.is_some() || self.anchored() {
            return None;
        }
        let c = self.rows.get(self.sel)?.commit.as_ref()?;
        if c.working_copy || c.immutable {
            return None;
        }
        self.id(self.sel)
    }

    /// The versions of the change the cursor's on a snapshot of.
    fn on_versions(&mut self) -> Option<(&mut Versions, usize)> {
        let (rev, j) = self.on.clone()?;
        let v = self
            .snapshots
            .iter_mut()
            .flatten()
            .find(|v| v.rev() == rev)?;
        Some((v, j))
    }

    /// Clears every range, of rows and of snapshots.
    fn clear_ranges(&mut self) {
        self.anchor = None;
        self.snapshots
            .iter_mut()
            .flatten()
            .for_each(Versions::clear_anchor);
    }

    pub fn anchored(&self) -> bool {
        self.anchor.is_some() || self.snapshots.iter().flatten().any(Versions::anchored)
    }

    /// Moves the cursor to `item`. A snapshot range ends when the cursor leaves its change.
    fn go(&mut self, item: Item) {
        let left = self.on.as_ref().map(|(rev, _)| rev.clone());
        match item {
            Item::Row(i) => {
                self.sel = i;
                self.on = None;
            }
            Item::Version(k, j) => {
                let Some(v) = self.snapshots.as_mut().and_then(|s| s.get_mut(k)) else {
                    return;
                };
                v.select(j);
                self.sel = v.row;
                self.on = Some((v.rev().to_string(), j));
            }
        }
        let now = self.on.as_ref().map(|(rev, _)| rev);
        if left.is_some() && left.as_ref() != now {
            self.snapshots
                .iter_mut()
                .flatten()
                .for_each(Versions::clear_anchor);
        }
    }

    fn step(&mut self, down: bool) {
        let (items, cursor) = self.items();
        let is_line = |item: &Item| match *item {
            Item::Row(i) => self.rows[i].commit.is_some(),
            Item::Version(..) => true,
        };
        let next = if down {
            items[cursor + 1..].iter().find(|it| is_line(it))
        } else {
            items[..cursor].iter().rev().find(|it| is_line(it))
        };
        if let Some(&item) = next {
            self.go(item);
        }
    }

    /// Opens every change's snapshots, or closes them.
    fn toggle_snapshots(&mut self) -> Outcome {
        if self.snapshots.take().is_some() {
            self.on = None;
            return Outcome::None;
        }
        self.anchor = None;
        self.snapshots = Some(self.all_versions(vec![]));
        Outcome::Versions(self.versions_revs())
    }

    /// Shows every version in each change's evolog, or only the labeled ones.
    fn toggle_all(&mut self) {
        let Some(list) = self.snapshots.as_mut() else {
            return;
        };
        let all = !list.first().is_some_and(|v| v.all);
        list.iter_mut().for_each(|v| v.set_all(all));
        let at = self.on_versions().and_then(|(v, _)| v.at());
        self.on = at.and_then(|j| Some((self.on.take()?.0, j)));
    }

    /// Starts a range where the cursor is, if there's none yet, and moves its other end.
    fn extend(&mut self, down: bool) {
        let unanchored = self.anchor.is_none();
        match self.on_versions() {
            Some((v, _)) => {
                if !v.anchored() {
                    v.toggle_anchor();
                }
                self.anchor = None;
            }
            None if unanchored => {
                self.clear_ranges();
                self.anchor = Some(self.sel);
            }
            None => {}
        }
        self.step(down);
    }

    /// What the jj menu acts on: the cursor's commit, the picked range, and the cursor
    /// commit's description. `None` before the log loads.
    pub fn motion_target(&self) -> Option<motions::Target> {
        Some(motions::Target {
            cursor: self.id(self.sel)?,
            picked: self.revset()?,
            description: self
                .rows
                .get(self.sel)?
                .commit
                .as_ref()?
                .description
                .clone(),
        })
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Char('v') if !self.git => return self.toggle_snapshots(),
            KeyCode::Char('a') if !self.git => self.toggle_all(),
            KeyCode::Char('j') | KeyCode::Down => self.step(true),
            KeyCode::Char('k') | KeyCode::Up => self.step(false),
            KeyCode::Char(c @ ('J' | 'K')) => self.extend(c == 'J'),
            KeyCode::Esc if self.anchored() => self.clear_ranges(),
            KeyCode::Esc => {
                if self.snapshots.is_some() {
                    return self.toggle_snapshots();
                }
            }
            KeyCode::Enter | KeyCode::Char('r') => {
                let edit = (key.code == KeyCode::Enter)
                    .then(|| self.to_edit())
                    .flatten();
                let target = match self.on_versions() {
                    Some((v, _)) => v.open(),
                    None => self.revset().map(Target::Revset),
                };
                match (target, edit) {
                    (Some(target), Some(rev)) => return Outcome::EditOpen { rev, target },
                    (Some(t), None) => return Outcome::Open(t),
                    _ => {}
                }
            }
            _ => {}
        }
        Outcome::None
    }

    /// Whether `(x, y)` is in the log as last drawn.
    pub fn contains(&self, x: u16, y: u16) -> bool {
        self.area.contains(ratatui::layout::Position { x, y })
    }

    /// A click puts the cursor on the item at row `y`, and a second click on it opens it as
    /// Enter does.
    pub fn click(&mut self, y: u16) -> Outcome {
        let (lines, _) = self.lines();
        let at = self.top + y.saturating_sub(self.area.y + 1) as usize;
        let Some(&(item, _)) = lines.get(at) else {
            return Outcome::None;
        };
        if matches!(item, Item::Row(i) if self.rows[i].commit.is_none()) {
            return Outcome::None;
        }
        let now = std::time::Instant::now();
        let double = self
            .clicked
            .is_some_and(|(was, when)| was == item && now - when < DOUBLE_CLICK);
        self.clicked = (!double).then_some((item, now));
        self.clear_ranges();
        self.go(item);
        if double {
            return self.key(KeyEvent::from(KeyCode::Enter));
        }
        Outcome::None
    }

    /// The wheel moves the cursor.
    pub fn scroll(&mut self, down: bool) {
        self.step(down);
    }

    pub fn hints(&self) -> Vec<Hint<'static>> {
        let mut hints = match self.git {
            true => vec![("Enter", "review", false)],
            false => vec![("Enter", "edit", false), ("r", "review", false)],
        };
        hints.extend([("J K", "range", false), ("Space", "menu", false)]);
        if self.anchored() {
            hints.push(("Esc", "clear", false));
        }
        match &self.snapshots {
            Some(list) => {
                let all = list.first().is_some_and(|v| v.all);
                hints.push(("a", "evolog", all));
                if !self.anchored() {
                    hints.push(("Esc", "close", false));
                }
            }
            None if !self.git => hints.push(("v", "snapshots", false)),
            None => {}
        }
        hints.extend([("?", "keys", false), (":q", "quit", false)]);
        hints
    }

    /// The picker's lines, and which one the cursor is on.
    fn items(&self) -> (Vec<Item>, usize) {
        let mut items = Vec::new();
        let (mut row, mut on) = (0, None);
        let list = self.snapshots.as_deref().unwrap_or_default();
        for i in 0..self.rows.len() {
            if i == self.sel {
                row = items.len();
            }
            items.push(Item::Row(i));
            for (k, v) in list.iter().enumerate().filter(|(_, v)| v.row == i) {
                for j in 0..v.shown().len() {
                    if self
                        .on
                        .as_ref()
                        .is_some_and(|(r, at)| r == v.rev() && *at == j)
                    {
                        on = Some(items.len());
                    }
                    items.push(Item::Version(k, j));
                }
            }
        }
        (items, on.unwrap_or(row))
    }

    /// The picker's screen lines, each an item and which of its lines it is, and the first
    /// line of the cursor's item. A commit takes two lines, like jj's: its change, then its
    /// description.
    fn lines(&self) -> (Vec<(Item, u8)>, usize) {
        let (items, cursor) = self.items();
        let mut lines = Vec::with_capacity(items.len() * 2);
        let mut at = 0;
        for (n, item) in items.into_iter().enumerate() {
            if n == cursor {
                at = lines.len();
            }
            lines.push((item, 0));
            if matches!(item, Item::Row(i) if self.rows[i].commit.is_some()) {
                lines.push((item, 1));
            }
        }
        (lines, at)
    }

    /// Draws the log into `body`, the cursor's row lit when `focused`.
    pub fn draw(&mut self, c: &mut Canvas, body: Rect, focused: bool) {
        self.area = body;
        let x0 = body.x;
        if let Some(e) = &self.error {
            c.put_lines(x0 + 1, body.y + 1, e, theme::fg(RED));
            return;
        }
        let height = body.height.saturating_sub(1) as usize;
        let (lines, cursor) = self.lines();
        let cursor_item = lines.get(cursor).map(|&(item, _)| item);
        let cursor_end = cursor
            + lines
                .iter()
                .skip(cursor)
                .take_while(|l| Some(l.0) == cursor_item)
                .count();
        if cursor < self.top {
            self.top = cursor;
        } else if height > 0 && cursor_end > self.top + height {
            self.top = cursor_end.saturating_sub(height);
        }
        let list = self.snapshots.as_deref().unwrap_or_default();
        let shown: Vec<_> = list.iter().map(Versions::shown).collect();
        let now = now();
        for (n, &(item, line)) in lines.iter().skip(self.top).take(height).enumerate() {
            let y = body.y + 1 + n as u16;
            let bg = match Some(item) == cursor_item {
                true if focused => SEL,
                true => PANEL,
                false => BG,
            };
            c.fill(x0, y, body.width, bg);
            match item {
                Item::Row(i) if line == 0 => self.draw_row(c, x0, y, i, bg, now),
                Item::Row(i) => self.draw_description(c, x0, y, i, bg),
                Item::Version(k, j) => draw_version(c, x0, y, &shown[k][j], bg),
            }
        }
        let error = list.iter().find_map(|v| v.error.as_deref());
        if let Some(e) = error {
            let y = body.y + 1 + lines.len().saturating_sub(self.top).min(height) as u16;
            c.put_lines(x0 + 1, y, e, theme::fg(RED));
        }
    }

    fn draw_row(
        &self,
        c: &mut Canvas,
        x0: u16,
        y: u16,
        i: usize,
        bg: ratatui::style::Color,
        now: f64,
    ) {
        let row = &self.rows[i];
        let w = c.area.width;
        let (first, last) = self.range();
        if self.anchor.is_some() && (first..=last).contains(&i) {
            let mark = match i {
                _ if first == last => "▸",
                _ if i == first => "┌",
                _ if i == last => "└",
                _ => "│",
            };
            c.put(x0 + 1, y, mark, theme::fg(ACCENT).bg(bg));
        }
        let Some(commit) = &row.commit else {
            draw_graph(c, x0 + 3, y, &row.graph[0], bg, None);
            return;
        };
        let mut x = draw_graph(c, x0 + 3, y, &row.graph[0], bg, Some(commit));
        let wc = commit.working_copy;
        let style = |color: Color| jj_style(color, wc, bg);
        x += 2;
        let id: String = commit.change_id.chars().take(8).collect();
        let unique = (commit.unique as usize).min(id.len());
        let change = bright(Color::Magenta, wc);
        x = c.put(
            x,
            y,
            &id[..unique],
            style(change).add_modifier(Modifier::BOLD),
        );
        x = c.put(x, y, &id[unique..], style(Color::DarkGray));
        let root = commit.commit_id.bytes().all(|b| b == b'0');
        if root {
            c.put(x + 1, y, "root()", style(Color::Green));
            return;
        }
        let when = ago(now - commit.time);
        let room = x0 + w - (when.len() as u16 + 3);
        let names = commit.bookmarks.iter().map(|b| (b.clone(), Color::Magenta));
        let spaces = commit
            .workspaces
            .iter()
            .map(|n| (format!("{n}@"), Color::Green));
        for (name, color) in names.chain(spaces) {
            if x + 1 >= room {
                break;
            }
            let mut sub = Canvas::new(&mut *c.buf, Rect::new(x + 1, y, room - x - 1, 1));
            x = sub.put(x + 1, y, &name, style(bright(color, wc)));
        }
        c.put_right(y, 1, &when, style(bright(Color::Cyan, wc)));
    }

    /// A commit's second line: its graph, then its description's first line.
    fn draw_description(
        &self,
        c: &mut Canvas,
        x0: u16,
        y: u16,
        i: usize,
        bg: ratatui::style::Color,
    ) {
        let row = &self.rows[i];
        let (first, last) = self.range();
        if self.anchor.is_some() && first <= i && i < last {
            c.put(x0 + 1, y, "│", theme::fg(ACCENT).bg(bg));
        }
        let graph = row.graph.get(1).map_or("", String::as_str);
        let x = draw_graph(c, x0 + 3, y, graph, bg, None);
        let Some(commit) = &row.commit else { return };
        if commit.commit_id.bytes().all(|b| b == b'0') {
            return;
        }
        let wc = commit.working_copy;
        let style = |color: Color| jj_style(color, wc, bg);
        let green = style(bright(Color::Green, wc));
        let summary = commit.description.lines().next().unwrap_or("");
        let (text, look) = match summary {
            "" if commit.empty => ("(no description set)", green),
            "" => ("(no description set)", style(Color::Yellow)),
            s => (s, style(Color::Reset)),
        };
        let x = (x + 2).max(x0 + 3 + width_of(&row.graph[0]) + 2);
        let right = x0 + c.area.width - 1;
        if x < right {
            let mut sub = Canvas::new(&mut *c.buf, Rect::new(x, y, right - x, 1));
            let x = match commit.empty {
                true => sub.put(x, y, "(empty)", green) + 1,
                false => x,
            };
            sub.put(x, y, text, look);
        }
    }
}

/// Draws a line of jj's graph from `x`, `commit`'s node in jj's colors. Returns where it
/// ends.
fn draw_graph(
    c: &mut Canvas,
    mut x: u16,
    y: u16,
    graph: &str,
    bg: Color,
    commit: Option<&LogCommit>,
) -> u16 {
    for ch in graph.trim_end().chars() {
        let node = match commit {
            Some(_) if ch == '@' => Some(Color::Green),
            Some(k) if ch == '×' || k.conflict && "○◆@".contains(ch) => Some(Color::Red),
            Some(_) if ch == '◆' => Some(Color::LightCyan),
            _ => None,
        };
        let look = match node {
            Some(color) => Style::new().fg(color).bg(bg).add_modifier(Modifier::BOLD),
            None => Style::new().fg(Color::Reset).bg(bg),
        };
        x = c.put(x, y, &ch.to_string(), look);
    }
    x
}

/// jj's look for a field of a commit's lines: bold on the working copy.
fn jj_style(color: Color, working_copy: bool, bg: Color) -> Style {
    let style = Style::new().fg(color).bg(bg);
    match working_copy {
        true => style.add_modifier(Modifier::BOLD),
        false => style,
    }
}

/// The bright form of an ANSI color, which jj uses on the working copy.
fn bright(color: Color, working_copy: bool) -> Color {
    match (color, working_copy) {
        (Color::Magenta, true) => Color::LightMagenta,
        (Color::Green, true) => Color::LightGreen,
        (Color::Cyan, true) => Color::LightCyan,
        (color, _) => color,
    }
}

/// How many columns a line of graph takes, without its trailing spaces.
fn width_of(graph: &str) -> u16 {
    graph.trim_end().chars().count() as u16
}

/// A version under its change's row: its time, commit and label.
fn draw_version(c: &mut Canvas, x0: u16, y: u16, v: &versions::Shown, bg: ratatui::style::Color) {
    if let Some(mark) = v.mark {
        c.put(x0 + 1, y, mark, theme::fg(ACCENT).bg(bg));
    }
    c.put(x0 + 3, y, "│", theme::fg(MUTED).bg(bg));
    let (dot, color) = match v.label {
        Some(_) => ("◇", AMBER),
        None => ("·", FAINT),
    };
    c.put(x0 + 6, y, dot, theme::fg(color).bg(bg));
    let id: String = v.version.commit_id.chars().take(8).collect();
    let x = c.put(x0 + 8, y, &id, theme::fg(MUTED).bg(bg));
    if let Some(label) = v.label {
        c.put(x + 2, y, label, theme::fg(FG).bg(bg));
    }
    c.put_right(y, 1, &v.version.clock, theme::fg(MUTED).bg(bg));
}

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// A short age, like jj's: `5m`, `3h`, `2d`.
fn ago(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    match s {
        0..60 => format!("{s}s"),
        60..3600 => format!("{}m", s / 60),
        3600..86400 => format!("{}h", s / 3600),
        86400..604800 => format!("{}d", s / 86400),
        604800..2592000 => format!("{}w", s / 604800),
        2592000..31536000 => format!("{}mo", s / 2592000),
        _ => format!("{}y", s / 31536000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lgtm_core::jj::LogCommit;
    use ratatui::crossterm::event::KeyModifiers;

    fn commit(change: &str, divergent: bool) -> LogRow {
        LogRow {
            graph: vec!["○  ".into()],
            commit: Some(LogCommit {
                commit_id: format!("{change}-commit"),
                change_id: change.into(),
                unique: 2,
                name: None,
                description: String::new(),
                bookmarks: vec![],
                author: String::new(),
                time: 0.0,
                working_copy: false,
                immutable: false,
                conflict: false,
                divergent,
                empty: false,
                workspaces: vec![],
            }),
        }
    }

    fn press(p: &mut Picker, code: KeyCode) -> Outcome {
        p.key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn opened(o: Outcome) -> String {
        match o {
            Outcome::Open(Target::Revset(r))
            | Outcome::EditOpen {
                target: Target::Revset(r),
                ..
            } => r,
            _ => panic!("nothing opened"),
        }
    }

    #[test]
    fn enter_opens_the_commit_or_the_range_oldest_first() {
        let graph_only = LogRow {
            graph: vec!["│".into()],
            commit: None,
        };
        let mut p = Picker::default();
        p.loaded(Ok(vec![
            commit("aaa", false),
            graph_only,
            commit("bbb", true),
            commit("ccc", false),
        ]));
        press(&mut p, KeyCode::Char('j'));
        // The line of graph alone is skipped.
        assert_eq!(opened(press(&mut p, KeyCode::Enter)), "bbb-commit");
        press(&mut p, KeyCode::Char('J'));
        assert_eq!(opened(press(&mut p, KeyCode::Enter)), "ccc::bbb-commit");
        press(&mut p, KeyCode::Esc);
        assert_eq!(opened(press(&mut p, KeyCode::Enter)), "ccc");
    }

    #[test]
    fn a_new_change_takes_the_cursor_once_and_other_reloads_keep_it() {
        let log = |at: &str| {
            ["new", "aaa", "bbb"]
                .iter()
                .map(|c| {
                    let mut row = commit(c, false);
                    row.commit.as_mut().unwrap().working_copy = *c == at;
                    row
                })
                .collect::<Vec<_>>()
        };
        let mut p = Picker::default();
        p.loaded(Ok(log("bbb")));
        press(&mut p, KeyCode::Char('j'));
        p.to_working_copy = true;
        p.loaded(Ok(log("new")));
        assert_eq!(p.id(p.sel).as_deref(), Some("new"));
        press(&mut p, KeyCode::Char('j'));
        p.loaded(Ok(log("new")));
        assert_eq!(p.id(p.sel).as_deref(), Some("aaa"));
    }

    #[test]
    fn enter_edits_one_mutable_commit_off_the_working_copy_and_r_only_reviews() {
        let mut at = commit("www", false);
        let mut root = commit("zzz", false);
        if let (Some(a), Some(z)) = (at.commit.as_mut(), root.commit.as_mut()) {
            a.working_copy = true;
            z.immutable = true;
        }
        let mut p = Picker::default();
        p.loaded(Ok(vec![at, commit("aaa", false), root]));
        let edits = |o: Outcome| match o {
            Outcome::EditOpen { rev, .. } => Some(rev),
            Outcome::Open(_) => None,
            _ => panic!("nothing opened"),
        };
        assert_eq!(edits(press(&mut p, KeyCode::Enter)), None);
        press(&mut p, KeyCode::Char('j'));
        assert_eq!(edits(press(&mut p, KeyCode::Enter)), Some("aaa".into()));
        assert_eq!(edits(press(&mut p, KeyCode::Char('r'))), None);
        press(&mut p, KeyCode::Char('J'));
        assert_eq!(edits(press(&mut p, KeyCode::Enter)), None);
        press(&mut p, KeyCode::Esc);
        press(&mut p, KeyCode::Char('j'));
        assert_eq!(edits(press(&mut p, KeyCode::Enter)), None);
    }

    #[test]
    fn a_commit_takes_two_lines_with_its_join_on_the_second() {
        let mut fork = commit("bbb", false);
        fork.graph = vec!["│ ○  ".into(), "├─╯  ".into()];
        let mut plain = commit("ccc", false);
        plain.graph = vec!["○  ".into(), "│  ".into()];
        let mut p = Picker::default();
        p.loaded(Ok(vec![fork, plain]));
        let (lines, cursor) = p.lines();
        let shown: Vec<_> = lines.iter().map(|&(_, line)| line).collect();
        assert_eq!(shown, [0, 1, 0, 1]);
        assert_eq!(cursor, 0);
        press(&mut p, KeyCode::Char('j'));
        assert_eq!(p.lines().1, 2);
        assert_eq!(opened(press(&mut p, KeyCode::Enter)), "ccc");
    }

    #[test]
    fn a_click_on_either_line_of_a_commit_picks_it_and_a_second_click_opens_it() {
        let mut p = Picker::default();
        p.loaded(Ok(vec![commit("aaa", false), commit("bbb", false)]));
        // The log's first line is under its title row.
        p.area = Rect::new(0, 5, 40, 10);
        assert!(matches!(p.click(8), Outcome::None));
        assert!(matches!(p.click(7), Outcome::None));
        assert_eq!(opened(p.click(6)), "aaa");
        // Below the last line does nothing.
        assert!(matches!(p.click(10), Outcome::None));
        assert_eq!(opened(press(&mut p, KeyCode::Enter)), "aaa");
    }

    fn version(id: &str, label: Option<&str>) -> (lgtm_core::jj::Version, Option<String>) {
        let v = lgtm_core::jj::Version {
            commit_id: id.into(),
            clock: "15:00".into(),
        };
        (v, label.map(str::to_string))
    }

    #[test]
    fn s_shows_every_changes_snapshots_and_j_walks_through_them() {
        let mut p = Picker::default();
        p.loaded(Ok(vec![commit("aaa", false), commit("bbb", false)]));
        match press(&mut p, KeyCode::Char('v')) {
            Outcome::Versions(revs) => assert_eq!(revs, ["aaa", "bbb"]),
            _ => panic!("no versions asked for"),
        }
        p.versions_loaded("aaa", Ok(vec![version("a1", Some("one"))]));
        p.versions_loaded(
            "bbb",
            Ok(vec![version("b2", None), version("b1", Some("two"))]),
        );
        press(&mut p, KeyCode::Char('j'));
        assert!(matches!(
            press(&mut p, KeyCode::Enter),
            Outcome::Open(Target::Between { from, .. }) if from == "a1"
        ));
        press(&mut p, KeyCode::Char('j'));
        assert_eq!(opened(press(&mut p, KeyCode::Enter)), "bbb");
        // The unlabeled version stays hidden.
        press(&mut p, KeyCode::Char('j'));
        assert!(matches!(
            press(&mut p, KeyCode::Enter),
            Outcome::Open(Target::Between { from, to, .. }) if from == "b1" && to == "bbb"
        ));
        press(&mut p, KeyCode::Char('v'));
        assert_eq!(opened(press(&mut p, KeyCode::Enter)), "bbb");
    }

    #[test]
    fn ages_round_down_to_the_largest_unit() {
        assert_eq!(ago(59.0), "59s");
        assert_eq!(ago(7200.0), "2h");
        assert_eq!(ago(86400.0 * 9.0), "1w");
    }
}
