//! Grouping files into components: a root's subfolders, or its loose files, reshaped by the
//! lens.

use super::roots::{Root, root_of};
use super::{ContainerDto, Lens, LensEdit};

pub struct Grouping {
    roots: Vec<Root>,
    lens: Lens,
    /// Names the container of files outside every root.
    repo: String,
}

impl Grouping {
    pub fn new(roots: Vec<Root>, lens: Lens, repo: &str) -> Self {
        Grouping {
            roots,
            lens,
            repo: repo.to_string(),
        }
    }

    /// The component a file is in.
    pub fn comp(&self, path: &str) -> String {
        let root = root_of(&self.roots, path).map_or("", |r| r.path.as_str());
        let rel = path
            .strip_prefix(root)
            .map_or(path, |r| r.trim_start_matches('/'));
        let parts: Vec<&str> = rel.split('/').collect();
        let mut id = root.to_string();
        if parts.len() > 1 {
            id = super::roots::join(root, parts[0]);
            let mut i = 1;
            while i + 1 < parts.len() && self.lens.split.contains(&id) {
                id = format!("{id}/{}", parts[i]);
                i += 1;
            }
        }
        self.resolve(&id)
    }

    /// The component `id` was merged into, or `id`.
    pub fn resolve(&self, id: &str) -> String {
        let mut id = id.to_string();
        for _ in 0..self.lens.merged.len() {
            match self.lens.merged.get(&id) {
                Some(into) => id = into.clone(),
                None => break,
            }
        }
        id
    }

    pub fn container(&self, comp: &str) -> String {
        root_of(&self.roots, comp).map_or(String::new(), |r| r.path.clone())
    }

    pub fn name(&self, comp: &str) -> String {
        if let Some(n) = self.lens.names.get(comp) {
            return n.clone();
        }
        if comp.is_empty() {
            return self.repo.clone();
        }
        match self.roots.iter().find(|r| r.path == comp) {
            Some(r) => r.name.clone(),
            None => comp.rsplit('/').next().unwrap_or(comp).to_string(),
        }
    }

    pub fn container_dto(&self, id: &str) -> ContainerDto {
        let name = match self.roots.iter().find(|r| r.path == id) {
            Some(r) => r.name.clone(),
            None => self.repo.clone(),
        };
        ContainerDto {
            id: id.to_string(),
            name,
        }
    }
}

/// Applies a lens edit.
pub fn edit(lens: &mut Lens, edit: LensEdit) {
    match edit {
        LensEdit::Rename { component, name } => {
            if name.trim().is_empty() {
                lens.names.remove(&component);
            } else {
                lens.names.insert(component, name.trim().to_string());
            }
        }
        LensEdit::Merge { component, into } => {
            let grouping = Grouping::new(vec![], lens.clone(), "");
            let into = grouping.resolve(&into);
            if into == component {
                return;
            }
            for target in lens.merged.values_mut() {
                if *target == component {
                    *target = into.clone();
                }
            }
            lens.merged.insert(component, into);
        }
        LensEdit::Split { component } => {
            lens.merged.retain(|_, into| *into != component);
            if !lens.split.contains(&component) {
                lens.split.push(component);
            }
        }
    }
}
