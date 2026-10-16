//! Intents and the architecture diff as text for agents.

use std::fmt::Write;

use super::{ArchDto, ComponentDto, EdgeDto, EdgeStatus, IntentDto, IntentKind, IntentState};
use crate::review::RepoCtx;

/// Import sites listed per edge in the architecture diff before the rest are counted.
const MAX_SITES: usize = 5;

/// A component as "`name` (folder)", or its id alone when it's unknown.
fn comp(comps: &[ComponentDto], id: &str) -> String {
    let folder = if id.is_empty() { "repo root" } else { id };
    match comps.iter().find(|c| c.id == id) {
        Some(c) => format!("`{}` ({folder})", c.name),
        None => format!("`{folder}`"),
    }
}

/// What intent `kind` asks for, in one line.
fn intent_line(kind: &IntentKind, comps: &[ComponentDto]) -> String {
    match kind {
        IntentKind::Wrong { from, to, reason } => {
            let mut line = format!(
                "{} must not import from {}.",
                comp(comps, from),
                comp(comps, to)
            );
            if let Some(r) = reason.as_deref().map(str::trim).filter(|r| !r.is_empty()) {
                write!(line, " Reason: {r}").unwrap();
            }
            line
        }
        IntentKind::Move { path, symbol, to } => {
            let what = match symbol {
                Some(s) => format!("`{s}` in `{path}`"),
                None => format!("`{path}`"),
            };
            format!("Move {what} into component {}.", comp(comps, to))
        }
        IntentKind::Risk { component } => {
            format!("{} is tagged as a risk.", comp(comps, component))
        }
        IntentKind::Trust { component } => {
            format!("{} is tagged as a trust boundary.", comp(comps, component))
        }
        IntentKind::Note { component, text } => {
            format!("Note on {}: {}", comp(comps, component), text.trim())
        }
    }
}

/// The current imports behind the edge from `from` to `to`, as "path:line imports path". The
/// edge is the one Wrong intent `id` marks when the lens merged either end.
fn sites<'a>(
    edges: &'a [EdgeDto],
    id: u32,
    from: &'a str,
    to: &'a str,
) -> impl Iterator<Item = String> + 'a {
    edges
        .iter()
        .filter(move |e| e.wrong == Some(id) || (e.from == from && e.to == to))
        .filter(|e| e.status != EdgeStatus::Removed)
        .flat_map(|e| &e.imports)
        .map(|i| format!("{}:{} imports {}", i.from_path, i.line, i.to_path))
}

/// The prompt asking an agent to make the code meet `intents` (tags are skipped), or `None`
/// when none of them can be acted on.
pub(super) fn intent_prompt(
    revset: &str,
    intents: &[&IntentDto],
    arch: &ArchDto,
) -> Option<String> {
    let mut items = String::new();
    let checked = intents
        .iter()
        .filter(|i| matches!(i.kind, IntentKind::Wrong { .. } | IntentKind::Move { .. }));
    for (n, i) in checked.enumerate() {
        writeln!(
            items,
            "{}. {}",
            n + 1,
            intent_line(&i.kind, &arch.components)
        )
        .unwrap();
        if let IntentKind::Wrong { from, to, .. } = &i.kind {
            for site in sites(&arch.edges, i.id, from, to) {
                writeln!(items, "   - {site}").unwrap();
            }
        }
    }
    if items.is_empty() {
        return None;
    }
    Some(format!(
        "Change the code in this repository to match these architecture decisions from a code \
         review of `{revset}`. A component is the folder in parentheses. Edit the files directly \
         and change only what each decision needs, updating the code that uses what you move. If \
         one turns out to be wrong, leave the code alone and say why. Then say in a line or two \
         per decision what you changed.\n\n{}",
        items.trim_end()
    ))
}

fn state_word(s: IntentState) -> &'static str {
    match s {
        IntentState::Broken => "broken",
        IntentState::Open => "open",
        IntentState::Met => "met",
        IntentState::Tag => "tag",
    }
}

