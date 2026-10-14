use crate::review::tests::{repo, row_of, write};
use crate::store::MarkColor;

use super::*;

#[test]
fn names_are_safe_path_components() {
    assert!(validate_name("fix-login_2.b").is_ok());
    for bad in ["", "a b", ".hidden", "-x", "a/b", "é"] {
        assert!(validate_name(bad).is_err(), "{bad:?}");
    }
}

#[tokio::test]
async fn a_workspace_has_its_own_working_copy_and_progress() {
    let (dir, mut ctx) = repo();
    let base = tempfile::tempdir().unwrap();
    ctx.workspace_base = Some(base.path().to_path_buf());
    write(dir.path(), "main.rs", "fn main() {}\n");

    assert!(!ctx.create_workspace("t1").await.unwrap());
    let all = ctx.list_workspaces().await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(
        (all[0].name.as_deref(), all[1].name.as_deref()),
        (None, Some("t1"))
    );
    let ws = ctx.workspace(Some("t1")).await.unwrap();
    assert!(under(&ws.root, base.path()));

    // The workspace's `@` is its own: it doesn't see the main workspace's edit.
    write(&ws.root, "a.py", "x = 1\ny = 2\n");
    let review = ctx.open(Some("t1"), "@", false).await.unwrap();
    let paths: Vec<&str> = review.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(paths, ["a.py"]);
    let row = row_of(&review, 0, "x = 1", 0);
    ctx.mark(review.id, &[row], Some(MarkColor::Cyan), &[])
        .unwrap();

    let p = ctx.progress(Some("t1")).await.unwrap();
    assert_eq!((p.revset.as_str(), p.done, p.changed), ("@", 1, 2));
    assert_eq!(p.marks.len(), 1);
    let main = ctx.progress(None).await.unwrap();
    assert_eq!((main.done, main.changed), (0, 1));

    let root = ws.root.clone();
    ctx.delete_workspace("t1", || ()).await.unwrap();
    assert!(!root.exists());
    assert_eq!(ctx.list_workspaces().await.unwrap().len(), 1);
}

#[tokio::test]
async fn a_busy_workspace_refuses_delete_before_touching_anything() {
    let (_dir, mut ctx) = repo();
    let base = tempfile::tempdir().unwrap();
    ctx.workspace_base = Some(base.path().to_path_buf());
    ctx.create_workspace("t1").await.unwrap();
    let ws = ctx.workspace(Some("t1")).await.unwrap();
    let busy = ws.claim().unwrap();

    let mut claimed = false;
    assert!(ctx.delete_workspace("t1", || claimed = true).await.is_err());
    assert!(!claimed);
    assert!(ws.root.is_dir());
    assert_eq!(ctx.list_workspaces().await.unwrap().len(), 2);

    drop(busy);
    ctx.delete_workspace("t1", || claimed = true).await.unwrap();
    assert!(claimed);
}

#[tokio::test]
async fn gc_removes_forgotten_workspace_dirs_and_forgets_missing_ones() {
    let (dir, mut ctx) = repo();
    let base = tempfile::tempdir().unwrap();
    ctx.workspace_base = Some(base.path().to_path_buf());
    ctx.create_workspace("kept").await.unwrap();
    ctx.create_workspace("forgotten").await.unwrap();
    ctx.create_workspace("deleted").await.unwrap();
    let dir_of = |name: &str| ctx.workspace_dir().unwrap().join(name);
    crate::review::tests::jj(dir.path(), &["workspace", "forget", "forgotten"]);
    std::fs::remove_dir_all(dir_of("deleted")).unwrap();
    let stray = base.path().join("other/stray");
    std::fs::create_dir_all(&stray).unwrap();

    let log = gc::run(base.path(), &[dir.path().to_path_buf()]);
    assert_eq!(log.len(), 3, "{log:#?}");
    assert!(dir_of("kept").exists());
    assert!(!dir_of("forgotten").exists());
    assert!(!stray.exists());
    let names: Vec<String> = ctx
        .list_workspaces()
        .await
        .unwrap()
        .into_iter()
        .map(|t| t.title)
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names.contains(&"kept".to_string()));
}
