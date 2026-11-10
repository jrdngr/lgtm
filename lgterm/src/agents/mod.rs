//! Agents in terminals: what each is doing, the messages queued for them, and who's waiting
//! on them. This decides; the window carries out the effects.

pub mod kind;
pub mod messages;
pub mod status;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use lgterm_proto::Reply;
use serde_json::Value;

use crate::model::TermId;
pub use kind::Kind;
pub use messages::Message;
pub use status::Status;

/// Polls in a row with no sign of work before a hooked agent that never said it stopped
/// counts as idle.
const QUIET_POLLS: u8 = 3;

/// How long after a message goes in before the next queued one can follow.
const DELIVERY_GAP: Duration = Duration::from_secs(3);

/// The same hook twice within this is one report that reached lgterm twice, as when both
/// `lgtm hook` and `lgterm hook` are set up.
const REPEAT: Duration = Duration::from_secs(1);

pub struct Agent {
    pub kind: Kind,
    pub status: Status,
    /// Whether its hooks report its status.
    hooked: bool,
    pub session: Option<String>,
    quiet: u8,
    busy_until: Option<Instant>,
    last_hook: Option<(Value, Instant)>,
}

impl Agent {
    fn new(kind: Kind) -> Self {
        Agent {
            kind,
            status: Status::Idle,
            hooked: false,
            session: None,
            quiet: 0,
            busy_until: None,
            last_hook: None,
        }
    }

    fn busy(&self) -> bool {
        self.busy_until.is_some_and(|t| Instant::now() < t)
    }

    /// Whether a queued message can go in now.
    fn can_take_message(&self) -> bool {
        self.status.ready() && !self.busy()
    }
}

/// Something for the window to do.
#[derive(Debug, PartialEq)]
pub enum Effect {
    /// Paste this into the terminal and press return.
    Deliver(TermId, String),
    /// The agent took what the user typed.
    ClearDraft(TermId),
    /// Tell the user the agent stopped.
    Alert(TermId, Status),
}

/// What the window knows that decisions depend on.
pub struct Env<'a> {
    /// The terminal the user is looking at, if the window is active.
    pub viewed: Option<TermId>,
    /// Whether the user is typing in a terminal.
    pub typing: &'a dyn Fn(TermId) -> bool,
}

/// What a poll saw in one terminal.
pub struct Seen<'a> {
    /// The agent its foreground process runs.
    pub kind: Option<Kind>,
    /// The foreground process is known, so `kind` being none means no agent runs.
    pub process_known: bool,
    /// The shell itself is in the foreground.
    pub shell_foreground: bool,
    /// The bottom of the screen, read only if needed.
    pub screen: &'a dyn Fn() -> String,
}

#[derive(Default)]
pub struct Agents {
    agents: HashMap<TermId, Agent>,
    pub messages: Vec<Message>,
    /// `lgterm wait` calls, by terminal.
    waiters: Vec<(TermId, async_channel::Sender<Reply>)>,
    effects: Vec<Effect>,
    /// Something on screen changed since the last `take`.
    changed: bool,
}

impl Agents {
    pub fn get(&self, term: TermId) -> Option<&Agent> {
        self.agents.get(&term)
    }

    pub fn status(&self, term: TermId) -> Option<Status> {
        Some(self.agents.get(&term)?.status)
    }

    /// The most urgent status among `terms`.
    pub fn worst(&self, terms: impl IntoIterator<Item = TermId>) -> Option<Status> {
        terms
            .into_iter()
            .filter_map(|t| self.status(t))
            .max_by_key(|s| s.urgency())
    }

    /// Agents waiting for the user: blocked or done.
    pub fn waiting(&self) -> usize {
        self.agents
            .values()
            .filter(|a| matches!(a.status, Status::Blocked | Status::Done))
            .count()
    }

    /// What to do, and whether anything shown changed, since the last call.
    pub fn take(&mut self) -> (Vec<Effect>, bool) {
        (
            std::mem::take(&mut self.effects),
            std::mem::take(&mut self.changed),
        )
    }

