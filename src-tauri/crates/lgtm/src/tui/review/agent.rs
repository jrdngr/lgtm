//! Answers agents' requests over MCP about the open review.

use lgtm_core::anchor::Side;
use lgtm_core::store::{MarkColor, Store};
use serde_json::json;

use super::Review;
use super::marks::State;
use crate::agent::{self, AgentMark, CheckFiles, MarkLines, Request, Response};

fn state_name(s: State) -> &'static str {
    match s {
        State::None => "not reviewed",
        State::Partial => "partly reviewed",
        State::Reviewed => "reviewed",
        State::Agent => "reviewed by agent",
        State::Flagged => "flagged",
        State::AgentFlagged => "flagged by agent",
    }
}

fn pretty(v: serde_json::Value) -> Response {
    serde_json::to_string_pretty(&v).map_err(|e| e.to_string())
}

impl Review {
    /// Answers `req`. `None` for the store when it couldn't be opened.
    pub fn answer(&mut self, req: Request, store: Option<&mut Store>) -> Response {
        match req {
            Request::GetReview => self.get_review(),
            Request::GetComments => self.get_comments(),
            Request::ResolveComment(r) => {
                let store = store.ok_or("lgtm couldn't open its store")?;
                let found = store
                    .set_agent_comment_done(r.number, true)
                    .map_err(|e| e.to_string())?;
                if !found {
                    return Err(format!("there's no comment C{}", r.number));
                }
                self.load_comments(store);
                Ok(format!("C{} is done.", r.number))
            }
            Request::CheckFiles(c) => self.check_files(c, store),
            Request::MarkLines(m) => self.mark_lines(m, store),
        }
    }

    fn get_review(&self) -> Response {
        let marks = self.marks.as_ref().ok_or("the review is still loading")?;
        let files: Vec<_> = (0..marks.file_count())
            .map(|i| {
                let p = marks.progress(i);
                let mut file = json!({
                    "path": marks.path(i),
                    "state": state_name(marks.state(i, None)),
                    "done": p.done,
                    "total": p.total,
                });
                if !p.left.is_empty() {
                    file["linesLeft"] = json!(agent::ranges(p.left));
                }
                if !p.removed_left.is_empty() {
                    file["removedLinesLeft"] = json!(agent::ranges(p.removed_left));
                }
                file
            })
            .collect();
        let commits: Vec<_> = self
            .changes
            .iter()
            .map(|c| json!({"changeId": c.change_id, "description": c.description}))
            .collect();
        pretty(json!({
            "revset": self.revset,
            "commits": commits,
            "percentDone": marks.percent(),
            "files": files,
        }))
    }

    fn get_comments(&self) -> Response {
        if self.comments.is_empty() && self.threads.is_empty() {
            return Ok("No open comments.".into());
        }
        let mut comments: Vec<_> = self
            .comments
            .iter()
            .map(|(c, at)| {
                let line = at.and_then(|(f, h, r)| {
                    let row = &self.files[f].hunks[h].rows[r];
                    match c.side {
                        Side::New => row.new_line,
                        Side::Old => row.old_line,
                    }
                });
                json!({
                    "number": format!("C{}", c.number),
                    "path": c.path,
                    "side": if c.side == Side::Old { "old" } else { "new" },
                    "line": line,
                    "code": c.line_text,
                    "body": c.body,
                })
            })
            .collect();
        comments.extend(self.threads.iter().map(|t| {
            let line = t.at.and_then(|(h, r)| {
                let row = &self.files[t.file].hunks[h].rows[r];
                row.new_line.or(row.old_line)
            });
            let posts: Vec<_> = t
                .comment
                .github
                .iter()
                .flat_map(|g| &g.posts)
                .map(|p| json!({"author": p.author, "body": p.body}))
                .collect();
            json!({
                "number": format!("#{}", t.comment.id),
                "path": t.comment.path,
                "line": if t.outdated() { None } else { line },
                "outdated": t.outdated(),
                "url": t.comment.github.as_ref().map(|g| g.url.clone()),
                "posts": posts,
            })
        }));
        pretty(json!(comments))
    }

    fn same_review(&self, revset: &str) -> Result<(), String> {
        if revset == self.revset {
            Ok(())
        } else {
            Err(format!(
                "lgtm now shows `{}`, not `{revset}`: call get_review again",
                self.revset
            ))
        }
    }

    fn check_files(&mut self, c: CheckFiles, store: Option<&mut Store>) -> Response {
        self.same_review(&c.revset)?;
        let store = store.ok_or("lgtm couldn't open its store")?;
        let marks = self.marks.as_mut().ok_or("the review is still loading")?;
        let files = c
            .paths
            .iter()
            .map(|p| {
                marks
                    .file_index(p)
                    .map(|i| i as u32)
                    .ok_or_else(|| format!("`{p}` isn't in the review"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        marks
            .agent_check(store, &files, c.checked)
            .map_err(|e| format!("{e:#}"))?;
        let verb = if c.checked { "Checked" } else { "Unchecked" };
        Ok(format!("{verb} {} files.", files.len()))
    }

    fn mark_lines(&mut self, m: MarkLines, store: Option<&mut Store>) -> Response {
        self.same_review(&m.revset)?;
        let store = store.ok_or("lgtm couldn't open its store")?;
        let marks = self.marks.as_mut().ok_or("the review is still loading")?;
        let file = marks
            .file_index(&m.path)
            .ok_or_else(|| format!("`{}` isn't in the review", m.path))?;
        let lines = agent::parse_lines(&m.lines)?;
        let removed = agent::parse_lines(&m.removed_lines)?;
        let color = m.mark.map(|k| match k {
            AgentMark::Reviewed => MarkColor::AgentReviewed,
            AgentMark::Flagged => MarkColor::AgentFlagged,
        });
        let n = marks
            .agent_mark(store, file, &lines, &removed, color)
            .map_err(|e| format!("{e:#}"))?;
        if n == 0 {
            return Err("none of those lines changed in the review".into());
        }
        let verb = if color.is_some() {
            "Marked"
        } else {
            "Cleared your marks on"
        };
        let lines = if n == 1 { "line" } else { "lines" };
        Ok(format!("{verb} {n} changed {lines}."))
    }
}
