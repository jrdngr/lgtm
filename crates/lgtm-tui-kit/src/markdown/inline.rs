//! Inline markdown inside one line of plain text: code spans, bold, italic, strikethrough and
//! links, for screens that lay out their own lines rather than render a whole document.

use ratatui::style::{Modifier, Style};

use super::CODE;
use crate::theme::{ACCENT, BLANK};

/// `text` as runs: `base` for plain text, code spans in the code color, and emphasis added to
/// `base`. A link shows its text. Markers that don't close stay as written.
pub fn spans(text: &str, base: Style) -> Vec<(String, Style)> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    parse(&chars, base, &mut out);
    out
}

fn push(out: &mut Vec<(String, Style)>, text: &str, style: Style) {
    match out.last_mut() {
        Some((t, s)) if *s == style => t.push_str(text),
        _ if text.is_empty() => {}
        _ => out.push((text.to_owned(), style)),
    }
}

fn starts(chars: &[char], at: usize, pat: &str) -> bool {
    pat.chars()
        .enumerate()
        .all(|(k, c)| chars.get(at + k) == Some(&c))
}

/// Where `delim` next closes emphasis opened before `from`: not right after a space, and for
/// `_`, not inside a word.
fn closing(chars: &[char], from: usize, delim: &str) -> Option<usize> {
    let n = delim.chars().count();
    (from + 1..chars.len()).find(|&j| {
        starts(chars, j, delim)
            && !chars[j - 1].is_whitespace()
            && !(delim.starts_with('_') && chars.get(j + n).is_some_and(|c| c.is_alphanumeric()))
            && !(n == 1 && chars.get(j + 1) == delim.chars().next().as_ref())
    })
}

fn parse(chars: &[char], base: Style, out: &mut Vec<(String, Style)>) {
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && chars.get(i + 1).is_some_and(|n| n.is_ascii_punctuation()) {
            push(out, &chars[i + 1].to_string(), base);
            i += 2;
            continue;
        }
        if c == '`' {
            let ticks = chars[i..].iter().take_while(|&&t| t == '`').count();
            let fence: String = "`".repeat(ticks);
            let close = (i + ticks..chars.len()).find(|&j| {
                starts(chars, j, &fence)
                    && chars.get(j + ticks) != Some(&'`')
                    && chars[j - 1] != '`'
            });
            if let Some(j) = close {
                let code: String = chars[i + ticks..j].iter().collect();
                let code = match code.strip_prefix(' ').and_then(|c| c.strip_suffix(' ')) {
                    Some(inner) if !inner.trim().is_empty() => inner.to_owned(),
                    _ => code,
                };
                push(out, &code, base.fg(CODE).bg(BLANK));
                i = j + ticks;
            } else {
                push(out, &fence, base);
                i += ticks;
            }
            continue;
        }
        let emphasis = [
            ("**", Modifier::BOLD),
            ("__", Modifier::BOLD),
            ("~~", Modifier::CROSSED_OUT),
            ("*", Modifier::ITALIC),
            ("_", Modifier::ITALIC),
        ];
        let opened = emphasis.iter().find(|(d, _)| {
            let n = d.chars().count();
            starts(chars, i, d)
                && chars.get(i + n).is_some_and(|c| !c.is_whitespace())
                && !(d.starts_with('_') && i > 0 && chars[i - 1].is_alphanumeric())
        });
        if let Some((d, m)) = opened {
            let n = d.chars().count();
            if let Some(j) = closing(chars, i + n, d).filter(|&j| j > i + n) {
                parse(&chars[i + n..j], base.add_modifier(*m), out);
                i = j + n;
                continue;
            }
        }
        if c == '['
            && let Some(mid) = (i + 1..chars.len()).find(|&j| starts(chars, j, "]("))
            && let Some(end) = (mid + 2..chars.len()).find(|&j| chars[j] == ')')
        {
            let link = base.fg(ACCENT).add_modifier(Modifier::UNDERLINED);
            parse(&chars[i + 1..mid], link, out);
            i = end + 1;
            continue;
        }
        push(out, &c.to_string(), base);
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::FG;

    fn shown(text: &str) -> Vec<(String, Style)> {
        spans(text, Style::default().fg(FG))
    }

    fn texts(text: &str) -> Vec<String> {
        shown(text).into_iter().map(|(t, _)| t).collect()
    }

    #[test]
    fn code_spans_lose_their_backticks_and_take_the_code_color() {
        let runs = shown("`_model_updating()` in `src/model.rs`");
        assert_eq!(runs[0].0, "_model_updating()");
        assert_eq!(runs[0].1.fg, Some(CODE));
        assert_eq!(runs[1].0, " in ");
        assert_eq!(texts("``a ` b`` and `x`")[0], "a ` b");
        assert_eq!(texts("an ` alone"), ["an ` alone"]);
    }

    #[test]
    fn emphasis_strike_and_links_style_their_text() {
        let runs = shown("**bold** and *it* and ~~gone~~ and [docs](https://x.y)");
        let text: Vec<&str> = runs.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(
            text,
            ["bold", " and ", "it", " and ", "gone", " and ", "docs"]
        );
        assert!(runs[0].1.add_modifier.contains(Modifier::BOLD));
        assert!(runs[2].1.add_modifier.contains(Modifier::ITALIC));
        assert!(runs[4].1.add_modifier.contains(Modifier::CROSSED_OUT));
        assert_eq!(runs[6].1.fg, Some(ACCENT));
    }

    #[test]
    fn markers_that_dont_open_or_close_stay_as_written() {
        assert_eq!(
            texts("snake_case_name and 2 * 3 * 4"),
            ["snake_case_name and 2 * 3 * 4"]
        );
        assert_eq!(texts("**open only"), ["**open only"]);
        assert_eq!(texts(r"\*not\* it"), ["*not* it"]);
        assert_eq!(texts("**`code` in bold**")[0], "code");
    }
}