    pub fn hook(&mut self, term: TermId, event: &Value, env: &Env) {
        let Some(report) = status::report(event) else {
            return;
        };
        let now = Instant::now();
        let repeat = self.agents.get(&term).is_some_and(|a| {
            a.last_hook
                .as_ref()
                .is_some_and(|(e, at)| e == event && now - *at < REPEAT)
        });
        if repeat {
            return;
        }
        if report.ended {
            self.left(term);
            return;
        }
        let new = !self.agents.contains_key(&term);
        let agent = self
            .agents
            .entry(term)
            .or_insert_with(|| Agent::new(report.kind));
        let was = (agent.kind, agent.session.clone());
        agent.kind = report.kind;
        agent.hooked = true;
        agent.last_hook = Some((event.clone(), now));
        if report.session.is_some() {
            agent.session = report.session;
        }
        if new || was != (agent.kind, agent.session.clone()) {
            self.changed = true;
        }
        if let Some(s) = report.status {
            self.set_status(term, s, env);
        }
    }

    fn set_status(&mut self, term: TermId, status: Status, env: &Env) {
        let reported = status;
        let status = if status == Status::Done && env.viewed == Some(term) {
            Status::Idle
        } else {
            status
        };
        let Some(agent) = self.agents.get_mut(&term) else {
            return;
        };
        agent.quiet = 0;
        let old = agent.status;
        agent.status = status;
        if old != status {
            if status == Status::Working {
                self.effects.push(Effect::ClearDraft(term));
            }
            if matches!(status, Status::Blocked | Status::Done) {
                self.effects.push(Effect::Alert(term, status));
            }
        }
        if status.ready() {
            self.deliver(term, env);
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
            self.changed = true;
        }
    }

    /// The agent quit: anything waiting on it stops waiting.
    fn left(&mut self, term: TermId) {
        self.agents.remove(&term);
        self.finish_waits(term, "gone");
        self.drop_queued(term);
        self.changed = true;
    }

    /// The terminal closed.
    pub fn closed(&mut self, term: TermId) {
        self.agents.remove(&term);
        self.finish_waits(term, "exited");
        self.drop_queued(term);
        self.changed = true;
    }

    fn finish_waits(&mut self, term: TermId, outcome: &str) {
        self.waiters.retain(|(t, reply)| {
            if *t != term {
                return !reply.is_closed();
            }
            let _ = reply.try_send(Reply::Ok(outcome.to_owned()));
            false
        });
    }

    /// Answers once the agent in `term` is done, blocked or gone.
    pub fn wait(&mut self, term: TermId, reply: async_channel::Sender<Reply>) {
        let queued = self.messages.iter().any(|m| m.queued && m.to == term);
        match self.status(term) {
            Some(s @ (Status::Done | Status::Blocked)) if !queued => {
                let _ = reply.try_send(Reply::Ok(s.name().into()));
            }
            _ => {
                self.waiters.retain(|(_, r)| !r.is_closed());
                self.waiters.push((term, reply));
            }
        }
    }

    /// Updates the agent in `term` from what's running there and what its screen shows.
    pub fn poll(&mut self, term: TermId, seen: Seen, env: &Env) {
        match (self.agents.get(&term), seen.kind, seen.process_known) {
            (Some(a), None, true) if !a.hooked || seen.shell_foreground => {
                self.left(term);
                return;
            }
            (None, Some(kind), _) => {
                self.agents.insert(term, Agent::new(kind));
                self.changed = true;
            }
            _ => {}
        }
        let Some(agent) = self.agents.get_mut(&term) else {
            return;
        };
        let kind = agent.kind;
        let scrape = !agent.hooked || !kind.behavior().hooks_say_everything;
        if !scrape && agent.status == Status::Done {
            return;
        }
        let screen = (seen.screen)();
        let next = match kind.screen_status(&screen) {
            Some(s) if scrape => Some(s),
            Some(s) => {
                agent.quiet = 0;
                (s == Status::Working && agent.status == Status::Idle).then_some(s)
            }
            // A message just went in, and the agent may not show it yet.
            None if agent.busy() => None,
            None if agent.status == Status::Working => {
                agent.quiet += 1;
                if scrape {
                    Some(Status::Done)
                } else {
                    (agent.quiet >= QUIET_POLLS).then_some(Status::Idle)
                }
            }
            None if scrape && agent.status == Status::Blocked => Some(Status::Done),
            None if agent.status == Status::Blocked && kind.interrupted(&screen) => {
                Some(Status::Idle)
            }
            None => None,
        };
        if let Some(s) = next {
            self.set_status(term, s, env);
        }
    }

