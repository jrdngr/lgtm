//! Carrying out what the agents decide: messages typed in, alerts, and the Dock badge.

use std::collections::HashMap;
use std::time::Duration;

use gpui::{App, Context, SystemNotification, Window};

use super::Workspace;
use super::terms::Term;
use crate::agents::{Agents, Effect, Env, Kind, Status};
use crate::dock;
use crate::model::{Tab, TermId};

/// How long after pasting a message its return is pressed.
const RETURN_AFTER: Duration = Duration::from_millis(150);

/// Rows at the bottom of an agent's screen that hold its prompt box.
const PROMPT_ROWS: usize = 16;

/// Whether the user is typing a prompt for an agent: they pressed a key just now, or its
/// prompt box has text, or they typed a prompt and haven't sent it. A prompt box that reads
/// empty lets an unsent prompt go only once its keys have been quiet a while, since the box
/// can be misread.
pub(super) fn typing<'a>(
    terms: &'a HashMap<TermId, Term>,
    app: &'a App,
) -> impl Fn(TermId, Kind) -> bool + 'a {
    move |t, kind| {
        terms.get(&t).is_some_and(|t| {
            let view = t.view.read(app);
            view.typed_recently()
                || match kind.prompt_typed(&view.bottom_rows(PROMPT_ROWS)) {
                    Some(true) => true,
                    Some(false) => view.recent_draft(),
                    None => view.draft(),
                }
        })
    }
}

impl Workspace {
    /// The terminal the user is looking at.
    pub(super) fn viewed(&self, window: &Window) -> Option<TermId> {
        window
            .is_window_active()
            .then(|| self.model.term())
            .flatten()
    }

    /// Runs `f` on the agents with what the window knows, then carries out what it decided.
    pub(super) fn with_agents<R>(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Agents, &Env) -> R,
    ) -> R {
        let viewed = self.viewed(window);
        let terms = &self.terms;
        let app: &App = cx;
        let typing = typing(terms, app);
        let out = f(
            &mut self.agents,
            &Env {
                viewed,
                typing: &typing,
            },
        );
        drop(typing);
        self.run_effects(cx);
        out
    }

    pub(super) fn run_effects(&mut self, cx: &mut Context<Self>) {
        let (effects, changed) = self.agents.take();
        for effect in effects {
            match effect {
                Effect::Deliver(term, text) => self.type_message(term, text, cx),
                Effect::ClearDraft(term) => {
                    if let Some(t) = self.terms.get(&term) {
                        t.view.update(cx, |v, _| v.clear_draft());
                    }
                }
                Effect::Alert(term, status) => self.alert(term, status, cx),
            }
        }
        if changed {
            self.update_badge();
            cx.notify();
        }
    }

    /// Pastes a message into `term` and sends it.
    fn type_message(&mut self, term: TermId, text: String, cx: &mut Context<Self>) {
        let Some(t) = self.terms.get(&term) else {
            return;
        };
        t.view.update(cx, |v, cx| v.paste_text(text, cx));
        self.press_return(term, cx);
    }

    /// Presses return in `term` a moment from now, so a program that reads a burst of input
    /// as a paste takes it as sending what came before.
    pub(super) fn press_return(&self, term: TermId, cx: &mut Context<Self>) {
        let Some(t) = self.terms.get(&term) else {
            return;
        };
        let view = t.view.clone();
        cx.spawn(async move |_, cx| {
            cx.background_executor().timer(RETURN_AFTER).await;
            view.update(cx, |v, cx| v.input(b"\r", cx));
        })
        .detach();
    }

    fn alert(&mut self, term: TermId, status: Status, cx: &mut Context<Self>) {
        let notify = match status {
            Status::Blocked => self.settings.notify_blocked,
            _ => self.settings.notify_done,
        };
        if notify {
            let space = self
                .model
                .space_of(Tab::Term(term))
                .map(|s| self.space_label(s))
                .unwrap_or_default();
            let state = match status {
                Status::Blocked => "Blocked",
                _ => "Done",
            };
            let title = self.term_label(Tab::Term(term));
            let body = format!("{state} · {space}");
            if self.settings.system_notifications {
                cx.show_system_notification(SystemNotification {
                    tag: term.to_string().into(),
                    title: title.clone().into(),
                    body: body.clone().into(),
                    actions: vec![],
                });
            }
            self.toast(term, status, title, body, cx);
        }
        if self.settings.sound {
            dock::play_sound();
        }
    }

    pub(super) fn update_badge(&mut self) {
        let count = if self.settings.dock_badge {
            self.agents.waiting()
        } else {
            0
        };
        if count != self.badge {
            self.badge = count;
            dock::set_badge(count);
        }
    }
}
