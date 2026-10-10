//! Content-addressed anchors for marked diff lines, and matching stored marks back onto the
//! rows of the current diff. Pure: no I/O.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Old,
    New,
}

/// How a language's lines are compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Norm {
    /// Whitespace carries no meaning: collapse all of it.
    Brace,
    /// Leading whitespace is syntax: strip only trailing whitespace.
    Indent,
}

/// Unknown languages get `Indent`, which never merges lines that differ in indentation.
pub fn norm_for(path: &str) -> Norm {
    let name = path.rsplit('/').next().unwrap_or(path);
    let ext = name.rsplit_once('.').map_or("", |(_, e)| e);
    let indent_names = ["Makefile", "makefile", "GNUmakefile"];
    if indent_names.contains(&name) {
        return Norm::Indent;
    }
    match ext {
        "rs" | "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "mts" | "cts" | "go" | "json"
        | "c" | "h" | "cc" | "cpp" | "hpp" | "java" | "kt" | "swift" | "cs" | "css" | "scss"
        | "php" | "toml" | "sh" | "bash" | "zsh" | "lua" | "zig" | "scala" | "dart" => Norm::Brace,
        _ => Norm::Indent,
    }
}

pub fn normalize(line: &str, norm: Norm) -> String {
    match norm {
        Norm::Brace => line.split_whitespace().collect(),
        Norm::Indent => line.trim_end().to_string(),
    }
}

pub fn hash_line(line: &str, norm: Norm) -> i64 {
    let digest = blake3::hash(normalize(line, norm).as_bytes());
    i64::from_le_bytes(digest.as_bytes()[..8].try_into().unwrap())
}

/// Lines with no letters or digits (`}`, `);`, blank) say nothing about where they are.
fn is_trivial(line: &str) -> bool {
    !line.chars().any(char::is_alphanumeric)
}

/// Line hashes for one side of a file, with which lines can serve as context.
#[derive(Clone)]
pub struct FileIndex {
    pub hashes: Vec<i64>,
    landmark: Vec<bool>,
}

impl FileIndex {
    pub fn new(text: &str, norm: Norm) -> Self {
        let lines: Vec<&str> = text.lines().collect();
        let hashes: Vec<i64> = lines.iter().map(|l| hash_line(l, norm)).collect();
        let mut counts: HashMap<i64, u32> = HashMap::new();
        for h in &hashes {
            *counts.entry(*h).or_default() += 1;
        }
        let landmark = lines
            .iter()
            .zip(&hashes)
            .map(|(l, h)| !is_trivial(l) && counts[h] == 1)
            .collect();
        Self { hashes, landmark }
    }

    /// The nearest `window` landmark lines above and below 1-based `line`, nearest first.
    pub fn context(&self, line: u32, window: usize) -> (Vec<i64>, Vec<i64>) {
        let idx = line as usize - 1;
        let pick = |range: &mut dyn Iterator<Item = usize>| -> Vec<i64> {
            range
                .filter(|&i| self.landmark[i])
                .take(window)
                .map(|i| self.hashes[i])
                .collect()
        };
        let above = pick(&mut (0..idx.min(self.hashes.len())).rev());
        let below = pick(&mut (idx + 1..self.hashes.len()));
        (above, below)
    }
}

/// What a mark remembers about its line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anchor {
    pub path: String,
    pub side: Side,
    pub line_hash: i64,
    pub above: Vec<i64>,
    pub below: Vec<i64>,
}

/// A changed row of the current diff that a mark can land on.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub id: usize,
    pub line_hash: i64,
    pub above: Vec<i64>,
    pub below: Vec<i64>,
}

/// A stored mark as matching sees it.
pub struct MarkRef<'a> {
    pub id: i64,
    pub anchor: &'a Anchor,
    pub created_at: i64,
    /// The candidate an exact line-diff remap from the mark's commit points at.
    pub remap: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub struct MatchConfig {
    pub window: usize,
    pub threshold: f64,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            window: 2,
            threshold: 0.5,
        }
    }
}

fn overlap(a: &[i64], b: &[i64]) -> usize {
    a.iter().filter(|h| b.contains(h)).count()
}

