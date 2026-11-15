//! An opened issue: its file rendered as markdown, under a two-line header.

use lgtm_tui_kit::flow::{self, Piece};
use lgtm_tui_kit::markdown::{Outcome, Preview};
use lgtm_tui_kit::paint::Canvas;
use lgtm_tui_kit::theme::{self, CYAN, FAINT, FG, MUTED, RED, UNIQUE};
use ratatui::crossterm::event::{KeyEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::{marks, qq};
use crate::fields::Kind;
use crate::issue::{Issue, Kind as LogKind};
use crate::time;
use crate::workflow::Workflow;

pub struct View {
    pub id: u32,
    preview: Preview,
    text: String,
    version: u64,
}

impl View {
    pub fn new(issue: &Issue) -> View {
        View {
            id: issue.id,
            preview: Preview::new(0),
            text: markdown(issue),
            version: 0,
        }
    }

    /// Takes in the issue as it is now on disk.
    pub fn refresh(&mut self, issue: &Issue) {
        let text = markdown(issue);
        if text != self.text {
            self.text = text;
            self.version += 1;
        }
    }

    pub fn mouse(&mut self, kind: MouseEventKind, x: u16, y: u16) {
        self.preview.mouse(kind, x, y);
    }

    pub fn selection(&self) -> Option<String> {
        Some(self.preview.selection()).filter(|t| !t.is_empty())
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        self.preview.key(key)
    }

    pub fn draw(&mut self, c: &mut Canvas, area: Rect, about: (&Issue, &Workflow, &[Issue])) {
        let head = header(c, area.x, area.y, about).min(area.height);
        let body = Rect {
            y: area.y + head,
            height: area.height - head,
            ..area
        };
        self.preview.draw(c, body, &self.text, self.version);
    }
}

/// Text cut to `w` columns, ending in `…` when it doesn't fit.
fn fit(s: &str, w: usize) -> String {
    if s.chars().count() <= w {
        return s.to_owned();
    }
    let mut t: String = s.chars().take(w.saturating_sub(1)).collect();
    t.push('…');
    t
}

/// `● 12 Title`, the title cut to the width, over the status, its open blockers, the
/// workflow's fields, the owner and who filed it, wrapping onto more rows when they don't
/// fit. Returns the rows it takes, with the blank row under it.
pub fn header(
    c: &mut Canvas,
    x: u16,
    y: u16,
    (issue, wf, all): (&Issue, &Workflow, &[Issue]),
) -> u16 {
    let (dot, color) = marks::mark(issue, wf, all);
    let right = c.area.right().saturating_sub(1);
    let x = c.put(x + 1, y, &dot, theme::fg(color));
    let x = c.put(x + 1, y, &format!("{} ", issue.id), theme::fg(MUTED));
    let room = right.saturating_sub(x) as usize;
    c.put(x, y, &fit(&issue.title, room), theme::bold(FG));
    let one = |t: String, col| Piece::new(2, t, theme::fg(col));
    let two = |a: &str, t: String, col| {
        Piece::new(2, a, theme::fg(FAINT)).then(format!("  {t}"), theme::fg(col))
    };
    let mut parts = Vec::new();
    match wf.status(&issue.status) {
        Some(s) => parts.push(one(s.label.clone(), marks::color(&s.color))),
        None => parts.push(one(issue.status.clone(), MUTED)),
    }
    let blockers = issue.open_blockers(all, |s| wf.is_done(s));
    if !blockers.is_empty() {
        let ids: Vec<String> = blockers.iter().map(|b| format!("#{b}")).collect();
        parts.push(two("blocked by", ids.join(" "), RED));
    }
    for f in &wf.fields {
        let Some(value) = issue.field(&f.name) else {
            continue;
        };
        match &f.kind {
            Kind::BlockedBy => {}
            Kind::Revision => parts.push(one(value, UNIQUE)),
            Kind::DuplicateOf => parts.push(two("duplicate of", format!("#{value}"), FG)),
            Kind::Choice(_) => {
                let color = f.color(&value).map_or(FG, marks::color);
                parts.push(one(value, color));
            }
            Kind::Text => parts.push(two(&f.name, value, FG)),
        }
    }
    if let Some(o) = &issue.owner {
        parts.push(one(o.clone(), CYAN));
    }
    if let Some(f) = &issue.filed_by {
        parts.push(two("filed by", f.clone(), CYAN));
    }
    flow::draw(c, (x, y + 1), right.saturating_sub(x), &parts, &[]) + 2
}

/// The issue as markdown to read: the description, then each log entry under a heading of
/// who, what and when, with picks spelled out as the option they chose.
pub fn markdown(issue: &Issue) -> String {
    let now = time::now();
    let mut out = String::new();
    if !issue.body.is_empty() {
        out += &format!("{}\n\n", issue.body);
    }
    for e in &issue.log {
        let age = time::age(e.at, now);
        let what = match &e.kind {
            LogKind::Note => String::new(),
            LogKind::Question { id, .. } => format!(" · question {id}"),
            LogKind::Answer { id } => format!(" · answer {id}"),
            LogKind::Reply { id } => format!(" · reply {id}"),
        };
        out += &format!("#### {}{what} · {age}\n\n", e.who);
        match &e.kind {
            LogKind::Answer { id } => {
                let (n, notes) = e.text.split_once('\n').unwrap_or((&e.text, ""));
                let name = asked(issue, id)
                    .and_then(|q| {
                        let n: usize = n.trim().parse().ok()?;
                        qq::options(&q.text).into_iter().find(|(k, _)| *k == n)
                    })
                    .map(|(_, name)| format!(" · {name}"))
                    .unwrap_or_default();
                out += &format!("**Picked {}**{name}\n\n", n.trim());
                if !notes.trim().is_empty() {
                    out += &format!("{}\n\n", notes.trim());
                }
            }
            LogKind::Question { .. } => out += &format!("{}\n\n", hard_breaks(&e.text)),
            _ => out += &format!("{}\n\n", e.text),
        }
    }
    out
}

/// `text` with its line breaks kept, as a qq question's lines are each their own.
fn hard_breaks(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        out += line;
        let next = lines.get(i + 1).map_or("", |l| l.trim());
        let item = next.starts_with("- ")
            || next
                .split_once(". ")
                .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
        if !line.trim().is_empty() && !next.is_empty() && !item {
            out += "\\";
        }
        out += "\n";
    }
    out
}

/// The question `qid` as asked last.
fn asked<'a>(issue: &'a Issue, qid: &str) -> Option<&'a crate::issue::Entry> {
    issue.log.iter().rev().find(|e| e.qid() == Some(qid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_read_as_the_option_they_chose() {
        let mut issue = Issue::new(1, "t", None, "Body.", 0);
        let q = LogKind::Question {
            id: "Q1".into(),
            after: None,
        };
        issue.push(
            "a",
            q,
            "Q1 — Where\n\nOptions:\n\n1. /login (recommended)\n2. /",
            0,
        );
        issue.push(
            "jordan",
            LogKind::Answer { id: "Q1".into() },
            "1\nkeep it short",
            0,
        );
        issue.push("jordan", LogKind::Reply { id: "Q1".into() }, "Neither", 0);
        let md = markdown(&issue);
        assert!(md.starts_with("Body.\n\n#### a · question Q1"), "{md}");
        assert!(
            md.contains("**Picked 1** · /login\n\nkeep it short"),
            "{md}"
        );
        assert!(md.contains("#### jordan · reply Q1 · "), "{md}");
        assert!(
            md.contains("Q1 — Where\n\nOptions:\n\n1. /login (recommended)\n2. /"),
            "{md}"
        );
    }
}
