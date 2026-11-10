//! Saving the layouts as they are, and opening them again with their agents resumed.

use std::time::Duration;

use gpui::{Context, Window};

use super::{Workspace, sidebar};
use crate::state::{SavedTerm, State};

/// How often the state is saved, if it changed.
const SAVE_EVERY: Duration = Duration::from_secs(5);

impl Workspace {
    fn snapshot(&self) -> State {
        let terms = self
            .terms
            .iter()
            .map(|(id, t)| {
                let agent = self
                    .agents
                    .get(*id)
                    .and_then(|a| Some((a.kind, a.session.clone()?)));
                let saved = SavedTerm {
                    cwd: t.cwd().to_path_buf(),
                    agent,
                };
                (*id, saved)
            })
            .collect();
        let mut collapsed: Vec<u64> = self.collapsed.iter().copied().collect();
        collapsed.sort_unstable();
        State::new(
            self.model.clone(),
            terms,
            &self.agents.messages,
            collapsed,
            self.sidebar,
            self.sidebar_width,
        )
    }

    /// Writes the state if anything changed since the last save.
    pub(super) fn save_state(&mut self) {
        if !std::mem::take(&mut self.dirty) {
            return;
        }
        let Some(json) = self.snapshot().to_json() else {
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
        self.agents.messages = state.messages;
        self.collapsed = state.collapsed.into_iter().collect();
        self.sidebar = state.sidebar;
        if let Some(w) = state.sidebar_width {
            self.sidebar_width = sidebar::snap(w);
        }
        let home = self.home_dir();
        for id in self.model.terms() {
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
    }

    pub(super) fn start_saving(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.observe_self(|w, _| w.dirty = true).detach();
        cx.on_app_quit(|w, _| {
            if w.settings.restore {
                w.dirty = true;
                w.save_state();
            }
            async {}
        })
        .detach();
        cx.on_release(|w, _| {
            if w.settings.restore {
                w.dirty = true;
                w.save_state();
            }
        })
        .detach();
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(SAVE_EVERY).await;
                let alive = this.update(cx, |w, _| {
                    if w.settings.restore {
                        w.save_state();
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
