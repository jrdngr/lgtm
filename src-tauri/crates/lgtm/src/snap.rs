//! `lgtm snap <label>`: starts a step named `label`. It snapshots the working copy and labels
//! the snapshot, so the review picker lists it under its change and opens the edits after it.

use lgtm_core::jj::Jj;

pub fn run(label: &[String]) -> i32 {
    let label = label.join(" ");
    let label = label.trim();
    if label.is_empty() {
        eprintln!("usage: lgtm snap <label>");
        return 2;
    }
    let Some(root) = std::env::current_dir()
        .ok()
        .and_then(|d| crate::agent::workspace(&d))
    else {
        eprintln!("lgtm: not in a jj repo");
        return 1;
    };
    let Some(store) = crate::tui::store(&root) else {
        eprintln!("lgtm: couldn't open the repo's store");
        return 1;
    };
    let commit = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(anyhow::Error::from)
        .and_then(|rt| {
            rt.block_on(async {
                let jj = Jj::new(&root);
                jj.snapshot().await?;
                let ids = jj.commit_ids("@").await?;
                ids.into_iter()
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("no working-copy commit"))
            })
        });
    let commit = match commit {
        Ok(c) => c,
        Err(e) => {
            eprintln!("lgtm: {e:#}");
            return 1;
        }
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    match store.label_snapshot(&commit, label, now) {
        Ok(()) => {
            println!("{} {label}", &commit[..commit.len().min(8)]);
            0
        }
        Err(e) => {
            eprintln!("lgtm: {e:#}");
            1
        }
    }
}
