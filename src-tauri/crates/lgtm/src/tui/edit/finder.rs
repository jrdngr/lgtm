//! The lists `space` opens over the editor: the repo's files, the open buffers, and the lines
//! of the repo that hold some text. Typing narrows the list.

use std::collections::HashMap;
use std::path::Path;

use lgtm_core::fuzzy;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

use crate::edit::keymap::Pick;
use crate::tui::paint::Canvas;
use crate::tui::theme::{self, ACCENT, FG, HAIR, MUTED, PANEL, SEL};

/// The most lines a search lists.
const MAX_LINES: usize = 500;
/// Files bigger than this aren't searched.
const MAX_FILE: u64 = 1 << 20;

/// Something to pick: a file, and in a search the line from 1 and its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub path: String,
    pub line: Option<u32>,
    text: String,
    /// The chars of the path, or of the text in a search, that matched.
    hits: Vec<usize>,
    dirty: bool,
}

pub enum Outcome {
    None,
    Close,
    Open(String, u32),
}

pub struct Finder {
    pub kind: Pick,
    query: String,
    /// What there is to pick from, each with whether it has unsaved edits.
    all: Vec<(String, bool)>,
    /// The text of each file a search reads, by path.
    texts: HashMap<String, String>,
    items: Vec<Item>,
    sel: usize,
    top: usize,
}

impl Finder {
    /// A list of `kind` over `paths`, the files in the workspace at `root` or the buffers.
    pub fn new(kind: Pick, root: &Path, paths: Vec<(String, bool)>) -> Self {
        let texts = match kind {
            Pick::Grep => read_all(root, &paths),
            _ => HashMap::new(),
        };
        let mut f = Self {
            kind,
            query: String::new(),
            all: paths,
            texts,
            items: vec![],
            sel: 0,
            top: 0,
        };
        f.narrow();
        f
    }

    fn narrow(&mut self) {
        self.sel = 0;
        self.top = 0;
        self.items = match self.kind {
            Pick::Files | Pick::Buffers if self.query.trim().is_empty() => self
                .all
                .iter()
                .map(|(p, dirty)| item(p, None, String::new(), vec![], *dirty))
                .collect(),
            Pick::Files | Pick::Buffers => fuzzy::rank(&self.query, self.all.iter(), |(p, _)| p)
                .into_iter()
                .map(|((p, dirty), m)| item(p, None, String::new(), m.positions, *dirty))
                .collect(),
            Pick::Grep => self.grep(),
        };
    }

    /// Lines holding the query, matching case only when it has a capital.
    fn grep(&self) -> Vec<Item> {
        if self.query.is_empty() {
            return vec![];
        }
        let exact = self.query.chars().any(char::is_uppercase);
        let fold = |s: &str| {
            if exact {
                s.to_string()
            } else {
                s.to_lowercase()
            }
        };
        let query = fold(&self.query);
        let mut out = Vec::new();
        for (path, dirty) in &self.all {
            let Some(text) = self.texts.get(path) else {
                continue;
            };
            for (n, line) in text.lines().enumerate() {
                let Some(at) = fold(line).find(&query) else {
                    continue;
                };
                let trimmed = line.trim_start();
                let indent = line.chars().count() - trimmed.chars().count();
                // Lowercasing can change byte lengths, so count chars in the folded line.
                let start = fold(line)[..at].chars().count().saturating_sub(indent);
                let hits = (start..start + query.chars().count()).collect();
                out.push(item(
                    path,
                    Some(n as u32 + 1),
                    trimmed.to_string(),
                    hits,
                    *dirty,
                ));
                if out.len() >= MAX_LINES {
                    return out;
                }
            }
        }
        out
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let last = self.items.len().saturating_sub(1);
        match key.code {
            KeyCode::Esc => return Outcome::Close,
            KeyCode::Char('c') if ctrl => return Outcome::Close,
            KeyCode::Enter => {
                return match self.items.get(self.sel) {
                    Some(it) => Outcome::Open(it.path.clone(), it.line.unwrap_or(0)),
                    None => Outcome::None,
                };
            }
            KeyCode::Down => self.sel = (self.sel + 1).min(last),
            KeyCode::Up => self.sel = self.sel.saturating_sub(1),
            KeyCode::Char('n' | 'j') if ctrl => self.sel = (self.sel + 1).min(last),
            KeyCode::Char('p' | 'k') if ctrl => self.sel = self.sel.saturating_sub(1),
            KeyCode::Backspace => {
                self.query.pop();
                self.narrow();
            }
            KeyCode::Char(c) if !ctrl => {
                self.query.push(c);
                self.narrow();
            }
            _ => {}
        }
        Outcome::None
    }

