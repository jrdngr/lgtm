//! The default change stack: one layer per changed component, the ones others depend on
//! first.

use std::collections::{BTreeMap, BTreeSet};

use super::group::Grouping;
use super::model::{Changed, Diff};
use super::{CompStatus, EdgeStatus, LayerDto};

pub fn default_layers(d: &Diff, changed: &[Changed], g: &Grouping) -> Vec<LayerDto> {
    let mut remaining: BTreeMap<String, String> = d
        .components
        .iter()
        .filter(|c| {
            matches!(
                c.status,
                CompStatus::Added | CompStatus::Changed | CompStatus::Removed
            )
        })
        .map(|c| (c.id.clone(), c.name.clone()))
        .collect();
    // What each changed component imports among the others at head.
    let mut deps: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for e in d.edges.iter().filter(|e| e.status != EdgeStatus::Removed) {
        if remaining.contains_key(&e.from) && remaining.contains_key(&e.to) {
            deps.entry(&e.from).or_default().insert(&e.to);
        }
    }
    let mut order: Vec<String> = Vec::new();
    while !remaining.is_empty() {
        let ready = |id: &String| {
            deps.get(id.as_str())
                .is_none_or(|ds| ds.iter().all(|d| !remaining.contains_key(*d)))
        };
        // The first ready by name, or on a cycle the first by name.
        let mut by_name: Vec<(&String, &String)> =
            remaining.iter().map(|(id, n)| (n, id)).collect();
        by_name.sort();
        let next = by_name
            .iter()
            .find(|(_, id)| ready(id))
            .unwrap_or(&by_name[0])
            .1
            .clone();
        remaining.remove(&next);
        order.push(next);
    }
    order
        .into_iter()
        .enumerate()
        .map(|(i, id)| {
            let files: Vec<&Changed> = changed
                .iter()
                .filter(|c| {
                    std::iter::once(&c.path)
                        .chain(&c.old_path)
                        .any(|p| super::extract::is_source(p) && g.comp(p) == id)
                })
                .collect();
            let n = files.len();
            LayerDto {
                n: i as u32 + 1,
                title: g.name(&id),
                summary: format!("{n} file{}", if n == 1 { "" } else { "s" }),
                ranges: files.iter().flat_map(|c| c.ranges.clone()).collect(),
                components: vec![id],
            }
        })
        .collect()
}
