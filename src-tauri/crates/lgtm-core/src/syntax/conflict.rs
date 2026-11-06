//! Conflict markers, jj's and git's, colored over a file's highlighting: the marker lines,
//! and the removed and added lines of jj's diff sections.

use super::{HIGHLIGHT_NAMES, Segment};

fn class(name: &str) -> Option<u8> {
    HIGHLIGHT_NAMES
        .iter()
        .position(|n| *n == name)
        .map(|i| i as u8)
}

/// The char a marker line repeats, when `line` is one: seven or more of it, then a space or
/// the end.
fn marker(line: &str) -> Option<char> {
    let c = line.chars().next()?;
    if !matches!(c, '<' | '>' | '%' | '\\' | '+' | '-' | '|' | '=') {
        return None;
    }
    let run = line.chars().take_while(|&x| x == c).count();
    let rest = &line[run..];
    (run >= 7 && (rest.is_empty() || rest.starts_with(' '))).then_some(c)
}

/// What a line of a conflict is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Marker,
    Removed,
    Added,
}

/// Each line's part in a conflict, from the first line, or None outside conflicts and for
/// lines that keep their highlighting.
fn kinds(source: &str) -> Vec<Option<Kind>> {
    let mut out = vec![];
    let mut inside = false;
    // Whether the lines are a diff section's, which start with -, + or a space.
    let mut diff = false;
    for line in source.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        let m = marker(line);
        let kind = match m {
            Some('<') if !inside => {
                inside = true;
                diff = false;
                Some(Kind::Marker)
            }
            Some('>') if inside => {
                inside = false;
                Some(Kind::Marker)
            }
            // jj makes markers longer than any line like them in the files.
            Some(c) if inside => {
                // `\\\\\\\` names the diff's other side; the section stays a diff.
                if c != '\\' {
                    diff = c == '%';
                }
                Some(Kind::Marker)
            }
            _ if inside && diff => match line.chars().next() {
                Some('-') => Some(Kind::Removed),
                Some('+') => Some(Kind::Added),
                _ => None,
            },
            _ => None,
        };
        out.push(kind);
    }
    out
}

/// `lines` with conflicts colored over them, for `source`. Without any, `lines` as it was;
/// with some, a line for each of `source`'s even where it had no highlighting.
pub fn overlay(source: &str, mut lines: Vec<Vec<Segment>>) -> Vec<Vec<Segment>> {
    if !source.contains("<<<<<<<") {
        return lines;
    }
    let kinds = kinds(source);
    if kinds.iter().all(Option::is_none) {
        return lines;
    }
    lines.resize(kinds.len(), vec![]);
    for ((text, kind), segs) in source.split('\n').zip(kinds).zip(lines.iter_mut()) {
        let name = match kind {
            Some(Kind::Marker) => "conflict.marker",
            Some(Kind::Removed) => "diff.minus",
            Some(Kind::Added) => "diff.plus",
            None => continue,
        };
        let len = text
            .strip_suffix('\r')
            .unwrap_or(text)
            .encode_utf16()
            .count() as u32;
        *segs = vec![(class(name), len)];
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(source: &str) -> Vec<&'static str> {
        overlay(source, vec![])
            .iter()
            .map(|segs| match segs.first() {
                Some((Some(c), _)) => HIGHLIGHT_NAMES[*c as usize],
                _ => "",
            })
            .collect()
    }

    #[test]
    fn a_jj_diff_conflict_colors_its_markers_and_the_lines_its_diffs_remove_and_add() {
        let src = "name = app\n<<<<<<< conflict 1 of 1\n%%%%%%% diff from: a\n\\\\\\\\\\\\\\        to: b\n-color = blue\n+color = red\n color = x\n+++++++ c\ncolor = yellow\n-------\n>>>>>>> conflict 1 of 1 ends\nmode = dev";
        assert_eq!(
            names(src),
            [
                "",
                "conflict.marker",
                "conflict.marker",
                "conflict.marker",
                "diff.minus",
                "diff.plus",
                "",
                "conflict.marker",
                "",
                "conflict.marker",
                "conflict.marker",
                ""
            ]
        );
    }

    #[test]
    fn git_conflicts_color_only_their_markers_and_text_without_conflicts_is_left_alone() {
        let src = "<<<<<<< ours\n-a\n||||||| base\nb\n=======\n+c\n>>>>>>> theirs\n";
        assert_eq!(
            names(src),
            [
                "conflict.marker",
                "",
                "conflict.marker",
                "",
                "conflict.marker",
                "",
                "conflict.marker",
                ""
            ]
        );
        let plain = vec![vec![(Some(1), 3)]];
        assert_eq!(overlay("abc", plain.clone()), plain);
        assert_eq!(
            overlay("======= heading\n>>>>>>> x", vec![]),
            Vec::<Vec<Segment>>::new()
        );
    }
}
