use super::*;
use crate::review::arch::model::Import;
use crate::review::arch::roots::Root;

#[test]
fn structure_counts_files_import_sites_and_entry_files_per_component() {
    let import = |to: &str, line| Import {
        to: to.into(),
        line,
    };
    let snap: model::Snapshot = [
        (
            "src/ui/a.ts",
            vec![import("src/db/x.ts", 1), import("src/db/y.ts", 2)],
        ),
        (
            "src/ui/b.ts",
            vec![import("src/db/x.ts", 4), import("src/ui/a.ts", 5)],
        ),
        ("src/db/x.ts", vec![]),
        ("src/db/y.ts", vec![]),
        ("build.ts", vec![import("src/ui/a.ts", 1)]),
    ]
    .into_iter()
    .map(|(p, i)| (p.to_string(), i))
    .collect();
    let roots = vec![Root {
        path: "src".into(),
        name: "app".into(),
    }];
    let g = Grouping::new(roots, "repo");
    let Structure {
        containers,
        components,
        edges,
        files: _,
    } = structure(&snap, &g);
    let files = |cs: &[SysComponentDto]| -> Vec<(String, String, u32)> {
        cs.iter()
            .map(|c| (c.id.clone(), c.container.clone(), c.files))
            .collect()
    };
    assert_eq!(
        containers
            .iter()
            .map(|c| (&*c.id, &*c.name, c.files))
            .collect::<Vec<_>>(),
        [("", "repo", 1), ("src", "app", 4)]
    );
    assert_eq!(
        files(&components),
        [
            ("".into(), "".into(), 1),
            ("src/db".into(), "src".into(), 2),
            ("src/ui".into(), "src".into(), 2),
        ]
    );
    assert_eq!(
        edges
            .iter()
            .map(|e| (&*e.from, &*e.to, e.imports))
            .collect::<Vec<_>>(),
        [("", "src/ui", 1), ("src/ui", "src/db", 3)]
    );
    assert_eq!(
        edges.iter().map(|e| (&*e.path, e.line)).collect::<Vec<_>>(),
        [("build.ts", 1), ("src/ui/a.ts", 1)]
    );
    let entry = |id: &str| {
        components
            .iter()
            .find(|c| c.id == id)
            .unwrap()
            .entry
            .clone()
    };
    assert_eq!(entry("src/db"), ["src/db/x.ts", "src/db/y.ts"]);
    assert_eq!(entry("src/ui"), ["src/ui/a.ts"]);
}

fn view(f: impl FnOnce(&mut ArchViewDto)) -> ArchViewDto {
    let mut v = ArchViewDto::default();
    f(&mut v);
    v
}

#[test]
fn hiding_small_containers_drops_their_components_files_and_edges() {
    let comp = |id: &str, container: &str| SysComponentDto {
        id: id.into(),
        name: id.into(),
        container: container.into(),
        files: 1,
        entry: vec![],
    };
    let edge = |from: &str, to: &str| SysEdgeDto {
        from: from.into(),
        to: to.into(),
        imports: 1,
        path: String::new(),
        line: 1,
    };
    let st = Structure {
        containers: vec![
            SysContainerDto {
                id: "big".into(),
                name: "big".into(),
                files: 3,
            },
            SysContainerDto {
                id: "tiny".into(),
                name: "tiny".into(),
                files: 2,
            },
        ],
        components: vec![
            comp("big/a", "big"),
            comp("big/b", "big"),
            comp("tiny/c", "tiny"),
        ],
        edges: vec![edge("big/a", "big/b"), edge("big/a", "tiny/c")],
        files: vec![
            SysFileDto {
                path: "big/a/x.rs".into(),
                component: "big/a".into(),
            },
            SysFileDto {
                path: "tiny/c/y.rs".into(),
                component: "tiny/c".into(),
            },
        ],
    };
    let out = without_small(st, &view(|v| v.small = false));
    assert_eq!(
        out.containers.iter().map(|c| &*c.id).collect::<Vec<_>>(),
        ["big"]
    );
    assert_eq!(
        out.components.iter().map(|c| &*c.id).collect::<Vec<_>>(),
        ["big/a", "big/b"]
    );
    assert_eq!(
        out.edges
            .iter()
            .map(|e| (&*e.from, &*e.to))
            .collect::<Vec<_>>(),
        [("big/a", "big/b")]
    );
    assert_eq!(
        out.files.iter().map(|f| &*f.path).collect::<Vec<_>>(),
        ["big/a/x.rs"]
    );
}

#[test]
fn hiding_outside_drops_people_externals_and_their_relations_and_groups_off_drops_groups() {
    let d: SystemDescDto = serde_json::from_value(serde_json::json!({
        "summary": "", "containers": [], "tiers": [], "components": [],
        "people": [{"name": "User", "description": ""}],
        "externals": [{"name": "GitHub", "kind": "API", "description": ""}],
        "groups": [{"name": "App", "description": "", "containers": ["src"]}],
        "relations": [
            {"from": "User", "to": "src", "label": ""},
            {"from": "src", "to": "GitHub", "label": ""},
            {"from": "src", "to": "db", "label": ""}
        ],
        "commit": "c", "createdAt": 0.0
    }))
    .unwrap();
    let out = viewed(
        d,
        &view(|v| {
            v.outside = false;
            v.groups = false;
        }),
    );
    assert!(out.people.is_empty() && out.externals.is_empty() && out.groups.is_empty());
    assert_eq!(
        out.relations
            .iter()
            .map(|r| (&*r.from, &*r.to))
            .collect::<Vec<_>>(),
        [("src", "db")]
    );
}