/// `2*above + below`, the numerator of `(2*above + below) / (3*window)`. Above counts
/// double because it usually holds the enclosing scope.
fn score(anchor: &Anchor, c: &Candidate) -> usize {
    2 * overlap(&anchor.above, &c.above) + overlap(&anchor.below, &c.below)
}

/// Assigns marks to candidates one-to-one. Returns candidate id → mark id.
///
/// 1. A line hash with one candidate matches outright.
/// 2. Duplicates match by context score at or above the threshold.
/// 3. Equal scores go to the mark's exact remap, then to the newer mark.
pub fn match_marks(
    marks: &[MarkRef],
    candidates: &[Candidate],
    cfg: MatchConfig,
) -> HashMap<usize, i64> {
    let min_score = (cfg.threshold * 3.0 * cfg.window as f64).ceil() as usize;
    let mut by_hash: HashMap<i64, (Vec<&MarkRef>, Vec<&Candidate>)> = HashMap::new();
    for m in marks {
        by_hash.entry(m.anchor.line_hash).or_default().0.push(m);
    }
    for c in candidates {
        if let Some(group) = by_hash.get_mut(&c.line_hash) {
            group.1.push(c);
        }
    }

    let mut out = HashMap::new();
    for (group_marks, group_cands) in by_hash.into_values() {
        let unique = group_cands.len() == 1;
        let mut pairs: Vec<((usize, bool, i64), &MarkRef, &Candidate)> = Vec::new();
        for m in &group_marks {
            for c in &group_cands {
                let s = score(m.anchor, c);
                if unique || s >= min_score {
                    pairs.push(((s, m.remap == Some(c.id), m.created_at), m, c));
                }
            }
        }
        pairs.sort_by_key(|p| std::cmp::Reverse(p.0));
        let mut used_marks = HashSet::new();
        for (_, m, c) in pairs {
            if !out.contains_key(&c.id) && used_marks.insert(m.id) {
                out.insert(c.id, m.id);
            }
        }
    }
    out
}

