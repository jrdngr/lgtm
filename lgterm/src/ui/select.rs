//! Spaces picked with ⌘-click on their titles, and the commands that act on them.

use gpui::{Context, Window};

use super::Workspace;
use crate::model::SpaceId;

impl Workspace {
    /// Adds a space to the picked ones, or takes it out. The first pick takes the focused
    /// space along with it.
    pub(super) fn toggle_selected(&mut self, id: SpaceId) {
        if let Some(i) = self.selected.iter().position(|&s| s == id) {
            self.selected.remove(i);
            return;
        }
        if self.selected.is_empty()
            && let Some(focused) = self.model.layout().slot().space
            && focused != id
        {
            self.selected.push(focused);
        }
        self.selected.push(id);
    }

    /// Evens out the picked spaces, or every slot with `all`.
    pub(super) fn distribute(&mut self, all: bool, window: &mut Window, cx: &mut Context<Self>) {
        let picked = std::mem::take(&mut self.selected);
        self.apply(window, cx, |m| m.distribute((!all).then_some(&picked[..])));
        self.selected = picked;
        cx.notify();
    }
}
