//! The issues screen inside lgtm: opened with `space i` or `lgtm issues`, on the folder lgtm
//! started in when it's an issues folder, else the one last picked for the repo, else a pick.

use std::path::Path;

use lgtm_issues::folder::Folder;
use lgtm_issues::tui::picker::{self, Picker};
use lgtm_issues::tui::{Issues, Outcome};
use lgtm_tui_kit::paint::Canvas;
use ratatui::crossterm::event::{KeyEvent, MouseEvent};

use super::{App, Screen, settings};

impl App {
    /// Goes to the issues screen, trying `from` first for its folder.
    pub(super) fn show_issues(&mut self, from: Option<&Path>) {
        if self.issues.is_none() {
            let remembered = self.settings.issues_folders.get(&self.repo_key()).cloned();
            let folder = from
                .map(Path::to_path_buf)
                .into_iter()
                .chain([self.root.clone()])
                .chain(remembered)
                .find_map(|dir| Folder::open(&dir).ok());
            match folder {
                Some(f) => self.issues = Some(Box::new(Issues::new(f, lgtm_issues::user()))),
                None => self.folders = Some(Picker::new(&self.root)),
            }
        }
        self.screen = Screen::Issues;
    }

    fn repo_key(&self) -> String {
        self.root.display().to_string()
    }

    /// A key on the issues screen. Returns whether lgtm quits.
    pub(super) fn issues_key(&mut self, key: KeyEvent) -> bool {
        if let Some(p) = self.folders.as_mut() {
            match p.key(key) {
                picker::Outcome::Open(f) => {
                    self.settings
                        .issues_folders
                        .insert(self.repo_key(), f.root.clone());
                    settings::save(&self.settings);
                    self.issues = Some(Box::new(Issues::new(f, lgtm_issues::user())));
                    self.folders = None;
                }
                picker::Outcome::Cancel => {
                    self.folders = None;
                    if self.issues.is_none() {
                        self.go_home();
                    }
                }
                picker::Outcome::None => {}
            }
            return false;
        }
        let Some(issues) = self.issues.as_mut() else {
            self.go_home();
            return false;
        };
        match issues.key(key) {
            Outcome::None => {}
            Outcome::Quit => return !self.buffers.blocks_quit(),
            Outcome::Home => self.go_home(),
            Outcome::Folders => self.folders = Some(Picker::new(&self.root)),
            Outcome::Edit(path) => {
                let name = path
                    .strip_prefix(&self.root)
                    .map_or_else(|_| path.display().to_string(), |p| p.display().to_string());
                match self.buffers.open(&name, 1) {
                    Ok(()) => {
                        self.screen = Screen::Editor;
                        self.editor_back = Some(Screen::Issues);
                    }
                    Err(e) => issues.notice(format!("couldn't open {name}: {e}")),
                }
            }
        }
        false
    }

    pub(super) fn issues_paste(&mut self, text: &str) {
        if self.folders.is_none()
            && let Some(i) = self.issues.as_mut()
        {
            i.paste(text);
        }
    }

    pub(super) fn issues_mouse(&mut self, ev: MouseEvent) {
        if let Some(i) = self.issues.as_mut().filter(|_| self.folders.is_none()) {
            i.mouse(ev);
        }
    }

    pub(super) fn issues_tick(&mut self) {
        if let Some(i) = self.issues.as_mut() {
            i.tick();
        }
    }

    pub(super) fn draw_issues(&mut self, c: &mut Canvas) {
        if let Some(p) = self.folders.as_mut() {
            p.draw(c);
        } else if let Some(i) = self.issues.as_mut() {
            i.draw(c);
        }
    }
}
