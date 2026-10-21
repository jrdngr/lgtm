//! Agent prompts the user can edit: a markdown file in the config dir, else the built-in
//! default. The result format each agent returns stays fixed in code.

use std::path::{Path, PathBuf};

use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::settings::config_dir;

/// An editable prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Prompt {
    /// How takes and replies on GitHub threads are written.
    Reply,
    /// How Q&A questions are built, after the qqueue skill.
    Qq,
    /// How a guided review picks, groups and rates the code a person reads.
    Guided,
}

impl Prompt {
    pub const ALL: [Prompt; 3] = [Prompt::Reply, Prompt::Qq, Prompt::Guided];

    fn file(self) -> &'static str {
        match self {
            Prompt::Reply => "reply.md",
            Prompt::Qq => "qq.md",
            Prompt::Guided => "guided.md",
        }
    }

    pub fn default_text(self) -> &'static str {
        match self {
            Prompt::Reply => include_str!("reply.md"),
            Prompt::Qq => include_str!("qq.md"),
            Prompt::Guided => include_str!("guided.md"),
        }
    }
}

/// A file that isn't a symlink.
fn is_plain_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.is_file())
}

/// A prompt and its file, for the Settings tab.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PromptInfo {
    pub prompt: Prompt,
    /// `None` without a home directory.
    pub path: Option<String>,
    /// Whether the file is used over the built-in prompt.
    pub exists: bool,
}

/// Where the prompt's file goes. `config` is lgtm's config dir.
fn path(prompt: Prompt, config: Option<&Path>) -> anyhow::Result<PathBuf> {
    Ok(config.context("no home directory")?.join(prompt.file()))
}

fn find(prompt: Prompt, config: Option<&Path>) -> Option<PathBuf> {
    path(prompt, config).ok().filter(|p| is_plain_file(p))
}

fn load_in(prompt: Prompt, config: Option<&Path>) -> anyhow::Result<String> {
    let Some(path) = find(prompt, config) else {
        return Ok(prompt.default_text().to_string());
    };
    read(&path)
}

fn read(path: &Path) -> anyhow::Result<String> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("can't read the prompt {}", path.display()))?;
    ensure!(
        !text.trim().is_empty(),
        "the prompt {} is empty",
        path.display()
    );
    Ok(text)
}

/// The prompt to give the agent.
pub fn load(prompt: Prompt) -> anyhow::Result<String> {
    load_in(prompt, config_dir().as_deref())
}

fn load_for_in(prompt: Prompt, repo: &Path, config: Option<&Path>) -> anyhow::Result<String> {
    let own = repo.join(".lgtm").join(prompt.file());
    if is_plain_file(&own) {
        return read(&own);
    }
    load_in(prompt, config)
}

/// The prompt to give the agent for the repo at `repo`: its `.lgtm/` file, else [`load`]'s.
pub fn load_for(prompt: Prompt, repo: &Path) -> anyhow::Result<String> {
    load_for_in(prompt, repo, config_dir().as_deref())
}

fn info_in(prompt: Prompt, config: Option<&Path>) -> PromptInfo {
    PromptInfo {
        prompt,
        path: path(prompt, config)
            .ok()
            .map(|p| p.to_string_lossy().into_owned()),
        exists: find(prompt, config).is_some(),
    }
}

/// Every prompt and whether its file is in use.
pub fn infos() -> Vec<PromptInfo> {
    let config = config_dir();
    Prompt::ALL
        .into_iter()
        .map(|p| info_in(p, config.as_deref()))
        .collect()
}

fn edit_in(prompt: Prompt, config: Option<&Path>) -> anyhow::Result<PathBuf> {
    let path = path(prompt, config)?;
    if !path.exists() {
        std::fs::create_dir_all(path.parent().context("prompt path has no parent")?)?;
        std::fs::write(&path, prompt.default_text())?;
    }
    Ok(path)
}

/// The prompt's file, written from the default first when there isn't one.
pub fn edit(prompt: Prompt) -> anyhow::Result<PathBuf> {
    edit_in(prompt, config_dir().as_deref())
}

fn remove_in(prompt: Prompt, config: Option<&Path>) -> anyhow::Result<()> {
    let path = path(prompt, config)?;
    if is_plain_file(&path) {
        std::fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

/// Deletes the prompt's file, so the built-in prompt is used.
pub fn remove(prompt: Prompt) -> anyhow::Result<()> {
    remove_in(prompt, config_dir().as_deref())
}

fn is_prompt_file_in(config: Option<&Path>, file: &Path) -> bool {
    is_plain_file(file)
        && Prompt::ALL
            .into_iter()
            .any(|prompt| path(prompt, config).is_ok_and(|p| p == file))
}

/// Whether `file` is a prompt file, which the app opens for editing. Symlinks aren't, so Edit
/// can't be pointed at some other file through one.
pub fn is_prompt_file(file: &Path) -> bool {
    is_prompt_file_in(config_dir().as_deref(), file)
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: Prompt = Prompt::Reply;

    #[test]
    fn the_prompt_file_wins_over_the_default() {
        let config = tempfile::tempdir().unwrap();
        let config = Some(config.path());
        assert_eq!(load_in(P, config).unwrap(), P.default_text());

        let file = edit_in(P, config).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), P.default_text());
        std::fs::write(&file, "mine").unwrap();
        assert_eq!(load_in(P, config).unwrap(), "mine");
        assert!(info_in(P, config).exists);

        // Opening it again keeps the edits.
        edit_in(P, config).unwrap();
        assert_eq!(load_in(P, config).unwrap(), "mine");

        std::fs::write(&file, "\n").unwrap();
        assert!(load_in(P, config).is_err());

        remove_in(P, config).unwrap();
        assert_eq!(load_in(P, config).unwrap(), P.default_text());
        assert!(!info_in(P, config).exists);
    }

    #[test]
    fn a_repos_own_prompt_wins_over_the_config_file() {
        let config = tempfile::tempdir().unwrap();
        let config = Some(config.path());
        let repo = tempfile::tempdir().unwrap();
        let g = Prompt::Guided;
        std::fs::write(edit_in(g, config).unwrap(), "mine").unwrap();
        assert_eq!(load_for_in(g, repo.path(), config).unwrap(), "mine");
        std::fs::create_dir(repo.path().join(".lgtm")).unwrap();
        std::fs::write(repo.path().join(".lgtm/guided.md"), "the repo's").unwrap();
        assert_eq!(load_for_in(g, repo.path(), config).unwrap(), "the repo's");
    }

    #[test]
    fn only_prompt_files_open_for_editing() {
        let dir = tempfile::tempdir().unwrap();
        let config = Some(dir.path());
        let file = edit_in(P, config).unwrap();
        assert!(is_prompt_file_in(config, &file));
        let other = dir.path().join("notes.md");
        std::fs::write(&other, "").unwrap();
        assert!(!is_prompt_file_in(config, &other));

        #[cfg(unix)]
        {
            std::fs::remove_file(&file).unwrap();
            std::fs::write(dir.path().join("run.command"), "").unwrap();
            std::os::unix::fs::symlink(dir.path().join("run.command"), &file).unwrap();
            assert!(!is_prompt_file_in(config, &file));
            // A symlinked prompt isn't used either.
            assert_eq!(load_in(P, config).unwrap(), P.default_text());
        }
    }
}
