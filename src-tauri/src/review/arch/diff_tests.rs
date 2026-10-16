//! The component diff, intents and the default layers, over hand-built models. Without roots,
//! each top-level folder is a component.

use super::compute::{Stored, compute};
use super::group::Grouping;
use super::model::{Changed, Import, Model, Snapshot, Use};
use super::*;
use crate::diff::FileStatus;

fn snap(files: &[(&str, &[(&str, u32)])]) -> Snapshot {
    files
        .iter()
        .map(|(from, is)| {
            let is = is
                .iter()
                .map(|(to, line)| Import {
                    to: to.to_string(),
                    line: *line,
                })
                .collect();
            (from.to_string(), is)
        })
        .collect()
}

fn changed(path: &str, status: FileStatus) -> Changed {
    Changed {
        path: path.into(),
        old_path: None,
        status,
        file: None,
        ranges: vec![],
    }
}

fn caller(from: &str, hop: u32, to: &str) -> Use {
    Use {
        from_path: from.into(),
        from_changed: false,
        hop,
        to_path: to.into(),
        to_name: "sym".into(),
        to_line: 1,
        to_hop: hop - 1,
    }
}

/// `api` changes and gains `new`; `old` is removed; `ui` uses `api`; `app` uses `ui`; `db`
/// is used by `api`; `misc` is unrelated.
fn model() -> Model {
    let shared: &[(&str, &[(&str, u32)])] = &[
        ("ui/view.ts", &[("api/client.ts", 1)]),
        ("app/main.ts", &[("ui/view.ts", 2)]),
        ("db/store.ts", &[]),
        ("misc/x.ts", &[]),
    ];
    let mut base = snap(shared);
    base.extend(snap(&[
        ("api/client.ts", &[("old/legacy.ts", 3)]),
        ("old/legacy.ts", &[]),
    ]));
    let mut head = snap(shared);
    head.extend(snap(&[
        ("api/client.ts", &[("db/store.ts", 4), ("new/fresh.ts", 5)]),
        ("new/fresh.ts", &[]),
    ]));
    Model {
        base,
        head,
        changed: vec![
            changed("api/client.ts", FileStatus::Modified),
            changed("old/legacy.ts", FileStatus::Removed),
            changed("new/fresh.ts", FileStatus::Added),
        ],
        uses: vec![
            caller("ui/view.ts", 1, "api/client.ts"),
            caller("app/main.ts", 2, "ui/view.ts"),
        ],
        ..Model::default()
    }
}

fn run(m: &Model, lens: Lens, transitive: bool, intents: Vec<(u32, IntentKind)>) -> ArchDto {
    let g = Grouping::new(vec![], lens, "repo");
    let stored = Stored {
        intents,
        checked: Default::default(),
        assessed: None,
    };
    compute(m, &g, transitive, stored)
}

fn status(a: &ArchDto, id: &str) -> CompStatus {
    a.components.iter().find(|c| c.id == id).unwrap().status
}

fn edge<'a>(a: &'a ArchDto, from: &str, to: &str) -> &'a EdgeDto {
    a.edges
        .iter()
        .find(|e| e.from == from && e.to == to)
        .unwrap()
}

#[test]
fn statuses_follow_changes_imports_and_symbol_uses() {
    let a = run(&model(), Lens::default(), false, vec![]);
    let got: Vec<(&str, CompStatus)> = a
        .components
        .iter()
        .map(|c| (c.id.as_str(), c.status))
        .collect();
    use CompStatus::*;
    assert_eq!(
        got,
        [
            ("api", Changed),
            ("app", Unchanged),
            ("db", Upstream),
            ("misc", Unchanged),
            ("new", Added),
            ("old", Removed),
            ("ui", Impacted),
        ]
    );
    assert_eq!(edge(&a, "api", "new").status, EdgeStatus::Added);
    assert_eq!(edge(&a, "api", "old").status, EdgeStatus::Removed);
    assert_eq!(edge(&a, "api", "old").imports[0].line, 3);
    assert_eq!(edge(&a, "ui", "api").status, EdgeStatus::Same);
    assert!(edge(&a, "ui", "api").impacted);
    assert!(!edge(&a, "app", "ui").impacted);
    assert_eq!(
        a.counts,
        CountsDto {
            changed: 3,
            upstream: 1,
            impacted: 1
        }
    );

    let a = run(&model(), Lens::default(), true, vec![]);
    assert_eq!(status(&a, "app"), Impacted);
    assert!(edge(&a, "app", "ui").impacted);
}

#[test]
fn a_symbol_use_without_an_import_edge_does_not_impact() {
    let mut m = model();
    m.uses.push(caller("misc/x.ts", 1, "api/client.ts"));
    let a = run(&m, Lens::default(), false, vec![]);
    assert_eq!(status(&a, "misc"), CompStatus::Unchanged);
    let used = a
        .look
        .iter()
        .find(|l| l.key == "rule:symbol:api/client.ts#sym")
        .unwrap();
    assert_eq!(used.title, "sym used by ui");
}

