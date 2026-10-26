//! Against pyright when it's on PATH; skipped otherwise.

use super::*;

fn pyright() -> bool {
    let found = crate::tools::which("pyright-langserver").is_some()
        || crate::tools::which("basedpyright-langserver").is_some();
    if !found {
        eprintln!("no pyright on PATH; skipped");
    }
    found
}

fn at<'a>(root: &'a Path, file: &'a Path, line: u32, character: u32) -> At<'a> {
    At {
        root,
        file,
        text: None,
        line,
        character,
    }
}

#[tokio::test]
async fn finds_definitions_in_the_repo_and_in_libraries_and_hovers() {
    if !pyright() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::write(root.join("a.py"), "def greet(name: str) -> str:\n    return name\n").unwrap();
    let b = root.join("b.py");
    std::fs::write(&b, "import os\nfrom a import greet\ngreet(os.sep)\n").unwrap();

    let defs = definition(at(&root, &b, 2, 1)).await.unwrap().unwrap();
    let local = Place {
        path: "a.py".into(),
        external: false,
        line: 0,
        character: 4,
    };
    assert_eq!(defs, [local]);

    let lib = definition(at(&root, &b, 2, 7)).await.unwrap().unwrap();
    assert!(lib[0].external, "{lib:?}");
    assert!(shown_external(Path::new(&lib[0].path)));

    let shown = hover(at(&root, &b, 2, 1)).await.unwrap().unwrap();
    assert!(shown.contains("def greet(name: str) -> str"), "{shown}");

    // Unsaved text is what the server reads.
    let edited = At {
        text: Some("from a import greet\n\ngreet('x')\n".into()),
        ..at(&root, &b, 2, 1)
    };
    let defs = definition(edited).await.unwrap().unwrap();
    assert_eq!(defs[0].path, "a.py");
}

#[tokio::test]
async fn a_file_no_server_takes_gets_none() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("notes.md");
    std::fs::write(&f, "# hi\n").unwrap();
    assert_eq!(definition(at(dir.path(), &f, 0, 2)).await.unwrap(), None);
}
