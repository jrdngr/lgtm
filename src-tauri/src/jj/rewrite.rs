//! Moving the working copy and rewriting commits, for refetching PRs and undoing agent turns.

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
}

/// The fileset naming exactly the file at repo-relative `path`.
fn root_file(path: &str) -> String {
    format!("root-file:\"{}\"", path.replace('\\', "\\\\").replace('"', "\\\""))
}
