//! Picking an issues folder: the folders under the one opened, as a tree, with issues
//! folders in green and how many questions wait in each.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use lgtm_tui_kit::keybar;
use lgtm_tui_kit::paint::Canvas;
use lgtm_tui_kit::theme::{self, ACCENT, AMBER, BG, FG, GREEN, MUTED, PANEL, RED, SEL};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use crate::folder::{Folder, MARKER};

/// Folders a walk skips: version control and build output.
const SKIP: [&str; 4] = [".git", ".jj", "node_modules", "target"];

pub enum Outcome {
    None,
    Open(Folder),
    Cancel,
}

pub struct Picker {
    root: PathBuf,
    /// Every folder under the root, and the issues folders' waiting question counts.
    dirs: BTreeSet<PathBuf>,
    issues: HashMap<PathBuf, usize>,
    open: BTreeSet<PathBuf>,
    sel: usize,
    top: usize,
    notice: Option<String>,
}

/// The folders under `root`, gitignored ones left out, and the issues folders among them.
fn walk(root: &Path) -> (BTreeSet<PathBuf>, HashMap<PathBuf, usize>) {
    let mut dirs = BTreeSet::new();
    let mut issues = HashMap::new();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .filter_entry(|e| !SKIP.iter().any(|s| e.file_name() == *s))
        .build();
    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|t| t.is_dir()) {
            continue;
        }
        let path = entry.into_path();
        if path.join(MARKER).is_file() {
            let waiting = Folder::open(&path)
                .and_then(|f| f.load())
                .map_or(0, |(all, _)| all.iter().map(|i| i.waiting().len()).sum());
            issues.insert(path.clone(), waiting);
        }
        dirs.insert(path);
    }
    (dirs, issues)
}

impl Picker {
    pub fn new(root: &Path) -> Picker {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let (dirs, issues) = walk(&root);
        let mut open = BTreeSet::from([root.clone()]);
        for found in issues.keys() {
            open.extend(found.ancestors().skip(1).map(Path::to_path_buf));
        }
        let mut picker = Picker {
            root,
            dirs,
            issues,
            open,
            sel: 0,
            top: 0,
            notice: None,
        };
        let rows = picker.rows();
        picker.sel = rows
            .iter()
            .position(|p| picker.issues.contains_key(p))
            .unwrap_or(0);
        picker
    }

    fn children(&self, dir: &Path) -> impl Iterator<Item = &PathBuf> {
        self.dirs.iter().filter(move |d| d.parent() == Some(dir))
    }

    /// The folders shown, in tree order.
    fn rows(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        self.push(&self.root, &mut out);
        out
    }

    fn push(&self, dir: &Path, out: &mut Vec<PathBuf>) {
        out.push(dir.to_path_buf());
        if self.open.contains(dir) {
            for child in self.children(dir) {
                self.push(child, out);
            }
        }
    }