/// Maps 1-based lines of `old` to lines of `new` that a line diff keeps unchanged.
pub fn remap_lines(old: &[i64], new: &[i64]) -> HashMap<u32, u32> {
    let mut map = HashMap::new();
    for op in similar::capture_diff_slices(similar::Algorithm::Myers, old, new) {
        if let similar::DiffOp::Equal {
            old_index,
            new_index,
            len,
        } = op
        {
            for i in 0..len {
                map.insert((old_index + i) as u32 + 1, (new_index + i) as u32 + 1);
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brace_languages_ignore_all_whitespace_and_indent_languages_keep_leading() {
        assert_eq!(
            hash_line("  let x = 1;", Norm::Brace),
            hash_line("let x=1;\t", Norm::Brace)
        );
        assert_eq!(
            hash_line("  x = 1  ", Norm::Indent),
            hash_line("  x = 1", Norm::Indent)
        );
        assert_ne!(
            hash_line("  x = 1", Norm::Indent),
            hash_line("x = 1", Norm::Indent)
        );
        assert_eq!(norm_for("a/b.py"), Norm::Indent);
        assert_eq!(norm_for("Makefile"), Norm::Indent);
        assert_eq!(norm_for("src/lib.rs"), Norm::Brace);
        assert_eq!(norm_for("notes.txt"), Norm::Indent);
    }

    #[test]
    fn context_skips_trivial_and_repeated_lines() {
        let text = "fn a() {\n    log();\n}\nfn b() {\n    log();\n    target();\n}\nfn c() {}\n";
        let idx = FileIndex::new(text, Norm::Brace);
        let (above, below) = idx.context(6, 2);
        let h = |s| hash_line(s, Norm::Brace);
        // `log();` appears twice and `}` is trivial, so neither is context.
        assert_eq!(above, [h("fn b() {"), h("fn a() {")]);
        assert_eq!(below, [h("fn c() {}")]);
    }

    fn anchor(line: &str, above: &[&str], below: &[&str]) -> Anchor {
        let h = |s: &&str| hash_line(s, Norm::Brace);
        Anchor {
            path: "f.rs".into(),
            side: Side::New,
            line_hash: hash_line(line, Norm::Brace),
            above: above.iter().map(h).collect(),
            below: below.iter().map(h).collect(),
        }
    }

    fn cand(id: usize, a: &Anchor) -> Candidate {
        Candidate {
            id,
            line_hash: a.line_hash,
            above: a.above.clone(),
            below: a.below.clone(),
        }
    }

    fn mark(id: i64, anchor: &Anchor, remap: Option<usize>) -> MarkRef<'_> {
        MarkRef {
            id,
            anchor,
            created_at: id,
            remap,
        }
    }

    #[test]
    fn unique_line_matches_even_when_context_changed() {
        let marked = anchor("total += x;", &["fn sum() {"], &[]);
        let now = anchor("total += x;", &["fn add_all() {"], &[]);
        let got = match_marks(
            &[mark(1, &marked, None)],
            &[cand(7, &now)],
            MatchConfig::default(),
        );
        assert_eq!(got, HashMap::from([(7, 1)]));
    }

    #[test]
    fn duplicate_lines_match_by_context() {
        let in_a = anchor("return Ok(());", &["fn a() {", "mod m {"], &["fn b() {"]);
        let in_b = anchor("return Ok(());", &["fn b() {", "fn a() {"], &["fn c() {"]);
        let marks = [mark(1, &in_b, None)];
        let got = match_marks(
            &marks,
            &[cand(0, &in_a), cand(1, &in_b)],
            MatchConfig::default(),
        );
        assert_eq!(got, HashMap::from([(1, 1)]));
    }

    #[test]
    fn duplicate_below_threshold_stays_unmatched() {
        let marked = anchor("x();", &["fn old_a() {", "fn old_b() {"], &["fn old_c() {"]);
        let c1 = anchor("x();", &["fn a() {"], &[]);
        let c2 = anchor("x();", &["fn b() {"], &[]);
        let got = match_marks(
            &[mark(1, &marked, None)],
            &[cand(0, &c1), cand(1, &c2)],
            MatchConfig::default(),
        );
        assert!(got.is_empty());
    }

    #[test]
    fn score_tie_goes_to_exact_remap() {
        let same = anchor("x();", &["fn a() {", "fn b() {"], &["fn c() {"]);
        let marks = [mark(1, &same, Some(1))];
        let got = match_marks(
            &marks,
            &[cand(0, &same), cand(1, &same)],
            MatchConfig::default(),
        );
        assert_eq!(got, HashMap::from([(1, 1)]));
    }

    #[test]
    fn higher_score_beats_remap() {
        let marked = anchor("x();", &["fn a() {", "fn b() {"], &["fn c() {"]);
        let weaker = anchor("x();", &["fn a() {"], &["fn c() {"]);
        let marks = [mark(1, &marked, Some(1))];
        let got = match_marks(
            &marks,
            &[cand(0, &marked), cand(1, &weaker)],
            MatchConfig::default(),
        );
        assert_eq!(got, HashMap::from([(0, 1)]));
    }

    #[test]
    fn matching_is_one_to_one() {
        let a = anchor("x();", &["fn a() {", "fn b() {"], &["fn c() {"]);
        let marks = [mark(1, &a, None), mark(2, &a, None)];
        let got = match_marks(&marks, &[cand(0, &a), cand(1, &a)], MatchConfig::default());
        let mut ids: Vec<_> = got.values().copied().collect();
        ids.sort();
        assert_eq!(ids, [1, 2]);
    }

    #[test]
    fn unique_candidate_takes_newest_of_competing_marks() {
        let a = anchor("x();", &[], &[]);
        let marks = [mark(1, &a, None), mark(5, &a, None)];
        let got = match_marks(&marks, &[cand(3, &a)], MatchConfig::default());
        assert_eq!(got, HashMap::from([(3, 5)]));
    }

    #[test]
    fn remap_follows_inserted_lines() {
        let old = [10, 20, 30];
        let new = [10, 99, 20, 30];
        assert_eq!(
            remap_lines(&old, &new),
            HashMap::from([(1, 1), (2, 3), (3, 4)])
        );
    }
}
