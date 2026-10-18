//! Stored reports, found by the id the user knows them by: `R<n>`, or the name they renamed
//! it to.

use anyhow::{Context, bail, ensure};

use super::BotRun;
use crate::ctx::RepoCtx;

impl BotRun {
    /// The id the user sees and tells agents: its name, or `R<id>`.
    pub fn label(&self) -> String {
        self.name.clone().unwrap_or_else(|| format!("R{}", self.id))
    }

    /// Whether finding `id` is "dismissed" or "open".
    pub fn finding_state(&self, id: &str) -> &'static str {
        if self.dismissed.iter().any(|d| d == id) {
            "dismissed"
        } else {
            "open"
        }
    }
}

/// Whether `name` has the shape of a default id, like `R12`.
fn looks_default(name: &str) -> bool {
    name.strip_prefix(['R', 'r'])
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

impl RepoCtx {
    /// The workspace's reports, newest first.
    pub fn bot_runs(&self, workspace: Option<&str>) -> anyhow::Result<Vec<BotRun>> {
        Ok(self
            .store
            .lock()
            .unwrap()
            .bot_runs(workspace.unwrap_or(""))?)
    }

    pub fn bot_run(&self, id: u32) -> anyhow::Result<BotRun> {
        self.store
            .lock()
            .unwrap()
            .bot_run(id)?
            .context("the report is gone")
    }

    pub fn delete_bots(&self, id: u32) -> anyhow::Result<()> {
        Ok(self.store.lock().unwrap().delete_bot_run(id)?)
    }

    /// Applies `f` to the stored report and saves it.
    pub(super) fn edit_run<T>(
        &self,
        id: u32,
        f: impl FnOnce(&mut BotRun) -> T,
    ) -> anyhow::Result<T> {
        let store = self.store.lock().unwrap();
        let mut run = store.bot_run(id)?.context("the report is gone")?;
        let out = f(&mut run);
        store.save_bot_run(&run)?;
        Ok(out)
    }

    /// Dismisses `findings`, or with `dismissed` false, brings them back.
    pub fn dismiss_findings(
        &self,
        id: u32,
        findings: &[String],
        dismissed: bool,
    ) -> anyhow::Result<BotRun> {
        self.edit_run(id, |run| {
            run.dismissed.retain(|f| !findings.contains(f));
            if dismissed {
                run.dismissed.extend(findings.iter().cloned());
            }
            run.clone()
        })
    }

    /// The workspace's report whose id is `label`, ignoring case.
    pub fn find_report(&self, workspace: Option<&str>, label: &str) -> anyhow::Result<BotRun> {
        let label = label.trim();
        self.bot_runs(workspace)?
            .into_iter()
            .find(|r| r.label().eq_ignore_ascii_case(label))
            .with_context(|| format!("there's no report {label} in this workspace"))
    }

    /// Renames report `id` to `name`, or back to `R<id>` when `name` is blank. Names are
    /// letters, digits, `-`, `_` and `.`, unique across the repository, and can't look like
    /// another report's default id.
    pub fn rename_report(&self, id: u32, name: &str) -> anyhow::Result<BotRun> {
        let name = name.trim();
        let default = format!("R{id}");
        let name = (!name.is_empty() && !name.eq_ignore_ascii_case(&default)).then_some(name);
        if let Some(name) = name {
            ensure!(
                name.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')),
                "a report id can only have letters, digits, -, _ and ."
            );
            if looks_default(name) {
                bail!("{name} looks like another report's id");
            }
            let taken = self
                .store
                .lock()
                .unwrap()
                .all_bot_runs()?
                .iter()
                .any(|r| r.id != id && r.label().eq_ignore_ascii_case(name));
            ensure!(!taken, "another report is already called {name}");
        }
        self.edit_run(id, |run| {
            run.name = name.map(str::to_string);
        })?;
        self.bot_run(id)
    }
}
