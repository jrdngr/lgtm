//! Talking to the lgterm this runs in: who's asking, and handing answers to agents.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use lgterm_proto::{Reply, Request};

const TIMEOUT: Duration = Duration::from_secs(2);

fn call(request: &Request) -> Result<String> {
    let (_, socket) = lgterm_proto::here().context("not in an lgterm terminal")?;
    match lgterm_proto::call(&socket, request, Some(TIMEOUT))? {
        Reply::Ok(text) => Ok(text),
        Reply::Err(e) => bail!(e),
    }
}

/// This terminal, as `t5`.
pub fn term() -> Option<String> {
    lgterm_proto::here().map(|(t, _)| t.to_string())
}

/// This terminal's tab name.
pub fn label() -> Option<String> {
    let me = term()?;
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(&call(&Request::Ls { json: true }).ok()?).ok()?;
    rows.iter()
        .find(|r| r["id"] == me.as_str())
        .and_then(|r| r["label"].as_str())
        .map(str::to_owned)
        .filter(|l| !l.trim().is_empty())
}

/// Who's running this: `--as`, else the tab's name, else the login name.
pub fn who(named: Option<String>) -> String {
    named
        .or_else(label)
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "user".into())
}

/// Queues `text` for the agent in terminal `to`.
pub fn send(to: &str, text: &str) -> Result<()> {
    call(&Request::Send {
        from: term(),
        to: to.to_owned(),
        text: text.to_owned(),
    })
    .map(|_| ())
}
