//! Terminals linked to a PR review. lgtm's `UserPromptSubmit` hook asks for each prompt's
//! context: the brief once per agent session, then the thread the user is on, or once after
//! unlinking, that the link is gone.

use serde::Deserialize;
use specta::Type;

use super::RepoCtx;

/// The thread the user is on, as the agent reads it.
#[derive(Clone, Debug, Deserialize, Type)]
pub struct LinkThread {
    /// The whole thread, for the first prompt on it.
    pub full: String,
    /// One line naming it, for later prompts.
    pub short: String,
}

pub(super) enum Link {
    Linked {
        /// How the agent works on the PR's threads.
        brief: Option<String>,
        thread: Option<LinkThread>,
        /// The thread text the agent last got whole.
        told: Option<String>,
    },
    /// Unlinked, and the agent hasn't heard yet.
    Unlinked,
}

const UNLINKED: &str = "lgtm: this terminal is no longer linked to the PR review. Drop the \
                        thread context earlier lgtm messages gave you.";

impl RepoCtx {
    /// Links `terminal`'s agent, with `brief` and `thread` as the user's thread, or with
    /// `linked` false unlinks it.
    pub fn set_agent_link(
        &self,
        terminal: &str,
        linked: bool,
        brief: Option<String>,
        thread: Option<LinkThread>,
    ) {
        let mut links = self.reviews.links.lock().unwrap();
        let told = match links.get_mut(terminal) {
            Some(Link::Linked { told, .. }) => told.take(),
            _ => None,
        };
        if linked {
            let link = Link::Linked {
                brief,
                thread,
                told,
            };
            links.insert(terminal.to_string(), link);
        } else if matches!(links.get(terminal), Some(Link::Linked { .. })) {
            links.insert(terminal.to_string(), Link::Unlinked);
        }
    }

    /// Forgets what `terminal`'s agent was told, for a new session there, which starts with
    /// none of it.
    pub fn new_agent_session(&self, terminal: &str) {
        self.reviews.briefed.lock().unwrap().remove(terminal);
        if let Some(Link::Linked { told, .. }) =
            self.reviews.links.lock().unwrap().get_mut(terminal)
        {
            *told = None;
        }
    }

    /// What to add to a prompt submitted in `terminal`: the brief when the agent hasn't had it
    /// as it is now, then the user's thread, whole when the agent hasn't had it as it is now,
    /// else one line naming it.
    pub fn prompt_context(&self, terminal: &str) -> Option<String> {
        let mut links = self.reviews.links.lock().unwrap();
        let (brief, thread, told) = match links.get_mut(terminal)? {
            Link::Unlinked => {
                links.remove(terminal);
                return Some(UNLINKED.into());
            }
            Link::Linked {
                brief,
                thread,
                told,
            } => (brief.as_ref(), thread.as_ref(), told),
        };
        let mut briefed = self.reviews.briefed.lock().unwrap();
        let brief = brief.filter(|b| briefed.get(terminal) != Some(*b));
        if let Some(b) = brief {
            briefed.insert(terminal.to_string(), b.clone());
        }
        let thread = thread.map(|t| {
            if told.as_deref() == Some(t.full.as_str()) {
                return t.short.clone();
            }
            *told = Some(t.full.clone());
            t.full.clone()
        });
        let parts: Vec<String> = brief.cloned().into_iter().chain(thread).collect();
        (!parts.is_empty()).then(|| parts.join("\n\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(id: u32) -> Option<LinkThread> {
        Some(LinkThread {
            full: format!("whole {id}"),
            short: format!("line {id}"),
        })
    }

    #[test]
    fn a_changed_thread_or_a_new_session_gets_it_whole_again() {
        let (_dir, ctx) = crate::review::tests::repo();
        ctx.set_agent_link("t", true, None, on(1));
        assert_eq!(ctx.prompt_context("t").unwrap(), "whole 1");
        let mut edited = on(1).unwrap();
        edited.full = "whole 1, with my reply".into();
        ctx.set_agent_link("t", true, None, Some(edited));
        assert_eq!(ctx.prompt_context("t").unwrap(), "whole 1, with my reply");
        assert_eq!(ctx.prompt_context("t").unwrap(), "line 1");
        ctx.new_agent_session("t");
        assert_eq!(ctx.prompt_context("t").unwrap(), "whole 1, with my reply");
    }

    #[test]
    fn a_thread_goes_whole_once_then_by_name() {
        let (_dir, ctx) = crate::review::tests::repo();
        assert_eq!(ctx.prompt_context("t"), None, "not linked");
        ctx.set_agent_link("t", true, None, None);
        assert_eq!(ctx.prompt_context("t"), None, "no thread picked");
        ctx.set_agent_link("t", true, None, on(1));
        assert_eq!(ctx.prompt_context("t").unwrap(), "whole 1");
        assert_eq!(ctx.prompt_context("t").unwrap(), "line 1");
        ctx.set_agent_link("t", true, None, on(2));
        assert_eq!(ctx.prompt_context("t").unwrap(), "whole 2");
        // Back and forth with no prompt between: the agent last had 2 whole.
        ctx.set_agent_link("t", true, None, on(1));
        ctx.set_agent_link("t", true, None, on(2));
        assert_eq!(ctx.prompt_context("t").unwrap(), "line 2");
    }

    #[test]
    fn the_brief_goes_once_per_agent_session_and_again_when_it_changes() {
        let (_dir, ctx) = crate::review::tests::repo();
        let brief = |b: &str| Some(b.to_string());
        ctx.set_agent_link("t", true, brief("brief"), None);
        assert_eq!(ctx.prompt_context("t").unwrap(), "brief");
        assert_eq!(ctx.prompt_context("t"), None);
        ctx.set_agent_link("t", true, brief("brief"), on(1));
        assert_eq!(ctx.prompt_context("t").unwrap(), "whole 1");
        // Unlinking and linking again in the same session needs no new brief.
        ctx.set_agent_link("t", false, None, None);
        assert_eq!(ctx.prompt_context("t").unwrap(), UNLINKED);
        ctx.set_agent_link("t", true, brief("brief"), on(1));
        assert_eq!(ctx.prompt_context("t").unwrap(), "whole 1");
        ctx.set_agent_link("t", true, brief("other PR"), on(1));
        assert_eq!(ctx.prompt_context("t").unwrap(), "other PR\n\nline 1");
        ctx.new_agent_session("t");
        assert_eq!(ctx.prompt_context("t").unwrap(), "other PR\n\nwhole 1");
    }

    #[test]
    fn unlinking_tells_the_agent_once() {
        let (_dir, ctx) = crate::review::tests::repo();
        ctx.set_agent_link("t", false, None, None);
        assert_eq!(ctx.prompt_context("t"), None, "never linked");
        ctx.set_agent_link("t", true, None, on(1));
        ctx.set_agent_link("t", false, None, None);
        assert_eq!(ctx.prompt_context("t").unwrap(), UNLINKED);
        assert_eq!(ctx.prompt_context("t"), None);
        // Linking again starts over, so the thread goes whole.
        ctx.set_agent_link("t", true, None, on(1));
        assert_eq!(ctx.prompt_context("t").unwrap(), "whole 1");
    }
}