    fn depth(&self, dir: &Path) -> usize {
        dir.strip_prefix(&self.root)
            .map_or(0, |rel| rel.components().count())
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        self.notice = None;
        let rows = self.rows();
        let Some(here) = rows.get(self.sel).cloned() else {
            return Outcome::Cancel;
        };
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.sel = (self.sel + 1).min(rows.len() - 1),
            KeyCode::Char('k') | KeyCode::Up => self.sel = self.sel.saturating_sub(1),
            KeyCode::Char('l') | KeyCode::Right => {
                self.open.insert(here);
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if !self.open.remove(&here)
                    && let Some(parent) = here.parent()
                    && let Some(p) = rows.iter().position(|r| r == parent)
                {
                    self.sel = p;
                }
            }
            KeyCode::Enter if self.issues.contains_key(&here) => match Folder::open(&here) {
                Ok(f) => return Outcome::Open(f),
                Err(e) => self.notice = Some(format!("{e:#}")),
            },
            KeyCode::Enter => {
                if !self.open.remove(&here) {
                    self.open.insert(here);
                }
            }
            KeyCode::Char('i') if !self.issues.contains_key(&here) => match Folder::init(&here) {
                Ok(f) => return Outcome::Open(f),
                Err(e) => self.notice = Some(format!("{e:#}")),
            },
            KeyCode::Esc | KeyCode::Char('q') => return Outcome::Cancel,
            _ => {}
        }
        Outcome::None
    }

    pub fn draw(&mut self, c: &mut Canvas) {
        let area = c.area;
        for y in area.y..area.bottom() {
            c.fill(area.x, y, area.width, BG);
        }
        if area.height < 4 {
            return;
        }
        let x = c.put(area.x, area.y, "issues folders", theme::fg(ACCENT));
        c.put(x + 2, area.y, &home(&self.root), theme::fg(FG));
        let rows = self.rows();
        let height = (area.height - 3) as usize;
        if self.sel < self.top {
            self.top = self.sel;
        } else if self.sel >= self.top + height {
            self.top = self.sel + 1 - height;
        }
        for (i, dir) in rows.iter().enumerate().skip(self.top).take(height) {
            let y = area.y + 2 + (i - self.top) as u16;
            let bg = if i == self.sel { SEL } else { BG };
            c.fill(area.x, y, area.width, bg);
            let has_children = self.children(dir).next().is_some();
            let mark = match (has_children, self.open.contains(dir)) {
                (false, _) => " ",
                (true, true) => "▾",
                (true, false) => "▸",
            };
            let x = area.x + 1 + 2 * self.depth(dir) as u16;
            let x = c.put(x, y, mark, theme::fg(MUTED).bg(bg));
            let name = if *dir == self.root {
                home(dir)
            } else {
                dir.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            };
            match self.issues.get(dir) {
                Some(waiting) => {
                    let x = c.put(x + 1, y, &format!("{name}/"), theme::bold(GREEN).bg(bg));
                    if *waiting > 0 {
                        c.put(x + 2, y, &format!("● {waiting}"), theme::fg(AMBER).bg(bg));
                    }
                }
                None => {
                    c.put(x + 1, y, &format!("{name}/"), theme::fg(FG).bg(bg));
                }
            }
        }
        let bottom = area.bottom() - 1;
        let here = rows
            .get(self.sel)
            .is_some_and(|d| self.issues.contains_key(d));
        let mut hints = vec![("j k", "move"), ("h l", "fold")];
        hints.push(if here {
            ("Enter", "open")
        } else {
            ("i", "new issues folder")
        });
        hints.push(("Esc", "back"));
        let hints: Vec<keybar::Hint> = hints.into_iter().map(|(k, l)| (k, l, false)).collect();
        keybar::draw_badged(c, bottom, Some(("ISS", AMBER)), &hints);
        if let Some(n) = &self.notice {
            c.fill(area.x, bottom, area.width, PANEL);
            c.put(area.x + 1, bottom, n, theme::fg(RED).bg(PANEL));
        }
    }
}

pub fn home(path: &Path) -> String {
    match std::env::var_os("HOME").map(PathBuf::from) {
        Some(h) if path.starts_with(&h) => {
            format!("~/{}", path.strip_prefix(&h).unwrap_or(path).display())
        }
        _ => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn press(p: &mut Picker, c: char) -> Outcome {
        p.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
    }

    #[test]
    fn issues_folders_start_unfolded_and_selected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        for d in ["a/b/tracker", "c/d", "target/x"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        Folder::init(&root.join("a/b/tracker")).unwrap();
        let p = Picker::new(&root);
        let names: Vec<String> = p
            .rows()
            .iter()
            .map(|r| r.strip_prefix(&root).unwrap().display().to_string())
            .collect();
        assert_eq!(names, ["", "a", "a/b", "a/b/tracker", "c"]);
        assert_eq!(p.rows()[p.sel], root.join("a/b/tracker"));
    }

    #[test]
    fn i_starts_an_issues_folder_where_the_cursor_is() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("plans")).unwrap();
        let mut p = Picker::new(dir.path());
        press(&mut p, 'j');
        match press(&mut p, 'i') {
            Outcome::Open(f) => assert!(f.root.ends_with("plans") && f.root.join(MARKER).is_file()),
            _ => panic!("didn't open"),
        }
    }
}