    /// Draws the list in a box over the middle of `area`.
    pub fn draw(&mut self, c: &mut Canvas, area: Rect) {
        let share = if self.kind == Pick::Grep { 3 } else { 2 };
        let w = (area.width * share / 5)
            .max(60)
            .min(area.width.saturating_sub(2));
        let rows = match self.kind {
            Pick::Buffers => self.items.len().max(1) as u16,
            _ => (area.height / 2).max(4),
        };
        let h = (rows + 4).min(area.height);
        let x0 = area.x + (area.width - w) / 2;
        let y0 = area.y + (area.height - h) / 2;
        let border = theme::fg(HAIR).bg(PANEL);
        for y in y0..y0 + h {
            c.fill(x0, y, w, PANEL);
            c.put(x0, y, "│", border);
            c.put(x0 + w - 1, y, "│", border);
        }
        let rule = "─".repeat(w.saturating_sub(2) as usize);
        c.put(x0, y0, &format!("╭{rule}╮"), border);
        c.put(x0, y0 + h - 1, &format!("╰{rule}╯"), border);
        let x = c.put(x0 + 2, y0 + 1, "› ", theme::bold(ACCENT).bg(PANEL));
        let x = c.put(x, y0 + 1, &self.query, theme::fg(FG).bg(PANEL));
        c.put(x, y0 + 1, " ", theme::fg(FG).bg(ACCENT));
        let count = match self.kind {
            Pick::Files => Some(format!("{} / {}", self.items.len(), self.all.len())),
            Pick::Grep if !self.query.is_empty() => Some(self.items.len().to_string()),
            _ => None,
        };
        if let Some(count) = count {
            let cx = x0 + w - 2 - count.chars().count() as u16;
            c.put(cx, y0 + 1, &count, theme::fg(MUTED).bg(PANEL));
        }
        let height = h.saturating_sub(4) as usize;
        if self.sel < self.top {
            self.top = self.sel;
        } else if height > 0 && self.sel >= self.top + height {
            self.top = self.sel + 1 - height;
        }
        let inner = Rect::new(x0 + 1, y0, w.saturating_sub(2), h);
        let mut sub = Canvas::new(&mut *c.buf, inner);
        let name_w = self
            .items
            .iter()
            .map(|it| name_of(&it.path).chars().count() + if it.dirty { 2 } else { 0 })
            .max()
            .unwrap_or(0)
            .clamp(14, 30) as u16;
        for (n, it) in self.items.iter().enumerate().skip(self.top).take(height) {
            let y = y0 + 2 + (n - self.top) as u16;
            let bg = if n == self.sel { SEL } else { PANEL };
            sub.fill(inner.x, y, inner.width, bg);
            match self.kind {
                Pick::Grep => draw_line(&mut sub, inner.x + 1, y, it, bg),
                _ => draw_file(&mut sub, inner.x + 1, y, it, name_w, bg),
            }
        }
    }
}

fn item(path: &str, line: Option<u32>, text: String, hits: Vec<usize>, dirty: bool) -> Item {
    Item {
        path: path.to_string(),
        line,
        text,
        hits,
        dirty,
    }
}