#[test]
fn merged_components_join_and_drop_their_internal_edges() {
    let lens = Lens {
        merged: [("new".to_string(), "api".to_string())].into(),
        ..Lens::default()
    };
    let a = run(&model(), lens, false, vec![]);
    assert!(a.components.iter().all(|c| c.id != "new"));
    let api = a.components.iter().find(|c| c.id == "api").unwrap();
    let files: Vec<&str> = api.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(files, ["api/client.ts", "new/fresh.ts"]);
    assert!(a.edges.iter().all(|e| e.from != e.to));
}

#[test]
fn intents_are_broken_open_or_met_against_the_diff() {
    let wrong = |from: &str, to: &str| IntentKind::Wrong {
        from: from.into(),
        to: to.into(),
        reason: None,
    };
    let mv = |path: &str, to: &str| IntentKind::Move {
        path: path.into(),
        symbol: None,
        to: to.into(),
    };
    let mut m = model();
    let mut moved = changed("ui/form.ts", FileStatus::Renamed);
    moved.old_path = Some("app/form.ts".into());
    m.changed.push(moved);
    m.base.insert("app/form.ts".into(), vec![]);
    m.head.insert("ui/form.ts".into(), vec![]);
    let lens = Lens {
        merged: [("fresh".to_string(), "new".to_string())].into(),
        ..Lens::default()
    };
    let intents = vec![
        (1, wrong("api", "fresh")),
        (2, wrong("ui", "api")),
        (3, wrong("api", "old")),
        (4, mv("app/form.ts", "ui")),
        (5, mv("api/client.ts", "db")),
        (6, mv("app/main.ts", "db")),
        (
            7,
            IntentKind::Risk {
                component: "api".into(),
            },
        ),
    ];
    let a = run(&m, lens, false, intents);
    let states: Vec<IntentState> = a.intents.iter().map(|i| i.state).collect();
    use IntentState::*;
    assert_eq!(states, [Broken, Open, Met, Met, Broken, Open, Tag]);
    assert_eq!(edge(&a, "api", "new").wrong, Some(1));
    let comp = |id: &str| a.components.iter().find(|c| c.id == id).unwrap();
    assert!(comp("api").risk);
    assert_eq!(comp("api").misplaced, ["api/client.ts"]);
    assert_eq!(comp("app").misplaced, ["app/main.ts"]);
    let broken = a.look.iter().find(|l| l.key == "rule:broken:1").unwrap();
    assert_eq!(broken.severity, LookSeverity::High);
    assert_eq!(comp("api").severity, Some(LookSeverity::High));
}

#[test]
fn default_layers_put_dependencies_first() {
    // c imports b imports a; d is independent and sorts by name among the ready ones.
    let base = snap(&[
        ("a/x.rs", &[]),
        ("b/x.rs", &[]),
        ("c/x.rs", &[]),
        ("d/x.rs", &[]),
    ]);
    let head = snap(&[
        ("a/x.rs", &[]),
        ("b/x.rs", &[("a/x.rs", 1)]),
        ("c/x.rs", &[("b/x.rs", 1)]),
        ("d/x.rs", &[]),
    ]);
    let mut m = Model {
        base,
        head,
        ..Model::default()
    };
    for c in ["c", "a", "d", "b"] {
        let mut ch = changed(&format!("{c}/x.rs"), FileStatus::Modified);
        ch.ranges = vec![RangeDto {
            path: ch.path.clone(),
            file: Some(0),
            start: 2,
            end: 4,
        }];
        m.changed.push(ch);
    }
    let a = run(&m, Lens::default(), false, vec![]);
    let order: Vec<(&str, &str)> = a
        .layers
        .iter()
        .map(|l| (l.components[0].as_str(), l.summary.as_str()))
        .collect();
    assert_eq!(
        order,
        [
            ("a", "1 file"),
            ("b", "1 file"),
            ("c", "1 file"),
            ("d", "1 file")
        ]
    );
    assert_eq!(a.layers[2].ranges[0].path, "c/x.rs");
    let c = a.components.iter().find(|c| c.id == "c").unwrap();
    assert_eq!(c.layer, Some(3));
}

#[test]
fn assessed_layers_point_at_the_current_review_files() {
    let mut m = model();
    m.changed[0].file = Some(0);
    let range = |path: &str| RangeDto {
        path: path.into(),
        file: Some(7),
        start: 1,
        end: 2,
    };
    let assessed = assess::Assessed {
        assessment: AssessmentDto {
            score: 3,
            risk: RiskLevel::Low,
            reason: String::new(),
            boundaries: vec![],
            created_at: 0.0,
        },
        layers: vec![LayerDto {
            n: 1,
            title: "API".into(),
            summary: String::new(),
            components: vec!["api".into()],
            ranges: vec![range("api/client.ts"), range("gone.ts")],
        }],
        look: vec![],
    };
    let stored = Stored {
        intents: vec![],
        checked: Default::default(),
        assessed: Some(assessed),
    };
    let a = compute(
        &m,
        &Grouping::new(vec![], Lens::default(), "repo"),
        false,
        stored,
    );
    let files: Vec<Option<u32>> = a.layers[0].ranges.iter().map(|r| r.file).collect();
    assert_eq!(files, [Some(0), None]);
}
