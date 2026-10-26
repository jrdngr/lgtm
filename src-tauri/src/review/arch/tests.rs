//! Import extraction and resolution on real files, and grouping with roots.

use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use super::group::Grouping;
use super::inputs::snapshot;
use super::model::Snapshot;
use super::roots::{self, Root};
use super::scan::scan;

fn write(dir: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let p = dir.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
}

/// The working copy's resolved imports, as (from, to, line).
fn imports_of(files: &[(&str, &str)]) -> Vec<(String, String, u32)> {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), files);
    let wc = scan(dir.path(), &HashSet::new());
    let snap: Snapshot = snapshot(&wc.paths, &wc.imports, &wc.manifests);
    snap.into_iter()
        .flat_map(|(from, is)| is.into_iter().map(move |i| (from.clone(), i.to, i.line)))
        .collect()
}

fn triples(t: &[(&str, &str, u32)]) -> Vec<(String, String, u32)> {
    t.iter()
        .map(|(a, b, l)| (a.to_string(), b.to_string(), *l))
        .collect()
}

#[test]
fn rust_paths_resolve_to_the_deepest_module_file() {
    let got = imports_of(&[
        ("core/Cargo.toml", "[package]\nname = \"core\"\n"),
        (
            "core/src/lib.rs",
            "mod a;\nmod b;\npub use crate::a::Thing;\n",
        ),
        (
            "core/src/a/mod.rs",
            "mod inner;\nuse super::b::{self, helper as h};\nuse std::fmt;\n",
        ),
        (
            "core/src/a/inner.rs",
            "use crate::b::deep::X;\nfn f() {\n    super::super::b::run();\n    let _ = self::g;\n}\n",
        ),
        (
            "core/src/b.rs",
            "use crate::{\n    a::inner,\n    missing::Y,\n};\n",
        ),
        // Outside the crate's src, so `crate` means nothing here.
        ("core/build.rs", "use crate::b;\n"),
    ]);
    assert_eq!(
        got,
        triples(&[
            ("core/src/a/inner.rs", "core/src/b.rs", 1),
            ("core/src/a/inner.rs", "core/src/b.rs", 3),
            ("core/src/a/mod.rs", "core/src/b.rs", 2),
            ("core/src/b.rs", "core/src/a/inner.rs", 2),
            ("core/src/b.rs", "core/src/lib.rs", 3),
            ("core/src/lib.rs", "core/src/a/mod.rs", 3),
        ])
    );
}

#[test]
fn js_relative_specifiers_resolve_over_extensions_and_index_files() {
    let got = imports_of(&[
        ("web/package.json", "{\"name\": \"web\"}"),
        (
            "web/src/main.ts",
            "import { x } from \"./lib/util\";\nimport y from \"react\";\nexport * from \"./types.js\";\nconst z = await import(\"./lazy\");\n",
        ),
        ("web/src/lib/util.ts", "export const x = 1;\n"),
        ("web/src/types.ts", "export type T = 1;\n"),
        ("web/src/lazy/index.tsx", "export default 1;\n"),
        (
            "web/src/old.cjs",
            "const u = require('../src/lib/util');\nrequire('./gone');\n",
        ),
    ]);
    assert_eq!(
        got,
        triples(&[
            ("web/src/main.ts", "web/src/lazy/index.tsx", 4),
            ("web/src/main.ts", "web/src/lib/util.ts", 1),
            ("web/src/main.ts", "web/src/types.ts", 3),
            ("web/src/old.cjs", "web/src/lib/util.ts", 1),
        ])
    );
}

#[test]
fn python_and_go_imports_resolve_within_the_repo() {
    let got = imports_of(&[
        ("pkg/__init__.py", ""),
        (
            "pkg/a.py",
            "from . import b\nfrom .sub.c import f\nimport pkg.b as pb\nimport os\n",
        ),
        ("pkg/b.py", ""),
        ("pkg/sub/__init__.py", ""),
        ("pkg/sub/c.py", "from .. import b\n"),
        ("go.mod", "module example.com/m\n\ngo 1.22\n"),
        (
            "main.go",
            "package main\n\nimport (\n\t\"fmt\"\n\t\"example.com/m/util\"\n)\n",
        ),
        ("util/u.go", "package util\n"),
        ("util/u_test.go", "package util\n"),
    ]);
    assert_eq!(
        got,
        triples(&[
            ("main.go", "util/u.go", 5),
            ("pkg/a.py", "pkg/b.py", 1),
            ("pkg/a.py", "pkg/b.py", 3),
            ("pkg/a.py", "pkg/sub/c.py", 2),
            ("pkg/sub/c.py", "pkg/b.py", 1),
        ])
    );
}