fn name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// A file's name, bold, then its folder.
fn draw_file(c: &mut Canvas, x0: u16, y: u16, it: &Item, name_w: u16, bg: ratatui::style::Color) {
    let name_start = it.path.len() - name_of(&it.path).len();
    let name_start = it.path[..name_start].chars().count();
    let chars: Vec<char> = it.path.chars().collect();
    let mut x = x0;
    for (i, ch) in chars.iter().enumerate().skip(name_start) {
        let color = if it.hits.contains(&i) { ACCENT } else { FG };
        x = c.put(x, y, &ch.to_string(), theme::bold(color).bg(bg));
    }
    if it.dirty {
        c.put(x + 1, y, "●", theme::fg(FG).bg(bg));
    }
    let mut x = x0 + name_w + 2;
    let dir = name_start.saturating_sub(1);
    for (i, ch) in chars.iter().enumerate().take(dir) {
        let color = if it.hits.contains(&i) { ACCENT } else { MUTED };
        x = c.put(x, y, &ch.to_string(), theme::fg(color).bg(bg));
    }
}

/// A search's line: where it is, then its text with the match picked out.
fn draw_line(c: &mut Canvas, x0: u16, y: u16, it: &Item, bg: ratatui::style::Color) {
    let at = format!("{}:{}", it.path, it.line.unwrap_or(0));
    let x = c.put(x0, y, &at, theme::fg(MUTED).bg(bg)) + 2;
    let mut x = x;
    for (i, ch) in it.text.chars().enumerate() {
        let style = if it.hits.contains(&i) {
            theme::bold(ACCENT).bg(bg)
        } else {
            theme::fg(FG).bg(bg)
        };
        let ch = if ch == '\t' { ' ' } else { ch };
        x = c.put(x, y, &ch.to_string(), style);
    }
}

/// The text of each file under `root` that's small enough and not binary.
fn read_all(root: &Path, paths: &[(String, bool)]) -> HashMap<String, String> {
    paths
        .iter()
        .filter_map(|(p, _)| {
            let full = root.join(p);
            let len = std::fs::metadata(&full).ok()?.len();
            if len > MAX_FILE {
                return None;
            }
            let bytes = std::fs::read(&full).ok()?;
            if bytes.contains(&0) {
                return None;
            }
            Some((p.clone(), String::from_utf8(bytes).ok()?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(f: &mut Finder, text: &str) {
        for c in text.chars() {
            f.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
    }

    fn paths(list: &[&str]) -> Vec<(String, bool)> {
        list.iter().map(|p| (p.to_string(), false)).collect()
    }

    #[test]
    fn files_narrow_by_fuzzy_match_and_enter_opens_the_best() {
        let mut f = Finder::new(
            Pick::Files,
            Path::new("."),
            paths(&["src/versions.rs", "src/review/mod.rs", "README.md"]),
        );
        assert_eq!(f.items.len(), 3);
        typed(&mut f, "revmod");
        assert_eq!(f.items.len(), 1);
        let enter = f.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(enter, Outcome::Open(p, 0) if p == "src/review/mod.rs"));
    }

    #[test]
    fn a_search_lists_each_line_holding_the_text() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "fn one() {}\n    let Two = 2;\n").unwrap();
        std::fs::write(dir.path().join("b.rs"), "two\n").unwrap();
        std::fs::write(dir.path().join("bin"), b"two\0").unwrap();
        let mut f = Finder::new(Pick::Grep, dir.path(), paths(&["a.rs", "b.rs", "bin"]));
        typed(&mut f, "two");
        let found: Vec<(&str, Option<u32>, &str)> = f
            .items
            .iter()
            .map(|it| (it.path.as_str(), it.line, it.text.as_str()))
            .collect();
        assert_eq!(
            found,
            [("a.rs", Some(2), "let Two = 2;"), ("b.rs", Some(1), "two")]
        );
        assert_eq!(f.items[0].hits, [4, 5, 6]);
        f.key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        f.key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        f.key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        typed(&mut f, "Two");
        assert_eq!(f.items.len(), 1);
        let enter = f.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(enter, Outcome::Open(p, 2) if p == "a.rs"));
    }
}
