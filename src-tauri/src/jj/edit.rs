//! Commands from the revisions panel that change the repo.

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
}

impl JjAction {
    fn args(&self) -> Vec<&str> {
        match self {
            JjAction::Edit { rev } => vec!["edit", rev],
            JjAction::Undo { .. } => vec!["undo"],
            JjAction::Redo { .. } => vec!["redo"],
        }
    }
}

impl Jj {
    /// Runs `action` and returns how many jj operations it took.
    pub async fn act(&self, action: &JjAction) -> anyhow::Result<u32> {
        let steps = match action {
            JjAction::Undo { steps } | JjAction::Redo { steps } => *steps,
            JjAction::Edit { .. } => 1,
        };
        let mut seen = self.writer.lock().await;
        let done = async {
            for _ in 0..steps {
                self.exec(&action.args(), false).await?;
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
