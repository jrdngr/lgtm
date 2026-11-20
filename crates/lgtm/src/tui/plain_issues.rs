//! The issues screen beside the editor outside source control, on the folder lgtm started in
//! when it's an issues folder, else a pick under it.

use std::path::{Path, PathBuf};

use lgtm_issues::folder::Folder;
use lgtm_issues::tui::picker::{self, Picker};
use lgtm_issues::tui::{Issues, Outcome};
use lgtm_tui_kit::mode::Mode;
use lgtm_tui_kit::paint::Canvas;
use ratatui::crossterm::event::{KeyEvent, MouseEvent};

/// What a key on the screen asks of the editor around it.
pub enum Asked {
    None,
    /// Show the editor again.
    Editor,
    /// Open this file, a name in the root, in the editor.
    Edit(String),
    Quit,
}

pub struct Pane {
    root: PathBuf,
    issues: Option<Box<Issues>>,
    folders: Option<Picker>,
}

impl Pane {
    pub fn new(root: &Path) -> Pane {
        let mut pane = Pane {
            root: root.to_path_buf(),
            issues: None,
            folders: None,
        };
        match Folder::open(root) {
            Ok(f) => pane.show(f),
            Err(_) => pane.folders = Some(Picker::new(root)),
        }
        pane
    }

    fn show(&mut self, folder: Folder) {
        let mut issues = Issues::new(folder, lgtm_issues::user());
        issues.set_modes(vec![Mode::Editor]);
        self.issues = Some(Box::new(issues));
    }

    pub fn key(&mut self, key: KeyEvent) -> Asked {
        if let Some(p) = self.folders.as_mut() {
            match p.key(key) {
                picker::Outcome::Open(f) => {
                    self.folders = None;
                    self.show(f);
                }
                picker::Outcome::Cancel => {
                    self.folders = None;
                    if self.issues.is_none() {
                        return Asked::Editor;
                    }
                }
                picker::Outcome::None => {}
            }
            return Asked::None;
        }
        let Some(issues) = self.issues.as_mut() else {
            return Asked::Editor;
        };
        match issues.key(key) {
            Outcome::Quit => Asked::Quit,
            Outcome::Mode(Mode::Editor) => Asked::Editor,
            Outcome::Folders => {
                self.folders = Some(Picker::new(&self.root));
                Asked::None
            }
            Outcome::Edit(path) => Asked::Edit(
                path.strip_prefix(&self.root)
                    .map_or_else(|_| path.display().to_string(), |p| p.display().to_string()),
            ),
            Outcome::Mode(_) | Outcome::None => Asked::None,
        }
    }

    pub fn paste(&mut self, text: &str) {
        if let Some(i) = self.issues.as_mut().filter(|_| self.folders.is_none()) {
            i.paste(text);
        }
    }

    pub fn mouse(&mut self, ev: MouseEvent) {
        if let Some(i) = self.issues.as_mut().filter(|_| self.folders.is_none()) {
            i.mouse(ev);
        }
    }

    pub fn tick(&mut self) {
        if let Some(i) = self.issues.as_mut() {
            i.tick();
        }
    }

    pub fn draw(&mut self, c: &mut Canvas) {
        if let Some(p) = self.folders.as_mut() {
            p.draw(c);
        } else if let Some(i) = self.issues.as_mut() {
            i.draw(c);
        }
    }
}
