//! Renaming a layout or space in place in the sidebar.

use gpui::{Context, FocusHandle, KeyDownEvent, Window};

use super::Workspace;
use super::field::{self, Key};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Layout(u64),
    Space(u64),
}

pub struct Rename {
    pub target: Target,
    pub text: String,
    pub focus: FocusHandle,
}

impl Workspace {
    pub(super) fn start_rename(
        &mut self,
        target: Target,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        self.rename = Some(Rename {
            target,
            text,
            focus,
        });
        cx.notify();
    }

    /// Saves the name being typed. An empty name goes back to following the folder.
    pub(super) fn finish_rename(&mut self, save: bool) {
        let Some(r) = self.rename.take() else { return };
        if !save {
            return;
        }
        let name = Some(r.text.trim().to_owned()).filter(|t| !t.is_empty());
        match r.target {
            Target::Layout(id) => {
                if let Some(l) = self.model.find_layout_mut(id) {
                    l.name = name;
                }
            }
            Target::Space(id) => {
                if let Some(s) = self.model.find_space_mut(id) {
                    s.name = name;
                }
            }
        }
    }

    pub(super) fn rename_key(
        &mut self,
        ev: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(r) = &mut self.rename else { return };
        match field::key(&mut r.text, &ev.keystroke) {
            Key::Other => return,
            Key::Enter | Key::Escape => {
                let save = ev.keystroke.key == "enter";
                self.finish_rename(save);
                self.focus_current(window, cx);
            }
            _ => {}
        }
        cx.stop_propagation();
        cx.notify();
    }
}
