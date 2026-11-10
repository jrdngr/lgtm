//! Saving the layouts as they are, and opening them again with their agents resumed.

use std::path::PathBuf;
use std::time::Duration;

use gpui::{Context, Window};

use super::Workspace;
use crate::state::{SavedTerm, State};

impl Workspace {
    fn snapshot(&self, cx: &gpui::App) -> State {
        let terms = self
            .terms
            .iter()
            .map(|(id, t)| {
                let agent = t
                    .agent
                    .as_ref()
                    .and_then(|a| Some((a.kind, a.session.clone()?)));
                let saved = SavedTerm {
                    cwd: self.cwd_of(*id, cx),
                    agent,
                };
                (*id, saved)
            })
            .collect();
        let mut collapsed: Vec<u64> = self.collapsed.iter().copied().collect();
        collapsed.sort_unstable();
        State {
            model: self.model.clone(),
            terms,
            messages_tab: self.messages_tab,
            messages: self.messages.clone(),
            collapsed,
            sidebar: self.sidebar,
        }
    }

    /// Writes the state if it changed since the last save.
    pub(super) fn save_state(&mut self, cx: &gpui::App) {
        let Some(json) = self.snapshot(cx).to_json() else {
            return;
        };
        if self.saved.as_deref() != Some(&json[..]) {
            State::write(&json);
            self.saved = Some(json);
        }
    }

    /// Opens every terminal in a saved state, resuming agents if settings say so.
    pub(super) fn restore(&mut self, state: State, window: &mut Window, cx: &mut Context<Self>) {
        self.model = state.model;
        self.messages_tab = state.messages_tab;
        self.messages = state.messages;
        self.collapsed = state.collapsed.into_iter().collect();
        self.sidebar = state.sidebar;
        let home = self.home.clone().unwrap_or_else(|| PathBuf::from("/"));
        for id in self.model.terms() {
            if Some(id) == self.messages_tab {
                continue;
            }
            let saved = state.terms.get(&id);
            let cwd = saved
                .map(|s| s.cwd.clone())
                .filter(|c| c.is_dir())
                .unwrap_or_else(|| home.clone());
            let _ = self.spawn(id, &cwd, &[], window, cx);
            let resume = saved
                .and_then(|s| s.agent.as_ref())
                .filter(|_| self.settings.resume);
            if let (Some((kind, session)), Some(t)) = (resume, self.terms.get(&id)) {
                let command = format!("{}\r", kind.resume(session));
                t.view.update(cx, |v, cx| v.input(command.as_bytes(), cx));
            }
        }
        if self.terms.is_empty() && self.messages_tab.is_none() {
            let (model, first) = crate::model::Model::new();
            self.model = model;
            let _ = self.spawn(first, &home, &[], window, cx);
        }
    }

    pub(super) fn start_saving(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.on_app_quit(|w, cx| {
            if w.settings.restore {
                w.save_state(cx);
            }
            async {}
        })
        .detach();
        cx.on_release(|w, cx| {
            if w.settings.restore {
                w.save_state(cx);
            }
        })
        .detach();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(5)).await;
                let alive = this.update(cx, |w, cx| {
                    if w.settings.restore {
                        w.save_state(cx);
                    }
                });
                if alive.is_err() {
                    return;
                }
            }
        })
        .detach();
    }
}
