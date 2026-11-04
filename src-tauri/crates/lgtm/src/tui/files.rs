//! The repo's files as a tree with folders that open and close, narrowed by a fuzzy search.
//! Home shows it beside the jj log, and the editor beside the text.

use std::collections::{BTreeMap, BTreeSet};

use lgtm_core::fuzzy;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::paint::Canvas;
use super::theme::{self, ACCENT, BG, FG, HAIR, MUTED, SEL};

/// The most files a search shows.
const MAX_SHOWN: usize = 200;

/// A line of the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// A folder, or a chain of folders that each hold only the next, with the path of the
    /// last.
    Dir {
        name: String,
        path: String,
        depth: usize,
        open: bool,
    },
    /// A file, and which chars of its name the search matched.
    File {
        name: String,
        path: String,
        depth: usize,
        hits: Vec<usize>,
    },
}

impl Row {
    pub fn path(&self) -> &str {
        match self {
            Row::Dir { path, .. } | Row::File { path, .. } => path,
        }
    }
}

pub enum Outcome {
    None,
    Open(String),
}

#[derive(Default)]
pub struct FileTree {
    paths: Vec<String>,
    /// Folders opened, by path, while there's no search.
    open: BTreeSet<String>,
    pub query: String,
    rows: Vec<Row>,
    /// How many files the search matched.
    matched: usize,
    sel: usize,
    top: usize,
    /// The cursor's row when last drawn: the view follows the cursor only when it moved, so
    /// the wheel can scroll away from it.
    drawn_sel: Option<usize>,
    /// Where the rows were last drawn, for clicks.
    area: Rect,
}

#[derive(Default)]
struct Node {
    dirs: BTreeMap<String, Node>,
    files: Vec<(String, Vec<usize>)>,
}

impl FileTree {
    pub fn set_paths(&mut self, mut paths: Vec<String>) {
        paths.sort();
        if paths == self.paths {
            return;
        }
        let at = self.selected().map(str::to_string);
        self.paths = paths;
        self.rebuild();
        if let Some(path) = at {
            self.select(&path);
        }
    }

    pub fn paths(&self) -> &[String] {
        &self.paths
    }

    pub fn selected(&self) -> Option<&str> {
        self.rows.get(self.sel).map(Row::path)
    }

    /// Opens the folders down to `path` and puts the cursor on it.
    pub fn reveal(&mut self, path: &str) {
        let mut dir = String::new();
        let mut changed = false;
        for part in path
            .split('/')
            .collect::<Vec<_>>()
            .iter()
            .rev()
            .skip(1)
            .rev()
        {
            if !dir.is_empty() {
                dir.push('/');
            }
            dir.push_str(part);
            changed |= self.open.insert(dir.clone());
        }
        if changed || self.rows.is_empty() {
            self.rebuild();
        }
        self.select(path);
    }

    fn select(&mut self, path: &str) {
        if let Some(i) = self.rows.iter().position(|r| r.path() == path) {
            self.sel = i;
        }
    }

    /// Lays out the rows again from the paths, the open folders and the search.
    fn rebuild(&mut self) {
        let mut root = Node::default();
        let mut best: Option<(i64, String)> = None;
        if self.query.trim().is_empty() {
            self.matched = self.paths.len();
            for p in &self.paths {
                add(&mut root, p, vec![]);
            }
        } else if self.query.contains('/') {
            // A path: every file in the folders it names, or the files it names.
            self.matched = 0;
            for p in &self.paths {
                if let Some(hits) = path_match(self.query.trim(), p) {
                    if best.is_none() {
                        best = Some((0, p.clone()));
                    }
                    self.matched += 1;
                    add(&mut root, p, hits);
                }
            }
        } else {
            let ranked = fuzzy::rank(&self.query, self.paths.iter(), |p| name_of(p));
            self.matched = 0;
            // Matches much weaker than the best are noise in a tree.
            let floor = ranked.first().map_or(0, |(_, m)| m.score * 2 / 3);
            for (p, m) in ranked.into_iter().take(MAX_SHOWN) {
                if m.score < floor {
                    break;
                }
                if best.is_none() {
                    best = Some((m.score, p.clone()));
                }
                self.matched += 1;
                add(&mut root, p, m.positions);
            }
        }
        let searching = !self.query.trim().is_empty();
        let mut rows = Vec::new();
        emit(
            &root,
            "",
            0,
            &|p| searching || self.open.contains(p),
            &mut rows,
        );
        self.rows = rows;
        self.sel = match best {
            Some((_, path)) => self.rows.iter().position(|r| r.path() == path).unwrap_or(0),
            None => self.sel.min(self.rows.len().saturating_sub(1)),
        };
    }

