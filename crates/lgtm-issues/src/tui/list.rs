//! The main screen: waiting questions on top, then every other issue one line each.

use lgtm_tui_kit::paint::Canvas;
use lgtm_tui_kit::theme::{self, ACCENT, AMBER, BG, CYAN, FAINT, FG, GREEN, HAIR, MUTED, SEL};
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::issue::{Issue, Status};
use crate::time::{self, Minute};

#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Question { id: u32, qid: String },
    Issue(u32),
    Rule,
    Done(usize),
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
    let mut rows: Vec<Row> = questions(issues, filter)
        .into_iter()
        .map(|(i, qid)| Row::Question { id: i.id, qid })
        .collect();
    if !rows.is_empty() {
        rows.push(Row::Rule);
    }
    let shown: Vec<&Issue> = issues
        .iter()
        .filter(|i| matches(i, filter) && i.waiting().is_empty())
        .collect();
    let mut working: Vec<&&Issue> = shown
        .iter()
        .filter(|i| matches!(i.status, Status::Working | Status::NeedsInput))
        .collect();
    working.sort_by_key(|i| std::cmp::Reverse(i.touched()));
    rows.extend(working.iter().map(|i| Row::Issue(i.id)));
    rows.extend(
        shown
            .iter()
            .filter(|i| i.status == Status::Open)
            .map(|i| Row::Issue(i.id)),
    );
    let done = shown.iter().filter(|i| i.status == Status::Done).count();
    if done > 0 {
        rows.push(Row::Done(done));
    }
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
            Row::Rule => {
                c.put(
                    area.x + 1,
                    y,
                    &"─".repeat(area.width.saturating_sub(2) as usize),
                    st(HAIR),
                );
            }
            Row::Done(n) => {
                let x = c.put(area.x + 1, y, "✓", st(GREEN));
                c.put(x + 1, y, &format!("{n} done"), st(MUTED));
            }
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
    fn questions_lead_oldest_first_then_working_newest_first_then_open() {
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
        ];
        let q = |qid: &str| Row::Question {
            id: 1,
            qid: qid.into(),
        };
        assert_eq!(
            rows(&issues, ""),
            [
                q("Q1"),
                q("Q2"),
                Row::Rule,
                Row::Issue(4),
                Row::Issue(3),
                Row::Issue(2),
                Row::Done(1)
            ]
        );
        assert_eq!(rows(&issues, "issue 3"), [Row::Issue(3)]);
    }
}
