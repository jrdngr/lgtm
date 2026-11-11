//! Fuzzy matching tuned for file paths: a query's chars in order, scored higher at the starts
//! of words and path parts, in runs, and in the file's name.
//!
//! Each space-separated part of the query has to match on its own. A query with a capital
//! letter matches case; one without ignores it.

/// A match: its score, higher is better, and the chars of the text it matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub score: i64,
    pub positions: Vec<usize>,
}

const MATCH: i64 = 16;
/// At the start of the text or after `/`.
const PART_START: i64 = 12;
/// After `_`, `-`, `.` or a space, or a capital after a small letter.
const WORD_START: i64 = 8;
/// A match that ends where a name or extension does, before `.`, `/` or the end.
const NAME_END: i64 = 8;
/// Each char after the first of a run.
const RUN: i64 = 10;
/// In the name after the last `/`.
const IN_NAME: i64 = 6;
const GAP_START: i64 = 10;
const GAP_EXTEND: i64 = 2;
/// No score at all, for cells that can't match.
const NONE: i64 = i64::MIN / 4;

/// Matches `query` against `text`, or `None` when some part of it doesn't match.
pub fn score(query: &str, text: &str) -> Option<Match> {
    let chars: Vec<char> = text.chars().collect();
    let mut total = 0;
    let mut positions = Vec::new();
    let mut any = false;
    for part in query.split_whitespace() {
        any = true;
        let m = score_part(part, &chars)?;
        total += m.score;
        positions.extend(m.positions);
    }
    if !any {
        return Some(Match {
            score: 0,
            positions: vec![],
        });
    }
    positions.sort_unstable();
    positions.dedup();
    // Shorter paths win ties.
    total -= chars.len() as i64 / 8;
    Some(Match {
        score: total,
        positions,
    })
}

/// Sorts `items` by how well `text` of each matches `query`, best first, dropping those that
/// don't match. Ties keep their order.
pub fn rank<T>(
    query: &str,
    items: impl IntoIterator<Item = T>,
    text: impl Fn(&T) -> &str,
) -> Vec<(T, Match)> {
    let mut out: Vec<(T, Match)> = items
        .into_iter()
        .filter_map(|item| {
            let m = score(query, text(&item))?;
            Some((item, m))
        })
        .collect();
    out.sort_by_key(|(_, m)| std::cmp::Reverse(m.score));
    out
}

fn bonus(chars: &[char], j: usize, name_start: usize) -> i64 {
    let here = chars[j];
    let start = match j.checked_sub(1).map(|i| chars[i]) {
        None | Some('/') => PART_START,
        Some('_' | '-' | '.' | ' ') => WORD_START,
        Some(prev) if prev.is_lowercase() && here.is_uppercase() => WORD_START,
        Some(prev) if !prev.is_alphanumeric() && here.is_alphanumeric() => WORD_START,
        _ => 0,
    };
    start + if j >= name_start { IN_NAME } else { 0 }
}

