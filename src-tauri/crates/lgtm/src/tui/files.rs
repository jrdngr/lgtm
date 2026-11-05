//! The repo's files as a tree with folders that open and close, narrowed by a fuzzy search.
//! Home shows it beside the jj log, and the editor beside the text. Files git ignores show
//! dimmed, and an ignored folder is read when it's opened.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use lgtm_core::fuzzy;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Color;

use super::decor::{Decor, Decorations};
use super::heat::{self, Heat};
use super::paint::Canvas;
use super::theme::{self, ACCENT, BG, FAINT, FG, HAIR, MUTED, SEL};
use super::walk::{self, Listing};

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
        ignored: bool,
    },
    /// A file, and which chars of its name the search matched.
    File {
        name: String,
        path: String,
        depth: usize,
        hits: Vec<usize>,
        ignored: bool,
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
    /// Where the paths are from.
    root: Option<std::path::PathBuf>,
    /// Every file known, ignored or not.
    paths: Vec<String>,
    /// Folders that hold no file known: ignored ones.
    dirs: BTreeSet<String>,
    /// Files and folders git ignores.
    ignored: HashSet<String>,
    /// Ignored folders not read yet.
    unread: BTreeSet<String>,
    /// Whether the files were listed.
    pub loaded: bool,
    /// How the working copy changes files, coloring their names.
    pub decor: Decorations,
    /// Recent writes, glowing on their rows while `heat_on`.
    pub heat: Heat,
    pub heat_on: bool,
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
    /// Takes a new listing. Ignored folders that were open are read again.
    pub fn set_listing(&mut self, listing: Listing) {
        self.loaded = true;
        let mut paths = listing.files;
        paths.extend(listing.ignored.iter().cloned());
        paths.sort();
        let dirs: BTreeSet<String> = listing.ignored_dirs.iter().cloned().collect();
        if paths == self.paths && dirs == self.dirs {
            return;
        }
        let at = self.selected().map(str::to_string);
        self.root = Some(listing.root);
        self.paths = paths;
        self.ignored = listing.ignored.into_iter().collect();
        self.ignored.extend(dirs.iter().cloned());
        self.unread = dirs.clone();
        self.dirs = dirs;
        let open: Vec<String> = self.open.iter().cloned().collect();
        for dir in open {
            self.read(&dir);
        }
        self.rebuild();
        if let Some(path) = at {
            self.select(&path);
        }
    }

    /// Reads the ignored folder `dir` when it wasn't read.
    fn read(&mut self, dir: &str) {
        let Some(root) = self.root.as_ref().filter(|_| self.unread.remove(dir)) else {
            return;
        };
        let (files, dirs) = walk::read_dir(root, dir);
        self.ignored.extend(files.iter().cloned());
        self.ignored.extend(dirs.iter().cloned());
        self.unread.extend(dirs.iter().cloned());
        self.dirs.extend(dirs);
        self.paths.extend(files);
        self.paths.sort();
    }

    /// The files git doesn't ignore.
    pub fn files(&self) -> impl Iterator<Item = &String> {
        self.paths.iter().filter(|p| !self.ignored.contains(*p))
    }

    /// Whether there's anything to show.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.dirs.is_empty()
    }

    /// Whether git ignores `path`, or a folder it's in.
    pub fn is_ignored(&self, path: &str) -> bool {
        let mut at = path;
        loop {
            if self.ignored.contains(at) {
                return true;
            }
            match at.rsplit_once('/') {
                Some((dir, _)) => at = dir,
                None => return false,
            }
        }
    }

    /// Adds the file `path` when it isn't listed, as one just saved.
    pub fn add(&mut self, path: &str) {
        if std::path::Path::new(path).is_absolute() || self.paths.iter().any(|p| p == path) {
            return;
        }
        self.paths.push(path.to_string());
        self.paths.sort();
        self.rebuild();
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
            self.read(&dir);
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
            for d in &self.dirs {
                add_dir(&mut root, d);
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
        let shape = Shape {
            is_open: &|p| searching || self.open.contains(p),
            is_ignored: &|p| self.is_ignored(p),
        };
        emit(&root, "", 0, &shape, &mut rows);
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
            self.read(path);
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
            let (level, decor) = match row {
                // An open folder shows its files' heat on their own rows.
                Row::Dir { path, open, .. } => {
                    let level = if *open { 0.0 } else { self.heat.dir(path) };
                    (level, self.decor.dir(path))
                }
                Row::File { path, .. } => (self.heat.file(path), self.decor.file(path)),
            };
            let tint = decor.unwrap_or(Decor::Modified).color();
            let hot = self.heat_on && level > 0.0;
            let bg = if hot { heat::glow(bg, tint, level) } else { bg };
            sub.fill(area.x, y, area.width, bg);
            if hot {
                sub.put(area.x, y, "▎", theme::fg(heat::mix(BG, tint, level)).bg(bg));
            }
            draw_row(&mut sub, area.x + 1, y, row, bg, marked, &self.decor);
        }
    }
}

