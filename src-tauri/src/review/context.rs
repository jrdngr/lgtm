//! Unchanged code between a file's hunks, which the review hides until asked for.

use anyhow::Context as _;
use serde::Serialize;
use specta::Type;

use super::RepoCtx;
use crate::syntax::{self, Segment};

/// One line of the new side, with its highlighting when the language is known.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ContextLine {
    pub line: u32,
    pub text: String,
    pub hl: Option<Vec<Segment>>,
}

/// Lines `from` to `to` of `text`, 1-based and inclusive, clipped to the text.
fn lines_in(text: &str, from: u32, to: u32) -> Vec<(u32, &str)> {
    text.lines()
        .zip(1..)
        .skip(from.saturating_sub(1) as usize)
        .take_while(|(_, n)| *n <= to)
        .map(|(t, n)| (n, t))
        .collect()
}

impl RepoCtx {
    /// Lines `from` to `to` of file `file`'s new side in review `id`.
    pub async fn context_lines(
        &self,
        id: u32,
        file: u32,
        from: u32,
        to: u32,
    ) -> anyhow::Result<Vec<ContextLine>> {
        let (ws, commit, path) = {
            let review = self.review(id)?;
            let review = review.lock().unwrap();
            let f = review.files.get(file as usize).context("no such file")?;
            (review.ws.clone(), f.new.commit.clone(), f.new.path.clone())
        };
        let bytes = ws.jj.file(commit.as_deref(), &path).await?;
        let text = String::from_utf8_lossy(&bytes);
        let wanted = lines_in(&text, from, to);
        let numbers: Vec<u32> = wanted.iter().map(|(n, _)| *n).collect();
        let mut hl = syntax::parse(&path, &text).map(|p| p.highlight(&text, &numbers));
        Ok(wanted
            .into_iter()
            .map(|(line, t)| ContextLine {
                line,
                text: t.to_string(),
                hl: hl.as_mut().map(|h| h.remove(&line).unwrap_or_default()),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_in_counts_from_one_and_stops_at_the_end_of_the_text() {
        let text = "a\nb\nc\nd\n";
        assert_eq!(lines_in(text, 2, 3), [(2, "b"), (3, "c")]);
        assert_eq!(lines_in(text, 3, 99), [(3, "c"), (4, "d")]);
        assert!(lines_in(text, 5, 9).is_empty());
    }
}
