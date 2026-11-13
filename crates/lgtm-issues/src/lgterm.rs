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

fn terminals() -> Option<Vec<serde_json::Value>> {
    serde_json::from_str(&call(&Request::Ls { json: true }).ok()?).ok()
}

/// This terminal's tab name.
pub fn label() -> Option<String> {
    let me = term()?;
    terminals()?
        .iter()
        .find(|r| r["id"] == me.as_str())
        .and_then(|r| r["label"].as_str())
        .map(str::to_owned)
        .filter(|l| !l.trim().is_empty())
}

/// A terminal by its id, like `t5`, or its tab name.
fn resolve(to: &str) -> Result<String> {
    let id = |r: &serde_json::Value| r["id"].as_str().map(str::to_owned);
    let rows = terminals().context("can't list lgterm's terminals")?;
    rows.iter()
        .find(|r| r["id"] == to)
        .or_else(|| rows.iter().find(|r| r["label"] == to))
        .and_then(id)
        .with_context(|| format!("no terminal {to}"))
}

/// Who's running this: `--as`, else the tab's name, else the login name.
pub fn who(named: Option<String>) -> String {
    named
        .or_else(label)
        .or_else(|| std::env::var("USER").ok())
        .unwrap_or_else(|| "user".into())
}

/// Queues `text` for the agent in terminal `to`, an id or a tab name.
pub fn send(to: &str, text: &str) -> Result<()> {
    call(&Request::Send {
        from: term(),
        to: resolve(to)?,
        text: text.to_owned(),
    })
    .map(|_| ())
}
