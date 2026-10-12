//! The walkthrough prompt: `walkthrough.md` in the repo's `.lgtm`, else in the config dir, else
//! the built-in default. The note format the agent returns stays fixed in code.

use std::path::{Path, PathBuf};

use anyhow::{Context, ensure};
use serde::Serialize;
use specta::Type;

use crate::bots::personas::{Scope, config_dir, is_plain_file};

pub const DEFAULT: &str = include_str!("default.md");

const FILE: &str = "walkthrough.md";

/// The prompt file a walkthrough uses.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PromptFile {
    pub scope: Scope,
    pub path: String,
}

/// Where `scope`'s prompt file goes. `config` is lgtm's config dir.
fn path(root: &Path, config: Option<&Path>, scope: Scope) -> anyhow::Result<PathBuf> {
    match scope {
        Scope::Global => Ok(config.context("no home directory")?.join(FILE)),
        Scope::Repo => Ok(root.join(".lgtm").join(FILE)),
    }
}

fn find(root: &Path, config: Option<&Path>) -> Option<(Scope, PathBuf)> {
    [Scope::Repo, Scope::Global].into_iter().find_map(|scope| {
        let path = path(root, config, scope).ok()?;
        is_plain_file(&path).then_some((scope, path))
    })
}

fn load_in(root: &Path, config: Option<&Path>) -> anyhow::Result<String> {
    let Some((_, path)) = find(root, config) else {
        return Ok(DEFAULT.to_string());
    };
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("can't read the walkthrough prompt {}", path.display()))?;
    ensure!(
        !text.trim().is_empty(),
        "the walkthrough prompt {} is empty",
        path.display()
    );
    Ok(text)
}

/// The prompt to give the agent.
pub fn load(root: &Path) -> anyhow::Result<String> {
    load_in(root, config_dir().as_deref())
}

/// The file in use, or `None` for the built-in prompt.
pub fn in_use(root: &Path) -> Option<PromptFile> {
    find(root, config_dir().as_deref()).map(|(scope, path)| PromptFile {
        scope,
        path: path.to_string_lossy().into_owned(),
    })
}

fn edit_in(root: &Path, config: Option<&Path>, scope: Scope) -> anyhow::Result<PathBuf> {
    let path = path(root, config, scope)?;
    if !path.exists() {
        std::fs::create_dir_all(path.parent().context("prompt path has no parent")?)?;
        std::fs::write(&path, DEFAULT)?;
    }
    Ok(path)
}

/// `scope`'s prompt file, written from the default first when there isn't one.
pub fn edit(root: &Path, scope: Scope) -> anyhow::Result<PathBuf> {
    edit_in(root, config_dir().as_deref(), scope)
}

fn is_prompt_file_in(root: &Path, config: Option<&Path>, file: &Path) -> bool {
    is_plain_file(file)
        && [Scope::Repo, Scope::Global]
            .into_iter()
            .any(|s| path(root, config, s).is_ok_and(|p| p == file))
}

/// Whether `file` is a walkthrough prompt, which the app opens for editing like a persona.
pub fn is_prompt_file(root: &Path, file: &Path) -> bool {
    is_prompt_file_in(root, config_dir().as_deref(), file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_repo_prompt_wins_then_the_global_one_then_the_default() {
        let repo = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        let (root, config) = (repo.path(), Some(config.path()));
        assert_eq!(load_in(root, config).unwrap(), DEFAULT);

        let global = edit_in(root, config, Scope::Global).unwrap();
        std::fs::write(&global, "global").unwrap();
        assert_eq!(load_in(root, config).unwrap(), "global");

        let local = edit_in(root, config, Scope::Repo).unwrap();
        assert_eq!(std::fs::read_to_string(&local).unwrap(), DEFAULT);
        std::fs::write(&local, "repo").unwrap();
        assert_eq!(load_in(root, config).unwrap(), "repo");
        assert_eq!(find(root, config).unwrap().0, Scope::Repo);

        // Opening it again keeps the edits.
        edit_in(root, config, Scope::Repo).unwrap();
        assert_eq!(load_in(root, config).unwrap(), "repo");

        std::fs::write(&local, "\n").unwrap();
        assert!(load_in(root, config).is_err());
    }

    #[test]
    fn only_the_two_prompt_files_open_for_editing() {
        let repo = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        let (root, config) = (repo.path(), Some(config.path()));
        let local = edit_in(root, config, Scope::Repo).unwrap();
        let global = edit_in(root, config, Scope::Global).unwrap();
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
            assert_eq!(find(root, config).unwrap().0, Scope::Global);
        }
    }
}
