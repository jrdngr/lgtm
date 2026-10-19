//! Rule Look items: what in the diff deserves a closer look.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use super::group::Grouping;
use super::model::{Diff, Model};
use super::{CompStatus, EdgeDto, EdgeStatus, LookDto, LookSeverity, LookSource};

fn item(key: String, severity: LookSeverity, title: String) -> LookDto {
    LookDto {
        key,
        source: LookSource::Rule,
        severity,
        title,
        detail: None,
        component: None,
        edge: None,
        path: None,
        line: None,
        checked: false,
    }
}

pub fn rules(d: &Diff, m: &Model, g: &Grouping) -> Vec<LookDto> {
    let name = |id: &str| d.comp(id).map_or_else(|| g.name(id), |c| c.name.clone());
    let mut out = Vec::new();
    for c in &d.components {
        let (key, severity, verb) = match c.status {
            CompStatus::Added => ("new-comp", LookSeverity::Medium, "New component"),
            CompStatus::Removed => ("removed-comp", LookSeverity::Low, "Removed component"),
            _ => continue,
        };
        let mut it = item(
            format!("rule:{key}:{}", c.id),
            severity,
            format!("{verb} {}", c.name),
        );
        it.component = Some(c.id.clone());
        out.push(it);
    }
    for e in &d.edges {
        let (key, severity, verb) = match e.status {
            EdgeStatus::Added => ("new-edge", LookSeverity::Medium, "New edge"),
            EdgeStatus::Removed => ("removed-edge", LookSeverity::Low, "Removed edge"),
            EdgeStatus::Same => continue,
        };
        out.push(edge_item(
            format!("rule:{key}:{}>{}", e.from, e.to),
            severity,
            format!("{verb} {} → {}", name(&e.from), name(&e.to)),
            Some(e),
            &e.from,
            &e.to,
        ));
    }
    out.extend(used_symbols(d, m, g, &name));
    for c in &m.changed {
        let Some(old) = &c.old_path else { continue };
        let (from, to) = (g.comp(old), g.comp(&c.path));
        if from != to && super::extract::is_source(&c.path) {
            let mut it = item(
                format!("rule:moved:{old}>{}", c.path),
                LookSeverity::Low,
                format!("Moved {old} to {}", name(&to)),
            );
            it.component = Some(to);
            it.path = Some(c.path.clone());
            out.push(it);
        }
    }
    out
}

fn edge_item(
    key: String,
    severity: LookSeverity,
    title: String,
    e: Option<&EdgeDto>,
    from: &str,
    to: &str,
) -> LookDto {
    let mut it = item(key, severity, title);
    it.component = Some(from.to_string());
    it.edge = Some((from.to_string(), to.to_string()));
    if let Some(first) = e.and_then(|e| e.imports.first()) {
        it.path = Some(first.from_path.clone());
        it.line = Some(first.line);
    }
    it
}

/// Changed symbols that code in other components uses. Symbols link by name, so a use only
/// counts when its component imports the symbol's.
fn used_symbols(d: &Diff, m: &Model, g: &Grouping, name: &dyn Fn(&str) -> String) -> Vec<LookDto> {
    // (path, symbol, line) -> components using it
    let mut users: BTreeMap<(&str, &str, u32), BTreeSet<String>> = BTreeMap::new();
    for u in m
        .uses
        .iter()
        .filter(|u| u.to_hop == 0 && u.hop == 1 && !u.from_changed)
    {
        let (from, to) = (g.comp(&u.from_path), g.comp(&u.to_path));
        let imports = d
            .edge(&from, &to)
            .is_some_and(|e| e.status != EdgeStatus::Removed);
        if from != to && imports {
            users
                .entry((&u.to_path, &u.to_name, u.to_line))
                .or_default()
                .insert(from);
        }
    }
    let mut seen = HashSet::new();
    users
        .into_iter()
        .filter(|((path, sym, _), _)| seen.insert((*path, *sym)))
        .map(|((path, sym, line), comps)| {
            let names: Vec<String> = comps.iter().map(|c| name(c)).collect();
            let mut it = item(
                format!("rule:symbol:{path}#{sym}"),
                LookSeverity::Medium,
                format!("{sym} used by {}", names.join(", ")),
            );
            it.component = Some(g.comp(path));
            it.path = Some(path.to_string());
            it.line = Some(line);
            it
        })
        .collect()
}
