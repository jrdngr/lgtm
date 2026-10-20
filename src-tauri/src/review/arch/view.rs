//! What the system map leaves out: the View menu's choices, stored per repo.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::ctx::RepoCtx;
use crate::store::Store;

const KEY: &str = "view";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum EdgeLabels {
    #[default]
    On,
    Hover,
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Unaffected {
    #[default]
    Hide,
    Fade,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchViewDto {
    pub untracked: bool,
    pub tests: bool,
    pub generated: bool,
    /// Containers with fewer than `SMALL` files.
    pub small: bool,
    /// People and external systems.
    pub outside: bool,
    pub groups: bool,
    pub labels: EdgeLabels,
    pub unaffected: Unaffected,
}

impl Default for ArchViewDto {
    fn default() -> Self {
        ArchViewDto {
            untracked: false,
            tests: false,
            generated: false,
            small: true,
            outside: true,
            groups: true,
            labels: EdgeLabels::On,
            unaffected: Unaffected::Hide,
        }
    }
}

/// Containers with fewer files than this are small.
pub const SMALL: u32 = 3;

pub fn view(store: &Store) -> anyhow::Result<ArchViewDto> {
    Ok(match store.arch_state(KEY)? {
        Some(json) => stored(&json).unwrap_or_default(),
        None => ArchViewDto::default(),
    })
}

/// A stored view over the defaults, so fields added since it was stored keep theirs.
fn stored(json: &str) -> Option<ArchViewDto> {
    let mut v = serde_json::to_value(ArchViewDto::default()).ok()?;
    let saved: serde_json::Map<String, serde_json::Value> = serde_json::from_str(json).ok()?;
    v.as_object_mut()?.extend(saved);
    serde_json::from_value(v).ok()
}

pub fn save_view(store: &Store, view: &ArchViewDto) -> anyhow::Result<()> {
    store.save_arch_state(KEY, &serde_json::to_string(view)?)?;
    Ok(())
}

/// Which of the working copy's files the map keeps.
#[derive(Clone)]
pub struct Files {
    view: ArchViewDto,
    /// The working-copy commit's files; `None` keeps untracked ones.
    tracked: Option<HashSet<String>>,
}

impl Files {
    pub fn keeps(&self, path: &str) -> bool {
        self.tracked.as_ref().is_none_or(|t| t.contains(path))
            && (self.view.tests || !is_test(path))
            && (self.view.generated || !is_generated(path))
    }
}

impl RepoCtx {
    pub fn arch_view(&self) -> anyhow::Result<ArchViewDto> {
        view(&self.store.lock().unwrap())
    }

    pub fn set_arch_view(&self, v: &ArchViewDto) -> anyhow::Result<()> {
        save_view(&self.store.lock().unwrap(), v)
    }

    /// The map's view and the files it keeps in `workspace`. Untracked files stay when jj
    /// can't list the tracked ones.
    pub(super) async fn view_files(
        &self,
        workspace: Option<&str>,
    ) -> anyhow::Result<(ArchViewDto, Files)> {
        let view = self.arch_view()?;
        let tracked = if view.untracked {
            None
        } else {
            let ws = self.workspace(workspace).await?;
            ws.jj.files("@").await.ok().map(HashSet::from_iter)
        };
        Ok((view, Files { view, tracked }))
    }
}

const TEST_DIRS: &[&str] = &[
    "test",
    "tests",
    "__tests__",
    "__mocks__",
    "spec",
    "testdata",
];
const GENERATED_DIRS: &[&str] = &[
    "dist",
    "build",
    "out",
    "gen",
    "generated",
    "__generated__",
    "vendor",
    "third_party",
];

/// Whether `path` is a test file or sits in a test folder.
pub fn is_test(path: &str) -> bool {
    let (dirs, file) = path.rsplit_once('/').unwrap_or(("", path));
    if dirs.split('/').any(|d| TEST_DIRS.contains(&d)) {
        return true;
    }
    let stem = file.split('.').next().unwrap_or(file);
    stem.starts_with("test_")
        || stem.ends_with("_test")
        || stem.ends_with("_spec")
        || file == "conftest.py"
        || file.contains(".test.")
        || file.contains(".spec.")
}

/// Whether `path` is generated or vendored code, by its folder or name.
pub fn is_generated(path: &str) -> bool {
    let (dirs, file) = path.rsplit_once('/').unwrap_or(("", path));
    dirs.split('/').any(|d| GENERATED_DIRS.contains(&d))
        || file.ends_with(".pb.go")
        || file.ends_with("_pb2.py")
        || file.ends_with(".min.js")
        || file.contains(".generated.")
        || file.contains(".gen.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tests_by_folder_or_name() {
        for p in [
            "tests/a.rs",
            "src/__tests__/x.ts",
            "pkg/foo_test.go",
            "app/test_auth.py",
            "app/conftest.py",
            "src/ui/Panel.test.tsx",
            "src/ui/panel.spec.ts",
        ] {
            assert!(is_test(p), "{p}");
        }
        for p in [
            "src/testing.rs",
            "src/contest.py",
            "src/attest/a.rs",
            "latest.ts",
        ] {
            assert!(!is_test(p), "{p}");
        }
    }

    #[test]
    fn generated_by_folder_or_name() {
        for p in [
            "web/dist/app.js",
            "vendor/lib/x.go",
            "api/user.pb.go",
            "api/user_pb2.py",
            "src/schema.generated.ts",
            "static/app.min.js",
        ] {
            assert!(is_generated(p), "{p}");
        }
        for p in [
            "src/builder.rs",
            "build.rs",
            "src/general.ts",
            "src/output/a.ts",
        ] {
            assert!(!is_generated(p), "{p}");
        }
    }

    #[test]
    fn a_stored_view_missing_newer_fields_keeps_their_defaults() {
        let v = stored(r#"{"tests":true}"#).unwrap();
        assert_eq!(
            v,
            ArchViewDto {
                tests: true,
                ..ArchViewDto::default()
            }
        );
    }
}
