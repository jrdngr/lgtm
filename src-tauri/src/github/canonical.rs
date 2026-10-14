//! GitHub's current name for each remote's repo, so a renamed or transferred repo still
//! matches its PRs. Looked up once per repo per run.

use std::collections::HashMap;
use std::sync::Mutex;

use super::{GhRepo, gh, remote};
use crate::jj::Jj;

static NAMES: Mutex<Option<HashMap<String, GhRepo>>> = Mutex::new(None);

/// The repo's current name, or `repo` itself when GitHub can't say.
pub async fn current(repo: &GhRepo) -> GhRepo {
    let key = repo.to_string().to_ascii_lowercase();
    if let Some(r) = NAMES.lock().unwrap().get_or_insert_default().get(&key) {
        return r.clone();
    }
    let found = gh::rest(&format!("repos/{repo}"))
        .await
        .ok()
        .and_then(|v| Some(v["full_name"].as_str()?.to_string()))
        .and_then(|full| {
            let (owner, name) = full.split_once('/')?;
            Some(GhRepo {
                owner: owner.to_string(),
                name: name.to_string(),
            })
        })
        .unwrap_or_else(|| repo.clone());
    NAMES
        .lock()
        .unwrap()
        .get_or_insert_default()
        .insert(key, found.clone());
    found
}

/// The repo's remotes, with each GitHub URL naming its repo as GitHub does now.
pub async fn remotes(jj: &Jj) -> anyhow::Result<Vec<(String, String)>> {
    let mut out = jj.remotes().await?;
    for (_, url) in &mut out {
        let Some(repo) = remote::parse_url(url).or_else(|| remote::any_repo(url)) else {
            continue;
        };
        let now = current(&repo).await;
        if now != repo {
            *url = remote::url_like(url, &now);
        }
    }
    Ok(out)
}
