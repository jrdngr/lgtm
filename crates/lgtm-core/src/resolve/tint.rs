//! The words a side changes from the base, to tint.

use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use specta::Type;

/// Chars `from..to` of `line`, from 0, that the side changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Tint {
    pub line: u32,
    pub from: u32,
    pub to: u32,
}

/// The runs of `text` that differ from `base`, word by word, one per line they cover. Runs
/// don't start or end with spaces.
pub fn tints(base: &str, text: &str) -> Vec<Tint> {
    let diff = TextDiff::from_words(base, text);
    // Each char of `text` but newlines: its line, column, whether it's new, and a space.
    let mut chars = vec![];
    let (mut line, mut col) = (0u32, 0u32);
    for change in diff.iter_all_changes() {
        if change.tag() == ChangeTag::Delete {
            continue;
        }
        for c in change.value().chars() {
            if c == '\n' {
                line += 1;
                col = 0;
                continue;
            }
            chars.push((
                line,
                col,
                change.tag() == ChangeTag::Insert,
                c.is_whitespace(),
            ));
            col += 1;
        }
    }
    let mut out: Vec<Tint> = vec![];
    let mut run: Option<Tint> = None;
    let end = |run: &mut Option<Tint>, out: &mut Vec<Tint>| {
        if let Some(t) = run.take()
            && t.to > t.from
        {
            out.push(t);
        }
    };
    for &(line, col, new, space) in &chars {
        if run.is_some_and(|t| t.line != line) {
            end(&mut run, &mut out);
        }
        if !new || run.is_none() && space {
            end(&mut run, &mut out);
            continue;
        }
        let t = run.get_or_insert(Tint {
            line,
            from: col,
            to: col,
        });
        if !space {
            t.to = col + 1;
        }
    }
    end(&mut run, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_words_are_tinted_and_unchanged_lines_are_not() {
        let t = tints("color = blue\nmode = dev\n", "color = yellow\nmode = dev\n");
        assert_eq!(
            t,
            [Tint {
                line: 0,
                from: 8,
                to: 14
            }]
        );
    }

    #[test]
    fn an_added_line_is_tinted_on_its_own_line() {
        let t = tints("a\nc\n", "a\nb b\nc\n");
        assert_eq!(
            t,
            [Tint {
                line: 1,
                from: 0,
                to: 3
            }]
        );
    }
}
