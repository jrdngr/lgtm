//! Runs wrapped at word breaks to fit a width.

use super::render::Run;

/// `runs` as lines of at most `avail` columns. Words wider than a line are cut across lines.
pub fn wrap(runs: impl Iterator<Item = Run>, avail: usize) -> Vec<Vec<Run>> {
    let avail = avail.max(1);
    let mut out = vec![];
    let mut line: Vec<Run> = vec![];
    let mut used = 0;
    for word in words(runs) {
        let w: usize = word.iter().map(Run::width).sum();
        let space = word.first().is_some_and(|r| r.text.starts_with(' '));
        if used + w > avail && used > 0 {
            out.push(std::mem::take(&mut line));
            used = 0;
        }
        for mut run in word {
            if used == 0 && space {
                run.text = run.text.trim_start().to_string();
            }
            while used + run.width() > avail {
                let take = avail - used;
                let head: String = run.text.chars().take(take).collect();
                run.text = run.text.chars().skip(take).collect();
                line.push(Run {
                    text: head,
                    ..run.clone()
                });
                out.push(std::mem::take(&mut line));
                used = 0;
            }
            used += run.width();
            if !run.text.is_empty() {
                line.push(run);
            }
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

/// Runs split into words, each a list of runs with the space before it.
fn words(runs: impl Iterator<Item = Run>) -> Vec<Vec<Run>> {
    let mut out: Vec<Vec<Run>> = vec![vec![]];
    for run in runs {
        let mut piece = String::new();
        for ch in run.text.chars() {
            if ch == ' ' && !piece.trim().is_empty() {
                out.last_mut().unwrap().push(Run {
                    text: std::mem::take(&mut piece),
                    ..run.clone()
                });
                out.push(vec![]);
            }
            piece.push(ch);
        }
        if !piece.is_empty() {
            out.last_mut().unwrap().push(Run { text: piece, ..run });
        }
    }
    out.retain(|w| !w.is_empty());
    out
}
