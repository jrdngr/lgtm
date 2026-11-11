//! Where a chunk's code came from: the prompts whose terminal agent edits wrote its lines, as
//! lgtm's Claude Code hooks recorded them.

use serde::Serialize;
use specta::Type;

use crate::ctx::RepoCtx;
use crate::store::AgentEdit;

/// A prompt whose edits wrote some of a chunk's lines.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PromptDto {
    pub prompt: String,
    pub session: String,
    /// When its last matching edit was made.
    pub at: f64,
}

/// Lines too short to say where they came from, like a lone brace.
const MIN_LINE: usize = 8;

/// The prompts of `edits` (newest first) that wrote one of `lines`, newest first, each once.
pub fn prompts_of(edits: &[AgentEdit], lines: &[String]) -> Vec<PromptDto> {
    let wanted: Vec<&str> = lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| l.len() >= MIN_LINE)
        .collect();
    let mut out: Vec<PromptDto> = Vec::new();
    for e in edits {
        let wrote = e.text.lines().map(str::trim).any(|l| wanted.contains(&l));
        let seen = out
            .iter()
            .any(|p| p.session == e.session && p.prompt == e.prompt);
        if wrote && !seen {
            out.push(PromptDto {
                prompt: e.prompt.clone(),
                session: e.session.clone(),
                at: e.at as f64,
            });
        }
    }
    out
}

impl RepoCtx {
    /// The prompts behind `lines`, the added lines of a chunk in `paths`.
    pub fn chunk_prompts(
        &self,
        paths: &[String],
        lines: &[String],
    ) -> anyhow::Result<Vec<PromptDto>> {
        let paths: Vec<&str> = paths.iter().map(String::as_str).collect();
        let edits = self.store.lock().unwrap().agent_edits(&paths)?;
        Ok(prompts_of(&edits, lines))
    }

    /// Records what a hook saw: the prompt a session started on, or an edit it made.
    pub fn record_hook(&self, session: &str, prompt: Option<&str>, edit: Option<(&str, &str)>) {
        let store = self.store.lock().unwrap();
        let at = crate::ctx::now_ms();
        if let Some(prompt) = prompt {
            store.record_agent_prompt(session, prompt, at).ok();
        }
        if let Some((path, text)) = edit {
            store.record_agent_edit(session, path, text, at).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(session: &str, prompt: &str, text: &str, at: i64) -> AgentEdit {
        AgentEdit {
            session: session.into(),
            prompt: prompt.into(),
            path: "/r/a.rs".into(),
            text: text.into(),
            at,
        }
    }

    #[test]
    fn a_prompt_counts_once_for_each_edit_that_wrote_a_line() {
        let edits = [
            edit("s", "Kill on timeout", "    child.kill()?;\n}", 3),
            edit("s", "Kill on timeout", "        child.kill()?;", 2),
            edit("t", "Spawn it", "let child = jj_spawn(ws)?;", 1),
            edit("t", "Rename", "let other = 1;", 0),
        ];
        let lines = [
            "child.kill()?;".to_string(),
            "let child = jj_spawn(ws)?;".into(),
        ];
        let got: Vec<String> = prompts_of(&edits, &lines)
            .into_iter()
            .map(|p| p.prompt)
            .collect();
        assert_eq!(got, ["Kill on timeout", "Spawn it"]);
    }

    #[test]
    fn short_lines_say_nothing_about_where_code_came_from() {
        let edits = [edit("s", "Anything", "}\n  }\nok", 1)];
        assert!(prompts_of(&edits, &["}".to_string(), "ok".into()]).is_empty());
    }
}