    fn step(&mut self, by: isize) {
        let last = self.rows.len().saturating_sub(1) as isize;
        self.sel = (self.sel as isize + by).clamp(0, last.max(0)) as usize;
    }

    fn set_open(&mut self, path: &str, open: bool) {
        if open {
            self.open.insert(path.to_string());
        } else {
            self.open.remove(path);
        }
        self.rebuild();
        self.select(path);
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let searching = !self.query.trim().is_empty();
        match key.code {
            KeyCode::Down => self.step(1),
            KeyCode::Up => self.step(-1),
            KeyCode::Char('n' | 'j') if ctrl => self.step(1),
            KeyCode::Char('p' | 'k') if ctrl => self.step(-1),
            KeyCode::PageDown => self.step(20),
            KeyCode::PageUp => self.step(-20),
            KeyCode::Home => self.sel = 0,
            KeyCode::End => self.step(isize::MAX / 2),
            KeyCode::Enter => match self.rows.get(self.sel).cloned() {
                Some(Row::File { path, .. }) => return Outcome::Open(path),
                Some(Row::Dir { path, open, .. }) if !searching => self.set_open(&path, !open),
                _ => {}
            },
            KeyCode::Right if !searching => {
                if let Some(Row::Dir {
                    path, open: false, ..
                }) = self.rows.get(self.sel).cloned()
                {
                    self.set_open(&path, true);
                }
            }
            KeyCode::Left if !searching => match self.rows.get(self.sel).cloned() {
                Some(Row::Dir {
                    path, open: true, ..
                }) => self.set_open(&path, false),
                Some(row) => self.go_to_parent(&row),
                None => {}
            },
            KeyCode::Esc => self.set_query(String::new()),
            KeyCode::Backspace => {
                let mut q = self.query.clone();
                q.pop();
                self.set_query(q);
            }
            KeyCode::Char(c) if !ctrl => {
                let q = format!("{}{c}", self.query);
                self.set_query(q);
            }
            _ => {}
        }
        Outcome::None
    }

    /// Forgets where the rows were drawn, when they're not on screen.
    pub fn hide(&mut self) {
        self.area = Rect::default();
    }

    /// Scrolls the rows by `n` without moving the cursor.
    pub fn scroll(&mut self, n: isize) {
        let max = self.rows.len().saturating_sub(self.area.height as usize);
        self.top = (self.top as isize + n).clamp(0, max as isize) as usize;
    }

    /// Whether the rows were last drawn over the cell at `x`, `y`.
    pub fn contains(&self, x: u16, y: u16) -> bool {
        self.area.contains(ratatui::layout::Position::new(x, y))
    }

    /// A click on the screen row `y`: opens a file, or opens or closes a folder.
    pub fn click(&mut self, y: u16) -> Outcome {
        let Some(n) = y.checked_sub(self.area.y) else {
            return Outcome::None;
        };
        let i = self.top + n as usize;
        if i >= self.rows.len() {
            return Outcome::None;
        }
        self.sel = i;
        // The click's row stays where it was clicked.
        self.drawn_sel = Some(i);
        match self.rows[i].clone() {
            Row::File { path, .. } => Outcome::Open(path),
            Row::Dir { path, open, .. } => {
                if self.query.trim().is_empty() {
                    let top = self.top;
                    self.set_open(&path, !open);
                    self.top = top;
                    self.drawn_sel = Some(self.sel);
                }
                Outcome::None
            }
        }
    }

