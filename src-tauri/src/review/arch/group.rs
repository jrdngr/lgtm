//! Grouping files into components: a root's subfolders, or its loose files.

use super::ContainerDto;
use super::roots::{Root, root_of};

pub struct Grouping {
    roots: Vec<Root>,
    /// Names the container of files outside every root.
    repo: String,
}

impl Grouping {
    pub fn new(roots: Vec<Root>, repo: &str) -> Self {
        Grouping {
            roots,
            repo: repo.to_string(),
        }
    }

    /// The component a file is in.
    pub fn comp(&self, path: &str) -> String {
        let root = root_of(&self.roots, path).map_or("", |r| r.path.as_str());
        let rel = path
            .strip_prefix(root)
            .map_or(path, |r| r.trim_start_matches('/'));
        match rel.split_once('/') {
            Some((dir, _)) => super::roots::join(root, dir),
            None => root.to_string(),
        }
    }

    pub fn container(&self, comp: &str) -> String {
        root_of(&self.roots, comp).map_or(String::new(), |r| r.path.clone())
    }

    pub fn name(&self, comp: &str) -> String {
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
