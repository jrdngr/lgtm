//! Talking a question over with the agent. A reply can end with tags that act on the
//! question: `<rewrite>` when the talk changes it, `<decide>` when the user settles it.

use anyhow::Context;
use serde::Deserialize;

use super::Session;
use super::agent::{Step, parse, rewrite};
use super::queue::{self, Answer, QState, QaQuestion, Rewrite, Turn};
use crate::agent::{Progress, Runs};
use crate::review::{RepoCtx, now_ms};
use crate::workspace::Workspace;

/// How a discussion reply acts on question `qid`.
fn rules(qid: &str) -> String {
    format!(
        "If this changes {qid}'s context, options or recommendation, end your reply with the \
         rewritten question by the same rules, as JSON with the fields id, what (what changed, \
         in a few words), subject, context, options and why, between <rewrite> and \
         </rewrite>. If I pick an option or otherwise settle {qid}, end your reply with \
         {{\"option\": the option's number from 1, or null for an answer outside them, \
         \"note\": what I added to it or null, \"other\": my answer in my words when option is \
         null, else null}} between <decide> and </decide>. Don't ask me new questions here: \
         after I decide, you update the open questions and add new ones there."
    )
}

/// The question's discussion, a paragraph a turn.
fn talk(q: &QaQuestion) -> String {
    q.discussion
        .iter()
        .map(|t| format!("{}: {}", if t.you { "Me" } else { "You" }, t.text))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// A decision made in a discussion. `option` counts from 1.
#[derive(Debug, PartialEq, Eq, Deserialize)]
struct Decision {
    option: Option<u32>,
    #[serde(default)]
    note: Option<String>,
    #[serde(default)]
    other: Option<String>,
}

/// What a discussion reply says and does.
#[derive(Debug, PartialEq, Eq)]
struct Parsed<'a> {
    text: &'a str,
    rewrite: Option<Rewrite>,
    decide: Option<Decision>,
}