#[test]
fn python_imports_stay_in_their_own_source_root() {
    let got = imports_of(&[
        ("services/a/pyproject.toml", "[project]\nname = \"a\"\n"),
        ("services/a/src/__init__.py", ""),
        ("services/a/src/routes/__init__.py", ""),
        ("services/a/src/routes/x.py", ""),
        (
            "services/a/src/main.py",
            "from src.routes.x import f\nfrom src.schema import S\nimport shared.util\n",
        ),
        ("services/b/pyproject.toml", "[project]\nname = \"b\"\n"),
        ("services/b/src/__init__.py", ""),
        ("services/b/src/routes/__init__.py", ""),
        ("services/b/src/schema.py", ""),
        (
            "services/b/src/app.py",
            "from src.routes import handler\nfrom src.schema import S\nfrom src.permissions import P\nimport common\n",
        ),
        // A package only one other root has, and one two other roots have.
        ("libs/one/pyproject.toml", "[project]\nname = \"one\"\n"),
        ("libs/one/shared/__init__.py", ""),
        ("libs/one/shared/util.py", ""),
        ("libs/one/common.py", ""),
        ("libs/two/pyproject.toml", "[project]\nname = \"two\"\n"),
        ("libs/two/common.py", ""),
    ]);
    assert_eq!(
        got,
        triples(&[
            ("services/a/src/main.py", "libs/one/shared/util.py", 3),
            ("services/a/src/main.py", "services/a/src/routes/x.py", 1),
            ("services/b/src/app.py", "services/b/src/routes/__init__.py", 1),
            ("services/b/src/app.py", "services/b/src/schema.py", 2),
        ])
    );
}

