//! Loads a review the way the app does, without its screen.

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use lgtm_core::repo::Repo;
use lgtm_core::store::Store;

use crate::open_review::Shown;
use crate::tui::load::{Loader, Msg, Target};
use crate::tui::review::Review;

/// How long a review may take to load.
const TIMEOUT: Duration = Duration::from_secs(300);

/// The review `-r` names in the workspace at `root`: `revset`, else the one lgtm shows there,
/// else `@`. A revset the open review goes by brings its versions and PR along.
pub fn shown(root: &Path, revset: Option<String>) -> Shown {
    let open = crate::open_review::folder(root).and_then(|f| crate::open_review::latest(&f));
    match (revset, open) {
        (None, Some(open)) => open,
        (Some(r), Some(open)) if r == open.revset => open,
        (r, _) => Shown {
            revset: r.unwrap_or_else(|| "@".into()),
            between: None,
            pr: None,
        },
    }
}

/// Loads `shown` in the workspace at `root`, with its marks and comments from `store`.
pub fn review(root: &Path, shown: Shown, store: Option<&Store>) -> anyhow::Result<Review> {
    let target = match shown.between {
        Some((from, to)) => Target::Between {
            from,
            to,
            change: String::new(),
            span: String::new(),
        },
        None => Target::Revset(shown.revset.clone()),
    };
    let (tx, rx) = mpsc::channel();
    let loader = Loader::new(Repo::open(root), tx)?;
    loader.review(1, target.clone());
    let mut review = Review::new(1, target);
    review.revset = shown.revset;
    if let Some(pr) = shown.pr {
        review.set_pr(pr);
    }
    loop {
        match rx.recv_timeout(TIMEOUT)? {
            Msg::Diff { result, .. } => {
                review.loaded(Ok(result?));
                if let Some(store) = store {
                    review.load_comments(store);
                }
            }
            Msg::Built { files, cfg, .. } => {
                review.built(files, cfg, store);
                return Ok(review);
            }
            _ => {}
        }
    }
}
