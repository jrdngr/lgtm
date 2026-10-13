//! Agent prompts the user can edit: a markdown file in the repo's `.lgtm`, else in the config
//! dir, else the built-in default. The result format each agent returns stays fixed in code.

use std::path::{Path, PathBuf};

use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::bots::personas::{Scope, config_dir, is_plain_file};

/// An editable prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Prompt {
    Walkthrough,
    /// How takes and replies on GitHub threads are written.
    Reply,
}

impl Prompt {
    pub const ALL: [Prompt; 2] = [Prompt::Walkthrough, Prompt::Reply];

    fn file(self) -> &'static str {
        match self {
            Prompt::Walkthrough => "walkthrough.md",
            Prompt::Reply => "reply.md",
        }
    }

    pub fn default_text(self) -> &'static str {
        match self {
            Prompt::Walkthrough => include_str!("walkthrough.md"),
            Prompt::Reply => include_str!("reply.md"),
        }
    }
}

/// A prompt and its files, for the Settings tab.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PromptInfo {
    pub prompt: Prompt,
    /// The file in use, `None` for the built-in prompt.
    pub in_use: Option<Scope>,
    pub global: Option<String>,
    pub repo: String,
    pub global_exists: bool,
    pub repo_exists: bool,
}

/// Where `scope`'s file goes. `config` is lgtm's config dir.
fn path(
    prompt: Prompt,
    root: &Path,
    config: Option<&Path>,
    scope: Scope,
) -> anyhow::Result<PathBuf> {
    match scope {
        Scope::Global => Ok(config.context("no home directory")?.join(prompt.file())),
        Scope::Repo => Ok(root.join(".lgtm").join(prompt.file())),
    }
}

fn find(prompt: Prompt, root: &Path, config: Option<&Path>) -> Option<(Scope, PathBuf)> {
    [Scope::Repo, Scope::Global].into_iter().find_map(|scope| {
        let path = path(prompt, root, config, scope).ok()?;
        is_plain_file(&path).then_some((scope, path))
    })
}

fn load_in(prompt: Prompt, root: &Path, config: Option<&Path>) -> anyhow::Result<String> {
    let Some((_, path)) = find(prompt, root, config) else {
        return Ok(prompt.default_text().to_string());
    };
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("can't read the prompt {}", path.display()))?;
    ensure!(
        !text.trim().is_empty(),
        "the prompt {} is empty",
        path.display()
    );
    Ok(text)
}

/// The prompt to give the agent.
pub fn load(prompt: Prompt, root: &Path) -> anyhow::Result<String> {
    load_in(prompt, root, config_dir().as_deref())
}

fn info_in(prompt: Prompt, root: &Path, config: Option<&Path>) -> PromptInfo {
    let global = path(prompt, root, config, Scope::Global).ok();
    let repo = root.join(".lgtm").join(prompt.file());
    PromptInfo {
        prompt,
        in_use: find(prompt, root, config).map(|(scope, _)| scope),
        global_exists: global.as_deref().is_some_and(is_plain_file),
        repo_exists: is_plain_file(&repo),
        global: global.map(|p| p.to_string_lossy().into_owned()),
        repo: repo.to_string_lossy().into_owned(),
    }
}

/// Every prompt and which of its files is in use.
pub fn infos(root: &Path) -> Vec<PromptInfo> {
    let config = config_dir();
    Prompt::ALL
        .into_iter()
        .map(|p| info_in(p, root, config.as_deref()))
        .collect()
}

fn edit_in(
    prompt: Prompt,
    root: &Path,
    config: Option<&Path>,
    scope: Scope,
) -> anyhow::Result<PathBuf> {
    let path = path(prompt, root, config, scope)?;
    if !path.exists() {
        std::fs::create_dir_all(path.parent().context("prompt path has no parent")?)?;
        std::fs::write(&path, prompt.default_text())?;
    }
    Ok(path)
}

