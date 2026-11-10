//! Carrying out what the agents decide: messages typed in, alerts, and the Dock badge.

use std::time::Duration;

use gpui::{App, Context, SystemNotification, Window};

use super::Workspace;
use crate::agents::{Agents, Effect, Env, Status};
use crate::dock;
use crate::model::{Tab, TermId};

/// How long after pasting a message its return is pressed.
const RETURN_AFTER: Duration = Duration::from_millis(150);

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
        let typing = |t: TermId| terms.get(&t).is_some_and(|t| t.view.read(app).typing());
        let out = f(
            &mut self.agents,
            &Env {
                viewed,
                typing: &typing,
            },
        );
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
        let view = t.view.clone();
        view.update(cx, |v, cx| v.paste_text(text, cx));
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
            cx.show_system_notification(SystemNotification {
                tag: term.to_string().into(),
                title: self.term_label(Tab::Term(term)).into(),
                body: format!("{state} · {space}").into(),
                actions: vec![],
            });
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
