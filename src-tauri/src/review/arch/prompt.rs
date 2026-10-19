//! The architecture diff as text for agents.

use std::fmt::Write;

use super::{ArchDto, EdgeStatus};

/// Import sites listed per edge in the architecture diff before the rest are counted.
const MAX_SITES: usize = 5;

fn status_word(s: EdgeStatus) -> &'static str {
    match s {
        EdgeStatus::Added => "added",
        EdgeStatus::Removed => "removed",
        EdgeStatus::Same => "same",
    }
}

/// The architecture diff, compactly, for Assess. Edges between two
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
        writeln!(out, "- `{}` {}: {status}", comp.id, comp.name).unwrap();
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
    out
}

#[cfg(test)]
mod tests {
    use super::super::{CompStatus, ComponentDto, CountsDto, EdgeDto, ImportDto};
    use super::*;

    fn component(id: &str, name: &str, status: CompStatus) -> ComponentDto {
        ComponentDto {
            id: id.into(),
            name: name.into(),
            container: "src".into(),
            files: vec![],
            status,
            layer: None,
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
        }
    }

    fn arch(components: Vec<ComponentDto>, edges: Vec<EdgeDto>) -> ArchDto {
        ArchDto {
            containers: vec![],
            components,
            edges,
            layers: vec![],
            look: vec![],
            assessment: None,
            counts: CountsDto {
                changed: 1,
                upstream: 0,
                impacted: 2,
            },
        }
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