fn draw_row(
    c: &mut Canvas,
    x0: u16,
    y: u16,
    row: &Row,
    bg: Color,
    marked: &dyn Fn(&str) -> bool,
    decor: &Decorations,
) {
    match row {
        Row::Dir {
            name,
            path,
            depth,
            open,
            ignored,
        } => {
            let arrow = if *open { "▾ " } else { "▸ " };
            let x = x0 + 2 * *depth as u16;
            let color = match decor.dir(path) {
                Some(d) => d.color(),
                None if *ignored => FAINT,
                None => MUTED,
            };
            c.put(x, y, &format!("{arrow}{name}/"), theme::fg(color).bg(bg));
        }
        Row::File {
            name,
            path,
            depth,
            hits,
            ignored,
        } => {
            let mut x = x0 + 2 * *depth as u16;
            let color = match decor.file(path) {
                Some(d) => d.color(),
                None if *ignored => MUTED,
                None => FG,
            };
            for (i, ch) in name.chars().enumerate() {
                let style = if hits.contains(&i) {
                    theme::bold(ACCENT).bg(bg)
                } else {
                    theme::fg(color).bg(bg)
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

/// Adds the folder `path`, which may hold nothing known.
fn add_dir(root: &mut Node, path: &str) {
    let mut node = root;
    for dir in path.split('/') {
        node = node.dirs.entry(dir.to_string()).or_default();
    }
}

/// Which folders are open, and what git ignores.
struct Shape<'a> {
    is_open: &'a dyn Fn(&str) -> bool,
    is_ignored: &'a dyn Fn(&str) -> bool,
}

/// Folders first, then files, each by name ignoring case.
fn emit(node: &Node, parent: &str, depth: usize, shape: &Shape, out: &mut Vec<Row>) {
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
        // A folder holding only one folder shares its line, unless it's ignored and may hold
        // more than was read.
        while dir.files.is_empty() && dir.dirs.len() == 1 && !(shape.is_ignored)(&path) {
            let (next, inner) = dir.dirs.iter().next().expect("one folder");
            label = format!("{label}/{next}");
            path = join(&path, next);
            dir = inner;
        }
        let open = (shape.is_open)(&path);
        out.push(Row::Dir {
            name: label,
            ignored: (shape.is_ignored)(&path),
            path: path.clone(),
            depth,
            open,
        });
        if open {
            emit(dir, &path, depth + 1, shape, out);
        }
    }
    let mut files: Vec<&(String, Vec<usize>)> = node.files.iter().collect();
    files.sort_by_key(|(name, _)| name.to_lowercase());
    for (name, hits) in files {
        let path = join(parent, name);
        out.push(Row::File {
            name: name.clone(),
            ignored: (shape.is_ignored)(&path),
            path,
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
        t.set_listing(Listing {
            files: paths.iter().map(|p| p.to_string()).collect(),
            ..Listing::default()
        });
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

    #[test]
    fn ignored_files_show_dimmed_and_an_ignored_folder_is_read_when_opened() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("target/debug")).unwrap();
        std::fs::write(dir.path().join("target/out"), "").unwrap();
        let mut t = FileTree::default();
        t.set_listing(Listing {
            root: dir.path().to_path_buf(),
            files: vec!["a.rs".into()],
            ignored: vec!["a.log".into()],
            ignored_dirs: vec!["target".into()],
        });
        let dimmed = |t: &FileTree| -> Vec<(String, bool)> {
            t.rows
                .iter()
                .map(|r| match r {
                    Row::Dir { path, ignored, .. } | Row::File { path, ignored, .. } => {
                        (path.clone(), *ignored)
                    }
                })
                .collect()
        };
        assert_eq!(
            dimmed(&t),
            [
                ("target".to_string(), true),
                ("a.log".into(), true),
                ("a.rs".into(), false)
            ]
        );
        assert_eq!(t.files().collect::<Vec<_>>(), ["a.rs"]);
        t.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(shown(&t)[..3], ["-target/", "  +debug/", "  out"]);
        assert!(t.is_ignored("target/debug/new.rs"));
        assert!(!t.is_ignored("src/new.rs"));
        // A new listing reads the folders that were open again.
        t.set_listing(Listing {
            root: dir.path().to_path_buf(),
            files: vec!["a.rs".into(), "b.rs".into()],
            ignored: vec![],
            ignored_dirs: vec!["target".into()],
        });
        assert_eq!(shown(&t)[..3], ["-target/", "  +debug/", "  out"]);
    }
}
