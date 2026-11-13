//! The only code that runs git, for repos without jj. lgtm only reads them: the working
//! copy shows as a commit lgtm makes from the working tree, which no ref points to.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, bail};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

mod diff;
mod files;
mod log;
mod names;

pub use names::WORKING_COPY;

pub struct Git {
    root: PathBuf,
}

/// Names the working tree's commits, which differ only by tree and parent, so the same
/// tree on the same parent gets the same id.
const SNAPSHOT_ENV: [(&str, &str); 6] = [
    ("GIT_AUTHOR_NAME", "lgtm"),
    ("GIT_AUTHOR_EMAIL", "lgtm@localhost"),
    ("GIT_AUTHOR_DATE", "1970-01-01T00:00:00Z"),
    ("GIT_COMMITTER_NAME", "lgtm"),
    ("GIT_COMMITTER_EMAIL", "lgtm@localhost"),
    ("GIT_COMMITTER_DATE", "1970-01-01T00:00:00Z"),
];

/// Tells apart the temporary index files of snapshots taken at once.
static INDEXES: AtomicU64 = AtomicU64::new(0);

impl Git {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.root)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .args([
                "--no-pager",
                "-c",
                "core.quotePath=false",
                "-c",
                "color.ui=never",
            ])
            .args(args)
            .stdin(Stdio::null())
            .kill_on_drop(true);
        cmd
    }

    async fn output(&self, mut cmd: Command, args: &[&str]) -> anyhow::Result<Vec<u8>> {
        let started = std::time::Instant::now();
        let out = cmd
            .output()
            .await
            .context("run git (is it installed and on PATH?)")?;
        let ms = started.elapsed().as_millis() as u64;
        tracing::debug!(ms, "git {}", args.join(" "));
        if !out.status.success() {
            bail!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(out.stdout)
    }

    async fn exec(&self, args: &[&str]) -> anyhow::Result<Vec<u8>> {
        self.output(self.command(args), args).await
    }

    async fn read(&self, args: &[&str]) -> anyhow::Result<String> {
        Ok(String::from_utf8_lossy(&self.exec(args).await?).into_owned())
    }

    /// Runs git with `input` on its stdin.
    async fn exec_with_input(&self, args: &[&str], input: &[u8]) -> anyhow::Result<Vec<u8>> {
        let mut cmd = self.command(args);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd
            .spawn()
            .context("run git (is it installed and on PATH?)")?;
        let mut stdin = child.stdin.take().context("git's stdin")?;
        let input = input.to_vec();
        let writer = tokio::spawn(async move {
            let _ = stdin.write_all(&input).await;
        });
        let out = child.wait_with_output().await?;
        let _ = writer.await;
        if !out.status.success() {
            bail!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(out.stdout)
    }

    /// The commit `rev` names, or `None` when it names none.
    async fn commit(&self, rev: &str) -> Option<String> {
        let spec = format!("{rev}^{{commit}}");
        let out = self
            .read(&[
                "rev-parse",
                "--verify",
                "--quiet",
                "--end-of-options",
                &spec,
            ])
            .await
            .ok()?;
        Some(out.trim().to_string()).filter(|s| !s.is_empty())
    }

    /// The commit `HEAD` is on, `None` before the first commit.
    async fn head(&self) -> Option<String> {
        self.commit("HEAD").await
    }

    /// A commit of the working tree as it is, untracked files too, on `HEAD`. Leaves the
    /// index and refs alone: it's built in an index of its own.
    pub async fn snapshot(&self) -> anyhow::Result<String> {
        let index_path = self.read(&["rev-parse", "--git-path", "index"]).await?;
        let index = self.root.join(index_path.trim());
        let n = INDEXES.fetch_add(1, Ordering::Relaxed);
        let temp = std::env::temp_dir().join(format!("lgtm-index-{}-{n}", std::process::id()));
        // A copy of the real index keeps git from reading every file again.
        if std::fs::copy(&index, &temp).is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        let tree = async {
            let mut add = self.command(&["add", "--all", "--", "."]);
            add.env("GIT_INDEX_FILE", &temp);
            self.output(add, &["add", "--all"]).await?;
            let mut write = self.command(&["write-tree"]);
            write.env("GIT_INDEX_FILE", &temp);
            let out = self.output(write, &["write-tree"]).await?;
            anyhow::Ok(String::from_utf8_lossy(&out).trim().to_string())
        }
        .await;
        let _ = std::fs::remove_file(&temp);
        let tree = tree?;
        let head = self.head().await;
        let mut args = vec!["commit-tree", tree.as_str(), "-m", "working copy"];
        if let Some(head) = &head {
            args.extend(["-p", head.as_str()]);
        }
        let mut cmd = self.command(&args);
        cmd.envs(SNAPSHOT_ENV);
        let out = self.output(cmd, &args).await?;
        Ok(String::from_utf8_lossy(&out).trim().to_string())
    }

    /// The paths with conflicts in the working tree, as of a merge or rebase stopping.
    pub async fn conflicted_files(&self) -> anyhow::Result<Vec<String>> {
        let out = self
            .read(&["diff", "--name-only", "--diff-filter=U", "-z"])
            .await?;
        Ok(out
            .split('\0')
            .filter(|p| !p.is_empty())
            .map(String::from)
            .collect())
    }

    /// Remote names and URLs.
    pub async fn remotes(&self) -> anyhow::Result<Vec<(String, String)>> {
        let out = self
            .read(&["config", "--get-regexp", r"^remote\..*\.url$"])
            .await
            .unwrap_or_default();
        Ok(out
            .lines()
            .filter_map(|l| {
                let (key, url) = l.split_once(' ')?;
                let name = key.strip_prefix("remote.")?.strip_suffix(".url")?;
                Some((name.to_string(), url.trim().to_string()))
            })
            .collect())
    }

    /// The commits `rev` names: one, or none when it names nothing.
    pub async fn commit_ids(&self, rev: &str) -> anyhow::Result<Vec<String>> {
        let rev = self.rev(rev).await?;
        Ok(self.commit(&rev).await.into_iter().collect())
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::Path;

    use super::Git;

    pub fn git(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .current_dir(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    /// A repo on branch `main` with one commit of `a.txt`.
    pub fn repo() -> (tempfile::TempDir, Git) {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main"]);
        write(dir.path(), "a.txt", "a\n");
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-q", "-m", "first"]);
        let g = Git::new(dir.path());
        (dir, g)
    }

    pub fn write(dir: &Path, path: &str, text: &str) {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{git, repo, write};

    #[tokio::test]
    async fn the_working_copy_commit_holds_untracked_edits_and_leaves_the_index_alone() {
        let (dir, g) = repo();
        write(dir.path(), "a.txt", "changed\n");
        write(dir.path(), "new.txt", "new\n");
        let snap = g.snapshot().await.unwrap();
        assert_eq!(
            git(dir.path(), &["show", &format!("{snap}:new.txt")]),
            "new"
        );
        assert_eq!(
            git(dir.path(), &["show", &format!("{snap}:a.txt")]),
            "changed"
        );
        assert_eq!(git(dir.path(), &["diff", "--cached", "--name-only"]), "");
        assert_eq!(g.snapshot().await.unwrap(), snap);
    }

    #[tokio::test]
    async fn ignored_files_stay_out_of_the_working_copy_commit() {
        let (dir, g) = repo();
        write(dir.path(), ".gitignore", "*.log\n");
        write(dir.path(), "x.log", "noise\n");
        let snap = g.snapshot().await.unwrap();
        let files = git(dir.path(), &["ls-tree", "--name-only", &snap]);
        assert_eq!(files.lines().collect::<Vec<_>>(), [".gitignore", "a.txt"]);
    }
}