/// `scope`'s prompt file, written from the default first when there isn't one.
pub fn edit(prompt: Prompt, root: &Path, scope: Scope) -> anyhow::Result<PathBuf> {
    edit_in(prompt, root, config_dir().as_deref(), scope)
}

fn remove_in(
    prompt: Prompt,
    root: &Path,
    config: Option<&Path>,
    scope: Scope,
) -> anyhow::Result<()> {
    let path = path(prompt, root, config, scope)?;
    if is_plain_file(&path) {
        std::fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

/// Deletes `scope`'s prompt file, so the next one down is used.
pub fn remove(prompt: Prompt, root: &Path, scope: Scope) -> anyhow::Result<()> {
    remove_in(prompt, root, config_dir().as_deref(), scope)
}

fn is_prompt_file_in(root: &Path, config: Option<&Path>, file: &Path) -> bool {
    is_plain_file(file)
        && Prompt::ALL.into_iter().any(|prompt| {
            [Scope::Repo, Scope::Global]
                .into_iter()
                .any(|s| path(prompt, root, config, s).is_ok_and(|p| p == file))
        })
}

/// Whether `file` is a prompt file, which the app opens for editing like a persona.
pub fn is_prompt_file(root: &Path, file: &Path) -> bool {
    is_prompt_file_in(root, config_dir().as_deref(), file)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: Prompt = Prompt::Walkthrough;

    #[test]
    fn the_repo_prompt_wins_then_the_global_one_then_the_default() {
        let repo = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        let (root, config) = (repo.path(), Some(config.path()));
        assert_eq!(load_in(W, root, config).unwrap(), W.default_text());

        let global = edit_in(W, root, config, Scope::Global).unwrap();
        std::fs::write(&global, "global").unwrap();
        assert_eq!(load_in(W, root, config).unwrap(), "global");

        let local = edit_in(W, root, config, Scope::Repo).unwrap();
        assert_eq!(std::fs::read_to_string(&local).unwrap(), W.default_text());
        std::fs::write(&local, "repo").unwrap();
        assert_eq!(load_in(W, root, config).unwrap(), "repo");
        assert_eq!(info_in(W, root, config).in_use, Some(Scope::Repo));

        // Opening it again keeps the edits.
        edit_in(W, root, config, Scope::Repo).unwrap();
        assert_eq!(load_in(W, root, config).unwrap(), "repo");

        std::fs::write(&local, "\n").unwrap();
        assert!(load_in(W, root, config).is_err());

        // Removing the repo file falls back to the global one, then the default.
        remove_in(W, root, config, Scope::Repo).unwrap();
        assert_eq!(load_in(W, root, config).unwrap(), "global");
        remove_in(W, root, config, Scope::Global).unwrap();
        let info = info_in(W, root, config);
        assert_eq!(
            (info.in_use, info.global_exists, info.repo_exists),
            (None, false, false)
        );
    }

    #[test]
    fn only_prompt_files_open_for_editing() {
        let repo = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        let (root, config) = (repo.path(), Some(config.path()));
        let local = edit_in(W, root, config, Scope::Repo).unwrap();
        let global = edit_in(W, root, config, Scope::Global).unwrap();
        assert!(is_prompt_file_in(root, config, &local));
        assert!(is_prompt_file_in(root, config, &global));
        std::fs::write(root.join("walkthrough.md"), "").unwrap();
        assert!(!is_prompt_file_in(
            root,
            config,
            &root.join("walkthrough.md")
        ));

        #[cfg(unix)]
        {
            std::fs::remove_file(&local).unwrap();
            std::fs::write(root.join("run.command"), "").unwrap();
            std::os::unix::fs::symlink(root.join("run.command"), &local).unwrap();
            assert!(!is_prompt_file_in(root, config, &local));
            // A symlinked prompt isn't used either.
            assert_eq!(find(W, root, config).unwrap().0, Scope::Global);
        }
    }
}
