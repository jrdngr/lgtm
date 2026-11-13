//! The main screen: issues grouped by status under a header each. Needs input lists its
//! waiting questions first, then working and open issues one line each; done and dropped
//! fold to a count.

use lgtm_tui_kit::paint::Canvas;
use lgtm_tui_kit::theme::{self, ACCENT, AMBER, BG, CYAN, FAINT, FG, GREEN, HAIR, MUTED, SEL};
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::issue::{Issue, Status};
use crate::time::{self, Minute};

#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Question {
        id: u32,
        qid: String,
    },
    Issue(u32),
    /// A status's header and how many rows it has.
    Header(Status, usize),
    /// Done or dropped issues, folded to their count.
    Folded(Status, usize),
    Gap,
}

impl Row {
    pub fn pickable(&self) -> bool {
        matches!(self, Row::Question { .. } | Row::Issue(_))
    }

    pub fn issue(&self) -> Option<u32> {
        match self {
            Row::Question { id, .. } | Row::Issue(id) => Some(*id),
            _ => None,
        }
    }
}

fn matches(issue: &Issue, filter: &str) -> bool {
    filter.is_empty() || issue.title.to_lowercase().contains(&filter.to_lowercase())
}

/// Waiting questions, oldest first, as (issue, question id).
pub fn questions<'a>(issues: &'a [Issue], filter: &str) -> Vec<(&'a Issue, String)> {
    let mut out: Vec<(&Issue, String, Minute)> = issues
        .iter()
        .filter(|i| matches(i, filter))
        .flat_map(|i| {
            i.waiting()
                .into_iter()
                .filter_map(move |q| Some((i, q.qid()?.to_owned(), q.at)))
        })
        .collect();
    out.sort_by_key(|(i, _, at)| (*at, i.id));
    out.into_iter().map(|(i, q, _)| (i, q)).collect()
}

pub fn rows(issues: &[Issue], filter: &str) -> Vec<Row> {
    let shown: Vec<&Issue> = issues.iter().filter(|i| matches(i, filter)).collect();
    // Issues with a status and no waiting question, newest first or, when open, by id.
    let quiet = |status: Status| {
        let mut out: Vec<&&Issue> = shown
            .iter()
            .filter(|i| i.status == status && i.waiting().is_empty())
            .collect();
        if status != Status::Open {
            out.sort_by_key(|i| std::cmp::Reverse(i.touched()));
        }
        out.into_iter()
            .map(|i| Row::Issue(i.id))
            .collect::<Vec<_>>()
    };
    let mut needs: Vec<Row> = questions(issues, filter)
        .into_iter()
        .map(|(i, qid)| Row::Question { id: i.id, qid })
        .collect();
    needs.extend(quiet(Status::NeedsInput));
    let mut rows = Vec::new();
    for (status, group) in [
        (Status::NeedsInput, needs),
        (Status::Working, quiet(Status::Working)),
        (Status::Open, quiet(Status::Open)),
    ] {
        if group.is_empty() {
            continue;
        }
        if !rows.is_empty() {
            rows.push(Row::Gap);
        }
        rows.push(Row::Header(status, group.len()));
        rows.extend(group);
    }
    let mut folded = [Status::Done, Status::Dropped]
        .into_iter()
        .map(|s| (s, shown.iter().filter(|i| i.status == s).count()))
        .filter(|(_, n)| *n > 0)
        .peekable();
    if folded.peek().is_some() && !rows.is_empty() {
        rows.push(Row::Gap);
    }
    rows.extend(folded.map(|(s, n)| Row::Folded(s, n)));
    rows
}

