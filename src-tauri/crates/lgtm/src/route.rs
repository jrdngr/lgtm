//! Claude Code registers `lgtm mcp` and `lgtm hook` once. Inside a desktop app terminal they
//! hand over to the app's binary, which talks to the app. Anywhere else in a jj workspace
//! `mcp` serves the terminal app's tools, and `hook` records the agent's turns.

use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use rmcp::ServiceExt;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// The desktop app's binary, when this runs in one of its terminals.
fn app_terminal() -> Option<String> {
    var("LGTM_SOCKET")?;
    var("LGTM_APP_BIN")
}

/// Replaces this process with `bin args`, keeping stdio. Only returns on failure.
fn hand_over(bin: &str, args: &[String]) -> i32 {
    let e = Command::new(bin).args(args).exec();
    eprintln!("lgtm: couldn't run {bin}: {e}");
    1
}

pub fn mcp() -> i32 {
    if let Some(app) = app_terminal() {
        return hand_over(&app, &["mcp".into()]);
    }
    let socket = std::env::current_dir()
        .ok()
        .and_then(|d| crate::agent::workspace(&d))
        .and_then(|root| crate::agent::socket_path(&root));
    match socket {
        Some(socket) => crate::mcp::serve(socket),
        None => serve_stdio(Stub(
            "This session isn't in a jj workspace, so lgtm has nothing to offer.",
        )),
    }
}

pub fn hook() -> i32 {
    match app_terminal() {
        Some(app) => hand_over(&app, &["hook".into()]),
        None => crate::turn_hook::run(),
    }
}

/// The desktop app's binary: the one that started this terminal, else an installed app.
fn app_bin() -> Option<String> {
    if let Some(bin) = var("LGTM_APP_BIN") {
        return Some(bin);
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    let on_path = std::env::split_paths(&path).map(|d| d.join("lgtm-app"));
    let installed = PathBuf::from("/Applications/lgtm.app/Contents/MacOS/lgtm-app");
    on_path
        .chain([installed])
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
}

/// `lgtm gc` cleans up the desktop app's workspaces, so the app does it.
pub fn gc(args: &[String]) -> i32 {
    let Some(app) = app_bin() else {
        eprintln!("lgtm: the desktop app isn't installed");
        return 1;
    };
    let mut all = vec!["gc".to_string()];
    all.extend_from_slice(args);
    hand_over(&app, &all)
}

/// A server with no tools, explaining why.
struct Stub(&'static str);

impl rmcp::ServerHandler for Stub {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::default())
            .with_server_info(Implementation::new("lgtm", env!("CARGO_PKG_VERSION")))
            .with_instructions(self.0)
    }
}

/// Serves `server` on stdin and stdout until the client goes.
pub fn serve_stdio(server: impl rmcp::ServerHandler) -> i32 {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let served = rt.block_on(async {
        server
            .serve((tokio::io::stdin(), tokio::io::stdout()))
            .await?
            .waiting()
            .await?;
        anyhow::Ok(())
    });
    // Tokio's stdin read can't be cancelled, so waiting on it would keep this running.
    rt.shutdown_background();
    match served {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("lgtm mcp: {e:#}");
            1
        }
    }
}
