//! The mark on an agent that has messages waiting to go in.

use gpui::{
    AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    StatefulInteractiveElement, Styled, div, rgb,
};

use super::Workspace;
use crate::model::TermId;
use crate::theme;

impl Workspace {
    /// ✉ beside a tab's or row's name while messages wait for the agent in `t`. Clicking it
    /// hands the agent the oldest one now.
    pub(super) fn waiting_mark(&self, t: TermId, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.agents
            .messages
            .iter()
            .any(|m| m.queued && m.to == t)
            .then(|| {
                div()
                    .id(("waiting", t.0))
                    .flex_none()
                    .cursor_pointer()
                    .text_size(theme::ui(12.))
                    .text_color(rgb(theme::WAITING))
                    .child("✉")
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |w, _, window, cx| {
                        cx.stop_propagation();
                        w.with_agents(window, cx, |a, _| a.deliver_now(t));
                    }))
                    .into_any_element()
            })
    }
}
