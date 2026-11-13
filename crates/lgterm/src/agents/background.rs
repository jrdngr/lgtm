//! Work an agent leaves running after its turn: subagents, which workflows also run, and
//! background shells. A ready agent with some still running shows it instead of done or idle.

use serde_json::Value;

use super::{Agents, Kind, Status};
use crate::model::TermId;

impl Agents {
    /// The status the agent in `term` shows: its own, or what it left running once it's ready.
    pub fn shown(&self, term: TermId) -> Option<Status> {
        let agent = self.agents.get(&term)?;
        Some(match agent.status {
            s if !s.ready() => s,
            _ if !agent.subagents.is_empty() => Status::Subagents,
            _ if agent.shells > 0 => Status::Shells,
            s => s,
        })
    }

    /// Takes a hook from a subagent. Says whether it was one, so the agent's own status stays
    /// as it is: a subagent at work after the agent stopped doesn't make the agent working.
    pub(super) fn subagent(&mut self, term: TermId, event: &Value) -> bool {
        let Some(id) = event["agent_id"].as_str() else {
            return false;
        };
        let Some(agent) = self.agents.get_mut(&term) else {
            return true;
        };
        let changed = match event["hook_event_name"].as_str() {
            Some("SubagentStart") => agent.subagents.insert(id.to_owned()),
            Some("SubagentStop") => agent.subagents.remove(id),
            _ => false,
        };
        self.changed |= changed;
        true
    }

    /// Counts the background shells of a ready Claude Code agent.
    pub(super) fn count_shells(&mut self, term: TermId, count: &dyn Fn() -> usize) {
        let Some(agent) = self.agents.get_mut(&term) else {
            return;
        };
        let shells = if agent.kind == Kind::Claude && agent.status.ready() {
            count()
        } else {
            0
        };
        if shells != agent.shells {
            agent.shells = shells;
            self.changed = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::agents::{Env, Seen};

    const T: TermId = TermId(1);

    fn no_one_typing(_: TermId, _: Kind) -> bool {
        false
    }

    const AWAY: Env = Env {
        viewed: None,
        typing: &no_one_typing,
    };

    fn hook(a: &mut Agents, name: &str, agent_id: Option<&str>) {
        let mut event = json!({"hook_event_name": name, "session_id": "s"});
        if let Some(id) = agent_id {
            event["agent_id"] = id.into();
        }
        a.hook(T, &event, &AWAY);
    }

    #[test]
    fn a_stopped_agent_shows_its_running_subagents_until_the_last_stops() {
        let mut a = Agents::default();
        hook(&mut a, "UserPromptSubmit", None);
        hook(&mut a, "SubagentStart", Some("a1"));
        hook(&mut a, "SubagentStart", Some("a2"));
        assert_eq!(a.shown(T), Some(Status::Working));
        hook(&mut a, "Stop", None);
        assert_eq!(a.shown(T), Some(Status::Subagents));
        // A subagent at work doesn't make the agent itself working again.
        hook(&mut a, "PreToolUse", Some("a1"));
        assert_eq!(a.status(T), Some(Status::Done));
        hook(&mut a, "SubagentStop", Some("a1"));
        assert_eq!(a.shown(T), Some(Status::Subagents));
        hook(&mut a, "SubagentStop", Some("a2"));
        assert_eq!(a.shown(T), Some(Status::Done));
    }

    #[test]
    fn shells_are_counted_only_once_the_agent_is_ready_and_subagents_show_first() {
        let mut a = Agents::default();
        let poll = |a: &mut Agents, shells: usize| {
            let seen = Seen {
                kind: Some(Kind::Claude),
                process_known: true,
                shell_foreground: false,
                screen: &String::new,
                shells: &move || shells,
            };
            a.poll(T, seen, &AWAY);
        };
        hook(&mut a, "UserPromptSubmit", None);
        poll(&mut a, 1);
        assert_eq!(a.shown(T), Some(Status::Working));
        hook(&mut a, "Stop", None);
        poll(&mut a, 1);
        assert_eq!(a.shown(T), Some(Status::Shells));
        hook(&mut a, "SubagentStart", Some("a1"));
        assert_eq!(a.shown(T), Some(Status::Subagents));
        hook(&mut a, "SubagentStop", Some("a1"));
        poll(&mut a, 0);
        assert_eq!(a.shown(T), Some(Status::Done));
    }
}
