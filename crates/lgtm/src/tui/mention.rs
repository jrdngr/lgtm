//! Sending where a selection is to an agent in the same lgterm space: its `path:lines`, pasted
//! at the agent's prompt without Enter, with the agent focused.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use lgterm_proto::{Reply, Request};
use lgtm_core::diff::{Row, RowKind};
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use serde::Deserialize;

const TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Deserialize)]
pub struct Agent {
    pub id: String,
    pub label: String,
}

/// Agents to pick from for `text`, numbered from 1.
pub struct Pick {
    text: String,
    agents: Vec<Agent>,
}

/// What came of sending: nothing more to do, a pick to show, or a notice.
pub enum Sent {
    Done,
    Pick(Pick),
    Notice(String),
}

fn ask(request: &Request) -> Result<String> {
    let (_, socket) = lgterm_proto::here().context("not in an lgterm terminal")?;
    match lgterm_proto::call(&socket, request, Some(TIMEOUT))? {
        Reply::Ok(text) => Ok(text),
        Reply::Err(e) => bail!(e),
    }
}

fn paste(agent: &Agent, text: &str) -> Result<()> {
    ask(&Request::Paste {
        to: agent.id.clone(),
        text: format!("{text}\n"),
        focus: true,
    })
    .map(|_| ())
}

/// Sends `text` to the agent in this space, or asks which when there are several.
pub fn send(text: String) -> Sent {
    let Some((term, _)) = lgterm_proto::here() else {
        return Sent::Notice("send to agent works in lgterm".into());
    };
    let agents = ask(&Request::Agents {
        term: term.to_string(),
    })
    .and_then(|json| Ok(serde_json::from_str::<Vec<Agent>>(&json)?));
    match agents {
        Err(e) => Sent::Notice(format!("send to agent: {e:#}")),
        Ok(agents) if agents.is_empty() => Sent::Notice("no agent in this space".into()),
        Ok(agents) if agents.len() == 1 => match paste(&agents[0], &text) {
            Ok(()) => Sent::Done,
            Err(e) => Sent::Notice(format!("send to agent: {e:#}")),
        },
        Ok(agents) => Sent::Pick(Pick { text, agents }),
    }
}

impl Pick {
    /// The agents for a menu, keyed `1`, `2`, ….
    pub fn menu(&self) -> Vec<(String, &str)> {
        self.agents
            .iter()
            .enumerate()
            .take(9)
            .map(|(i, a)| ((i + 1).to_string(), a.label.as_str()))
            .collect()
    }

    /// Sends to the agent whose number was pressed. Any other key cancels. A notice when
    /// sending failed.
    pub fn key(self, key: KeyEvent) -> Option<String> {
        let KeyCode::Char(c @ '1'..='9') = key.code else {
            return None;
        };
        let agent = self.agents.get(c as usize - '1' as usize)?;
        paste(agent, &self.text)
            .err()
            .map(|e| format!("send to agent: {e:#}"))
    }
}

/// `path:line` or `path:start-end` for lines `from..=to`, from 1.
pub fn lines(path: &str, from: usize, to: usize) -> String {
    format!("{path}:{}", range(&[from as u32, to as u32]))
}

fn range(lines: &[u32]) -> String {
    let (lo, hi) = (lines.iter().min(), lines.iter().max());
    match (lo, hi) {
        (Some(lo), Some(hi)) if lo != hi => format!("{lo}-{hi}"),
        (Some(lo), _) => lo.to_string(),
        _ => String::new(),
    }
}

/// Where `rows` of one hunk are: the new side's lines, then the removed rows' old lines, or
/// only those.
pub fn rows(path: &str, rows: &[&Row]) -> String {
    let added: Vec<u32> = rows.iter().filter_map(|r| r.new_line).collect();
    let removed: Vec<u32> = rows
        .iter()
        .filter(|r| r.kind == RowKind::Removed)
        .filter_map(|r| r.old_line)
        .collect();
    if added.is_empty() {
        return format!("{path}:{} (removed, old version)", range(&removed));
    }
    let at = format!("{path}:{}", range(&added));
    if removed.is_empty() {
        return at;
    }
    let plural = if removed.len() > 1 { "s" } else { "" };
    format!("{at}, and removed old line{plural} {}", range(&removed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(kind: RowKind, old: Option<u32>, new: Option<u32>) -> Row {
        Row {
            kind,
            old_line: old,
            new_line: new,
            text: String::new(),
        }
    }

    #[test]
    fn rows_say_new_lines_then_removed_old_ones() {
        let ctx = row(RowKind::Context, Some(3), Some(4));
        let gone = row(RowKind::Removed, Some(4), None);
        let gone2 = row(RowKind::Removed, Some(5), None);
        let new = row(RowKind::Added, None, Some(5));
        assert_eq!(rows("a.rs", &[&ctx, &new]), "a.rs:4-5");
        assert_eq!(
            rows("a.rs", &[&ctx, &gone, &gone2, &new]),
            "a.rs:4-5, and removed old lines 4-5"
        );
        assert_eq!(rows("a.rs", &[&gone]), "a.rs:4 (removed, old version)");
    }

    #[test]
    fn a_single_line_has_no_range() {
        assert_eq!(lines("a.rs", 7, 7), "a.rs:7");
        assert_eq!(lines("a.rs", 7, 9), "a.rs:7-9");
    }
}
