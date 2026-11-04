//! `lgtm snap <label>` starts a step named `label`. It ends the agent's running turn in the
//! workspace and starts one named for the step, or takes a checkpoint named `label` when no
//! agent is working. Both apps list it with the workspace's turns.

use std::path::Path;
use std::sync::Mutex;

use lgtm_core::jj::Jj;
use lgtm_core::turns;

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
    match block_on(start(&root, label)) {
        Ok(commit) => {
            println!("{} {label}", &commit[..commit.len().min(8)]);
            0
        }
        Err(e) => {
            eprintln!("lgtm: {e:#}");
            1
        }
    }
}

/// Starts step `label` in the workspace at `root`. Returns the commit it starts from.
pub async fn start(root: &Path, label: &str) -> anyhow::Result<String> {
    let store =
        crate::tui::store(root).ok_or_else(|| anyhow::anyhow!("couldn't open the repo's store"))?;
    let store = Mutex::new(store);
    let jj = Jj::new(root);
    let key = turns::workspace_key(&jj, root).await?;
    let (change, commit) = turns::at(&jj).await?;
    turns::start_step(&store, &jj, &key, (&change, &commit), label).await?;
    Ok(commit)
}

pub fn block_on<T>(f: impl std::future::Future<Output = anyhow::Result<T>>) -> anyhow::Result<T> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(f)
}
