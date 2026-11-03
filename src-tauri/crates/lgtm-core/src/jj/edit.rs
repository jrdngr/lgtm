//! Commands from the revisions log that change the repo.

use serde::Deserialize;
use specta::Type;

use super::Jj;

/// A change to the repo, on a revision given by change id.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum JjAction {
    Edit {
        rev: String,
    },
    /// Undo or redo the last `steps` operations, as many as the action being undone ran.
    Undo {
        steps: u32,
    },
    Redo {
        steps: u32,
    },
    /// A new, empty commit as `rev`'s child, before its children, and `@` on it.
    NewAfter {
        rev: String,
    },
    /// A new, empty commit as `rev`'s parent, after its parents, and `@` on it.
    NewBefore {
        rev: String,
    },
    /// Undoes what the commits of `revs` do to `paths`, in the newest of them.
    Discard {
        revs: String,
        paths: Vec<String>,
    },
    /// Squashes the commits of `revs` into the parent of the oldest, keeping its description.
    Squash {
        revs: String,
    },
    /// Rebases `rev` and its branch onto `trunk()`.
    RebaseOnTrunk {
        rev: String,
    },
    /// Moves the closest bookmarks behind `@` up to it.
    BookmarkAdvance,
    /// Makes a new bookmark `name` at `rev`. Fails when `name` is taken.
    BookmarkCreate {
        name: String,
        rev: String,
    },
    /// Sets `rev`'s description to `message`.
    Describe {
        rev: String,
        message: String,
    },
    /// Moves bookmark `name` to `rev`, backwards or sideways too.
    BookmarkSet {
        name: String,
        rev: String,
    },
    Fetch,
    /// `jj git push`. With `new_bookmarks`, those go too, by name, so they start tracking
    /// `remote`, along with what plain push sends.
    Push {
        #[serde(default)]
        remote: Option<String>,
        /// The push plan's range, which `new_bookmarks` go with.
        #[serde(default)]
        range: Option<String>,
        #[serde(default, rename = "newBookmarks")]
        new_bookmarks: Vec<String>,
    },
}

impl JjAction {
    fn args(&self) -> Vec<String> {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect();
        match self {
            JjAction::Edit { rev } => v(&["edit", rev]),
            JjAction::Undo { .. } => v(&["undo"]),
            JjAction::Redo { .. } => v(&["redo"]),
            JjAction::NewAfter { rev } => v(&["new", "--insert-after", rev]),
            JjAction::NewBefore { rev } => v(&["new", "--insert-before", rev]),
            JjAction::Discard { revs, paths } => {
                let mut args = v(&[
                    "restore",
                    "--from",
                    &format!("roots({revs})-"),
                    "--into",
                    &format!("heads({revs})"),
                    "--",
                ]);
                args.extend(paths.iter().map(|p| super::rewrite::root_file(p)));
                args
            }
            JjAction::Squash { revs } => v(&[
                "squash",
                "--from",
                revs,
                "--into",
                &format!("roots({revs})-"),
                "--use-destination-message",
            ]),
            JjAction::RebaseOnTrunk { rev } => v(&["rebase", "-b", rev, "-d", "trunk()"]),
            JjAction::BookmarkAdvance => v(&["bookmark", "advance"]),
            JjAction::BookmarkCreate { name, rev } => {
                v(&["bookmark", "create", "-r", rev, "--", name])
            }
            JjAction::Describe { rev, message } => {
                v(&["describe", "-r", rev, &format!("--message={message}")])
            }
            JjAction::BookmarkSet { name, rev } => v(&[
                "bookmark",
                "set",
                "-r",
                rev,
                "--allow-backwards",
                "--",
                name,
            ]),
            JjAction::Fetch => v(&["git", "fetch"]),
            JjAction::Push {
                remote,
                range,
                new_bookmarks,
            } => {
                let mut args = v(&["git", "push"]);
                if let (Some(remote), Some(range), false) =
                    (remote, range, new_bookmarks.is_empty())
                {
                    args.extend(["--remote".into(), format!("exact:{remote}")]);
                    args.extend(["--config".into(), super::push::remote_binding(remote)]);
                    args.extend(["-r".into(), range.clone()]);
                    for b in new_bookmarks {
                        args.extend(["-b".into(), format!("exact:{b}")]);
                    }
                }
                args
            }
        }
    }
}

