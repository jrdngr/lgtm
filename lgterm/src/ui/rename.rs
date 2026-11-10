//! Renaming a layout or space in place in the sidebar.

use gpui::{Context, FocusHandle, KeyDownEvent, Subscription, Window};

use super::Workspace;
use super::overlay::Overlay;
use crate::field::{self, Key};
use crate::model::{LayoutId, SpaceId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Layout(LayoutId),
    Space(SpaceId),
}

pub struct Rename {
    pub target: Target,
    pub text: String,
    pub focus: FocusHandle,
    /// Saves the name when the keyboard goes elsewhere, like on a click off the field.
    _blur: Subscription,
}

impl Workspace {
    pub(super) fn start_rename(
        &mut self,
        target: Target,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_rename(true);
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let blur = cx.on_focus_out(&focus, window, |w, _, window, cx| {
            if window.is_window_active() {
                w.finish_rename(true);
                cx.notify();
            }
        });
        self.overlay = Some(Overlay::Rename(Rename {
            target,
            text,
            focus,
            _blur: blur,
        }));
        cx.notify();
    }

    /// Saves the name being typed. An empty space name goes back to following the folder; an
    /// empty layout name keeps the old one.
    pub(super) fn finish_rename(&mut self, save: bool) {
        if !matches!(self.overlay, Some(Overlay::Rename(_))) {
            return;
        }
        let Some(Overlay::Rename(r)) = self.overlay.take() else {
            return;
        };
        if !save {
            return;
        }
        let name = Some(r.text.trim().to_owned()).filter(|t| !t.is_empty());
        match r.target {
            Target::Layout(id) => {
                if let Some(l) = self.model.find_layout_mut(id)
                    && name.is_some()
                {
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

    /// Adds a layout and puts its name field up, empty: leaving it empty keeps "Layout N".
    /// The badges-only sidebar has no name field.
    pub(super) fn new_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply(window, cx, |m| m.new_layout());
        if !self.sidebar_icons() {
            let id = self.model.layout().id;
            self.start_rename(Target::Layout(id), String::new(), window, cx);
        }
    }

    pub(super) fn renaming(&self) -> Option<&Rename> {
        match &self.overlay {
            Some(Overlay::Rename(r)) => Some(r),
            _ => None,
        }
    }

    pub(super) fn rename_key(
        &mut self,
        ev: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(Overlay::Rename(r)) = &mut self.overlay else {
            return;
        };
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
