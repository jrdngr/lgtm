//! Moving the working copy and rewriting commits, for refetching PRs and undoing or moving
//! agent turns.

use anyhow::Context;

use super::Jj;

impl Jj {
    /// Runs a write that can move `@`, and notes where `@` ends up as the app's own move.
    async fn write(&self, args: &[&str]) -> anyhow::Result<()> {
        let mut seen = self.writer.lock().await;
        let done = self.exec(args, false).await.map(drop);
        self.note_working_copy(&mut seen).await;
        done
    }

    /// Puts `@` on a new commit on top of `rev`.
    pub async fn new_on(&self, rev: &str) -> anyhow::Result<()> {
        self.write(&["new", rev]).await
    }

    #[cfg(test)]
    pub async fn edit(&self, rev: &str) -> anyhow::Result<()> {
        self.write(&["edit", rev]).await
    }

    /// Puts `paths` in `@` back as they are in `from`.
    pub async fn restore_paths(&self, from: &str, paths: &[String]) -> anyhow::Result<()> {
        let sets: Vec<String> = paths.iter().map(|p| root_file(p)).collect();
        let mut args = vec!["restore", "--from", from, "--into", "@", "--"];
        args.extend(sets.iter().map(String::as_str));
        self.write(&args).await
    }

    /// Rebases the commits in `source` and their descendants onto `dest`.
    pub async fn rebase(&self, source: &str, dest: &str) -> anyhow::Result<()> {
        self.write(&["rebase", "-s", source, "-o", dest]).await
    }

    /// Slices `@` by time at `from` and `to`: a change for what came before `from`, one for
    /// what changed from `from` to `to` (only in `paths` unless they're `None`), and `@` for
    /// the rest, in that order. `@` keeps its files. A slice that comes out empty is dropped;
    /// when it's the last, `@` takes the middle one. Returns the before and middle changes.
    pub async fn slice_working_copy(
        &self,
        from: &str,
        to: &str,
        paths: Option<&[String]>,
    ) -> anyhow::Result<(Option<String>, String)> {
        let before = self.insert_below_at().await?;
        self.restore_keeping_descendants(from, &before, None)
            .await?;
        let middle = self.insert_below_at().await?;
        self.restore_keeping_descendants(to, &middle, paths).await?;
        let empty = |rev: &str| format!("{rev} & empty()");
        let before = if self.commit_ids(&empty(&before)).await?.is_empty() {
            Some(before)
        } else {
            self.write(&["abandon", &before]).await?;
            None
        };
        if self.commit_ids(&empty("@")).await?.is_empty() {
            return Ok((before, middle));
        }
        self.write(&["squash", "--from", &middle, "--into", "@", "-u"])
            .await?;
        let at = self.changes("@").await?.pop().context("no working copy")?;
        Ok((before, at.change_id))
    }

    /// Puts a new empty change between `@` and its parents, and returns it.
    async fn insert_below_at(&self) -> anyhow::Result<String> {
        self.write(&["new", "--no-edit", "-B", "@"]).await?;
        let parent = self.changes("@-").await?.pop().context("no parent")?;
        Ok(parent.change_id)
    }

    /// Makes `into` hold `paths`, or every file, as they are in `from`, without changing the
    /// files of its descendants.
    async fn restore_keeping_descendants(
        &self,
        from: &str,
        into: &str,
        paths: Option<&[String]>,
    ) -> anyhow::Result<()> {
        let sets: Vec<String> = paths
            .unwrap_or_default()
            .iter()
            .map(|p| root_file(p))
            .collect();
        let mut args = vec![
            "restore",
            "--from",
            from,
            "--into",
            into,
            "--restore-descendants",
        ];
        if paths.is_some() {
            args.push("--");
            args.extend(sets.iter().map(String::as_str));
        }
        self.write(&args).await
    }
}

/// The fileset naming exactly the file at repo-relative `path`.
pub(super) fn root_file(path: &str) -> String {
    format!(
        "root-file:\"{}\"",
        path.replace('\\', "\\\\").replace('"', "\\\"")
    )
}