fn cut(s: &str, w: usize) -> String {
    if s.chars().count() <= w {
        format!("{s:<w$}")
    } else {
        let mut t: String = s.chars().take(w.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or_default()
}

pub fn draw(c: &mut Canvas, area: Rect, issues: &[Issue], rows: &[Row], sel: usize, top: usize) {
    let now = time::now();
    let find = |id: u32| issues.iter().find(|i| i.id == id);
    for (n, row) in rows.iter().enumerate().skip(top) {
        let y = area.y + (n - top) as u16;
        if y >= area.bottom() {
            break;
        }
        let bg = if n == sel { SEL } else { BG };
        c.fill(area.x, y, area.width, bg);
        let st = |color| Style::default().fg(color).bg(bg);
        let bold = |color| theme::bold(color).bg(bg);
        let right =
            |c: &mut Canvas, age: String| c.put_right(y, 1, &format!("{age:>4}"), st(FAINT));
        match row {
            Row::Question { id, qid } => {
                let Some(issue) = find(*id) else { continue };
                let qs = issue.questions();
                let Some(at) = qs.iter().position(|(q, _)| q.qid() == Some(qid)) else {
                    continue;
                };
                let q = qs[at].0;
                let mut x = c.put(area.x + 1, y, "●", st(AMBER));
                x = c.put(x + 1, y, &format!("{id:<3}"), st(FG));
                x = c.put(x + 1, y, &format!("{qid:<4}"), bold(AMBER));
                x = c.put(x, y, &cut(q.subject(), 44), st(FG));
                x = c.put(
                    x + 1,
                    y,
                    &format!("{:<9}", format!("{} of {}", at + 1, qs.len())),
                    st(FAINT),
                );
                c.put(x, y, &cut(&q.who, 11), st(CYAN));
                right(c, time::age(q.at, now));
            }
            Row::Issue(id) => {
                let Some(issue) = find(*id) else { continue };
                let open = issue.status == Status::Open;
                let (dot, color) = if open {
                    ("○", MUTED)
                } else {
                    ("●", ACCENT)
                };
                let mut x = c.put(area.x + 1, y, dot, st(color));
                x = c.put(x + 1, y, &format!("{id:<3}"), st(FG));
                x = c.put(x + 1, y, &cut(&issue.title, 36), st(FG));
                if !open {
                    x = c.put(
                        x + 1,
                        y,
                        &cut(issue.owner.as_deref().unwrap_or(""), 11),
                        st(CYAN),
                    );
                    let latest = issue.latest().map_or("", |e| first_line(&e.text));
                    let room = (area.right().saturating_sub(x + 7)) as usize;
                    c.put(x, y, cut(latest, room).trim_end(), st(MUTED));
                    right(c, time::age(issue.touched(), now));
                }
            }
            Row::Header(status, n) => {
                let color = match status {
                    Status::NeedsInput => AMBER,
                    Status::Open => MUTED,
                    _ => ACCENT,
                };
                let mut x = c.put(area.x + 1, y, &status.name().replace('-', " "), bold(color));
                x = c.put(x + 2, y, &n.to_string(), st(FAINT));
                let rule = area.right().saturating_sub(x + 2) as usize;
                c.put(x + 1, y, &"─".repeat(rule), st(HAIR));
            }
            Row::Folded(status, n) => {
                let (glyph, color) = match status {
                    Status::Done => ("✓", GREEN),
                    _ => ("×", FAINT),
                };
                let mut x = c.put(area.x + 1, y, glyph, st(color));
                x = c.put(x + 1, y, status.name(), st(MUTED));
                c.put(x + 2, y, &n.to_string(), st(FAINT));
            }
            Row::Gap => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issue::Kind;

    fn issue(id: u32, status: Status, touched: Minute) -> Issue {
        let mut i = Issue::new(id, &format!("Issue {id}"), None, "", 0);
        i.status = status;
        i.push("a", Kind::Note, "x", touched);
        i
    }

    fn ask(i: &mut Issue, qid: &str, at: Minute) {
        let q = Kind::Question {
            id: qid.into(),
            after: None,
        };
        i.push("a", q, "q", at);
        i.status = Status::NeedsInput;
    }

    #[test]
    fn issues_group_by_status_with_questions_oldest_first_and_working_newest_first() {
        let mut asking = issue(1, Status::Working, 5);
        ask(&mut asking, "Q2", 9);
        ask(&mut asking, "Q1", 7);
        let issues = vec![
            asking,
            issue(2, Status::Open, 1),
            issue(3, Status::Working, 3),
            issue(4, Status::Working, 8),
            issue(5, Status::Done, 2),
            issue(6, Status::Dropped, 2),
            issue(7, Status::NeedsInput, 4),
            issue(8, Status::Done, 2),
        ];
        let q = |qid: &str| Row::Question {
            id: 1,
            qid: qid.into(),
        };
        assert_eq!(
            rows(&issues, ""),
            [
                Row::Header(Status::NeedsInput, 3),
                q("Q1"),
                q("Q2"),
                Row::Issue(7),
                Row::Gap,
                Row::Header(Status::Working, 2),
                Row::Issue(4),
                Row::Issue(3),
                Row::Gap,
                Row::Header(Status::Open, 1),
                Row::Issue(2),
                Row::Gap,
                Row::Folded(Status::Done, 2),
                Row::Folded(Status::Dropped, 1),
            ]
        );
        assert_eq!(
            rows(&issues, "issue 3"),
            [Row::Header(Status::Working, 1), Row::Issue(3)]
        );
    }
}
