use crate::review::tests::{repo, write};

use super::*;

#[test]
fn names_are_safe_path_components() {
    assert!(validate_name("fix-login_2.b").is_ok());
    for bad in ["", "a b", ".hidden", "-x", "a/b", "é"] {
        assert!(validate_name(bad).is_err(), "{bad:?}");
    }
}

#[tokio::test]
async fn a_workspace_has_its_own_working_copy() {
    let (dir, mut ctx) = repo();
    let base = tempfile::tempdir().unwrap();
    ctx.workspace_base = Some(base.path().to_path_buf());
    write(dir.path(), "main.rs", "fn main() {}\n");

    assert_eq!(ctx.create_workspace("t1").await.unwrap(), Setup::None);
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
async fn a_workspace_whose_files_are_gone_can_still_be_deleted() {
    let (_dir, mut ctx) = repo();
    let base = tempfile::tempdir().unwrap();
    ctx.workspace_base = Some(base.path().to_path_buf());
    ctx.create_workspace("t1").await.unwrap();
    let root = ctx.workspace(Some("t1")).await.unwrap().root.clone();
    std::fs::remove_dir_all(&root).unwrap();
    let listed = ctx.list_workspaces().await.unwrap();
    assert!(listed.iter().any(|w| w.name.as_deref() == Some("t1") && w.missing));

    ctx.delete_workspace("t1", || {}).await.unwrap();
    let listed = ctx.list_workspaces().await.unwrap();
    assert!(listed.iter().all(|w| w.name.as_deref() != Some("t1")));
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

    let log = gc::run(base.path(), &[dir.path().to_path_buf()], false);
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

#[tokio::test]
async fn gc_keeps_a_workspace_whose_repo_moved_unless_forced() {
    let parent = tempfile::tempdir().unwrap();
    let (dir, mut ctx) = repo();
    let base = tempfile::tempdir().unwrap();
    ctx.workspace_base = Some(base.path().to_path_buf());
    ctx.create_workspace("t1").await.unwrap();
    let ws = ctx.workspace(Some("t1")).await.unwrap().root.clone();
    std::fs::rename(dir.path(), parent.path().join("moved")).unwrap();

    let log = gc::run(base.path(), &[], false);
    assert!(ws.is_dir(), "{log:#?}");
    gc::run(base.path(), &[], true);
    assert!(!ws.exists());
}

#[tokio::test]
async fn same_named_repos_can_have_same_named_workspaces() {
    let dir = tempfile::tempdir().unwrap();
    let base = tempfile::tempdir().unwrap();
    for parent in ["work", "oss"] {
        let root = dir.path().join(parent).join("app");
        std::fs::create_dir_all(&root).unwrap();
        let init = std::process::Command::new("jj")
            .args(["git", "init"])
            .arg(&root)
            .output()
            .unwrap();
        assert!(init.status.success());
        let mut ctx = RepoCtx::new(
            root.clone(),
            Jj::new(&root),
            crate::store::Store::in_memory(),
        );
        ctx.workspace_base = Some(base.path().to_path_buf());
        ctx.create_workspace("fix").await.unwrap();
    }
}

#[tokio::test]
async fn a_recreated_workspace_starts_without_the_old_ones_history() {
    let (_dir, mut ctx) = repo();
    let base = tempfile::tempdir().unwrap();
    ctx.workspace_base = Some(base.path().to_path_buf());
    ctx.create_workspace("t1").await.unwrap();
    {
        let store = ctx.store.lock().unwrap();
        store.save_qa_session("t1", "q1", "{}").unwrap();
        store.start_session("@", "t1", "@-", 0).unwrap();
    }
    ctx.delete_workspace("t1", || ()).await.unwrap();
    ctx.create_workspace("t1").await.unwrap();

    let store = ctx.store.lock().unwrap();
    assert!(store.qa_sessions("t1").unwrap().is_empty());
    assert_eq!(store.last_revset("t1").unwrap(), None);
}

#[tokio::test]
async fn adding_a_taken_name_keeps_the_live_workspaces_history() {
    let (_dir, mut ctx) = repo();
    let base = tempfile::tempdir().unwrap();
    ctx.workspace_base = Some(base.path().to_path_buf());
    let elsewhere = tempfile::tempdir().unwrap();
    let t1 = elsewhere.path().join("t1");
    ctx.main.jj.workspace_add("t1", &t1, "@").await.unwrap();
    ctx.store
        .lock()
        .unwrap()
        .save_qa_session("t1", "q1", "{}")
        .unwrap();

    assert!(ctx.create_workspace("t1").await.is_err());
    assert_eq!(
        ctx.store.lock().unwrap().qa_sessions("t1").unwrap().len(),
        1
    );
}

#[tokio::test]
async fn only_workspaces_the_ui_opened_are_watched_and_once_each() {
    let (_dir, mut ctx) = repo();
    let base = tempfile::tempdir().unwrap();
    ctx.workspace_base = Some(base.path().to_path_buf());
    *ctx.emit.lock().unwrap() = Some(Arc::new(|_, _| {}));
    ctx.create_workspace("t1").await.unwrap();
    let first = ctx.workspace(Some("t1")).await.unwrap();
    assert!(first.watch.lock().unwrap().is_none());

    ctx.open_workspace(Some("t1")).await.unwrap();
    assert!(first.watch.lock().unwrap().is_some());

    // Forgotten and added again elsewhere, as a terminal might: the old one stops, and the
    // new one is watched in its place.
    let again = base.path().join("t1-again");
    crate::jj::test_support::jj(&ctx.root, &["workspace", "forget", "t1"]);
    crate::jj::test_support::jj(
        &ctx.root,
        &["workspace", "add", "--name", "t1", again.to_str().unwrap()],
    );
    ctx.list_workspaces().await.unwrap();
    let second = ctx.workspace(Some("t1")).await.unwrap();
    assert!(!Arc::ptr_eq(&first, &second));
    assert!(first.watch.lock().unwrap().is_none());
    assert!(second.watch.lock().unwrap().is_some());
}
