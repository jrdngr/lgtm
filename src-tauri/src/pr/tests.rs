//! Fetching PR heads from a local stand-in for GitHub, and moving a workspace onto a new head.

use std::path::Path;
use std::process::Command;

use super::checkout::{Base, fetch};
use super::refetch::move_onto;
use crate::github::PrInfo;
use crate::github::pr::{HeadRepo, Login};
use crate::jj::Jj;
use crate::review::tests::{jj, repo, write};

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A git repo standing in for GitHub: `main`, and `feature` one commit on. Returns the
/// feature commit, also published as `refs/pull/7/head`.
fn upstream(dir: &Path) -> String {
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@t"]);
    git(dir, &["config", "user.name", "t"]);
    write(dir, "a.txt", "a\n");
    git(dir, &["add", "."]);
    git(dir, &["commit", "-qm", "base"]);
    git(dir, &["checkout", "-qb", "feature"]);
    write(dir, "a.txt", "a\nb\n");
    git(dir, &["commit", "-qam", "feature"]);
    let head = git(dir, &["rev-parse", "HEAD"]);
    git(dir, &["update-ref", "refs/pull/7/head", &head]);
    git(dir, &["checkout", "-q", "main"]);
    head
}

fn info(head: &str) -> PrInfo {
    PrInfo {
        id: "PR_x".into(),
        number: 7,
        title: "t".into(),
        url: String::new(),
        state: "OPEN".into(),
        author: None,
        base_ref_name: "main".into(),
        head_ref_name: "feature".into(),
        head_ref_oid: head.into(),
        is_cross_repository: false,
        head_repository: Some(HeadRepo {
            name: "r".into(),
            owner: Login { login: "o".into() },
        }),
    }
}

#[tokio::test]
async fn fetches_the_head_branch_or_falls_back_to_the_pull_ref() {
    let up = tempfile::tempdir().unwrap();
    let head = upstream(up.path());
    let (_dir, ctx) = repo();
    let j: &Jj = &ctx.main.jj;
    let url = up.path().to_string_lossy().to_string();
    j.remote_add("origin", &url).await.unwrap();
    let base = Base {
        remote: "origin".into(),
        url,
    };

    assert_eq!(
        fetch(j, &info(&head), &base)
            .await
            .unwrap()
            .head_remote
            .as_deref(),
        Some("origin")
    );
    assert!(j.has_commit(&head).await);

    git(up.path(), &["branch", "-qD", "feature"]);
    let mut gone = info(&head);
    gone.head_ref_oid = git(up.path(), &["rev-parse", "refs/pull/7/head"]);
    let (_dir2, ctx2) = repo();
    let j2: &Jj = &ctx2.main.jj;
    j2.remote_add("origin", &base.url).await.unwrap();
    assert_eq!(fetch(j2, &gone, &base).await.unwrap().head_remote, None);
    assert!(j2.has_commit(&head).await);
    assert_eq!(j2.bookmarks("lgtm/pr-*").await.unwrap(), ["lgtm/pr-7"]);
}

#[tokio::test]
async fn the_workspace_follows_the_new_head_with_its_own_commits() {
    let (dir, ctx) = repo();
    let d = dir.path();
    let j = &ctx.main.jj;
    write(d, "a.txt", "a\n");
    jj(d, &["commit", "-m", "old head"]);
    let old = j.commit_ids("@-").await.unwrap().remove(0);
    jj(d, &["new", "root()", "-m", "new head"]);
    write(d, "b.txt", "b\n");
    jj(d, &["new"]);
    let new = j.commit_ids("@-").await.unwrap().remove(0);

    // A workspace editing the old head itself gets a commit on the new one.
    jj(d, &["edit", &old]);
    move_onto(j, &old, &new).await.unwrap();
    assert_eq!(
        j.commit_ids("@-").await.unwrap(),
        std::slice::from_ref(&new)
    );

    // Its own commits above the old head move with their edits.
    jj(d, &["new", &old]);
    write(d, "mine.txt", "m\n");
    j.snapshot().await.unwrap();
    move_onto(j, &old, &new).await.unwrap();
    assert_eq!(
        j.commit_ids("@-").await.unwrap(),
        std::slice::from_ref(&new)
    );
    assert!(d.join("mine.txt").exists() && d.join("b.txt").exists());

    // Pushing its own commit makes that the new head, and nothing needs to move.
    let mine = j.commit_ids("@").await.unwrap().remove(0);
    jj(d, &["new"]);
    move_onto(j, &new, &mine).await.unwrap();
    assert_eq!(j.commit_ids("@-").await.unwrap(), [mine]);
}

