//! The mark on an agent that has messages waiting to go in.

use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px, rgb};

use super::Workspace;
use crate::model::TermId;
use crate::theme;

impl Workspace {
    /// ✉ beside a tab's or row's name while messages wait for the agent in `t`.
    pub(super) fn waiting_mark(&self, t: TermId) -> Option<AnyElement> {
        self.agents
            .messages
            .iter()
            .any(|m| m.queued && m.to == t)
            .then(|| {
                div()
                    .flex_none()
                    .text_size(px(12.))
                    .text_color(rgb(theme::WAITING))
                    .child("✉")
                    .into_any_element()
            })
    }
}
