//! `lgtm hook` outside the desktop app's terminals: records the agent's turns in the store,
//! as the app does, so both apps list them. It never fails, so it can't get in Claude Code's
//! way.

use std::io::Read;
use std::path::PathBuf;
use std::sync::Mutex;

use lgtm_core::jj::Jj;
use lgtm_core::turns::{self, Point};

/// The hook's input, read from stdin.
pub fn read_input() -> String {
    let mut input = String::new();
    std::io::stdin()
        .take(1 << 20)
        .read_to_string(&mut input)
        .ok();
    input
}

pub fn run_on(input: &str) -> i32 {
    let Ok(input) = serde_json::from_str::<serde_json::Value>(input) else {
        return 0;
    };
    // A hook that can't record leaves the turn out rather than fail.
    let _ = crate::snap::block_on(record(&input));
    0
}

async fn record(input: &serde_json::Value) -> anyhow::Result<()> {
    let Some(point) = turns::checkpoint(input) else {
        return Ok(());
    };
    let Some(session) = input["session_id"].as_str() else {
        return Ok(());
    };
    let dir = input["cwd"]
        .as_str()
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok());
    let Some(root) = dir.and_then(|d| crate::agent::workspace(&d)) else {
        return Ok(());
    };
    let Some(store) = crate::tui::store(&root) else {
        return Ok(());
    };
    let store = Mutex::new(store);
    let jj = Jj::new(&root);
    let key = turns::workspace_key(&jj, &root).await?;
    let (change, commit) = turns::at(&jj).await?;
    let prompt = (input["hook_event_name"] == "UserPromptSubmit")
        .then(|| input["prompt"].as_str())
        .flatten();
    let p = Point {
        key: &key,
        here: (&change, &commit),
        session,
        tool: input["tool_use_id"].as_str(),
        prompt,
    };
    turns::record(&store, &jj, &p, point).await?;
    Ok(())
}
