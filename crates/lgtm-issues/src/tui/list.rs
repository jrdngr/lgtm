//! The main screen: issues grouped by status under a header each. Needs input lists waiting
//! questions first, then each status in the workflow's order, one line per issue; statuses
//! that fold show a count, and list their issues once expanded.

use std::cmp::Reverse;

use lgtm_tui_kit::paint::Canvas;
use lgtm_tui_kit::theme::{self, AMBER, BG, CYAN, FAINT, FG, HAIR, MUTED, RED, SEL};
use ratatui::layout::Rect;
use ratatui::style::Style;

use super::marks;
use crate::issue::Issue;
use crate::time::{self, Minute};
use crate::workflow::{Sort, Status, Verb, Workflow};

#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Question {
        id: u32,
        qid: String,
    },
    Issue(u32),
    /// The needs input header and how many questions it has.
    Needs(usize),
    /// A status's header, by id, and how many issues it has.
    Header(String, usize),
    /// A folding status: its count, and whether its issues are listed below it.
    Folded(String, usize, bool),
    Gap,
}

impl Row {
    pub fn pickable(&self) -> bool {
        matches!(self, Row::Question { .. } | Row::Issue(_) | Row::Folded(..))
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

/// The issues in `status` with no waiting question, in the status's order.
fn in_status<'a>(shown: &[&'a Issue], status: &Status, wf: &Workflow) -> Vec<&'a Issue> {
    let mut out: Vec<&Issue> = shown
        .iter()
        .copied()
        .filter(|i| i.status == status.id && i.waiting().is_empty())
        .collect();
    match &status.sort {
        Sort::Id => out.sort_by_key(|i| i.id),
        Sort::Touched => out.sort_by_key(|i| Reverse(i.touched())),
        Sort::Field(f) => {
            let rank = |i: &Issue| {
                let field = wf.field(f)?;
                field.rank(&i.field(f)?)
            };
            out.sort_by_key(|i| (Reverse(rank(i)), Reverse(i.touched())))
        }
    }
    out
}

pub fn rows(issues: &[Issue], wf: &Workflow, filter: &str, expanded: &[String]) -> Vec<Row> {
    let shown: Vec<&Issue> = issues.iter().filter(|i| matches(i, filter)).collect();
    let mut rows = Vec::new();
    let needs: Vec<Row> = questions(issues, filter)
        .into_iter()
        .map(|(i, qid)| Row::Question { id: i.id, qid })
        .collect();
    if !needs.is_empty() {
        rows.push(Row::Needs(needs.len()));
        rows.extend(needs);
    }
    for status in wf.statuses.iter().filter(|s| !s.fold) {
        let group = in_status(&shown, status, wf);
        if group.is_empty() {
            continue;
        }
        if !rows.is_empty() {
            rows.push(Row::Gap);
        }
        rows.push(Row::Header(status.id.clone(), group.len()));
        rows.extend(group.iter().map(|i| Row::Issue(i.id)));
    }
    let mut gap = !rows.is_empty();
    for status in wf.statuses.iter().filter(|s| s.fold) {
        let group = in_status(&shown, status, wf);
        if group.is_empty() {
            continue;
        }
        if std::mem::take(&mut gap) {
            rows.push(Row::Gap);
        }
        let open = expanded.contains(&status.id);
        rows.push(Row::Folded(status.id.clone(), group.len(), open));
        if open {
            rows.extend(group.iter().map(|i| Row::Issue(i.id)));
        }
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

pub fn draw(
    c: &mut Canvas,
    area: Rect,
    (issues, wf): (&[Issue], &Workflow),
    rows: &[Row],
    sel: usize,
    top: usize,
) {
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
                let open = wf.verb(Verb::New).is_ok_and(|s| s.id == issue.status);
                let (dot, color) = marks::mark(issue, wf, issues);
                let mut x = c.put(area.x + 1, y, &dot, st(color));
                x = c.put(x + 1, y, &format!("{id:<3}"), st(FG));
                x = c.put(x + 1, y, &cut(&issue.title, 36), st(FG));
                let blockers = issue.open_blockers(issues, |s| wf.is_done(s));
                if !blockers.is_empty() {
                    let ids: Vec<String> = blockers.iter().map(|b| format!("#{b}")).collect();
                    x = c.put(x + 1, y, &ids.join(" "), st(RED));
                }
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
            Row::Needs(n) => header(c, area, y, ("needs input", AMBER), *n, bg),
            Row::Header(id, n) => {
                if let Some(s) = wf.status(id) {
                    header(c, area, y, (&s.label, marks::color(&s.color)), *n, bg);
                }
            }
            Row::Folded(id, n, open) => {
                let Some(s) = wf.status(id) else { continue };
                let mut x = c.put(area.x + 1, y, &s.glyph, st(marks::color(&s.color)));
                x = c.put(x + 1, y, &s.label, st(MUTED));
                x = c.put(x + 2, y, &n.to_string(), st(FAINT));
                c.put(x + 1, y, if *open { "▾" } else { "▸" }, st(FAINT));
            }
            Row::Gap => {}
        }
    }
}

