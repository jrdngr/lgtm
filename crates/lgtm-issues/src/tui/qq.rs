//! A question in the qq shape, styled: decisions, context, numbered options with their
//! pros, cons and risks, and the recommendation, with markdown inside the text: code spans,
//! emphasis, links, headings and fenced code.

use lgtm_tui_kit::markdown::{CODE, inline};
use lgtm_tui_kit::theme::{self, ACCENT, AMBER, BLANK, FAINT, FG, GREEN, MUTED, RED};
use ratatui::style::{Color, Style};

use super::wrap::{Runs, wrap};

/// A screen line, and the option it belongs to.
pub struct Line {
    pub runs: Runs,
    pub option: Option<usize>,
}

/// An option's number, from a `2. ` line at the left edge.
fn option_number(line: &str) -> Option<usize> {
    let (n, rest) = line.split_once(". ")?;
    (!n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) && !rest.is_empty())
        .then(|| n.parse().ok())
        .flatten()
}

/// The option each source line belongs to: from its numbered line through the indented
/// lines after it.
fn owners(lines: &[&str]) -> Vec<Option<usize>> {
    let mut out = vec![None; lines.len()];
    let mut in_options = false;
    let mut current = None;
    for (i, line) in lines.iter().enumerate() {
        if line.trim_end() == "Options:" {
            in_options = true;
            current = None;
            continue;
        }
        if line.trim().is_empty() {
            let next = lines[i + 1..].iter().find(|l| !l.trim().is_empty());
            if !next.is_some_and(|l| l.starts_with(' ')) {
                current = None;
            }
            out[i] = current;
            continue;
        }
        if !line.starts_with(' ') {
            current = in_options.then(|| option_number(line)).flatten();
            if current.is_none() {
                in_options = false;
            }
        }
        out[i] = current;
    }
    out
}

/// The options' names, by number, without `(recommended)`.
pub fn options(text: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let owner = owners(&lines);
    lines
        .iter()
        .zip(owner)
        .filter(|(l, o)| o.is_some() && !l.starts_with(' '))
        .filter_map(|(l, _)| {
            let n = option_number(l)?;
            let name = l.split_once(". ")?.1;
            Some((n, name.replace("(recommended)", "").trim().to_owned()))
        })
        .collect()
}

fn s(text: &str, c: Color) -> (String, Style) {
    (text.to_owned(), theme::fg(c))
}

fn b(text: &str, c: Color) -> (String, Style) {
    (text.to_owned(), theme::bold(c))
}

/// Text with its inline markdown, over `style`.
fn md(text: &str, style: Style) -> Runs {
    inline(text, style)
}

/// One source line's runs, and the indent its wrapped lines hang at.
fn style(line: &str, is_option: bool) -> (Runs, usize) {
    let indent = line.len() - line.trim_start().len();
    let pad = " ".repeat(indent);
    let t = line.trim_start();
    let mut runs = vec![s(&pad, FG)];
    let mut hang = indent;
    if t.is_empty() {
        return (Vec::new(), 0);
    } else if let Some(rest) = t.strip_prefix("✅ ") {
        let (id, rest) = rest.split_once(':').unwrap_or((rest, ""));
        runs.extend([s("✅ ", GREEN), s(&format!("{id}:"), GREEN)]);
        runs.extend(md(rest, theme::fg(FG)));
    } else if is_option && indent == 0 {
        let (n, rest) = t.split_once(". ").unwrap_or((t, ""));
        runs.push(b(&format!("{n}. "), ACCENT));
        hang = n.len() + 2;
        match rest.split_once("(recommended)") {
            Some((name, after)) => {
                runs.extend(md(name.trim_end(), theme::bold(FG)));
                runs.push(s(" (recommended)", GREEN));
                runs.extend(md(after, theme::fg(FG)));
            }
            None => runs.extend(md(rest, theme::bold(FG))),
        }
    } else if let Some(rest) = t.strip_prefix("- ") {
        hang = indent + 2;
        runs.push(s("- ", FAINT));
        let label = ["Pro:", "Con:", "Risk:"]
            .into_iter()
            .zip([GREEN, AMBER, RED])
            .find(|(l, _)| rest.starts_with(l));
        match label {
            Some((l, c)) => {
                runs.push(b(l, c));
                runs.extend(md(&rest[l.len()..], theme::fg(FG)));
            }
            None if is_effect(rest) => {
                let (head, tail) = rest.split_once(':').unwrap_or((rest, ""));
                runs.push(s(&format!("{head}:"), FAINT));
                runs.extend(md(tail, theme::fg(MUTED)));
            }
            None => runs.extend(md(rest, theme::fg(FG))),
        }
    } else if let Some(rest) = t.strip_prefix("Recommendation:") {
        runs.push(b("Recommendation:", FG));
        runs.extend(md(rest, theme::fg(FG)));
    } else if t.starts_with("Question ") && t.contains(" of ") && t.len() < 24 {
        runs.push(s(t, FAINT));
    } else if let Some((id, subject)) = t.split_once(" — ").filter(|(id, _)| is_qid(id)) {
        runs.push(b(&format!("{id} — "), AMBER));
        runs.extend(md(subject, theme::bold(FG)));
    } else if let Some(heading) = heading(t) {
        runs.extend(md(heading, theme::bold(FG)));
    } else if t.ends_with(':') && t.len() < 48 {
        runs.extend(md(t, theme::bold(MUTED)));
    } else {
        runs.extend(md(t, theme::fg(FG)));
    }
    (runs, hang)
}