/// The JSON between `<tag>` and `</tag>`. One that isn't valid is reported and skipped.
fn tagged<T: serde::de::DeserializeOwned>(reply: &str, tag: &str) -> Option<T> {
    let (_, rest) = reply.split_once(&format!("<{tag}>"))?;
    let json = rest.split(&format!("</{tag}>")).next()?.trim();
    let json = json
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```");
    serde_json::from_str(json)
        .inspect_err(|e| eprintln!("lgtm: discussion {tag}: {e}"))
        .ok()
}

/// The reply's text is everything before its first tag.
fn split(reply: &str) -> Parsed<'_> {
    let end = ["<rewrite>", "<decide>"]
        .iter()
        .filter_map(|t| reply.find(t))
        .min()
        .unwrap_or(reply.len());
    Parsed {
        text: reply[..end].trim(),
        rewrite: tagged(reply, "rewrite"),
        decide: tagged(reply, "decide"),
    }
}

impl RepoCtx {
    /// A discussion turn on question `qid`, which can rewrite or decide it. Streams the reply.
    pub async fn discuss_question(
        &self,
        ws: &Workspace,
        id: &str,
        qid: &str,
        message: &str,
        runs: &Runs,
        on_text: impl Fn(String),
    ) -> anyhow::Result<Session> {
        let s = self.qa_session(ws, id)?;
        let question = s
            .queue
            .get(qid)
            .with_context(|| format!("no question {qid}"))?;
        let so_far = if question.discussion.is_empty() {
            String::new()
        } else {
            format!("Our discussion so far:\n\n{}\n\n", talk(question))
        };
        let opening = format!(
            "Let's discuss this question. It's a discussion: answer briefly. {}\n\n{}\n\n\
             {so_far}{message}",
            rules(qid),
            queue::question_text(question)
        );
        let (turn, restart) = if question.discussion.is_empty() {
            (opening, None)
        } else {
            (
                format!("About {qid}: {message}\n\n({})", rules(qid)),
                Some(opening),
            )
        };
        let reply = self
            .qa_step(
                ws,
                id,
                runs,
                Step {
                    turn,
                    edit: false,
                    schema: None,
                    fresh: false,
                    restart,
                    decided: false,
                },
                |p| {
                    if let Progress::Text(t) = p {
                        on_text(t)
                    }
                },
            )
            .await?;
        if reply.stopped {
            return self.qa_session(ws, id);
        }
        let parsed = split(&reply.text);
        self.edit_qa(ws, id, |s| {
            s.queue.discuss(
                qid,
                Turn {
                    you: true,
                    text: message.to_string(),
                },
            )?;
            if !parsed.text.is_empty() {
                s.queue.discuss(
                    qid,
                    Turn {
                        you: false,
                        text: parsed.text.to_string(),
                    },
                )?;
            }
            if let Some(mut r) = parsed.rewrite {
                r.id = qid.to_string();
                s.queue.reask(r)?;
            }
            if let Some(d) = parsed.decide
                && (d.option.is_some_and(|o| o >= 1)
                    || d.other.as_deref().is_some_and(|o| !o.trim().is_empty()))
                && s.queue.get(qid).is_some_and(|q| q.state == QState::Open)
            {
                s.queue
                    .decide(
                        qid,
                        Answer {
                            option: d.option.and_then(|o| o.checked_sub(1)),
                            note: d.note.filter(|n| !n.trim().is_empty()),
                            other: d.other.filter(|o| !o.trim().is_empty()),
                            at: now_ms() as f64,
                        },
                    )
                    .inspect_err(|e| eprintln!("lgtm: discussion decide: {e:#}"))
                    .ok();
            }
            Ok(())
        })?;
        self.qa_session(ws, id)
    }

    /// Rewrites question `qid` with what came out of its discussion.
    pub async fn reask_question(
        &self,
        ws: &Workspace,
        id: &str,
        qid: &str,
        runs: &Runs,
    ) -> anyhow::Result<Session> {
        let q = self.qa_session(ws, id)?.queue;
        let question = q.get(qid).with_context(|| format!("no question {qid}"))?;
        let turn = format!(
            "Rewrite {qid} with what came out of our discussion, by the same rules, keeping its \
             id. Say in a few words what changed.\n\n{}\n\nOur discussion:\n\n{}",
            queue::question_text(question),
            talk(question)
        );
        let reply = self
            .qa_step(
                ws,
                id,
                runs,
                Step {
                    turn,
                    edit: false,
                    schema: Some(rewrite()),
                    fresh: false,
                    restart: None,
                    decided: false,
                },
                |_| {},
            )
            .await?;
        if !reply.stopped {
            let mut r: Rewrite = parse(&reply, "re-ask")?;
            r.id = qid.to_string();
            self.edit_qa(ws, id, |s| s.queue.reask(r))?;
        }
        self.qa_session(ws, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_splits_into_text_rewrite_and_decision() {
        assert_eq!(split(" Sure. ").text, "Sure.");
        let reply = "Adding C and going with it.\n\n<rewrite>\n```json\n{\"id\":\"Q2\",\
                     \"what\":\"added C\",\"subject\":\"s\",\"context\":[],\"options\":[{\"label\":\
                     \"C\",\"means\":\"m\",\"pros\":[],\"cons\":[],\"risks\":[]}],\"why\":\"w\"}\n```\n</rewrite>\n<decide>{\"option\":1,\
                     \"note\":\"keep it small\",\"other\":null}</decide>";
        let p = split(reply);
        assert_eq!(p.text, "Adding C and going with it.");
        assert_eq!(p.rewrite.unwrap().what, "added C");
        assert_eq!(
            p.decide,
            Some(Decision {
                option: Some(1),
                note: Some("keep it small".into()),
                other: None
            })
        );
        let p = split("Hm.\n<decide>{not json</decide>");
        assert_eq!((p.text, p.rewrite, p.decide), ("Hm.", None, None));
    }
}
