use super::*;
use crate::review::arch::system::{SysComponentDto, SysContainerDto};

fn map() -> SystemDto {
    SystemDto {
        repo: "app".into(),
        containers: vec![SysContainerDto {
            id: "src".into(),
            name: "app".into(),
            files: 3,
        }],
        components: vec![SysComponentDto {
            id: "src/ui".into(),
            name: "ui".into(),
            container: "src".into(),
            files: 3,
            entry: vec![],
        }],
        edges: vec![],
        files: vec![],
        description: None,
        stale: vec![],
        tours: vec![],
        grouped: true,
    }
}

fn step(node: &str) -> TourStepDto {
    TourStepDto {
        node: node.into(),
        path: None,
        symbol: None,
        line: None,
        to: None,
        text: "Text".into(),
    }
}

fn relation(from: &str, to: &str) -> RelationDto {
    RelationDto {
        from: from.into(),
        to: to.into(),
        label: "Uses".into(),
    }
}

#[test]
fn validate_drops_unknown_components_tiers_relation_ends_and_grouped_containers_resolves_group_names_and_keeps_the_last_tours()
 {
    let d = Description {
        summary: "An app".into(),
        people: vec![ActorDto {
            name: "User".into(),
            description: "Uses it".into(),
        }],
        externals: vec![ExternalDto {
            name: "GitHub".into(),
            kind: "API".into(),
            description: "PRs".into(),
        }],
        containers: vec![ContainerDescDto {
            id: "db".into(),
            name: "Store".into(),
            tech: "SQLite".into(),
            description: "State".into(),
        }],
        tiers: vec!["Entry".into()],
        components: vec![
            ComponentDescDto {
                id: "src/ui".into(),
                description: "Screens".into(),
                tier: Some("Nowhere".into()),
            },
            ComponentDescDto {
                id: "src/gone".into(),
                description: "Old".into(),
                tier: Some("Entry".into()),
            },
        ],
        groups: vec![
            GroupDto {
                name: "App".into(),
                description: "The app".into(),
                containers: vec!["src".into(), "nowhere".into(), "Store".into()],
            },
            GroupDto {
                name: "Again".into(),
                description: "Repeats".into(),
                containers: vec!["src".into()],
            },
        ],
        relations: vec![
            relation("User", "src"),
            relation("src", "db"),
            relation("src", "GitHub"),
            relation("src", "Slack"),
            relation("src", "src"),
        ],
    };
    let last = SystemDescDto {
        summary: String::new(),
        people: vec![],
        externals: vec![],
        containers: vec![],
        tiers: vec![],
        components: vec![],
        groups: vec![],
        relations: vec![],
        tour: vec![step("system")],
        features: vec![step("src/ui")],
        commit: "old".into(),
        created_at: 0.0,
    };
    let out = validate(d, &map(), "abc".into(), Some(last));
    assert_eq!(
        out.components
            .iter()
            .map(|c| (&*c.id, c.tier.as_deref()))
            .collect::<Vec<_>>(),
        [("src/ui", None)]
    );
    assert_eq!(
        out.relations
            .iter()
            .map(|r| (&*r.from, &*r.to))
            .collect::<Vec<_>>(),
        [("User", "src"), ("src", "db"), ("src", "GitHub")]
    );
    assert_eq!(
        out.tour.iter().map(|t| &*t.node).collect::<Vec<_>>(),
        ["system"]
    );
    assert_eq!(
        out.features.iter().map(|t| &*t.node).collect::<Vec<_>>(),
        ["src/ui"]
    );
    assert_eq!(
        out.groups
            .iter()
            .map(|g| (&*g.name, g.containers.clone()))
            .collect::<Vec<_>>(),
        [("App", vec!["src".to_string(), "db".to_string()])]
    );
    assert_eq!(out.commit, "abc");
}

#[test]
fn map_text_keeps_the_heaviest_edges() {
    let mut m = map();
    m.edges = (0..MAX_EDGES as u32 + 2)
        .map(|i| crate::review::arch::system::SysEdgeDto {
            from: format!("a{i}"),
            to: "b".into(),
            imports: i,
            path: String::new(),
            line: 1,
        })
        .collect();
    let text = map_text(&m);
    assert!(text.contains(&format!("`a{}` -> `b`", MAX_EDGES + 1)));
    assert!(!text.contains("`a0` -> `b`"));
    assert!(text.contains("and 2 lighter edges"));
}