fn status_word(s: EdgeStatus) -> &'static str {
    match s {
        EdgeStatus::Added => "added",
        EdgeStatus::Removed => "removed",
        EdgeStatus::Same => "same",
    }
}

/// The architecture diff and the user's intents, compactly, for Assess. Edges between two
/// unchanged components that the change keeps are only counted.
pub(super) fn arch_text(arch: &ArchDto) -> String {
    let c = arch.counts;
    let mut out = format!(
        "Components are folders under source roots, with ids as given. An edge means `from` \
         imports from `to`. Blast radius: {} changed, {} upstream, {} impacted.\n\nComponents \
         (id, name, status):\n\n",
        c.changed, c.upstream, c.impacted
    );
    for comp in &arch.components {
        let status = format!("{:?}", comp.status).to_lowercase();
        write!(out, "- `{}` {}: {status}", comp.id, comp.name).unwrap();
        if !comp.misplaced.is_empty() {
            write!(out, ", misplaced: {}", comp.misplaced.join(", ")).unwrap();
        }
        out.push('\n');
    }
    let quiet = |id: &str| {
        arch.components
            .iter()
            .any(|c| c.id == id && c.status == super::CompStatus::Unchanged)
    };
    let (shown, hidden): (Vec<_>, Vec<_>) = arch
        .edges
        .iter()
        .partition(|e| e.status != EdgeStatus::Same || !(quiet(&e.from) && quiet(&e.to)));
    out.push_str("\nEdges (from -> to: status, import sites):\n\n");
    for e in shown {
        write!(
            out,
            "- `{}` -> `{}`: {}",
            e.from,
            e.to,
            status_word(e.status)
        )
        .unwrap();
        let mut sites = e
            .imports
            .iter()
            .map(|i| format!("{}:{}", i.from_path, i.line));
        let listed: Vec<String> = sites.by_ref().take(MAX_SITES).collect();
        if !listed.is_empty() {
            write!(out, ", {}", listed.join(", ")).unwrap();
        }
        let more = sites.count();
        if more > 0 {
            write!(out, " and {more} more").unwrap();
        }
        out.push('\n');
    }
    if !hidden.is_empty() {
        writeln!(
            out,
            "- unchanged edges between unchanged components left out: {}",
            hidden.len()
        )
        .unwrap();
    }
    if !arch.intents.is_empty() {
        out.push_str("\nThe reviewer's architecture decisions (state: decision):\n\n");
        for i in &arch.intents {
            let line = intent_line(&i.kind, &arch.components);
            writeln!(out, "- {}: {line}", state_word(i.state)).unwrap();
        }
    }
    out
}

impl RepoCtx {
    /// The prompt asking an agent to make the code meet intents `ids`, for Fix and Send to
    /// terminal.
    pub async fn intent_prompt(&self, review: u32, ids: &[u32]) -> anyhow::Result<String> {
        let revset = self.review(review)?.lock().unwrap().revset.clone();
        let arch = self.arch(review, false, None).await?;
        let picked: Vec<&IntentDto> = arch
            .intents
            .iter()
            .filter(|i| ids.contains(&i.id))
            .collect();
        intent_prompt(&revset, &picked, &arch)
            .ok_or_else(|| anyhow::anyhow!("pick a Wrong or Move intent to fix"))
    }
}

#[cfg(test)]
mod tests {
    use super::super::{CompStatus, CountsDto, ImportDto};
    use super::*;

    fn component(id: &str, name: &str, status: CompStatus) -> ComponentDto {
        ComponentDto {
            id: id.into(),
            name: name.into(),
            container: "src".into(),
            files: vec![],
            status,
            layer: None,
            risk: false,
            trust: false,
            note: None,
            misplaced: vec![],
            severity: None,
        }
    }