    /// Moves the cursor to the folder that holds `row`.
    fn go_to_parent(&mut self, row: &Row) {
        let depth = match row {
            Row::Dir { depth, .. } | Row::File { depth, .. } => *depth,
        };
        if depth == 0 {
            return;
        }
        let above = self.rows[..self.sel]
            .iter()
            .rposition(|r| matches!(r, Row::Dir { depth: d, .. } if *d < depth));
        if let Some(i) = above {
            self.sel = i;
        }
    }

    pub fn set_query(&mut self, query: String) {
        if query == self.query {
            return;
        }
        let at = self.selected().map(str::to_string);
        self.query = query;
        self.rebuild();
        if self.query.trim().is_empty()
            && let Some(path) = at
        {
            self.reveal(&path);
        }
    }

    /// How many files match, and how many there are, while searching.
    pub fn count(&self) -> Option<String> {
        (!self.query.trim().is_empty()).then(|| format!("{} / {}", self.matched, self.paths.len()))
    }

    /// Draws the rows into `area`. `focused` paints the cursor's row, and `marked` puts ●
    /// after the files it holds.
    pub fn draw(
        &mut self,
        c: &mut Canvas,
        area: Rect,
        focused: bool,
        marked: &dyn Fn(&str) -> bool,
    ) {
        let height = area.height as usize;
        self.area = area;
        if self.drawn_sel != Some(self.sel) {
            self.drawn_sel = Some(self.sel);
            if self.sel < self.top {
                self.top = self.sel;
            } else if height > 0 && self.sel >= self.top + height {
                self.top = self.sel + 1 - height;
            }
        }
        self.top = self.top.min(self.rows.len().saturating_sub(height));
        let mut sub = Canvas::new(&mut *c.buf, area);
        for (n, row) in self.rows.iter().enumerate().skip(self.top).take(height) {
            let y = area.y + (n - self.top) as u16;
            let bg = match (n == self.sel, focused) {
                (true, true) => SEL,
                (true, false) => HAIR,
                _ => BG,
            };
            sub.fill(area.x, y, area.width, bg);
            draw_row(&mut sub, area.x + 1, y, row, bg, marked);
        }
    }
}

fn draw_row(c: &mut Canvas, x0: u16, y: u16, row: &Row, bg: Color, marked: &dyn Fn(&str) -> bool) {
    match row {
        Row::Dir {
            name, depth, open, ..
        } => {
            let arrow = if *open { "▾ " } else { "▸ " };
            let x = x0 + 2 * *depth as u16;
            c.put(x, y, &format!("{arrow}{name}/"), theme::fg(MUTED).bg(bg));
        }
        Row::File {
            name,
            path,
            depth,
            hits,
        } => {
            let mut x = x0 + 2 * *depth as u16;
            for (i, ch) in name.chars().enumerate() {
                let style = if hits.contains(&i) {
                    theme::bold(ACCENT).bg(bg)
                } else {
                    theme::fg(FG).bg(bg)
                };
                x = c.put(x, y, &ch.to_string(), style);
            }
            if marked(path) {
                c.put(x + 1, y, "●", theme::fg(FG).bg(bg));
            }
        }
    }
}

