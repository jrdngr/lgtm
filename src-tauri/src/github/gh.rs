//! Runs `gh`, which handles GitHub auth and hosts, so the app never sees a token.

use std::process::Stdio;

use anyhow::{Context, bail};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use super::GhRepo;

async fn run(args: &[&str], input: Option<&[u8]>) -> anyhow::Result<Vec<u8>> {
    let mut child = Command::new("gh")
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("couldn't start `gh`. Is the GitHub CLI installed?")?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin.write_all(input).await?;
    }
    let out = child.wait_with_output().await?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        // GraphQL errors come back on stdout with a failing status.
        let body = String::from_utf8_lossy(&out.stdout);
        bail!(
            "gh {}: {}",
            args[..2.min(args.len())].join(" "),
            graphql_error(&body).unwrap_or_else(|| err.trim().to_string())
        );
    }
    Ok(out.stdout)
}

/// The first message of a GraphQL error response.
fn graphql_error(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body).ok()?;
    Some(v.pointer("/errors/0/message")?.as_str()?.to_string())
}

/// Runs a GraphQL query or mutation and returns its `data`.
pub async fn graphql(query: &str, variables: Value) -> anyhow::Result<Value> {
    let body = serde_json::to_vec(&json!({ "query": query, "variables": variables }))?;
    let out = run(&["api", "graphql", "--input", "-"], Some(&body)).await?;
    let mut v: Value = serde_json::from_slice(&out).context("gh returned bad JSON")?;
    if let Some(msg) = graphql_error(&String::from_utf8_lossy(&out)) {
        bail!("GitHub: {msg}");
    }
    Ok(v["data"].take())
}

/// A GET on the REST API, like `repos/o/n/compare/a...b`.
pub async fn rest(path: &str) -> anyhow::Result<Value> {
    let out = run(&["api", path], None).await?;
    serde_json::from_slice(&out).context("gh returned bad JSON")
}

/// A PR's diff against its base, as GitHub shows it.
pub async fn pr_diff(repo: &GhRepo, number: u32) -> anyhow::Result<String> {
    let (repo, number) = (repo.to_string(), number.to_string());
    let out = run(
        &["pr", "diff", &number, "--repo", &repo, "--color", "never"],
        None,
    )
    .await?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}

/// Opens a PR's page in the default browser.
pub async fn browse_pr(repo: &GhRepo, number: u32) -> anyhow::Result<()> {
    let (repo, number) = (repo.to_string(), number.to_string());
    run(&["pr", "view", &number, "--repo", &repo, "--web"], None).await?;
    Ok(())
}

/// Takes `T` out of `v` at `pointer`, failing with what was missing.
pub fn at<T: serde::de::DeserializeOwned>(v: &mut Value, pointer: &str) -> anyhow::Result<T> {
    let found = v
        .pointer_mut(pointer)
        .filter(|x| !x.is_null())
        .with_context(|| format!("GitHub's reply has no {pointer}"))?
        .take();
    serde_json::from_value(found)
        .with_context(|| format!("GitHub's {pointer} has an unexpected shape"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphql_errors_and_paths() {
        let body = r#"{"data":null,"errors":[{"message":"Could not resolve"}]}"#;
        assert_eq!(graphql_error(body).as_deref(), Some("Could not resolve"));
        assert_eq!(graphql_error(r#"{"data":{}}"#), None);
        let mut v = json!({"a": {"b": 3, "c": null}});
        assert_eq!(at::<u32>(&mut v, "/a/b").unwrap(), 3);
        assert!(at::<u32>(&mut v, "/a/c").is_err());
    }
}
