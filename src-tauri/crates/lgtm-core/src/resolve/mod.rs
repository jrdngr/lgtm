//! Resolving conflicts: reading jj's markers, picking sides, and writing the file back. The
//! app and the terminal draw the same session.

mod merge;
mod parse;
mod session;
mod tint;
mod view;

use std::path::Path;

pub use parse::Label;
pub use session::{Choice, Fate, Pick, Saved, Session};
pub use tint::Tint;
pub use view::{ConflictView, PartView, ResolveView, SideView};

use crate::jj::Jj;

/// Whether `text` has a conflict to resolve.
pub fn has_conflicts(text: &str) -> bool {
    !parse::parse(text).conflicts.is_empty()
}

/// A session on `path` in `root`, its labels filled in from jj when `jj` is given and knows
/// the commits. jj failing leaves the markers' own labels.
pub async fn load(root: &Path, path: &str, jj: Option<&Jj>) -> std::io::Result<Session> {
    let text = std::fs::read_to_string(root.join(path))?;
    let mut session = Session::new(path, &text);
    if let Some(jj) = jj {
        fill(&mut session, jj).await;
    }
    Ok(session)
}

/// Fills in each label's bookmarks and unique prefix, and what a side did to the file.
pub async fn fill(session: &mut Session, jj: &Jj) {
    let mut ids: Vec<String> = session
        .labels_mut()
        .map(|l| l.commit_id.clone())
        .filter(|id| !id.is_empty())
        .collect();
    ids.sort();
    ids.dedup();
    if let Ok(found) = jj.marker_commits(&ids).await {
        for label in session.labels_mut() {
            let id = &label.commit_id;
            if let Some(c) = found
                .iter()
                .find(|c| !id.is_empty() && c.commit_id.starts_with(id.as_str()))
            {
                label.unique = c.unique;
                label.bookmarks = c.bookmarks.clone();
            }
        }
    }
    let Some(first) = session.conflicts().first() else {
        return;
    };
    // A file outside the workspace, as a merge tool's, is no commit's to delete or move.
    if Path::new(&session.path).is_absolute() {
        return;
    }
    let base = first.base.label.commit_id.clone();
    let sides: Vec<(u8, Label)> = first
        .sides
        .iter()
        .enumerate()
        .map(|(i, s)| (i as u8 + 1, s.label.clone()))
        .collect();
    let mut gone = vec![];
    let mut fate = Fate::Here;
    for (key, label) in sides {
        if label.commit_id.is_empty() {
            continue;
        }
        let path = session.path.clone();
        if jj.has_file(&label.commit_id, &path).await.unwrap_or(true) {
            continue;
        }
        gone.push(key);
        let moved = match base.is_empty() {
            true => None,
            false => jj
                .moved_to(&base, &label.commit_id, &path)
                .await
                .ok()
                .flatten(),
        };
        if fate == Fate::Here {
            fate = match moved {
                Some(to) => Fate::Moved { to, by: label },
                None => Fate::Deleted { by: label },
            };
        }
    }
    session.set_fate(fate, gone);
}
