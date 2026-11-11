//! The app side of `lgtm_arch::design::prompt`.

use super::{covered, rev};
use crate::ctx::RepoCtx;
use crate::review::arch::describe::map_text;
use anyhow::bail;
pub use lgtm_arch::design::prompt::*;
use std::fmt::Write;

impl RepoCtx {
    /// The prompt asking a terminal agent to write design `id`'s map.
    pub async fn design_prompt(&self, workspace: Option<&str>, id: &str) -> anyhow::Result<String> {
        let d = self.design(id)?;
        let ws = self.workspace(workspace).await?;
        ws.jj.snapshot_or_stale().await?;
        let commit = rev::resolve(&ws.jj, &d.revision).await?.commit;
        let files = covered(&ws.jj.commit_files(&commit).await?, &d.files);
        if files.is_empty() {
            bail!("`{}` has none of the design's files", d.revision);
        }
        let map = self.system_map(workspace).await?;
        let mut out = format!(
            "Design \"{}\". Its spec files are at commit {commit} (revision `{}`); read each \
             with `jj file show -r {commit} <path>`:\n\n",
            d.name, d.revision
        );
        for f in files.iter().take(MAX_FILES) {
            writeln!(out, "- {f}").unwrap();
        }
        if files.len() > MAX_FILES {
            writeln!(
                out,
                "- and {} more under the same folders",
                files.len() - MAX_FILES
            )
            .unwrap();
        }
        if !d.prompt.trim().is_empty() {
            write!(out, "\nScope, from the user:\n\n{}\n", d.prompt.trim()).unwrap();
        }
        if map.containers.is_empty() {
            out.push_str("\nThere is no code yet.\n");
        } else {
            write!(
                out,
                "\nThe code's system map, from the working copy's imports:\n\n{}",
                map_text(&map)
            )
            .unwrap();
        }
        write!(out, "\n{INSTRUCTIONS} `{id}`.").unwrap();
        out.push_str(&canvas_text(&d.canvas));
        Ok(out)
    }
}