/// A `## Heading` line's text.
fn heading(t: &str) -> Option<&str> {
    let hashes = t.chars().take_while(|&c| c == '#').count();
    (1..=6)
        .contains(&hashes)
        .then(|| t[hashes..].strip_prefix(' '))
        .flatten()
}

/// Whether a line opens or closes a fenced code block.
fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

fn is_qid(s: &str) -> bool {
    s.strip_prefix('Q')
        .is_some_and(|r| r.starts_with(|c: char| c.is_ascii_digit()) && r.len() <= 4)
}

/// An effect line under the last decision: `Q5 dropped: settled by Q2.`
fn is_effect(rest: &str) -> bool {
    let mut words = rest.split_whitespace();
    words.next().is_some_and(is_qid)
        && words.next().is_some_and(|w| {
            ["dropped:", "rewritten:", "reopened:", "added:", "deferred"]
                .iter()
                .any(|k| w.starts_with(k))
        })
}

/// `text` as screen lines `width` wide.
pub fn render(text: &str, width: usize) -> Vec<Line> {
    let lines: Vec<&str> = text.lines().collect();
    let owner = owners(&lines);
    let mut out = Vec::new();
    let mut code = false;
    for (line, option) in lines.iter().zip(owner) {
        if is_fence(line) {
            code = !code;
            continue;
        }
        let (runs, hang) = if code {
            let style = Style::default().fg(CODE).bg(BLANK);
            (vec![(line.to_string(), style)], 0)
        } else {
            style(line, option.is_some())
        };
        for runs in wrap(&runs, width, hang) {
            out.push(Line { runs, option });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const Q: &str = "✅ Q2: Middleware.

- Q5 rewritten: no guards.

Question 3 of 5
Q3 — Where an expired session goes

Context:

- Loops today.

Options:

1. /login?next=<path> (recommended)
   Back to login.
   - Pro: one path

   - Con: no reason shown
2. / with a toast
   Land on home.

Recommendation: option 1.
";

    fn text(line: &Line) -> String {
        line.runs.iter().map(|(t, _)| t.as_str()).collect()
    }

    #[test]
    fn options_are_the_numbered_lines_under_options() {
        let names = options(Q);
        assert_eq!(
            names,
            [
                (1, "/login?next=<path>".to_string()),
                (2, "/ with a toast".to_string())
            ]
        );
        assert!(options("1. not under Options:\n").is_empty());
    }

    #[test]
    fn an_option_owns_its_indented_lines_until_the_next_one() {
        let lines = render(Q, 80);
        let owned = |n| {
            lines
                .iter()
                .filter(|l| l.option == Some(n) && !text(l).trim().is_empty())
                .map(text)
                .collect::<Vec<_>>()
        };
        assert_eq!(owned(1).len(), 4);
        assert_eq!(owned(2), ["2. / with a toast", "   Land on home."]);
        assert!(
            lines
                .iter()
                .any(|l| text(l) == "Recommendation: option 1." && l.option.is_none())
        );
    }

    #[test]
    fn markdown_in_the_text_is_rendered() {
        let q = "Context:\n\n- `_model_updating()` in **two** places\n\n## Notes\n\n```\nlet x = 1;\n```\n";
        let lines = render(q, 80);
        let shown: Vec<String> = lines.iter().map(text).collect();
        assert!(
            shown.contains(&"- _model_updating() in two places".to_string()),
            "{shown:?}"
        );
        assert!(shown.contains(&"Notes".to_string()), "{shown:?}");
        assert!(shown.contains(&"let x = 1;".to_string()), "{shown:?}");
        assert!(!shown.iter().any(|l| l.contains("```")), "{shown:?}");
        let code = lines
            .iter()
            .flat_map(|l| &l.runs)
            .find(|(t, _)| t == "_model_updating()")
            .unwrap();
        assert_eq!(code.1.fg, Some(CODE));
    }

    #[test]
    fn labels_get_their_colors() {
        let lines = render(Q, 80);
        let style_of = |word: &str| {
            lines
                .iter()
                .flat_map(|l| &l.runs)
                .find(|(t, _)| t == word)
                .map(|(_, s)| s.fg)
                .unwrap()
        };
        assert_eq!(style_of("Pro:"), Some(GREEN));
        assert_eq!(style_of("Con:"), Some(AMBER));
        assert_eq!(style_of("Q3 — "), Some(AMBER));
        assert_eq!(style_of(" (recommended)"), Some(GREEN));
        assert_eq!(style_of("- Q5 rewritten:"), Some(FAINT));
    }
}
