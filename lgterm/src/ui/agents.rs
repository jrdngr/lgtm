//! Agents in terminals: their statuses, and getting the user's attention when they stop.

use std::time::{Duration, Instant};

use gpui::{Context, SystemNotification, Window};
use serde_json::Value;

use super::Workspace;
use crate::dock;
use crate::ipc::Reply;
use crate::status::{self, Kind, Status};

/// Polls in a row with no sign of work before a hooked agent that never said it stopped
/// counts as idle.
const QUIET_POLLS: u8 = 3;

/// How long after a message goes in before the next queued one can follow.
pub(super) const DELIVERY_GAP: Duration = Duration::from_secs(3);

pub struct Agent {
    pub kind: Kind,
    pub status: Status,
    /// Whether its hooks report its status.
    pub hooked: bool,
    pub session: Option<String>,
    pub(super) quiet: u8,
    pub(super) busy_until: Option<Instant>,
}

impl Agent {
    pub fn new(kind: Kind) -> Self {
        Agent {
            kind,
            status: Status::Idle,
            hooked: false,
            session: None,
            quiet: 0,
            busy_until: None,
        }
    }

    /// Whether a queued message can go in now.
    pub fn can_take_message(&self) -> bool {
        self.status.ready() && self.busy_until.is_none_or(|t| Instant::now() >= t)
    }
}

impl Workspace {
    pub(super) fn status(&self, term: u64) -> Option<Status> {
        Some(self.terms.get(&term)?.agent.as_ref()?.status)
    }

    /// The most urgent status among `terms`.
    pub(super) fn worst(&self, terms: impl IntoIterator<Item = u64>) -> Option<Status> {
        terms
            .into_iter()
            .filter_map(|t| self.status(t))
            .max_by_key(|s| s.urgency())
    }

    pub(super) fn any_working(&self) -> bool {
        self.terms.values().any(|t| {
            t.agent
                .as_ref()
                .is_some_and(|a| a.status == Status::Working)
        })
    }

    /// Whether the user is looking at `term`.
    fn viewed(&self, term: u64, window: &Window) -> bool {
        window.is_window_active() && self.model.term() == term
    }

