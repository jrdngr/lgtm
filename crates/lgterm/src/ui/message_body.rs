//! A message's text laid out: paragraphs, heads, bullets, and indented rows as two
//! columns, with `code` and URLs set apart.

use gpui::{
    AnyElement, FontWeight, IntoElement, ParentElement, SharedString, Styled, StyledText, TextRun,
    div, font, px, rgb,
};

use crate::fonts;
use crate::theme;

#[derive(Debug, PartialEq)]
pub enum Block {
    Para(String),
    /// A short line ending in `:` before rows or bullets.
    Head(String),
    /// Indented lines, each split at its first gap of two or more spaces.
    Rows(Vec<(String, String)>),
    Bullets(Vec<String>),
}

pub fn blocks(text: &str) -> Vec<Block> {
    let lines: Vec<&str> = text.lines().collect();
    let indented = |l: &str| l.starts_with("  ") && !l.trim().is_empty();
    let bullet = |l: &str| l.trim_start().starts_with("- ");
    let mut out = vec![];
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.trim().is_empty() {
            i += 1;
        } else if indented(line) {
            let mut rows = vec![];
            while i < lines.len() && indented(lines[i]) {
                rows.push(split_row(lines[i].trim()));
                i += 1;
            }
            out.push(Block::Rows(rows));
        } else if bullet(line) {
            let mut items = vec![];
            while i < lines.len() && bullet(lines[i]) {
                items.push(lines[i].trim_start()[2..].to_string());
                i += 1;
            }
            out.push(Block::Bullets(items));
        } else {
            let next = lines.get(i + 1).copied().unwrap_or("");
            if !out.is_empty()
                && line.ends_with(':')
                && line.len() < 100
                && (indented(next) || bullet(next))
            {
                out.push(Block::Head(line.trim_end_matches(':').to_string()));
                i += 1;
                continue;
            }
            let mut para = vec![];
            while i < lines.len()
                && !lines[i].trim().is_empty()
                && !indented(lines[i])
                && !bullet(lines[i])
            {
                para.push(lines[i].trim());
                i += 1;
            }
            out.push(Block::Para(para.join(" ")));
        }
    }
    out
}

/// `key    value` at its first gap of two or more spaces.
fn split_row(line: &str) -> (String, String) {
    match line.find("  ") {
        Some(at) => (line[..at].to_string(), line[at..].trim().to_string()),
        None => (line.to_string(), String::new()),
    }
}

/// Text with `code` in the mono font and URLs in the link color.
pub fn inline(text: &str, color: u32) -> StyledText {
    let plain = font(fonts::ui());
    let mono = font(fonts::mono());
    let mut shown = String::new();
    let mut runs = Vec::new();
    let mut push = |s: &str, code: bool, link: bool| {
        if s.is_empty() {
            return;
        }
        shown.push_str(s);
        runs.push(TextRun {
            len: s.len(),
            font: if code { mono.clone() } else { plain.clone() },
            color: rgb(if code {
                theme::CODE
            } else if link {
                theme::FOCUS
            } else {
                color
            })
            .into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        });
    };
    for (i, part) in text.split('`').enumerate() {
        if i % 2 == 1 {
            push(part, true, false);
            continue;
        }
        let mut rest = part;
        while let Some(at) = rest.find("http") {
            let tail = &rest[at..];
            if !(tail.starts_with("http://") || tail.starts_with("https://")) {
                push(&rest[..at + 4], false, false);
                rest = &rest[at + 4..];
                continue;
            }
            push(&rest[..at], false, false);
            let end = tail
                .find(|c: char| c.is_whitespace() || c == ',' || c == ')')
                .unwrap_or(tail.len());
            push(&tail[..end], false, true);
            rest = &tail[end..];
        }
        push(rest, false, false);
    }
    StyledText::new(shown).with_runs(runs)
}

/// The blocks of `text`, stacked.
pub fn render(text: &str) -> AnyElement {
    let blocks = blocks(text);
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .min_w_0()
        .text_size(px(13.))
        .line_height(px(20.))
        .children(blocks.into_iter().map(|b| {
            match b {
                Block::Para(t) => div().min_w_0().child(inline(&t, theme::TEXT)),
                Block::Head(t) => div()
                    .pt(px(4.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(theme::TEXT_STRONG))
                    .child(SharedString::from(t)),
                Block::Bullets(items) => {
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .children(items.into_iter().map(|t| {
                            div()
                                .flex()
                                .gap(px(8.))
                                .child(div().flex_none().text_color(rgb(theme::ICON)).child("•"))
                                .child(div().min_w_0().child(inline(&t, theme::TEXT)))
                        }))
                }
                Block::Rows(rows) => {
                    // Keys too long for the column take a line of their own.
                    const LONG: usize = 26;
                    let key_w = rows
                        .iter()
                        .map(|(k, _)| k.chars().count())
                        .filter(|&n| n <= LONG)
                        .max()
                        .unwrap_or(0) as f32
                        * 7.3
                        + 4.;
                    let key = |k: String| {
                        div()
                            .font_family(fonts::mono())
                            .text_size(px(12.))
                            .text_color(rgb(theme::CODE))
                            .child(SharedString::from(k))
                    };
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(5.))
                        .px(px(12.))
                        .py(px(9.))
                        .rounded(px(6.))
                        .bg(rgb(theme::WINDOW))
                        .children(rows.into_iter().map(move |(k, v)| {
                            if k.chars().count() > LONG {
                                return div().flex().flex_col().child(key(k)).child(
                                    div()
                                        .pl(px(key_w + 16.))
                                        .min_w_0()
                                        .child(inline(&v, theme::TEXT)),
                                );
                            }
                            div()
                                .flex()
                                .items_start()
                                .gap(px(16.))
                                .child(key(k).w(px(key_w)).flex_none())
                                .child(div().flex_1().min_w_0().child(inline(&v, theme::TEXT)))
                        }))
                }
            }
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heads_rows_bullets_and_paragraphs_are_told_apart() {
        let text = "Intro line\nstill intro.\n\nCommands:\n  run.sh    start it\n  \
                    docker compose down    stop\n\nRules:\n- one\n- two\n\nBye.";
        assert_eq!(
            blocks(text),
            [
                Block::Para("Intro line still intro.".into()),
                Block::Head("Commands".into()),
                Block::Rows(vec![
                    ("run.sh".into(), "start it".into()),
                    ("docker compose down".into(), "stop".into()),
                ]),
                Block::Head("Rules".into()),
                Block::Bullets(vec!["one".into(), "two".into()]),
                Block::Para("Bye.".into()),
            ]
        );
    }

    #[test]
    fn a_line_ending_in_a_colon_before_prose_stays_prose() {
        assert_eq!(
            blocks("Note:\nthis follows."),
            [Block::Para("Note: this follows.".into())]
        );
    }
}
