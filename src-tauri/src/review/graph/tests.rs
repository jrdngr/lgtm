use super::*;
use crate::review::tests::{jj, repo, write};

fn node<'a>(g: &'a GraphDto, name: &str) -> &'a GraphNode {
    g.nodes.iter().find(|n| n.name == name).unwrap()
}

fn linked(g: &GraphDto, from: &str, to: &str) -> bool {
    let (from, to) = (node(g, from).id, node(g, to).id);
    g.edges.contains(&GraphEdge { from, to })
}

#[tokio::test]
async fn links_changed_symbols_and_finds_unchanged_callers() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(
        d,
        "a.rs",
        "fn helper() -> u32 {\n    1\n}\n\nfn gone() {}\n\nfn untouched() {}\n",
    );
    write(d, "b.rs", "fn uses_helper() -> u32 {\n    helper()\n}\n");
    jj(d, &["commit", "-m", "base"]);
    write(
        d,
        "a.rs",
        "fn helper() -> u32 {\n    2\n}\n\nfn fresh() -> u32 {\n    helper()\n}\n\nfn untouched() {}\n",
    );

    let review = ctx.open(None, "@", false).await.unwrap();
    let g = ctx.graph(review.id).await.unwrap();

    assert_eq!(node(&g, "helper").status, NodeStatus::Modified);
    assert_eq!(node(&g, "fresh").status, NodeStatus::Added);
    assert_eq!(node(&g, "gone").status, NodeStatus::Removed);
    assert!(g.nodes.iter().all(|n| n.name != "untouched"));
    assert!(linked(&g, "fresh", "helper"));

    let caller = node(&g, "uses_helper");
    assert_eq!((caller.status, caller.file), (NodeStatus::Caller, None));
    assert_eq!(caller.path, "b.rs");
    assert!(linked(&g, "uses_helper", "helper"));
}

#[tokio::test]
async fn method_keeps_its_enclosing_type() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(
        d,
        "a.rs",
        "struct S;\n\nimpl S {\n    fn run(&self) {}\n}\n",
    );
    jj(d, &["commit", "-m", "base"]);
    write(
        d,
        "a.rs",
        "struct S;\n\nimpl S {\n    fn run(&self) {\n        let _ = 1;\n    }\n}\n",
    );

    let review = ctx.open(None, "@", false).await.unwrap();
    let g = ctx.graph(review.id).await.unwrap();
    let run = node(&g, "run");
    assert_eq!(run.status, NodeStatus::Modified);
    assert_eq!(run.parent.as_deref(), Some("S"));
    assert_eq!(g.nodes.len(), 1, "{:?}", g.nodes);
}

/// Times the graph of a real repo's `@`. Run with `LGTM_GRAPH_REPO=<path> --ignored`.
#[tokio::test]
#[ignore]
async fn graph_of_a_real_repo() {
    let Some(root) = std::env::var_os("LGTM_GRAPH_REPO") else {
        return;
    };
    let root = std::path::PathBuf::from(root);
    let ctx = RepoCtx::new(
        root.clone(),
        crate::jj::Jj::new(&root),
        crate::store::Store::in_memory(),
    );
    let review = ctx.open(None, "@", false).await.unwrap();
    if let Some(out) = std::env::var_os("LGTM_REVIEW_OUT") {
        std::fs::write(out, serde_json::to_string(&review).unwrap()).unwrap();
    }
    let t = std::time::Instant::now();
    let g = ctx.graph(review.id).await.unwrap();
    let callers = g
        .nodes
        .iter()
        .filter(|n| n.status == NodeStatus::Caller)
        .count();
    eprintln!(
        "{:?}: {} nodes ({callers} callers), {} edges",
        t.elapsed(),
        g.nodes.len(),
        g.edges.len()
    );
    if let Some(out) = std::env::var_os("LGTM_GRAPH_OUT") {
        std::fs::write(out, serde_json::to_string(&g).unwrap()).unwrap();
    }
}

#[tokio::test]
async fn follows_callers_of_callers_and_counts_hops() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn helper() -> u32 {\n    1\n}\n");
    write(d, "b.rs", "fn uses_helper() -> u32 {\n    helper()\n}\n");
    write(
        d,
        "c.rs",
        "fn outer_caller() -> u32 {\n    uses_helper()\n}\n",
    );
    jj(d, &["commit", "-m", "base"]);
    write(d, "a.rs", "fn helper() -> u32 {\n    2\n}\n");

    let review = ctx.open(None, "@", false).await.unwrap();
    let g = ctx.graph(review.id).await.unwrap();

    assert_eq!(node(&g, "helper").hop, 0);
    assert_eq!(node(&g, "uses_helper").hop, 1);
    assert_eq!(node(&g, "outer_caller").hop, 2);
    assert!(linked(&g, "outer_caller", "uses_helper"));
    assert!(!linked(&g, "outer_caller", "helper"));
}
