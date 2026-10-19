//! Assembling the architecture DTO from a model and stored review state.

use std::collections::{BTreeSet, HashSet};

use super::assess::Assessed;
use super::group::Grouping;
use super::model::{self, Model};
use super::{ArchDto, CompStatus, CountsDto, layers, look};

pub struct Stored {
    /// Checked Look keys.
    pub checked: HashSet<String>,
    pub assessed: Option<Assessed>,
}

pub fn compute(m: &Model, g: &Grouping, transitive: bool, stored: Stored) -> ArchDto {
    let mut d = model::diff(m, g, transitive);
    let mut look = look::rules(&d, m, g);
    let (layers, assessment) = match stored.assessed {
        Some(mut a) => {
            look.extend(a.look);
            // The stored indexes are the review's files when Assess ran, which may have changed.
            for r in a.layers.iter_mut().flat_map(|l| &mut l.ranges) {
                r.file = m
                    .changed
                    .iter()
                    .find(|c| c.path == r.path)
                    .and_then(|c| c.file);
            }
            (a.layers, Some(a.assessment))
        }
        None => (layers::default_layers(&d, &m.changed, g), None),
    };
    for it in &mut look {
        it.checked = stored.checked.contains(&it.key);
    }
    let mut counts = CountsDto::default();
    for c in &mut d.components {
        c.layer = layers
            .iter()
            .find(|l| l.components.contains(&c.id))
            .map(|l| l.n);
        c.severity = look
            .iter()
            .filter(|it| !it.checked && it.component.as_deref() == Some(c.id.as_str()))
            .map(|it| it.severity)
            .max();
        match c.status {
            CompStatus::Added | CompStatus::Removed | CompStatus::Changed => counts.changed += 1,
            CompStatus::Upstream => counts.upstream += 1,
            CompStatus::Impacted => counts.impacted += 1,
            CompStatus::Unchanged => {}
        }
    }
    let containers: BTreeSet<&str> = d.components.iter().map(|c| c.container.as_str()).collect();
    ArchDto {
        containers: containers.into_iter().map(|c| g.container_dto(c)).collect(),
        components: d.components,
        edges: d.edges,
        layers,
        look,
        assessment,
        counts,
    }
}
