//! Where the working copy sits against a review.

use anyhow::Context;

use super::Jj;

/// Where `@` is: in the review, right on top of it, or elsewhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum AtPlace {
    Inside,
    OnTop,
    Outside,
}

/// The workspace's working-copy commit, and where it sits against a review.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkingCopyDto {
    pub change_id: String,
    pub commit_id: String,
    /// Length of the shortest prefix of `change_id` that's unique in the repo.
    pub unique: u32,
    pub empty: bool,
    pub description: String,
    pub place: AtPlace,
}

impl Jj {
    /// `@`, and whether it's in `revset`, sits right on it, or neither. One read: `@`'s row
    /// says which of the two it's in, and any other row is a commit between them.
    pub async fn working_copy(&self, revset: &str) -> anyhow::Result<WorkingCopyDto> {
        let within = |r: &str| template_literal(r);
        let template = format!(
            r#"if(self.contained_in({}), "in", if(self.contained_in({}), "on", "out")) ++ "\t" ++ {}"#,
            within(revset),
            within(&format!("({revset})::")),
            super::CHANGE_TEMPLATE,
        );
        let rows = format!("@ | (({revset})::@ ~ ({revset}) ~ @)");
        let out = self
            .read(&["log", "--no-graph", "-r", &rows, "-T", &template])
            .await?;
        let rows: Vec<(&str, super::Change)> = out
            .lines()
            .filter_map(|l| {
                let (place, rest) = l.split_once('\t')?;
                Some((place, super::parse_changes(rest).pop()?))
            })
            .collect();
        let (place, at) = rows
            .iter()
            .find(|(_, c)| c.working_copy)
            .context("no working copy")?;
        let place = match *place {
            "in" => AtPlace::Inside,
            "on" if rows.len() == 1 => AtPlace::OnTop,
            _ => AtPlace::Outside,
        };
        let at = at.clone();
        Ok(WorkingCopyDto {
            change_id: at.change_id,
            commit_id: at.commit_id,
            unique: at.unique,
            empty: at.empty,
            description: at.description,
            place,
        })
    }
}

/// `text` as a jj template string literal.
fn template_literal(text: &str) -> String {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::AtPlace;
    use crate::jj::test_support::{jj, repo, write};

    #[tokio::test]
    async fn places_the_working_copy_against_a_review() {
        let (dir, j) = repo();
        let d = dir.path();
        write(d, "a.txt", "a\n");
        jj(d, &["commit", "-m", "one"]);
        write(d, "b.txt", "b\n");
        jj(d, &["commit", "-m", "two"]);
        jj(d, &["bookmark", "create", "pr", "-r", "@-"]);
        let place = async |r: &str| j.working_copy(r).await.unwrap().place;

        assert_eq!(place("root()..pr").await, AtPlace::OnTop);
        assert_eq!(place("@").await, AtPlace::Inside);
        // A revset with quotes in it.
        assert_eq!(
            place(r#"description(substring:"two")"#).await,
            AtPlace::OnTop
        );
        jj(d, &["edit", "pr-"]);
        assert_eq!(place("root()..pr").await, AtPlace::Inside);
        assert_eq!(place("pr").await, AtPlace::Outside);
    }

    #[tokio::test]
    async fn commits_between_put_the_working_copy_outside() {
        let (dir, j) = repo();
        let d = dir.path();
        write(d, "a.txt", "a\n");
        jj(d, &["commit", "-m", "reviewed"]);
        jj(d, &["bookmark", "create", "reviewed", "-r", "@-"]);
        let place = async || j.working_copy("reviewed").await.unwrap().place;

        assert_eq!(place().await, AtPlace::OnTop);
        write(d, "b.txt", "b\n");
        jj(d, &["commit", "-m", "between"]);
        assert_eq!(place().await, AtPlace::Outside);
    }
}
