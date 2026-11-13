//! Claude Code runs `lgtm mcp` and `lgtm hook`. In a jj workspace `mcp` serves the TUI's
//! tools, and `hook` records the agent's turns and, in an lgterm terminal, passes the hook on
//! to lgterm.

use rmcp::ServiceExt;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};

pub fn mcp() -> i32 {
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
    let input = crate::turn_hook::read_input();
    tell_lgterm(&input);
    crate::turn_hook::run_on(&input)
}

/// Passes a hook on to lgterm when this runs in one of its terminals, so it can show the
/// agent's status, and prints what lgterm has for the hook to say. Waits briefly for lgterm
/// to take it, so hooks arrive in order.
fn tell_lgterm(input: &str) {
    use lgterm_proto::{Reply, Request};

    let Some((term, socket)) = lgterm_proto::here() else {
        return;
    };
    let Ok(event) = serde_json::from_str(input) else {
        return;
    };
    let hook = Request::Hook {
        term: term.to_string(),
        event,
    };
    let wait = std::time::Duration::from_secs(2);
    if let Ok(Reply::Ok(out)) = lgterm_proto::call(&socket, &hook, Some(wait))
        && !out.is_empty()
    {
        println!("{out}");
    }
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