fn grouping() -> Grouping {
    let paths: BTreeSet<String> = [
        "core/Cargo.toml",
        "core/src/lib.rs",
        "core/src/jj/mod.rs",
        "core/src/review/mod.rs",
        "core/src/review/arch/mod.rs",
        "web/package.json",
        "web/app.ts",
        "web/ui/button.ts",
        "docs/gen.py",
        "top.rs",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let manifests = roots::Manifests(
        [
            (
                "core/Cargo.toml".into(),
                "[package]\nname = \"engine\"\n".into(),
            ),
            ("web/package.json".into(), "{\"name\": \"site\"}".into()),
        ]
        .into(),
    );
    let roots = roots::detect(&paths, &manifests, "repo");
    assert_eq!(
        roots,
        vec![
            Root {
                path: "core/src".into(),
                name: "engine".into()
            },
            Root {
                path: "web".into(),
                name: "site".into()
            },
        ]
    );
    Grouping::new(roots, "repo")
}

#[test]
fn roots_group_subfolders_and_loose_files() {
    let g = grouping();
    let comp = |p: &str| {
        let id = g.comp(p);
        (id.clone(), g.name(&id), g.container(&id))
    };
    let s = |a: &str, b: &str, c: &str| (a.to_string(), b.to_string(), c.to_string());
    assert_eq!(comp("core/src/lib.rs"), s("core/src", "engine", "core/src"));
    assert_eq!(
        comp("core/src/review/arch/mod.rs"),
        s("core/src/review", "review", "core/src")
    );
    assert_eq!(comp("web/ui/button.ts"), s("web/ui", "ui", "web"));
    assert_eq!(comp("web/app.ts"), s("web", "site", "web"));
    assert_eq!(comp("docs/gen.py"), s("docs", "docs", ""));
    assert_eq!(comp("top.rs"), s("", "repo", ""));
}

#[tokio::test]
async fn review_diff_history_and_checks_come_from_the_repo() {
    use crate::review::tests::{jj, repo};
    let (dir, ctx) = repo();
    let d = dir.path();
    write(
        d,
        &[
            ("Cargo.toml", "[package]\nname = \"app\"\n"),
            ("src/lib.rs", "mod db;\nmod web;\n"),
            ("src/db/mod.rs", "pub fn query() -> u32 {\n    1\n}\n"),
            (
                "src/web/mod.rs",
                "pub fn page() -> u32 {\n    crate::db::query()\n}\n",
            ),
        ],
    );
    jj(d, &["commit", "-m", "base"]);
    write(d, &[("src/auth/mod.rs", "pub fn check() {}\n")]);
    jj(d, &["commit", "-m", "add auth"]);
    // The new file doesn't exist at the first commit, which history reads it at.
    write(
        d,
        &[
            (
                "src/db/mod.rs",
                "use crate::auth::check;\n\npub fn query() -> u32 {\n    check();\n    2\n}\n",
            ),
            ("src/auth/form.rs", "fn form() {}\n"),
        ],
    );

    let review = ctx.open(None, "@- | @", false).await.unwrap();
    let a = ctx.arch(review.id, false, None).await.unwrap();
    let status = |id: &str| a.components.iter().find(|c| c.id == id).unwrap().status;
    assert_eq!(status("src/auth"), super::CompStatus::Added);
    assert_eq!(status("src/db"), super::CompStatus::Changed);
    assert_eq!(status("src/web"), super::CompStatus::Impacted);
    let e = a
        .edges
        .iter()
        .find(|e| e.from == "src/db" && e.to == "src/auth")
        .unwrap();
    assert_eq!(e.status, super::EdgeStatus::Added);
    assert_eq!(
        (e.imports[0].from_path.as_str(), e.imports[0].line),
        ("src/db/mod.rs", 1)
    );
    let layers: Vec<&str> = a.layers.iter().map(|l| l.title.as_str()).collect();
    assert_eq!(layers, ["auth", "db"]);

    let key = "rule:new-edge:src/db>src/auth";
    assert!(a.look.iter().any(|l| l.key == key));
    ctx.check_look(review.id, key, true).unwrap();
    let a = ctx.arch(review.id, false, None).await.unwrap();
    assert!(a.look.iter().find(|l| l.key == key).unwrap().checked);

    let history = ctx.arch_history(review.id).await.unwrap();
    let got: Vec<(&str, u32, u32)> = history
        .iter()
        .map(|h| (h.description.trim(), h.components_added, h.edges_added))
        .collect();
    assert_eq!(got, [("add auth", 1, 0), ("", 0, 1)]);

    // The first commit alone: db's import of auth isn't there yet.
    let range = super::ArchRange {
        from: history[0].parent.clone(),
        to: history[0].commit_id.clone(),
    };
    let a = ctx.arch(review.id, false, Some(range)).await.unwrap();
    let status = |id: &str| a.components.iter().find(|c| c.id == id).unwrap().status;
    assert_eq!(status("src/auth"), super::CompStatus::Added);
    assert_eq!(status("src/db"), super::CompStatus::Unchanged);
    assert!(a.edges.iter().all(|e| e.to != "src/auth"));
}

#[tokio::test]
async fn an_assessment_replaces_the_layers_and_unchecks_the_agent_items() {
    use crate::review::tests::{jj, repo};
    let (dir, ctx) = repo();
    let d = dir.path();
    write(
        d,
        &[
            ("Cargo.toml", "[package]\nname = \"app\"\n"),
            ("src/lib.rs", "mod db;\nmod web;\n"),
            ("src/db/mod.rs", "pub fn query() {}\n"),
            ("src/web/mod.rs", "pub fn page() {}\n"),
        ],
    );
    jj(d, &["commit", "-m", "base"]);
    write(
        d,
        &[(
            "src/web/mod.rs",
            "pub fn page() {\n    crate::db::query()\n}\n",
        )],
    );
    let review = ctx.open(None, "@", false).await.unwrap();
    let rule = "rule:new-edge:src/web>src/db";
    ctx.check_look(review.id, rule, true).unwrap();
    ctx.check_look(review.id, "agent:1", true).unwrap();

    let reply = serde_json::json!({
        "revset": "@", "score": 2, "risk": "high", "reason": "Web reaches into db.",
        "boundaries": [],
        "layers": [{ "title": "Page", "summary": "s", "components": ["src/web", "src/ghost"],
                     "ranges": [{ "path": "src/web/mod.rs", "start": 1, "end": 3 }] }],
        "look": [{ "source": "agent", "severity": "medium", "title": "Coupling",
                   "from": "src/web", "to": "src/db" }]
    });
    let prompt = ctx.assess_prompt(review.id).await.unwrap();
    let whole = ctx.review(review.id).unwrap().lock().unwrap().whole.clone();
    let from = whole.from.clone().unwrap();
    assert!(prompt.contains(&format!("from {from} to {}", whole.to)));
    let asked = (from.as_str(), whole.to.as_str());
    let wrong = ctx
        .save_assessment(review.id, "@-", asked, reply.clone())
        .await;
    assert!(wrong.is_err(), "the review shows another revset");
    let wrong = ctx
        .save_assessment(review.id, "@", (&from, "x"), reply.clone())
        .await;
    assert!(wrong.is_err(), "the prompt named other commits");
    // The revset moves before the reply, but the assessment is of the commits the review loaded.
    write(d, &[("src/web/extra.rs", "pub fn x() {}\n")]);
    jj(d, &["status"]);
    let a = ctx
        .save_assessment(review.id, "@", asked, reply.clone())
        .await
        .unwrap();
    assert_eq!(a.score, 2);
    assert!(!a.stale);

    let arch = ctx.arch(review.id, false, None).await.unwrap();
    assert_eq!(arch.assessment, Some(a));
    let layers: Vec<_> = arch
        .layers
        .iter()
        .map(|l| (l.title.as_str(), l.components.clone(), l.ranges[0].file))
        .collect();
    assert_eq!(layers, [("Page", vec!["src/web".to_string()], Some(0))]);
    let checked = |key: &str| arch.look.iter().find(|l| l.key == key).unwrap().checked;
    assert!(checked(rule));
    assert!(!checked("agent:1"));

    ctx.refresh(review.id, None).await.unwrap();
    let arch = ctx.arch(review.id, false, None).await.unwrap();
    assert!(
        arch.assessment.unwrap().stale,
        "the review now shows other commits"
    );
    // A reply to a prompt from before the refresh is refused.
    let Err(err) = ctx.save_assessment(review.id, "@", asked, reply).await else {
        panic!("an assessment of the old commits was stored");
    };
    assert!(err.to_string().contains("Assess again"), "{err}");
}