fn name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Whether `path` is in the folders `query` names, `/` between them, each part found in a
/// folder's name and the folders one inside the next. The last part can find a file's name
/// instead, and a `/` at the start ties the first part to the top folder. Returns which chars
/// of the file's name matched.
fn path_match(query: &str, path: &str) -> Option<Vec<usize>> {
    let anchored = query.starts_with('/');
    let parts: Vec<&str> = query
        .trim_start_matches('/')
        .trim_end_matches('/')
        .split('/')
        .collect();
    let exact = query.chars().any(char::is_uppercase);
    let fold = |s: &str| {
        if exact {
            s.to_string()
        } else {
            s.to_lowercase()
        }
    };
    let comps: Vec<&str> = path.split('/').collect();
    let last_comp = comps.len() - 1;
    let starts = if anchored { 0..1 } else { 0..comps.len() };
    for start in starts {
        let fits = parts.iter().enumerate().all(|(i, part)| {
            let at = start + i;
            let last_part = i == parts.len() - 1;
            at <= last_comp
                && (at < last_comp || last_part)
                && fold(comps[at]).contains(&fold(part))
        });
        if !fits {
            continue;
        }
        let end = start + parts.len() - 1;
        if end < last_comp {
            return Some(vec![]);
        }
        // The last part found the file's name.
        let part = fold(parts[parts.len() - 1]);
        let name = fold(comps[last_comp]);
        let at = name.find(&part).map_or(0, |b| name[..b].chars().count());
        return Some((at..at + part.chars().count()).collect());
    }
    None
}

fn add(root: &mut Node, path: &str, hits: Vec<usize>) {
    let mut parts: Vec<&str> = path.split('/').collect();
    let name = parts.pop().unwrap_or_default();
    let mut node = root;
    for dir in parts {
        node = node.dirs.entry(dir.to_string()).or_default();
    }
    node.files.push((name.to_string(), hits));
}

