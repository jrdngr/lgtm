//! Picking what to review from the jj log: one commit, or a range picked with Space.

use lgtm_core::jj::LogRow;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;

use super::keybar::Hint;
use super::paint::Canvas;
use super::theme::{self, ACCENT, BG, FAINT, FG, FUCHSIA, GREEN, MUTED, RED, SEL, UNIQUE};

pub enum Outcome {
    None,
    Open(String),
    Quit,
}

#[derive(Default)]
pub struct Picker {
    rows: Vec<LogRow>,
    error: Option<String>,
    /// Index into `rows`, always a commit's row once there is one.
    sel: usize,
    /// The other end of a range, set with Space.
    anchor: Option<usize>,
    top: usize,
}

pub const KEYS: &[(&str, &str)] = &[
    ("j k", "move"),
    ("g G", "first, last"),
    ("Space", "range"),
    ("Esc", "clear range"),
    ("Enter", "open"),
    ("q", "quit"),
];

impl Picker {
    pub fn loaded(&mut self, result: anyhow::Result<Vec<LogRow>>) {
        match result {
            Ok(rows) => {
                self.rows = rows;
                self.error = None;
                self.sel = self.commits().first().copied().unwrap_or(0);
                self.anchor = None;
            }
            Err(e) => self.error = Some(format!("{e:#}")),
        }
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

    /// The revset for the selection: the commit, or the range from its oldest to its newest.
    fn revset(&self) -> Option<String> {
        let (newest, oldest) = self.range();
        let id = |i: usize| {
            let c = self.rows.get(i)?.commit.as_ref()?;
            Some(if c.divergent {
                c.commit_id.clone()
            } else {
                c.change_id.clone()
            })
        };
        let (newest, oldest) = (id(newest)?, id(oldest)?);
        Some(if newest == oldest {
            newest
        } else {
            format!("{oldest}::{newest}")
        })
    }

    fn step(&mut self, down: bool) {
        let next = if down {
            self.commits().into_iter().find(|&i| i > self.sel)
        } else {
            self.commits().into_iter().rev().find(|&i| i < self.sel)
        };
        if let Some(i) = next {
            self.sel = i;
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.step(true),
            KeyCode::Char('k') | KeyCode::Up => self.step(false),
            KeyCode::Char('g') | KeyCode::Home => {
                self.sel = self.commits().first().copied().unwrap_or(0)
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.sel = self.commits().last().copied().unwrap_or(0)
            }
            KeyCode::Char(' ') => {
                self.anchor = match self.anchor {
                    Some(_) => None,
                    None => Some(self.sel),
                }
            }
            KeyCode::Esc => self.anchor = None,
            KeyCode::Enter => {
                if let Some(revset) = self.revset() {
                    return Outcome::Open(revset);
                }
            }
            KeyCode::Char('q') => return Outcome::Quit,
            _ => {}
        }
        Outcome::None
    }

    pub fn hints(&self) -> Vec<Hint<'static>> {
        let mut hints = vec![("Enter", "open", false), ("Space", "range", false)];
        if self.anchor.is_some() {
            hints.push(("Esc", "clear", false));
        }
        hints.extend([("?", "keys", false), ("q", "quit", false)]);
        hints
    }

    pub fn draw(&mut self, c: &mut Canvas, body: Rect) {
        let x0 = body.x;
        let w = body.width;
        if let Some(e) = &self.error {
            c.put_lines(x0 + 1, body.y + 1, e, theme::fg(RED));
            return;
        }
        let height = body.height.saturating_sub(1) as usize;
        if self.sel < self.top {
            self.top = self.sel;
        } else if height > 0 && self.sel >= self.top + height {
            self.top = self.sel + 1 - height;
        }
        let (first, last) = self.range();
        let now = now();
        for (n, (i, row)) in self
            .rows
            .iter()
            .enumerate()
            .skip(self.top)
            .take(height)
            .enumerate()
        {
            let y = body.y + 1 + n as u16;
            let bg = if i == self.sel { SEL } else { BG };
            c.fill(x0, y, w, bg);
            if self.anchor.is_some() && (first..=last).contains(&i) {
                let mark = match i {
                    _ if first == last => "▸",
                    _ if i == first => "┌",
                    _ if i == last => "└",
                    _ => "│",
                };
                c.put(x0 + 1, y, mark, theme::fg(ACCENT).bg(bg));
            }
            let mut x = x0 + 3;
            for ch in row.graph[0].trim_end().chars() {
                let color = match ch {
                    '@' => GREEN,
                    '×' => RED,
                    '◆' | '~' => MUTED,
                    _ => FAINT,
                };
                x = c.put(x, y, &ch.to_string(), theme::fg(color).bg(bg));
            }
            let Some(commit) = &row.commit else { continue };
            x += 2;
            let id: String = commit.change_id.chars().take(8).collect();
            let unique = (commit.unique as usize).min(id.len());
            x = c.put(x, y, &id[..unique], theme::bold(UNIQUE).bg(bg));
            x = c.put(x, y, &id[unique..], theme::fg(MUTED).bg(bg));
            x += 2;
            for b in &commit.bookmarks {
                x = c.put(x, y, b, theme::fg(FUCHSIA).bg(bg)) + 1;
            }
            let root = commit.commit_id.bytes().all(|b| b == b'0');
            let summary = commit.description.lines().next().unwrap_or("");
            let (text, color) = match summary {
                _ if root => ("root()", FG),
                "" => ("(no description set)", MUTED),
                s => (s, FG),
            };
            let when = if root {
                String::new()
            } else {
                ago(now - commit.time)
            };
            let room = x0 + w - (when.len() as u16 + 3);
            if x + 1 < room {
                let mut sub = Canvas::new(&mut *c.buf, Rect::new(x, y, room - x, 1));
                sub.put(x + 1, y, text, theme::fg(color).bg(bg));
            }
            c.put_right(y, 1, &when, theme::fg(MUTED).bg(bg));
        }
    }
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
                description: String::new(),
                bookmarks: vec![],
                author: String::new(),
                time: 0.0,
                working_copy: false,
                immutable: false,
                conflict: false,
                divergent,
                empty: false,
            }),
        }
    }

    fn press(p: &mut Picker, code: KeyCode) -> Outcome {
        p.key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn opened(o: Outcome) -> String {
        match o {
            Outcome::Open(r) => r,
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
        press(&mut p, KeyCode::Char(' '));
        press(&mut p, KeyCode::Char('j'));
        assert_eq!(opened(press(&mut p, KeyCode::Enter)), "ccc::bbb-commit");
        press(&mut p, KeyCode::Esc);
        assert_eq!(opened(press(&mut p, KeyCode::Enter)), "ccc");
    }

    #[test]
    fn ages_round_down_to_the_largest_unit() {
        assert_eq!(ago(59.0), "59s");
        assert_eq!(ago(7200.0), "2h");
        assert_eq!(ago(86400.0 * 9.0), "1w");
    }
}
