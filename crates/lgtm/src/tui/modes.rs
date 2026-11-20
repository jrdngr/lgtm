//! Switching screens with `space m`.

use lgtm_tui_kit::mode::Mode;

use super::load::Target;
use super::{App, Screen};

impl App {
    /// Goes to `mode`'s screen: the editor on the files it has open, the open review or else
    /// the working copy's, the issues, or home.
    pub(super) fn switch(&mut self, mode: Mode) {
        let from = self.screen;
        match mode {
            Mode::Editor => {
                self.screen = Screen::Editor;
                self.editor_back = Some(from);
            }
            Mode::Review => {
                match self.review.is_some() {
                    true => self.screen = Screen::Review,
                    false => self.open(Target::Revset("@".into())),
                }
                self.review_back = Some(from);
            }
            Mode::Issues => self.show_issues(None),
            Mode::Home => self.go_home(),
        }
    }
}
