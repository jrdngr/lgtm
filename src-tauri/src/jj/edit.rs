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
    NewAfter {
        rev: String,
    },
    NewBefore {
        rev: String,
    },
    /// Squash into the parent, keeping the parent's description so no editor opens. Squashing
    /// `@` leaves `@` on the parent rather than on a new empty commit.
    Squash {
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
            JjAction::NewAfter { rev } => vec!["new", rev],
            JjAction::NewBefore { rev } => vec!["new", "--insert-before", rev],
            JjAction::Squash { rev } => vec!["squash", "--use-destination-message", "-r", rev],
            JjAction::Undo { .. } => vec!["undo"],
            JjAction::Redo { .. } => vec!["redo"],
        }
    }
}

impl Jj {
    /// Runs `action` and returns how many jj operations it took.
    pub async fn act(&self, action: &JjAction) -> anyhow::Result<u32> {
        let parent = match action {
            JjAction::Squash { rev } if self.includes_working_copy(rev).await? => {
                let parents = self.changes(&format!("({rev})-")).await?;
                anyhow::ensure!(parents.len() == 1, "{rev} has more than one parent");
                Some(parents[0].change_id.clone())
            }
            _ => None,
        };
        let steps = match action {
            JjAction::Undo { steps } | JjAction::Redo { steps } => *steps,
            _ => 1,
        };
        let _guard = self.writer.lock().await;
        for _ in 0..steps {
            self.exec(&action.args(), false).await?;
        }
        if let Some(parent) = parent {
            // jj leaves `@` on a new empty commit. Moving to the parent abandons it.
            self.exec(&["edit", &parent], false).await?;
            return Ok(2);
        }
        Ok(steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::tests::{jj, write};

    async fn change_id(j: &Jj, rev: &str) -> String {
        j.changes(rev).await.unwrap()[0].change_id.clone()
    }

    #[tokio::test]
    async fn actions_move_the_working_copy_and_squash() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        std::process::Command::new("jj")
            .args(["git", "init"])
            .arg(d)
            .output()
            .unwrap();
        let j = Jj::new(d);
        write(d, "a.txt", "a\n");
        jj(d, &["describe", "-m", "base"]);
        let base = j.commit_ids("@").await.unwrap();
        let base_change = change_id(&j, "@").await;

        j.act(&JjAction::NewAfter {
            rev: base_change.clone(),
        })
        .await
        .unwrap();
        write(d, "b.txt", "b\n");
        j.snapshot().await.unwrap();
        let top = change_id(&j, "@").await;
        assert_eq!(j.act(&JjAction::Squash { rev: top }).await.unwrap(), 2);
        assert_eq!(change_id(&j, "@").await, base_change);
        let base_now = j.changes(&base_change).await.unwrap();
        assert_eq!(base_now[0].description, "base");
        assert_ne!(j.commit_ids(&base_change).await.unwrap(), base);

        j.act(&JjAction::Undo { steps: 2 }).await.unwrap();
        assert_eq!(j.changes(&base_change).await.unwrap()[0].commit_id, base[0]);
        assert_ne!(change_id(&j, "@").await, base_change);
        j.act(&JjAction::Redo { steps: 2 }).await.unwrap();
        assert_eq!(change_id(&j, "@").await, base_change);
        let edit = |rev: &str| JjAction::Edit {
            rev: rev.to_string(),
        };
        j.act(&JjAction::NewAfter {
            rev: base_change.clone(),
        })
        .await
        .unwrap();
        j.act(&edit(&base_change)).await.unwrap();
        assert_eq!(change_id(&j, "@").await, base_change);
    }
}