/// Checks out a real fork PR on GitHub (read-only), so it's skipped unless asked for with
/// `--ignored`. `LGTM_SMOKE_PR` picks another PR of octocat/Hello-World.
#[tokio::test]
#[ignore]
async fn checks_out_and_pulls_a_real_fork_pr() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("hello");
    let out = Command::new("jj")
        .args(["git", "clone", "https://github.com/octocat/Hello-World"])
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut ctx = crate::ctx::RepoCtx::new(
        root.clone(),
        Jj::new(&root),
        crate::store::Store::in_memory(),
    );
    ctx.workspace_base = Some(dir.path().join("ws"));
    let pr = std::env::var("LGTM_SMOKE_PR").unwrap_or_else(|_| "11541".into());
    let name = ctx.open_pr(&pr).await.unwrap();
    let record = ctx.pr_record(Some(&name)).unwrap().unwrap();
    eprintln!("head from {:?}", record.head_remote);
    let review = ctx
        .open(Some(&name), &record.revset(), false)
        .await
        .unwrap();
    assert!(!review.files.is_empty(), "the PR's diff is empty");
    ctx.pull_pr_comments(review.id).await.unwrap();
    let d = ctx.pr_discussion(Some(&name)).await.unwrap();
    assert_eq!(d.pr.number, record.number);
    assert!(!ctx.pr_head(Some(&name)).await.unwrap().moved);
    ctx.refetch_pr(Some(&name), Some(review.id)).await.unwrap();
    ctx.delete_workspace(&name, || ()).await.unwrap();
    assert!(
        ctx.main
            .jj
            .remotes()
            .await
            .unwrap()
            .iter()
            .all(|(n, _)| !n.starts_with("gh-"))
    );
}

#[tokio::test]
async fn the_pr_view_leaves_out_changes_on_top_of_the_pr() {
    let (dir, ctx) = repo();
    let d = dir.path();
    let commit = |file: &str, bookmark: &str| {
        write(d, file, file);
        jj(d, &["commit", "-m", file]);
        jj(d, &["bookmark", "create", bookmark, "-r", "@-"]);
    };
    commit("base", "main");
    commit("pr", "feat/pr");
    jj(d, &["new", "feat/pr"]);
    write(d, "fix", "fix");
    jj(d, &["describe", "-m", "fix"]);
    let revset = format!("main..{}", super::head_revset("feat/pr"));
    let changes = ctx.main.jj.changes(&revset).await.unwrap();
    let got: Vec<_> = changes.into_iter().map(|c| c.description).collect();
    assert_eq!(got, ["pr"]);
}

#[tokio::test]
async fn off_the_stack_the_pr_view_is_the_prs_own_commits() {
    let (dir, ctx) = repo();
    let d = dir.path();
    let commit = |file: &str, bookmark: &str| {
        write(d, file, file);
        jj(d, &["commit", "-m", file]);
        jj(d, &["bookmark", "create", bookmark, "-r", "@-"]);
    };
    commit("base", "main");
    commit("one", "first");
    commit("two", "feat/pr");
    commit("three", "feat/next");
    // `@` went elsewhere, on trunk.
    jj(d, &["new", "main", "-m", "elsewhere"]);
    let revset = format!("main..{}", super::head_revset("feat/pr"));
    let changes = ctx.main.jj.changes(&revset).await.unwrap();
    let got: Vec<_> = changes.into_iter().map(|c| c.description).collect();
    assert_eq!(got, ["one", "two"]);
}

#[tokio::test]
async fn a_stack_prs_commits_stop_at_the_open_pr_below_it() {
    let up = tempfile::tempdir().unwrap();
    upstream(up.path());
    let (dir, ctx) = repo();
    let d = dir.path();
    let j: &Jj = &ctx.main.jj;
    j.remote_add("origin", &up.path().to_string_lossy())
        .await
        .unwrap();
    j.fetch("origin", &["main"]).await.unwrap();
    jj(d, &["new", "main@origin"]);
    for (file, bookmark) in [("one", "low"), ("two", "")] {
        write(d, file, file);
        jj(d, &["commit", "-m", file]);
        if !bookmark.is_empty() {
            jj(d, &["bookmark", "create", bookmark, "-r", "@-"]);
        }
    }
    write(d, "three", "three");
    jj(d, &["describe", "-m", "three"]);
    jj(d, &["bookmark", "create", "high", "-r", "@"]);
    let record = |below: Option<&str>| crate::store::PrRecord {
        workspace: String::new(),
        repo: crate::github::GhRepo {
            owner: "o".into(),
            name: "r".into(),
        },
        number: 2,
        id: String::new(),
        title: String::new(),
        url: String::new(),
        author: String::new(),
        base_ref: "main".into(),
        base_remote: "origin".into(),
        head_ref: "high".into(),
        head_remote: None,
        added_remote: None,
        head: String::new(),
        stack: true,
        below: below.map(String::from),
    };
    let described = |revset: String| async move {
        let changes = j.changes(&revset).await.unwrap();
        changes.into_iter().map(|c| c.description).collect::<Vec<_>>()
    };
    assert_eq!(described(record(None).revset()).await, ["one", "two", "three"]);
    assert_eq!(described(record(Some("low")).revset()).await, ["two", "three"]);
}