/// The best alignment of `query`'s chars, in order, over `text`.
fn score_part(query: &str, text: &[char]) -> Option<Match> {
    let exact = query.chars().any(char::is_uppercase);
    let same = |q: char, t: char| {
        if exact {
            q == t
        } else {
            t.to_lowercase().eq(q.to_lowercase())
        }
    };
    let q: Vec<char> = query.chars().collect();
    let (n, m) = (q.len(), text.len());
    if n > m {
        return None;
    }
    // A quick check that the chars are there in order at all.
    let mut at = 0;
    for &c in &q {
        at += text[at..].iter().position(|&t| same(c, t))? + 1;
    }
    let name_start = text.iter().rposition(|&c| c == '/').map_or(0, |i| i + 1);
    // best[i][j]: the best score with query char `i` on text char `j`. from[i][j]: where
    // query char `i - 1` was then.
    let mut best = vec![vec![NONE; m]; n];
    let mut from = vec![vec![0usize; m]; n];
    for (j, &t) in text.iter().enumerate() {
        if same(q[0], t) {
            best[0][j] = MATCH + bonus(text, j, name_start);
        }
    }
    for i in 1..n {
        // The best earlier cell of the row above with a gap before `j`, and where it was.
        let mut gap = (NONE, 0);
        for j in i..m {
            if j >= 2 {
                let skip = best[i - 1][j - 2];
                gap = if skip - GAP_START >= gap.0 - GAP_EXTEND {
                    (skip - GAP_START, j - 2)
                } else {
                    (gap.0 - GAP_EXTEND, gap.1)
                };
            }
            if !same(q[i], text[j]) {
                continue;
            }
            let run = best[i - 1][j - 1] + RUN;
            let (prev, k) = if run >= gap.0 { (run, j - 1) } else { gap };
            if prev <= NONE / 2 {
                continue;
            }
            best[i][j] = prev + MATCH + bonus(text, j, name_start);
            from[i][j] = k;
        }
    }
    let ends = |j: usize| match text.get(j + 1) {
        None | Some('.' | '/') => NAME_END,
        _ => 0,
    };
    let (mut j, score) = best[n - 1]
        .iter()
        .enumerate()
        .map(|(j, &s)| (j, s + ends(j)))
        .max_by_key(|&(_, s)| s)?;
    if score <= NONE / 2 {
        return None;
    }
    let mut positions = vec![0; n];
    for i in (0..n).rev() {
        positions[i] = j;
        j = from[i][j];
    }
    Some(Match { score, positions })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATHS: &[&str] = &[
        "src-tauri/crates/lgtm/src/tui/versions.rs",
        "src-tauri/crates/lgtm/src/tui/review/mod.rs",
        "src-tauri/crates/lgtm/src/tui/review/draw.rs",
        "src-tauri/crates/lgtm/src/tui/editor.rs",
        "src-tauri/crates/lgtm/src/edit/buffer.rs",
        "src-tauri/crates/lgtm/src/main.rs",
        "src-tauri/crates/lgtm-core/src/jj/edit.rs",
        "src-tauri/crates/lgtm-core/src/review/edits.rs",
        "src/editor/EditorPanel.tsx",
        "src/canvas/edits.ts",
        "src/main.tsx",
        "README.md",
        "justfile",
    ];

    fn top(query: &str) -> &'static str {
        rank(query, PATHS.iter().copied(), |p| p)
            .first()
            .map(|(p, _)| *p)
            .expect("a match")
    }

    #[test]
    fn rev_puts_review_above_versions() {
        let ranked: Vec<&str> = rank("rev", PATHS.iter().copied(), |p| p)
            .into_iter()
            .map(|(p, _)| p)
            .collect();
        let at = |p: &str| ranked.iter().position(|r| r.ends_with(p)).unwrap();
        assert!(at("review/mod.rs") < at("versions.rs"), "{ranked:?}");
    }

    #[test]
    fn file_names_beat_matches_spread_over_folders() {
        assert_eq!(top("buffer"), "src-tauri/crates/lgtm/src/edit/buffer.rs");
        assert_eq!(top("main"), "src/main.tsx");
        assert_eq!(top("just"), "justfile");
        assert_eq!(top("editor"), "src-tauri/crates/lgtm/src/tui/editor.rs");
    }

    #[test]
    fn word_starts_beat_letters_inside_words() {
        assert_eq!(top("EP"), "src/editor/EditorPanel.tsx");
        assert_eq!(top("rdraw"), "src-tauri/crates/lgtm/src/tui/review/draw.rs");
    }

    #[test]
    fn every_part_of_the_query_has_to_match() {
        assert_eq!(
            top("core edit"),
            "src-tauri/crates/lgtm-core/src/jj/edit.rs"
        );
        assert!(score("core zzz", PATHS[6]).is_none());
    }

    #[test]
    fn capitals_match_case_and_small_letters_dont() {
        assert!(score("readme", "README.md").is_some());
        assert!(score("Readme", "README.md").is_none());
    }

    #[test]
    fn positions_are_the_matched_chars() {
        let m = score("edrs", "edit/x.rs").unwrap();
        assert_eq!(m.positions, vec![0, 1, 7, 8]);
    }

    #[test]
    fn a_query_with_more_chars_than_the_text_doesnt_match() {
        assert!(score("abcdef", "abc").is_none());
        assert!(score("", "abc").is_some());
    }
}
