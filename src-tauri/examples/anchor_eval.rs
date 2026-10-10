//! Measures mark matching on a real repo's history.
//!
//! For commits C and D = C plus `gap` first-parent commits, every line C added to a file is
//! marked, then matched against every line of that file at D. The line diff from C to D says
//! where each marked line really went. Remapping uses that same diff, so the table also
//! reports matching without it, which is what the hash and context stages do alone.
//!
//!     cargo run --release --example anchor_eval -- <repo> [commits] [gap]

use std::collections::HashMap;
use std::process::Command;

use lgtm_lib::anchor::{
    Anchor, Candidate, FileIndex, MarkRef, MatchConfig, Side, hash_line, match_marks, norm_for,
    remap_lines,
};
use lgtm_lib::diff::{RowKind, parse_git};

fn jj(repo: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("jj")
        .args([
            "-R",
            repo,
            "--ignore-working-copy",
            "--no-pager",
            "--color=never",
        ])
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[derive(Default, Clone, Copy)]
struct Tally {
    marks: usize,
    correct: usize,
    wrong: usize,
    missed: usize,
    gone: usize,
    spurious: usize,
}

struct Case {
    marks: Vec<(u32, Anchor)>,
    candidates: Vec<Candidate>,
    /// Marked line → its line at D, when the line diff kept it.
    truth: HashMap<u32, u32>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let repo = args
        .first()
        .expect("usage: anchor_eval <repo> [commits] [gap]")
        .as_str();
    let n: usize = args.get(1).map_or(150, |s| s.parse().unwrap());
    let gap: usize = args.get(2).map_or(10, |s| s.parse().unwrap());

    let log = jj(
        repo,
        &[
            "log",
            "--no-graph",
            "-r",
            &format!("first_ancestors(trunk(), {})", n + gap),
            "-T",
            r#"commit_id ++ " " ++ parents.map(|p| p.commit_id()).join(",") ++ "\n""#,
        ],
    )
    .expect("jj log");
    // Newest first.
    let commits: Vec<(String, String)> = log
        .lines()
        .filter_map(|l| {
            let (c, ps) = l.split_once(' ')?;
            let p = ps.split(',').next()?;
            (!p.is_empty()).then(|| (c.to_string(), p.to_string()))
        })
        .collect();

    let mut cases = Vec::new();
    for i in gap..commits.len() {
        let (c, p) = &commits[i];
        let d = &commits[i - gap].0;
        let Some(git) = jj(repo, &["diff", "--from", p, "--to", c, "--git"]) else {
            continue;
        };
        for file in parse_git(&git).unwrap_or_default() {
            let Some(path) = file.header.split(" b/").nth(1) else {
                continue;
            };
            if file.binary || file.hunks.is_empty() {
                continue;
            }
            let fileset = format!("root-file:\"{path}\"");
            let (Some(at_c), Some(at_d)) = (
                jj(repo, &["file", "show", "-r", c, &fileset]),
                jj(repo, &["file", "show", "-r", d, &fileset]),
            ) else {
                continue;
            };
            if at_c.len() > 200_000 {
                continue;
            }
            let norm = norm_for(path);
            let (ic, id) = (FileIndex::new(&at_c, norm), FileIndex::new(&at_d, norm));
            let marks = file
                .hunks
                .iter()
                .flat_map(|h| &h.rows)
                .filter(|r| r.kind == RowKind::Added)
                .filter_map(|r| {
                    let line = r.new_line?;
                    let text = at_c.lines().nth(line as usize - 1)?;
                    Some((line, (text.to_string(), path.to_string())))
                })
                .map(|(line, (text, path))| (line, text, path))
                .collect::<Vec<_>>();
            if marks.is_empty() {
                continue;
            }
            cases.push((marks, ic, id, at_d, norm));
        }
    }

    let configs: Vec<MatchConfig> = [1, 2, 3, 4]
        .into_iter()
        .flat_map(|window| [0.34, 0.5, 0.67].map(|threshold| MatchConfig { window, threshold }))
        .collect();

    println!("{} file cases, gap {gap}", cases.len());
    println!("window threshold remap   marks  correct  wrong  missed  gone→matched");
    for cfg in configs {
        for use_remap in [false, true] {
            let mut t = Tally::default();
            for (marks, ic, id, at_d, norm) in &cases {
                let case = build(marks, ic, id, at_d, *norm, cfg.window);
                score(&case, cfg, use_remap, &mut t);
            }
            println!(
                "{:>6} {:>9} {:>5} {:>7} {:>7.2}% {:>5.2}% {:>6.2}% {:>6.2}%",
                cfg.window,
                cfg.threshold,
                if use_remap { "yes" } else { "no" },
                t.marks,
                pct(t.correct, t.marks - t.gone),
                pct(t.wrong, t.marks - t.gone),
                pct(t.missed, t.marks - t.gone),
                pct(t.spurious, t.gone),
            );
        }
    }
}

fn pct(a: usize, b: usize) -> f64 {
    if b == 0 {
        0.0
    } else {
        100.0 * a as f64 / b as f64
    }
}

fn build(
    marks: &[(u32, String, String)],
    ic: &FileIndex,
    id: &FileIndex,
    at_d: &str,
    norm: lgtm_lib::anchor::Norm,
    window: usize,
) -> Case {
    let anchors = marks
        .iter()
        .map(|(line, text, path)| {
            let (above, below) = ic.context(*line, window);
            let anchor = Anchor {
                path: path.clone(),
                side: Side::New,
                line_hash: hash_line(text, norm),
                above,
                below,
            };
            (*line, anchor)
        })
        .collect();
    let candidates = at_d
        .lines()
        .enumerate()
        .map(|(i, text)| {
            let (above, below) = id.context(i as u32 + 1, window);
            Candidate {
                id: i + 1,
                line_hash: hash_line(text, norm),
                above,
                below,
            }
        })
        .collect();
    Case {
        marks: anchors,
        candidates,
        truth: remap_lines(&ic.hashes, &id.hashes),
    }
}

fn score(case: &Case, cfg: MatchConfig, use_remap: bool, t: &mut Tally) {
    let refs: Vec<MarkRef> = case
        .marks
        .iter()
        .map(|(line, anchor)| MarkRef {
            id: *line as i64,
            anchor,
            created_at: 0,
            remap: if use_remap {
                case.truth.get(line).map(|l| *l as usize)
            } else {
                None
            },
        })
        .collect();
    let matched: HashMap<i64, usize> = match_marks(&refs, &case.candidates, cfg)
        .into_iter()
        .map(|(c, m)| (m, c))
        .collect();
    for (line, _) in &case.marks {
        t.marks += 1;
        let got = matched.get(&(*line as i64));
        match (case.truth.get(line), got) {
            (Some(want), Some(got)) if *want as usize == *got => t.correct += 1,
            (Some(_), Some(_)) => t.wrong += 1,
            (Some(_), None) => t.missed += 1,
            (None, got) => {
                t.gone += 1;
                if got.is_some() {
                    t.spurious += 1;
                }
            }
        }
    }
}
