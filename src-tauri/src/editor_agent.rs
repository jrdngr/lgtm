//! Asking an agent about lines selected in an editor tab, or having it change them.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::agent::{self, Runs};
use crate::bots::fence;
use crate::workspace::Workspace;

/// Lines of a file as selected in its tab, numbered from 1.
#[derive(Clone, Deserialize, Type)]
pub struct SelectedLines {
    pub path: String,
    pub start: u32,
    pub end: u32,
    pub text: String,
}

/// One message to the agent of an editor tab's agent box.
#[derive(Deserialize, Type)]
pub struct SelectionAsk {
    pub lines: SelectedLines,
    pub message: String,
    /// Have the agent edit the working copy.
    pub change: bool,
    /// The box's session, to continue.
    pub session: Option<String>,
}

#[derive(Serialize, Type)]
pub struct SelectionReply {
    pub text: String,
    pub session: Option<String>,
    /// Stopped by the user. `text` is what had streamed so far.
    pub stopped: bool,
}

const EDIT: &str = "\n\nEdit the files directly and change only what's needed. Then say in a \
sentence or two what you changed.";

/// One turn, with the selection when it's `first` in its session.
fn prompt(sel: &SelectedLines, message: &str, change: bool, first: bool) -> String {
    let turn = match change {
        true => format!("Make this change: {message}{EDIT}"),
        false => message.to_string(),
    };
    if !first {
        return turn;
    }
    let lines = match sel.start == sel.end {
        true => format!("line {}", sel.start),
        false => format!("lines {}-{}", sel.start, sel.end),
    };
    let fence = fence(&sel.text);
    format!(
        "I'm looking at {lines} of `{path}` in this repository:\n\n{fence}\n{text}\n{fence}\n\n\
         {turn}\n\nKeep replies short.",
        path = sel.path,
        text = sel.text,
    )
}

/// Sends `ask` to the agent in `ws`, streaming its reply through `on_text`. A change claims the
/// workspace while it runs and snapshots the working copy after.
pub async fn ask(
    ws: &Workspace,
    runs: &Runs,
    key: &str,
    ask: &SelectionAsk,
    on_text: impl Fn(String),
) -> anyhow::Result<SelectionReply> {
    let _busy = ask.change.then(|| ws.claim()).transpose()?;
    let task = |prompt, resume| agent::Task {
        cwd: &ws.root,
        prompt,
        resume,
        fork: false,
        edit: ask.change,
        schema: None,
        purpose: agent::Purpose::Ask,
    };
    let on = |p| {
        if let agent::Progress::Text(t) = p {
            on_text(t)
        }
    };
    let (sel, message, resume) = (&ask.lines, ask.message.trim(), ask.session.as_deref());
    let turn = prompt(sel, message, ask.change, resume.is_none());
    let mut reply = agent::run(runs, key, task(&turn, resume), &on).await;
    // A session Claude Code no longer has starts over with the selection.
    if resume.is_some() && reply.as_ref().err().is_some_and(agent::session_missing) {
        let intro = prompt(sel, message, ask.change, true);
        reply = agent::run(runs, key, task(&intro, None), &on).await;
    }
    if ask.change {
        // Pick up the edits now rather than waiting on the watcher.
        ws.snapshot().await.ok();
    }
    let reply = reply?;
    Ok(SelectionReply {
        text: reply.text,
        session: reply.session,
        stopped: reply.stopped,
    })
}