/// Folders first, then files, each by name ignoring case.
fn emit(
    node: &Node,
    parent: &str,
    depth: usize,
    is_open: &dyn Fn(&str) -> bool,
    out: &mut Vec<Row>,
) {
    let join = |a: &str, b: &str| {
        if a.is_empty() {
            b.to_string()
        } else {
            format!("{a}/{b}")
        }
    };
    let mut dirs: Vec<(&String, &Node)> = node.dirs.iter().collect();
    dirs.sort_by_key(|(name, _)| name.to_lowercase());
    for (name, mut dir) in dirs {
        let mut label = name.clone();
        let mut path = join(parent, name);
        // A folder holding only one folder shares its line.
        while dir.files.is_empty() && dir.dirs.len() == 1 {
            let (next, inner) = dir.dirs.iter().next().expect("one folder");
            label = format!("{label}/{next}");
            path = join(&path, next);
            dir = inner;
        }
        let open = is_open(&path);
        out.push(Row::Dir {
            name: label,
            path: path.clone(),
            depth,
            open,
        });
        if open {
            emit(dir, &path, depth + 1, is_open, out);
        }
    }
    let mut files: Vec<&(String, Vec<usize>)> = node.files.iter().collect();
    files.sort_by_key(|(name, _)| name.to_lowercase());
    for (name, hits) in files {
        out.push(Row::File {
            name: name.clone(),
            path: join(parent, name),
            depth,
            hits: hits.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(paths: &[&str]) -> FileTree {
        let mut t = FileTree::default();
        t.set_paths(paths.iter().map(|p| p.to_string()).collect());
        t
    }

    fn shown(t: &FileTree) -> Vec<String> {
        t.rows
            .iter()
            .map(|r| match r {
                Row::Dir {
                    name, depth, open, ..
                } => {
                    format!(
                        "{}{}{name}/",
                        "  ".repeat(*depth),
                        if *open { "-" } else { "+" }
                    )
                }
                Row::File { name, depth, .. } => format!("{}{name}", "  ".repeat(*depth)),
            })
            .collect()
    }

    const PATHS: &[&str] = &[
        "README.md",
        "justfile",
        "src/main.tsx",
        "src-tauri/crates/lgtm/src/main.rs",
        "src-tauri/crates/lgtm/src/tui/editor.rs",
        "src-tauri/crates/lgtm/Cargo.toml",
        "CLAUDE.md",
    ];

    #[test]
    fn folders_come_first_and_names_sort_ignoring_case() {
        let t = tree(PATHS);
        assert_eq!(
            shown(&t),
            [
                "+src/",
                "+src-tauri/crates/lgtm/",
                "CLAUDE.md",
                "justfile",
                "README.md"
            ]
        );
    }

    #[test]
    fn reveal_opens_the_folders_down_to_a_file() {
        let mut t = tree(PATHS);
        t.reveal("src-tauri/crates/lgtm/src/tui/editor.rs");
        assert_eq!(
            shown(&t)[1..5],
            [
                "-src-tauri/crates/lgtm/",
                "  -src/",
                "    -tui/",
                "      editor.rs"
            ]
        );
        assert_eq!(
            t.selected(),
            Some("src-tauri/crates/lgtm/src/tui/editor.rs")
        );
    }

    #[test]
    fn a_search_shows_only_matches_with_their_folders_open() {
        let mut t = tree(PATHS);
        for c in "main".chars() {
            t.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        assert_eq!(
            shown(&t),
            [
                "-src/",
                "  main.tsx",
                "-src-tauri/crates/lgtm/src/",
                "  main.rs"
            ]
        );
        assert_eq!(t.count().as_deref(), Some("2 / 7"));
        let picked = t.selected().map(str::to_string);
        t.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(t.count(), None);
        assert_eq!(t.selected().map(str::to_string), picked);
    }

    #[test]
    fn a_search_puts_the_cursor_on_the_best_match() {
        let mut t = tree(&["a/edit_tests.rs", "b/edit.rs", "c/sendItems.ts"]);
        t.set_query("edit".into());
        assert_eq!(t.selected(), Some("b/edit.rs"));
        // A slash matches folders too.
        t.set_query("c/send".into());
        assert_eq!(t.selected(), Some("c/sendItems.ts"));
    }

    #[test]
    fn a_search_with_slashes_shows_the_folder_it_names() {
        let mut t = tree(&[
            "services/auth/docs/services/a.md",
            "services/auth/docs/services/b.md",
            "services/auth/docs/other.md",
            "services/billing/docs/services/c.md",
        ]);
        t.set_query("auth/docs/services".into());
        assert_eq!(t.count().as_deref(), Some("2 / 4"));
        assert_eq!(t.selected(), Some("services/auth/docs/services/a.md"));
        t.set_query("auth/docs/oth".into());
        assert_eq!(t.count().as_deref(), Some("1 / 4"));
        t.set_query("/auth".into());
        assert_eq!(t.count().as_deref(), Some("0 / 4"));
        t.set_query("docs/".into());
        assert_eq!(t.count().as_deref(), Some("4 / 4"));
    }

    #[test]
    fn a_click_opens_a_folder_or_a_file() {
        let mut t = tree(PATHS);
        t.area = Rect::new(0, 5, 30, 10);
        assert!(matches!(t.click(5), Outcome::None));
        assert_eq!(shown(&t)[..2], ["-src/", "  main.tsx"]);
        assert!(matches!(t.click(6), Outcome::Open(p) if p == "src/main.tsx"));
        assert!(matches!(t.click(40), Outcome::None));
    }

    #[test]
    fn enter_toggles_a_folder_and_left_goes_up_to_it() {
        let mut t = tree(PATHS);
        let press = |t: &mut FileTree, code| t.key(KeyEvent::new(code, KeyModifiers::NONE));
        press(&mut t, KeyCode::Enter);
        assert_eq!(shown(&t)[..2], ["-src/", "  main.tsx"]);
        press(&mut t, KeyCode::Down);
        press(&mut t, KeyCode::Left);
        assert_eq!(t.selected(), Some("src"));
        press(&mut t, KeyCode::Left);
        assert_eq!(shown(&t)[0], "+src/");
        assert!(matches!(press(&mut t, KeyCode::Enter), Outcome::None));
        press(&mut t, KeyCode::Down);
        assert_eq!(t.selected(), Some("src/main.tsx"));
        assert!(matches!(press(&mut t, KeyCode::Enter), Outcome::Open(p) if p == "src/main.tsx"));
    }
}
