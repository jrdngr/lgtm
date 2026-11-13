//! An opened issue: its file rendered as markdown, the header drawn as one line above it.

use lgtm_tui_kit::markdown::{Outcome, Preview};
use lgtm_tui_kit::paint::Canvas;
use lgtm_tui_kit::theme::{self, ACCENT, AMBER, BG, CYAN, FAINT, FG, GREEN, MUTED, RED, UNIQUE};
use ratatui::crossterm::event::{KeyEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::qq;
use crate::fields::Severity;
use crate::issue::{Issue, Kind, Status};
use crate::time;

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

    pub fn draw(&mut self, c: &mut Canvas, area: Rect, issue: &Issue) {
        c.fill(area.x, area.y, area.width, BG);
        c.fill(area.x, area.y + 1, area.width, BG);
        header(c, area.x, area.y, issue);
        let body = Rect {
            y: area.y + HEADER,
            height: area.height.saturating_sub(HEADER),
            ..area
        };
        c.fill(area.x, area.y + 2, area.width, BG);
        self.preview.draw(c, body, &self.text, self.version);
    }
}

/// Rows the header takes, with the blank row under it.
pub const HEADER: u16 = 3;

/// Text cut to `w` columns, ending in `…` when it doesn't fit.
fn fit(s: &str, w: usize) -> String {
    if s.chars().count() <= w {
        return s.to_owned();
    }
    let mut t: String = s.chars().take(w.saturating_sub(1)).collect();
    t.push('…');
    t
}

/// `● 12 Title` over `status  severity  … owner  filed by x`, each cut to the width.
pub fn header(c: &mut Canvas, x: u16, y: u16, issue: &Issue) {
    let (dot, color) = status_mark(issue);
    let right = c.area.right().saturating_sub(1);
    let x = c.put(x + 1, y, dot, theme::fg(color));
    let x = c.put(x + 1, y, &format!("{} ", issue.id), theme::fg(MUTED));
    let room = right.saturating_sub(x) as usize;
    c.put(x, y, &fit(&issue.title, room), theme::bold(FG));
    let mut parts = vec![(issue.status.name().replace('-', " "), color)];
    if let Some(s) = issue.severity {
        parts.push((s.name().into(), severity_color(s)));
    }
    if let Some(d) = issue.duplicate_of {
        parts.push(("duplicate of".into(), FAINT));
        parts.push((format!("#{d}"), FG));
    }
    if !issue.blocked_by.is_empty() {
        let ids: Vec<String> = issue.blocked_by.iter().map(|b| format!("#{b}")).collect();
        parts.push(("blocked by".into(), FAINT));
        parts.push((ids.join(" "), FG));
    }
    if let Some(r) = &issue.revision {
        parts.push((r.clone(), UNIQUE));
    }
    if let Some(o) = &issue.owner {
        parts.push((o.clone(), CYAN));
    }
    if let Some(f) = &issue.filed_by {
        parts.push(("filed by".into(), FAINT));
        parts.push((f.clone(), CYAN));
    }
    let mut px = x;
    for (t, col) in parts {
        let room = right.saturating_sub(px) as usize;
        if room == 0 {
            break;
        }
        let shown = fit(&t, room);
        let cut = shown != t;
        px = c.put(px, y + 1, &shown, theme::fg(col)) + 2;
        if cut {
            break;
        }
    }
}

pub fn severity_color(s: Severity) -> ratatui::style::Color {
    match s {
        Severity::Low => MUTED,
        Severity::Medium => FG,
        Severity::High => AMBER,
        Severity::Critical => RED,
    }
}

pub fn status_mark(issue: &Issue) -> (&'static str, ratatui::style::Color) {
    match issue.status {
        Status::NeedsInput if !issue.waiting().is_empty() => ("●", AMBER),
        Status::Open => ("○", MUTED),
        Status::Done => ("✓", GREEN),
        Status::Dropped => ("×", FAINT),
        _ => ("●", ACCENT),
    }
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
            Kind::Note => String::new(),
            Kind::Question { id, .. } => format!(" · question {id}"),
            Kind::Answer { id } => format!(" · answer {id}"),
            Kind::Reply { id } => format!(" · reply {id}"),
        };
        out += &format!("#### {}{what} · {age}\n\n", e.who);
        match &e.kind {
            Kind::Answer { id } => {
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
            Kind::Question { .. } => out += &format!("{}\n\n", hard_breaks(&e.text)),
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
        let q = Kind::Question {
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
            Kind::Answer { id: "Q1".into() },
            "1\nkeep it short",
            0,
        );
        issue.push("jordan", Kind::Reply { id: "Q1".into() }, "Neither", 0);
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
