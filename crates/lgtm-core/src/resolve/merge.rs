//! A three-way merge of lines, for carrying a moved file's edits to where it went.

use std::ops::Range;

use similar::TextDiff;

/// A change to the base's lines `at`, which become `lines`.
#[derive(Clone, Debug)]
struct Edit<'a> {
    at: Range<usize>,
    lines: Vec<&'a str>,
}

fn edits<'a>(base: &'a str, side: &'a str) -> Vec<Edit<'a>> {
    let diff = TextDiff::from_lines(base, side);
    let new: Vec<&str> = side.split_inclusive('\n').collect();
    diff.ops()
        .iter()
        .filter(|op| op.tag() != similar::DiffTag::Equal)
        .map(|op| Edit {
            at: op.old_range(),
            lines: new[op.new_range()].to_vec(),
        })
        .collect()
}

/// The base's lines `lo..hi` with `edits` in them made.
fn apply<'a>(base: &[&'a str], lo: usize, hi: usize, edits: &[&Edit<'a>]) -> Vec<&'a str> {
    let mut out = vec![];
    let mut at = lo;
    for e in edits {
        out.extend_from_slice(&base[at..e.at.start]);
        out.extend_from_slice(&e.lines);
        at = e.at.end;
    }
    out.extend_from_slice(&base[at..hi]);
    out
}

/// `ours` with the changes `theirs` made to `base`. Where both changed the same lines
/// differently, the error holds the merge with git-style markers there, labeled `labels`.
pub fn merge(base: &str, ours: &str, theirs: &str, labels: [&str; 3]) -> Result<String, String> {
    let lines: Vec<&str> = base.split_inclusive('\n').collect();
    let mut all: Vec<(bool, Edit)> = edits(base, ours)
        .into_iter()
        .map(|e| (true, e))
        .chain(edits(base, theirs).into_iter().map(|e| (false, e)))
        .collect();
    all.sort_by_key(|(_, e)| (e.at.start, e.at.end));
    let mut out = String::new();
    let mut clean = true;
    let mut at = 0;
    let mut i = 0;
    while i < all.len() {
        // Edits that overlap or touch, from either side, go together.
        let (lo, mut hi) = (all[i].1.at.start, all[i].1.at.end);
        let mut j = i + 1;
        while j < all.len() && all[j].1.at.start <= hi {
            hi = hi.max(all[j].1.at.end);
            j += 1;
        }
        out.extend(lines[at..lo].iter().copied());
        let group = &all[i..j];
        let ours: Vec<&Edit> = group.iter().filter(|(o, _)| *o).map(|(_, e)| e).collect();
        let theirs: Vec<&Edit> = group.iter().filter(|(o, _)| !*o).map(|(_, e)| e).collect();
        let a = apply(&lines, lo, hi, &ours);
        let b = apply(&lines, lo, hi, &theirs);
        if theirs.is_empty() || a == b {
            out.extend(a);
        } else if ours.is_empty() {
            out.extend(b);
        } else {
            clean = false;
            let end = |s: &mut String| {
                if !s.is_empty() && !s.ends_with('\n') {
                    s.push('\n');
                }
            };
            out.push_str(&format!("<<<<<<< {}\n", labels[0]));
            out.extend(a);
            end(&mut out);
            out.push_str(&format!("||||||| {}\n", labels[1]));
            out.extend(lines[lo..hi].iter().copied());
            end(&mut out);
            out.push_str("=======\n");
            out.extend(b);
            end(&mut out);
            out.push_str(&format!(">>>>>>> {}\n", labels[2]));
        }
        at = hi;
        i = j;
    }
    out.extend(lines[at..].iter().copied());
    if clean { Ok(out) } else { Err(out) }
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: [&str; 3] = ["h.txt", "base", "edit"];

    #[test]
    fn edits_to_different_lines_both_land() {
        let base = "one\ntwo\nthree\nfour\n";
        let ours = "ONE\ntwo\nthree\nfour\n";
        let theirs = "one\ntwo\nthree\nFOUR\nfive\n";
        assert_eq!(
            merge(base, ours, theirs, L),
            Ok("ONE\ntwo\nthree\nFOUR\nfive\n".into())
        );
    }

    #[test]
    fn the_same_edit_on_both_sides_lands_once() {
        let base = "a\nb\n";
        assert_eq!(merge(base, "a\nB\n", "a\nB\n", L), Ok("a\nB\n".into()));
    }

    #[test]
    fn different_edits_to_one_line_come_back_marked() {
        let got = merge("a\nb\nc\n", "a\nX\nc\n", "a\nY\nc\n", L).unwrap_err();
        assert_eq!(
            got,
            "a\n<<<<<<< h.txt\nX\n||||||| base\nb\n=======\nY\n>>>>>>> edit\nc\n"
        );
        let parsed = super::super::parse::parse(&got);
        assert_eq!(parsed.conflicts[0].sides[1].text, "Y\n");
    }
}