    fn edge(from: &str, to: &str, status: EdgeStatus, lines: &[u32]) -> EdgeDto {
        EdgeDto {
            from: from.into(),
            to: to.into(),
            status,
            impacted: false,
            imports: lines
                .iter()
                .map(|&line| ImportDto {
                    from_path: format!("{from}/mod.rs"),
                    to_path: format!("{to}/mod.rs"),
                    line,
                })
                .collect(),
            wrong: None,
        }
    }

    fn intent(id: u32, kind: IntentKind, state: IntentState) -> IntentDto {
        IntentDto { id, kind, state }
    }

    fn arch(components: Vec<ComponentDto>, edges: Vec<EdgeDto>) -> ArchDto {
        ArchDto {
            containers: vec![],
            components,
            edges,
            layers: vec![],
            look: vec![],
            intents: vec![],
            assessment: None,
            counts: CountsDto {
                changed: 1,
                upstream: 0,
                impacted: 2,
            },
        }
    }

    #[test]
    fn intent_prompt_lists_wrong_imports_and_moves_and_skips_tags() {
        let a = arch(
            vec![
                component("src/ui", "ui", CompStatus::Changed),
                component("src/db", "storage", CompStatus::Unchanged),
            ],
            vec![edge("src/ui", "src/db", EdgeStatus::Added, &[3, 9])],
        );
        let intents = [
            intent(
                1,
                IntentKind::Wrong {
                    from: "src/ui".into(),
                    to: "src/db".into(),
                    reason: Some("UI goes through the API".into()),
                },
                IntentState::Broken,
            ),
            intent(
                2,
                IntentKind::Risk {
                    component: "src/db".into(),
                },
                IntentState::Tag,
            ),
            intent(
                3,
                IntentKind::Move {
                    path: "src/ui/query.rs".into(),
                    symbol: Some("load".into()),
                    to: "src/db".into(),
                },
                IntentState::Open,
            ),
        ];
        let picked: Vec<&IntentDto> = intents.iter().collect();
        let text = intent_prompt("main..@", &picked, &a).unwrap();
        let (intro, items) = text.split_once("\n\n").unwrap();
        assert!(intro.contains("code review of `main..@`"));
        assert_eq!(
            items,
            "1. `ui` (src/ui) must not import from `storage` (src/db). Reason: UI goes through \
             the API\n   \
             - src/ui/mod.rs:3 imports src/db/mod.rs\n   \
             - src/ui/mod.rs:9 imports src/db/mod.rs\n\
             2. Move `load` in `src/ui/query.rs` into component `storage` (src/db)."
        );
    }

    #[test]
    fn intent_prompt_is_none_for_tags_alone() {
        let a = arch(vec![], vec![]);
        let tag = intent(
            1,
            IntentKind::Trust {
                component: "src/api".into(),
            },
            IntentState::Tag,
        );
        assert_eq!(intent_prompt("@", &[&tag], &a), None);
    }

    #[test]
    fn arch_text_counts_quiet_edges_and_caps_import_sites() {
        let a = arch(
            vec![
                component("src/ui", "ui", CompStatus::Changed),
                component("src/db", "db", CompStatus::Unchanged),
                component("src/log", "log", CompStatus::Unchanged),
            ],
            vec![
                edge(
                    "src/ui",
                    "src/db",
                    EdgeStatus::Added,
                    &[1, 2, 3, 4, 5, 6, 7],
                ),
                edge("src/db", "src/log", EdgeStatus::Same, &[1]),
                edge("src/log", "src/db", EdgeStatus::Removed, &[]),
            ],
        );
        let text = arch_text(&a);
        assert!(text.contains("1 changed, 0 upstream, 2 impacted"));
        assert!(text.contains(
            "- `src/ui` -> `src/db`: added, src/ui/mod.rs:1, src/ui/mod.rs:2, src/ui/mod.rs:3, \
             src/ui/mod.rs:4, src/ui/mod.rs:5 and 2 more\n"
        ));
        assert!(text.contains("- `src/log` -> `src/db`: removed\n"));
        assert!(!text.contains("`src/db` -> `src/log`"));
        assert!(text.contains("- unchanged edges between unchanged components left out: 1"));
    }
}