    /// After a poll or a change of focus: the agent the user is looking at isn't done any
    /// more, and ready agents take their queued messages.
    pub fn settle(&mut self, env: &Env) {
        if let Some(term) = env.viewed
            && self.status(term) == Some(Status::Done)
        {
            self.set_status(term, Status::Idle, env);
        }
        let ready: Vec<TermId> = self
            .agents
            .iter()
            .filter(|(_, a)| a.can_take_message())
            .map(|(id, _)| *id)
            .collect();
        for id in ready {
            self.deliver(id, env);
        }
    }

    /// Queues a message for the agent in `to`, and hands it over if it's ready. Says whether
    /// it was sent or queued.
    pub fn send(&mut self, mut message: Message, env: &Env) -> Result<&'static str, String> {
        let to = message.to;
        if !self.agents.contains_key(&to) {
            return Err(format!("{to} isn't running an agent"));
        }
        message.queued = true;
        self.messages.push(message);
        messages::trim(&mut self.messages);
        self.deliver(to, env);
        self.changed = true;
        let queued = self.messages.last().is_some_and(|m| m.queued);
        Ok(if queued { "queued" } else { "sent" })
    }

    /// Gives up on messages queued for `term`, whose agent is gone.
    fn drop_queued(&mut self, term: TermId) {
        for m in self
            .messages
            .iter_mut()
            .filter(|m| m.queued && m.to == term)
        {
            m.queued = false;
            m.dropped = true;
        }
    }

    /// Hands `term` its oldest queued message, if it's ready for one and the user isn't
    /// typing there. The agent counts as working until its own status says otherwise.
    fn deliver(&mut self, term: TermId, env: &Env) {
        let Some(agent) = self.agents.get_mut(&term).filter(|a| a.can_take_message()) else {
            return;
        };
        if (env.typing)(term) {
            return;
        }
        let Some(m) = self.messages.iter_mut().find(|m| m.queued && m.to == term) else {
            return;
        };
        m.queued = false;
        agent.busy_until = Some(Instant::now() + DELIVERY_GAP);
        agent.status = Status::Working;
        agent.quiet = 0;
        self.effects
            .push(Effect::Deliver(term, messages::envelope(m)));
        self.changed = true;
    }

    /// The next blocked agent after `from` in `order`, or failing that the next done one.
    pub fn next_waiting(&self, order: &[TermId], from: Option<TermId>) -> Option<TermId> {
        let at = from
            .and_then(|f| order.iter().position(|t| *t == f))
            .unwrap_or(0);
        let after: Vec<TermId> = order.iter().skip(at + 1).chain(order).copied().collect();
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const T: TermId = TermId(1);

    fn not_typing(_: TermId) -> bool {
        false
    }

    fn typing(_: TermId) -> bool {
        true
    }

    const AWAY: Env = Env {
        viewed: None,
        typing: &not_typing,
    };

    fn hook(a: &mut Agents, event: Value) {
        a.hook(T, &event, &AWAY);
    }

    fn message(text: &str) -> Message {
        Message {
            time: "09:00".into(),
            from: Some(TermId(2)),
            from_label: "a".into(),
            to: T,
            to_label: "b".into(),
            text: text.into(),
            queued: true,
            dropped: false,
        }
    }

    fn alerts(a: &mut Agents) -> usize {
        a.take()
            .0
            .iter()
            .filter(|e| matches!(e, Effect::Alert(..)))
            .count()
    }

    #[test]
    fn messages_wait_for_the_agent_to_finish_its_turn() {
        let mut a = Agents::default();
        hook(&mut a, json!({"hook_event_name": "UserPromptSubmit"}));
        assert_eq!(a.take().0, [Effect::ClearDraft(T)]);
        assert_eq!(a.send(message("first"), &AWAY), Ok("queued"));
        assert!(a.take().0.is_empty());

        hook(&mut a, json!({"hook_event_name": "Stop"}));
        let (effects, changed) = a.take();
        assert!(changed);
        let delivered =
            |e: &Effect| matches!(e, Effect::Deliver(T, text) if text.contains("first"));
        assert!(effects.iter().any(delivered));
        assert_eq!(a.status(T), Some(Status::Working));
    }

    #[test]
    fn typing_holds_a_message_back() {
        let mut a = Agents::default();
        hook(&mut a, json!({"hook_event_name": "SessionStart"}));
        let busy = Env {
            viewed: None,
            typing: &typing,
        };
        assert_eq!(a.send(message("hi"), &busy), Ok("queued"));
        a.settle(&AWAY);
        assert!(matches!(&a.take().0[..], [Effect::Deliver(T, _)]));
    }

    #[test]
    fn a_stop_the_user_is_watching_is_idle_but_still_ends_waits() {
        let mut a = Agents::default();
        hook(&mut a, json!({"hook_event_name": "UserPromptSubmit"}));
        let (tx, rx) = async_channel::bounded(1);
        a.wait(T, tx);
        let watching = Env {
            viewed: Some(T),
            typing: &not_typing,
        };
        a.hook(T, &json!({"hook_event_name": "Stop"}), &watching);
        assert_eq!(a.status(T), Some(Status::Idle));
        assert_eq!(rx.try_recv(), Ok(Reply::Ok("done".into())));
        assert_eq!(alerts(&mut a), 0);
    }

    #[test]
    fn the_same_hook_twice_counts_once() {
        let mut a = Agents::default();
        let ask = json!({"hook_event_name": "PermissionRequest"});
        hook(&mut a, ask.clone());
        hook(&mut a, json!({"hook_event_name": "PreToolUse"}));
        hook(&mut a, ask.clone());
        assert_eq!(alerts(&mut a), 2);
        hook(&mut a, ask);
        assert_eq!(alerts(&mut a), 0);
    }

    #[test]
    fn a_closed_terminal_drops_its_queue_and_its_waits() {
        let mut a = Agents::default();
        hook(&mut a, json!({"hook_event_name": "PreToolUse"}));
        a.send(message("later"), &AWAY).unwrap();
        let (tx, rx) = async_channel::bounded(1);
        a.wait(T, tx);
        a.closed(T);
        assert_eq!(rx.try_recv(), Ok(Reply::Ok("exited".into())));
        assert!(a.messages[0].dropped && !a.messages[0].queued);
        assert_eq!(
            a.send(message("x"), &AWAY),
            Err("t1 isn't running an agent".into())
        );
    }

    #[test]
    fn unhooked_agents_follow_their_screen() {
        let mut a = Agents::default();
        let poll = |a: &mut Agents, kind, screen: &str| {
            let screen = screen.to_owned();
            let seen = Seen {
                kind,
                process_known: true,
                shell_foreground: false,
                screen: &move || screen.clone(),
            };
            a.poll(T, seen, &AWAY);
        };
        poll(&mut a, Some(Kind::Codex), "• Working (esc to interrupt)");
        assert_eq!(a.status(T), Some(Status::Working));
        poll(&mut a, Some(Kind::Codex), "› ");
        assert_eq!(a.status(T), Some(Status::Done));
        poll(&mut a, None, "$ ");
        assert_eq!(a.status(T), None);
    }

    #[test]
    fn next_waiting_prefers_blocked_and_wraps_around() {
        let mut a = Agents::default();
        let ask = json!({"hook_event_name": "PermissionRequest"});
        let stop = json!({"hook_event_name": "Stop"});
        a.hook(TermId(1), &ask, &AWAY);
        a.hook(TermId(2), &stop, &AWAY);
        a.hook(TermId(3), &stop, &AWAY);
        let order = [TermId(1), TermId(2), TermId(3)];
        assert_eq!(a.next_waiting(&order, Some(TermId(2))), Some(TermId(1)));
        assert_eq!(a.next_waiting(&order, Some(TermId(1))), Some(TermId(1)));
        a.hook(TermId(1), &json!({"hook_event_name": "PreToolUse"}), &AWAY);
        assert_eq!(a.next_waiting(&order, Some(TermId(2))), Some(TermId(3)));
        assert_eq!(a.next_waiting(&[], None), None);
    }
}
