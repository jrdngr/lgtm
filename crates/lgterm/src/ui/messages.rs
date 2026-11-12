//! Sending messages between agents, and the Messages tab that lists them.

mod convo;
mod transcript;
mod view;

pub use view::View;

use gpui::{Context, Window};

use super::Workspace;
use crate::agents::Message;
use crate::agents::messages::SpaceTag;
use crate::agents::messages::clock;
use crate::ipc::Reply;
use crate::model::{Tab, TermId};

impl Workspace {
    pub(super) fn send_message(
        &mut self,
        from: Option<TermId>,
        to: TermId,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Reply {
        let message = Message {
            time: clock(),
            from,
            from_label: from
                .map(|f| self.term_label(Tab::Term(f)))
                .unwrap_or_default(),
            to,
            to_label: self.term_label(Tab::Term(to)),
            text,
            queued: true,
            dropped: false,
            read: from.is_none(),
            from_space: from.and_then(|f| self.space_tag(f)),
            to_space: self.space_tag(to),
        };
        match self.with_agents(window, cx, |a, env| a.send(message, env)) {
            Ok(outcome) => Reply::Ok(outcome.into()),
            Err(e) => Reply::Err(e),
        }
    }
}

impl Workspace {
    /// `t`'s space as it is now, kept with a message for after it closes.
    fn space_tag(&self, t: TermId) -> Option<SpaceTag> {
        let s = self.model.space_of(Tab::Term(t))?;
        Some(SpaceTag {
            id: s.id.0,
            name: self.space_label(s),
            color: s.color,
        })
    }
}