impl Jj {
    /// Runs `action` and returns how many jj operations it took.
    pub async fn act(&self, action: &JjAction) -> anyhow::Result<u32> {
        let steps = match action {
            JjAction::Undo { steps } | JjAction::Redo { steps } => *steps,
            _ => 1,
        };
        let args = action.args();
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut seen = self.writer.lock().await;
        let done = async {
            for _ in 0..steps {
                self.exec(&args, false).await?;
            }
            Ok(steps)
        }
        .await;
        self.note_working_copy(&mut seen).await;
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jj::test_support::{jj, repo, write};

    async fn change_id(j: &Jj, rev: &str) -> String {
        j.changes(rev).await.unwrap()[0].change_id.clone()
    }

    #[tokio::test]
    async fn describe_sets_a_commits_description_even_one_that_starts_with_a_dash() {
        let (dir, j) = repo();
        let d = dir.path();
        let j = &j;
        jj(d, &["new"]);
        let rev = change_id(j, "@-").await;
        let describe = JjAction::Describe {
            rev: rev.clone(),
            message: "-v flag".into(),
        };
        j.act(&describe).await.unwrap();
        assert_eq!(j.changes(&rev).await.unwrap()[0].description, "-v flag");
    }

    #[tokio::test]
    async fn edit_moves_the_working_copy_and_undo_takes_it_back() {
        let (dir, j) = repo();
        let d = dir.path();
        let j = &j;
        write(d, "a.txt", "a\n");
        jj(d, &["commit", "-m", "base"]);
        let base = change_id(j, "@-").await;
        let top = change_id(j, "@").await;

        let edit = JjAction::Edit { rev: base.clone() };
        assert_eq!(j.act(&edit).await.unwrap(), 1);
        assert_eq!(change_id(j, "@").await, base);
        j.act(&JjAction::Undo { steps: 1 }).await.unwrap();
        assert_eq!(change_id(j, "@").await, top);
        j.act(&JjAction::Redo { steps: 1 }).await.unwrap();
        assert_eq!(change_id(j, "@").await, base);
    }

    #[tokio::test]
    async fn new_after_and_before_insert_around_a_commit_and_move_the_working_copy() {
        let (dir, j) = repo();
        let d = dir.path();
        let j = &j;
        jj(d, &["describe", "-m", "a"]);
        jj(d, &["new", "-m", "b"]);
        let a = change_id(j, "@-").await;
        let b = change_id(j, "@").await;

        j.act(&JjAction::NewAfter { rev: a.clone() }).await.unwrap();
        let after = change_id(j, "@").await;
        assert_eq!(change_id(j, "@-").await, a);
        assert_eq!(change_id(j, &format!("{b}-")).await, after);

        j.act(&JjAction::NewBefore { rev: a.clone() })
            .await
            .unwrap();
        assert_eq!(
            change_id(j, &format!("{a}-")).await,
            change_id(j, "@").await
        );
    }

    #[tokio::test]
    async fn squash_moves_a_range_into_the_parent_of_its_oldest_and_keeps_that_description() {
        let (dir, j) = repo();
        let d = dir.path();
        let j = &j;
        write(d, "a.txt", "a\n");
        jj(d, &["commit", "-m", "target"]);
        write(d, "b.txt", "b\n");
        jj(d, &["commit", "-m", "one"]);
        write(d, "c.txt", "c\n");
        jj(d, &["commit", "-m", "two"]);
        let target = change_id(j, "@---").await;
        let one = change_id(j, "@--").await;
        let two = change_id(j, "@-").await;

        let revs = format!("{one}::{two}");
        j.act(&JjAction::Squash { revs }).await.unwrap();
        let parent = &j.changes("@-").await.unwrap()[0];
        assert_eq!(parent.change_id, target);
        assert_eq!(parent.description.trim(), "target");
        let files = j.read(&["file", "list", "-r", "@-"]).await.unwrap();
        assert!(
            files.contains("b.txt") && files.contains("c.txt"),
            "{files}"
        );
    }

    #[tokio::test]
    async fn discard_undoes_a_range_s_changes_to_one_file_and_keeps_the_rest() {
        let (dir, j) = repo();
        let d = dir.path();
        let j = &j;
        write(d, "a.txt", "a\n");
        jj(d, &["commit", "-m", "base"]);
        write(d, "a.txt", "a2\n");
        write(d, "b.txt", "b\n");
        jj(d, &["commit", "-m", "one"]);
        write(d, "a.txt", "a3\n");
        jj(d, &["commit", "-m", "two"]);
        let one = change_id(j, "@--").await;
        let two = change_id(j, "@-").await;

        let revs = format!("{one}::{two}");
        let discard = JjAction::Discard {
            revs: revs.clone(),
            paths: vec!["a.txt".into()],
        };
        j.act(&discard).await.unwrap();
        let from = format!("roots({revs})-");
        let to = format!("heads({revs})");
        let changed = j
            .read(&["diff", "--name-only", "--from", &from, "--to", &to])
            .await
            .unwrap();
        assert_eq!(changed.trim(), "b.txt");
    }

    #[tokio::test]
    async fn bookmark_advance_moves_the_closest_bookmark_up_to_the_working_copy() {
        let (dir, j) = repo();
        let d = dir.path();
        let j = &j;
        jj(d, &["describe", "-m", "a"]);
        jj(d, &["bookmark", "create", "old", "-r", "@"]);
        jj(d, &["new", "-m", "b"]);
        jj(d, &["bookmark", "create", "near", "-r", "@"]);
        jj(d, &["new", "-m", "c"]);
        let b = change_id(j, "@-").await;

        j.act(&JjAction::BookmarkAdvance).await.unwrap();
        assert_eq!(change_id(j, "near").await, change_id(j, "@").await);
        assert_eq!(
            change_id(j, "old").await,
            change_id(j, &format!("{b}-")).await
        );
    }

    #[tokio::test]
    async fn undo_fails_with_jjs_error_when_another_workspace_made_the_last_operation() {
        let (dir, j) = repo();
        let other = tempfile::tempdir().unwrap();
        let ws = other.path().join("ws");
        jj(dir.path(), &["workspace", "add", ws.to_str().unwrap()]);
        jj(&ws, &["describe", "-m", "theirs"]);

        let err = j.act(&JjAction::Undo { steps: 1 }).await.unwrap_err();
        let err = format!("{err:#}");
        assert!(err.contains("Refusing to undo operation"), "{err}");
        assert!(!err.contains("Attempting to lock"), "{err}");
    }
}
