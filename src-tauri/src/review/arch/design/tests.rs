use super::*;
use crate::review::tests::{jj, repo, write};

fn states(items: &[DesignItemDto]) -> Vec<(&str, DesignState)> {
    items
        .iter()
        .map(|i| (i.design.name.as_str(), i.state))
        .collect()
}

#[tokio::test]
async fn a_design_goes_stale_only_when_its_own_files_change_at_its_revision() {
    let (dir, ctx) = repo();
    let d = dir.path();
    std::fs::create_dir_all(d.join("docs")).unwrap();
    write(d, "docs/phase1.md", "one\n");
    write(d, "docs/phase2.md", "two\n");
    write(d, "notes.txt", "x\n");
    let design = |name: &str, file: &str| DesignInput {
        name: name.into(),
        revision: "@".into(),
        files: vec![file.into()],
        prompt: String::new(),
    };
    let empty = serde_json::json!({"summary": "", "people": [], "externals": [],
        "containers": [], "components": [], "relations": [], "differences": []});
    let one = ctx
        .create_design(None, design("One", "docs/phase1.md"))
        .await
        .unwrap();
    let two = ctx
        .create_design(None, design("Two", "docs/phase2.md"))
        .await
        .unwrap();
    assert_ne!(one.revision, "@", "stores the change id, not @");
    assert_eq!(
        states(&ctx.designs(None).await.unwrap()),
        [("One", DesignState::Missing), ("Two", DesignState::Missing)]
    );
    for id in [&one.id, &two.id] {
        ctx.save_design_map(None, id, empty.clone()).await.unwrap();
    }
    let fresh = [("One", DesignState::Fresh), ("Two", DesignState::Fresh)];
    assert_eq!(states(&ctx.designs(None).await.unwrap()), fresh);

    // An unrelated file amended into the revision, a new description, and @ moving on.
    write(d, "notes.txt", "y\n");
    jj(d, &["describe", "-m", "specs"]);
    jj(d, &["new"]);
    assert_eq!(states(&ctx.designs(None).await.unwrap()), fresh);

    jj(d, &["edit", "@-"]);
    write(d, "docs/phase1.md", "one, changed\n");
    assert_eq!(
        states(&ctx.designs(None).await.unwrap()),
        [("One", DesignState::Stale), ("Two", DesignState::Fresh)]
    );
    let listed = ctx.designs(None).await.unwrap();
    assert_eq!(listed[0].resolved.as_ref().unwrap().description, "specs");
}

#[tokio::test]
async fn create_design_marks_folders_allows_none_and_refuses_paths_not_at_the_bookmark() {
    let (dir, ctx) = repo();
    let d = dir.path();
    std::fs::create_dir_all(d.join("docs/notify")).unwrap();
    write(d, "docs/notify/a.md", "# A\n");
    write(d, "docs/plan.md", "# Plan\n");
    jj(d, &["bookmark", "create", "spec", "-r", "@"]);
    let input = |files: &[&str]| DesignInput {
        name: "Notify".into(),
        revision: "spec".into(),
        files: files.iter().map(|f| f.to_string()).collect(),
        prompt: String::new(),
    };

    let made = ctx
        .create_design(None, input(&["./docs/notify", "docs/plan.md"]))
        .await
        .unwrap();
    assert_eq!(made.files, ["docs/notify/", "docs/plan.md"]);
    assert_eq!(ctx.design(&made.id).unwrap().name, "Notify");

    let err = ctx
        .create_design(None, input(&["docs/notify/", "docs/gone.md"]))
        .await
        .unwrap_err();
    assert_eq!(err.to_string(), "not at `spec`: docs/gone.md");
    let empty = ctx.create_design(None, input(&[])).await.unwrap();
    assert!(empty.files.is_empty() && empty.map.is_none());
    assert!(
        ctx.create_design(
            None,
            DesignInput {
                revision: "nope".into(),
                ..input(&["docs/plan.md"])
            }
        )
        .await
        .is_err()
    );
}

#[test]
fn a_design_stored_with_a_bookmark_loads_with_it_as_its_revision() {
    let d = parse(
        r#"{"id":"d1","name":"N","bookmark":"spec","files":[],"prompt":"","createdAt":0,"map":null}"#,
    )
    .unwrap();
    assert_eq!(d.revision, "spec");
}

#[test]
fn covered_takes_files_and_everything_under_folders() {
    let all: Vec<String> = [
        "docs/design/a.md",
        "docs/design/b/c.md",
        "docs/designs.md",
        "x.md",
    ]
    .map(String::from)
    .to_vec();
    assert_eq!(
        covered(&all, &["docs/design/".into(), "x.md".into()]),
        ["docs/design/a.md", "docs/design/b/c.md", "x.md"]
    );
}