    pub(super) fn hook(
        &mut self,
        term: u64,
        event: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Reply {
        let Some(report) = status::report(event) else {
            return Reply::Ok(String::new());
        };
        let Some(t) = self.terms.get_mut(&term) else {
            return Reply::Err(format!("no terminal t{term}"));
        };
        if report.ended {
            t.agent = None;
            self.agent_left(term, cx);
            return Reply::Ok(String::new());
        }
        let agent = t.agent.get_or_insert_with(|| Agent::new(report.kind));
        agent.kind = report.kind;
        agent.hooked = true;
        if report.session.is_some() {
            agent.session = report.session;
        }
        if let Some(s) = report.status {
            self.set_status(term, s, window, cx);
        }
        cx.notify();
        Reply::Ok(String::new())
    }

    pub(super) fn set_status(
        &mut self,
        term: u64,
        status: Status,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let reported = status;
        let viewed = self.viewed(term, window);
        let status = if status == Status::Done && viewed {
            Status::Idle
        } else {
            status
        };
        let Some(agent) = self.terms.get_mut(&term).and_then(|t| t.agent.as_mut()) else {
            return;
        };
        agent.quiet = 0;
        let old = agent.status;
        agent.status = status;
        if old != status {
            if status == Status::Working
                && let Some(t) = self.terms.get(&term)
            {
                t.view.update(cx, |v, _| v.clear_draft());
            }
            if matches!(status, Status::Blocked | Status::Done) {
                self.alert(term, status, cx);
            }
        }
        if status.ready() {
            self.deliver(term, cx);
        }
        // A queued message went in, so waiters hear about the turn it starts instead.
        let delivered = status != Status::Working && self.status(term) == Some(Status::Working);
        if !delivered {
            if reported == Status::Done {
                self.finish_waits(term, "done");
            } else if old != status
                && status != Status::Working
                && (status != Status::Idle || old == Status::Working)
            {
                self.finish_waits(term, status.name());
            }
        }
        if old != status || delivered {
            self.update_badge();
            cx.notify();
        }
    }

    /// An agent quit: anything waiting on it stops waiting.
    pub(super) fn agent_left(&mut self, term: u64, cx: &mut Context<Self>) {
        self.finish_waits(term, "gone");
        self.drop_queued(term);
        self.update_badge();
        cx.notify();
    }

    pub(super) fn finish_waits(&mut self, term: u64, outcome: &str) {
        self.waiters.retain(|(t, reply)| {
            if *t != term {
                return !reply.is_closed();
            }
            let _ = reply.try_send(Reply::Ok(outcome.to_owned()));
            false
        });
    }

    /// Updates agents from what's running in each terminal and what their screens show.
    pub(super) fn poll_agents(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ids: Vec<u64> = self.terms.keys().copied().collect();
        for id in ids {
            let Some(t) = self.terms.get_mut(&id) else {
                continue;
            };
            let kind = t.info.kind;
            match (&t.agent, kind, t.info.process.is_some()) {
                (Some(a), None, true) if !a.hooked || t.info.shell_foreground => {
                    t.agent = None;
                    self.agent_left(id, cx);
                    continue;
                }
                (None, Some(kind), _) => t.agent = Some(Agent::new(kind)),
                _ => {}
            }
            let Some(agent) = &mut t.agent else { continue };
            let scrape = !agent.hooked || agent.kind == Kind::Codex;
            let kind = agent.kind;
            let screen = t.view.read(cx).screen_text();
            let seen = status::from_screen(kind, &screen);
            let agent = t.agent.as_mut().expect("agent");
            let next = match seen {
                Some(s) if scrape => Some(s),
                Some(s) => {
                    agent.quiet = 0;
                    (s == Status::Working && agent.status == Status::Idle).then_some(s)
                }
                // A message just went in, and the agent may not show it yet.
                None if agent.busy_until.is_some_and(|t| Instant::now() < t) => None,
                None if agent.status == Status::Working => {
                    agent.quiet += 1;
                    (scrape || agent.quiet >= QUIET_POLLS).then_some(if scrape {
                        Status::Done
                    } else {
                        Status::Idle
                    })
                }
                None if scrape && agent.status == Status::Blocked => Some(Status::Done),
                None if agent.status == Status::Blocked
                    && kind == Kind::Claude
                    && status::claude_interrupted(&screen) =>
                {
                    Some(Status::Idle)
                }
                None => None,
            };
            if let Some(s) = next {
                self.set_status(id, s, window, cx);
            }
        }
        self.clear_viewed(window, cx);
        let ready: Vec<u64> = self
            .terms
            .iter()
            .filter(|(_, t)| t.agent.as_ref().is_some_and(Agent::can_take_message))
            .map(|(id, _)| *id)
            .collect();
        for id in ready {
            self.deliver(id, cx);
        }
    }

    /// A done agent the user is looking at goes back to idle.
    pub(super) fn clear_viewed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let term = self.model.term();
        if self.status(term) == Some(Status::Done) && self.viewed(term, window) {
            self.set_status(term, Status::Idle, window, cx);
        }
    }

    fn alert(&mut self, term: u64, status: Status, cx: &mut Context<Self>) {
        let notify = match status {
            Status::Blocked => self.settings.notify_blocked,
            _ => self.settings.notify_done,
        };
        if notify {
            let space = self
                .model
                .locate(term)
                .and_then(|p| self.model.layouts[p.layout].spaces.find(p.space))
                .map(|s| self.space_label(s, cx))
                .unwrap_or_default();
            let state = match status {
                Status::Blocked => "Blocked",
                _ => "Done",
            };
            cx.show_system_notification(SystemNotification {
                tag: format!("t{term}").into(),
                title: self.term_label(term, cx).into(),
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
            self.terms
                .values()
                .filter_map(|t| t.agent.as_ref())
                .filter(|a| matches!(a.status, Status::Blocked | Status::Done))
                .count()
        } else {
            0
        };
        if count != self.badge {
            self.badge = count;
            dock::set_badge(count);
        }
    }

    /// Terminals in sidebar order.
    pub(super) fn ordered_terms(&self) -> Vec<u64> {
        self.model
            .layouts
            .iter()
            .flat_map(|l| l.spaces.leaves())
            .flat_map(|s| s.terms().collect::<Vec<_>>())
            .collect()
    }

    /// The next blocked agent after the focused terminal, or failing that the next done one.
    pub(super) fn next_waiting(&self) -> Option<u64> {
        let order = self.ordered_terms();
        let at = order
            .iter()
            .position(|t| *t == self.model.term())
            .unwrap_or(0);
        let after = order[at + 1..].iter().chain(&order[..=at]).copied();
        let after: Vec<u64> = after.collect();
        [Status::Blocked, Status::Done]
            .into_iter()
            .find_map(|want| {
                after
                    .iter()
                    .copied()
                    .find(|t| self.status(*t) == Some(want))
            })
    }
}
