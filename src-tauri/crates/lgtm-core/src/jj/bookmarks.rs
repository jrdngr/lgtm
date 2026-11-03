//! Bookmarks a revision could take over.

use super::Jj;

/// A local bookmark and the change it's on.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkSpot {
    pub name: String,
    /// The change id, short.
    pub change: String,
    /// Length of the shortest prefix of `change` that's unique in the repo.
    pub unique: u32,
}

/// The closest mutable bookmarked commits below and above `rev`, not counting `rev` itself.
fn nearest(rev: &str) -> String {
    format!(
        "heads(::({rev})- & mutable() & bookmarks()) | roots(({rev})+:: & mutable() & bookmarks())"
    )
}

const TEMPLATE: &str = r#"change_id.short(8) ++ "\t" ++ change_id.shortest().prefix().len() ++ "\t"
  ++ local_bookmarks.map(|b| b.name()).join("\x1e") ++ "\n""#;

impl Jj {
    /// The bookmarks closest to `rev` on either side, newest first.
    pub async fn nearest_bookmarks(&self, rev: &str) -> anyhow::Result<Vec<BookmarkSpot>> {
        let revset = nearest(rev);
        let out = self
            .read(&["log", "--no-graph", "-r", &revset, "-T", TEMPLATE])
            .await?;
        Ok(parse(&out))
    }
}

fn parse(out: &str) -> Vec<BookmarkSpot> {
    let mut spots = Vec::new();
    for line in out.lines() {
        let mut f = line.split('\t');
        let (Some(change), Some(unique), Some(names)) = (f.next(), f.next(), f.next()) else {
            continue;
        };
        let unique = unique.parse().unwrap_or(change.len() as u32);
        for name in names.split('\x1e').filter(|n| !n.is_empty()) {
            spots.push(BookmarkSpot {
                name: name.into(),
                change: change.into(),
                unique,
            });
        }
    }
    spots
}

#[cfg(test)]
mod tests {
    use crate::jj::JjAction;
    use crate::jj::test_support::{jj, repo};

    #[tokio::test]
    async fn nearest_bookmarks_are_the_closest_on_each_side_set_moves_one_and_create_wants_a_new_name()
     {
        let (dir, j) = repo();
        let d = dir.path();
        let j = &j;
        for (msg, bookmark) in [("a", Some("far")), ("b", Some("below")), ("c", None)] {
            jj(d, &["describe", "-m", msg]);
            if let Some(b) = bookmark {
                jj(d, &["bookmark", "create", b, "-r", "@"]);
            }
            jj(d, &["new"]);
        }
        jj(d, &["describe", "-m", "d"]);
        jj(d, &["bookmark", "create", "above", "-r", "@"]);
        let c = j.changes("@-").await.unwrap()[0].change_id.clone();

        let mut names: Vec<_> = j.nearest_bookmarks(&c).await.unwrap();
        names.sort_by(|a, b| a.name.cmp(&b.name));
        let names: Vec<_> = names.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["above", "below"]);

        for name in ["above", "below"] {
            let set = JjAction::BookmarkSet {
                name: name.into(),
                rev: c.clone(),
            };
            j.act(&set).await.unwrap();
            assert_eq!(j.changes(name).await.unwrap()[0].change_id, c);
        }

        let create = |name: &str| JjAction::BookmarkCreate {
            name: name.into(),
            rev: c.clone(),
        };
        j.act(&create("fresh")).await.unwrap();
        assert_eq!(j.changes("fresh").await.unwrap()[0].change_id, c);
        assert!(j.act(&create("far")).await.is_err());
    }
}
