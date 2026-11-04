//! Syntax colors for diff rows, from highlighting each side's whole file.

use lgtm_core::syntax::{self, HIGHLIGHT_NAMES, Items, Segment};
use ratatui::style::Color;

use super::theme::{self, FG};

/// Highlight segments per line of a file's old and new sides, empty where there's no grammar
/// or no such side.
#[derive(Default)]
pub struct Sides {
    pub old: Vec<Vec<Segment>>,
    pub new: Vec<Vec<Segment>>,
    /// Where each line's syntax item steps to, on each side.
    pub old_items: Items,
    pub new_items: Items,
}

impl Sides {
    pub fn of(path: &str, old: Option<&str>, new: Option<&str>) -> Self {
        let side = |s: Option<&str>| s.map_or_else(Vec::new, |s| syntax::highlight_file(path, s));
        let items = |s: Option<&str>| s.map_or_else(Items::default, |s| syntax::items(path, s));
        Self {
            old: side(old),
            new: side(new),
            old_items: items(old),
            new_items: items(new),
        }
    }
}

/// `text` split into colored runs, tabs expanded to four spaces. `segs` measure UTF-16 units.
pub fn runs(text: &str, segs: &[Segment]) -> Vec<(String, Color)> {
    let mut out: Vec<(String, Color)> = Vec::new();
    let mut segs = segs.iter().copied();
    let mut current: Option<Segment> = segs.next();
    for ch in text.chars() {
        let color = current.map_or(FG, |(class, _)| color(class));
        let piece = if ch == '\t' {
            "    ".to_string()
        } else {
            ch.to_string()
        };
        match out.last_mut() {
            Some((s, c)) if *c == color => s.push_str(&piece),
            _ => out.push((piece, color)),
        }
        if let Some((_, len)) = current.as_mut() {
            let used = ch.len_utf16() as u32;
            if *len > used {
                *len -= used;
            } else {
                current = segs.next();
            }
        }
    }
    out
}

fn color(class: Option<u8>) -> Color {
    class
        .and_then(|c| HIGHLIGHT_NAMES.get(c as usize))
        .map_or(FG, |name| theme::syntax(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_follow_utf16_segments_and_expand_tabs() {
        let keyword = HIGHLIGHT_NAMES
            .iter()
            .position(|n| *n == "keyword")
            .unwrap() as u8;
        let string = HIGHLIGHT_NAMES.iter().position(|n| *n == "string").unwrap() as u8;
        // "😀" is two UTF-16 units, so the string segment covers `"😀"` as four.
        let text = "\tlet s = \"😀\";";
        let segs = [
            (None, 1),
            (Some(keyword), 3),
            (None, 5),
            (Some(string), 4),
            (None, 1),
        ];
        let runs = runs(text, &segs);
        assert_eq!(
            runs,
            vec![
                ("    ".to_string(), FG),
                ("let".to_string(), theme::syntax("keyword")),
                (" s = ".to_string(), FG),
                ("\"😀\"".to_string(), theme::syntax("string")),
                (";".to_string(), FG),
            ]
        );
    }

    #[test]
    fn text_past_the_segments_is_plain() {
        let runs = runs("ab", &[]);
        assert_eq!(runs, vec![("ab".to_string(), FG)]);
    }
}