/// A group's header: its label in bold, its count, and a rule to the edge.
fn header(
    c: &mut Canvas,
    area: Rect,
    y: u16,
    (label, color): (&str, ratatui::style::Color),
    n: usize,
    bg: ratatui::style::Color,
) {
    let mut x = c.put(area.x + 1, y, label, theme::bold(color).bg(bg));
    x = c.put(x + 2, y, &n.to_string(), Style::default().fg(FAINT).bg(bg));
    let rule = area.right().saturating_sub(x + 2) as usize;
    c.put(
        x + 1,
        y,
        &"─".repeat(rule),
        Style::default().fg(HAIR).bg(bg),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issue::Kind;

    fn issue(id: u32, status: &str, touched: Minute) -> Issue {
        let mut i = Issue::new(id, &format!("Issue {id}"), None, "", 0);
        i.status = status.into();
        i.push("a", Kind::Note, "x", touched);
        i
    }

    fn ask(i: &mut Issue, qid: &str, at: Minute) {
        let q = Kind::Question {
            id: qid.into(),
            after: None,
        };
        i.push("a", q, "q", at);
    }

    fn workflow() -> (tempfile::TempDir, Workflow) {
        let dir = tempfile::tempdir().unwrap();
        crate::workflow::write_default(dir.path()).unwrap();
        let w = Workflow::load(dir.path()).unwrap();
        (dir, w)
    }

    #[test]
    fn issues_group_by_status_in_order_after_questions_and_done_ones_fold() {
        let (_dir, wf) = workflow();
        let mut asking = issue(1, "working", 5);
        ask(&mut asking, "Q2", 9);
        ask(&mut asking, "Q1", 7);
        let issues = vec![
            asking,
            issue(2, "open", 9),
            issue(3, "working", 3),
            issue(4, "working", 8),
            issue(5, "done", 2),
            issue(6, "dropped", 2),
            issue(7, "open", 1),
            issue(8, "done", 2),
        ];
        let q = |qid: &str| Row::Question {
            id: 1,
            qid: qid.into(),
        };
        let h = |s: &str, n| Row::Header(s.into(), n);
        let f = |s: &str, n, open| Row::Folded(s.into(), n, open);
        assert_eq!(
            rows(&issues, &wf, "", &[]),
            [
                Row::Needs(2),
                q("Q1"),
                q("Q2"),
                Row::Gap,
                h("open", 2),
                Row::Issue(2),
                Row::Issue(7),
                Row::Gap,
                h("working", 2),
                Row::Issue(4),
                Row::Issue(3),
                Row::Gap,
                f("done", 2, false),
                f("dropped", 1, false),
            ]
        );
        assert_eq!(
            rows(&issues, &wf, "issue", &["done".into()])[12..],
            [
                f("done", 2, true),
                Row::Issue(5),
                Row::Issue(8),
                f("dropped", 1, false)
            ]
        );
        assert_eq!(
            rows(&issues, &wf, "issue 3", &[]),
            [h("working", 1), Row::Issue(3)]
        );
    }

    #[test]
    fn a_status_can_sort_by_a_choice_field_highest_first() {
        let (dir, _) = workflow();
        std::fs::write(
            dir.path().join("open/STATUS.md"),
            "---\nsort: severity\n---\n",
        )
        .unwrap();
        let wf = Workflow::load(dir.path()).unwrap();
        let mut issues: Vec<Issue> = (1..=3).map(|id| issue(id, "open", id as Minute)).collect();
        issues[0].set_field("severity", Some("critical".into()));
        issues[2].set_field("severity", Some("low".into()));
        assert_eq!(
            rows(&issues, &wf, "", &[])[1..],
            [Row::Issue(1), Row::Issue(3), Row::Issue(2)]
        );
    }
}
