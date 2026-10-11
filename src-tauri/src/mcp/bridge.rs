//! `lgtm mcp`: what Claude Code runs as the `lgtm` MCP server. Inside an lgtm terminal it
//! pipes stdio to the app's socket. Anywhere else it's a server with no tools, so sessions
//! outside lgtm don't show a failed server.

use rmcp::ServiceExt;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;

use super::Hello;

pub fn main() -> i32 {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    match rt.block_on(run()) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("lgtm mcp: {e:#}");
            1
        }
    }
}

async fn run() -> anyhow::Result<()> {
    let var = |name| std::env::var(name).ok().filter(|v| !v.is_empty());
    let (Some(socket), Some(root)) = (var("LGTM_SOCKET"), var("LGTM_ROOT")) else {
        return stub("This session isn't in an lgtm terminal, so lgtm has nothing to offer.").await;
    };
    let Ok(mut stream) = UnixStream::connect(&socket).await else {
        return stub("lgtm isn't running, so it has nothing to offer.").await;
    };
    let hello = Hello {
        root,
        workspace: var("LGTM_WORKSPACE"),
    };
    stream
        .write_all(format!("{}\n", serde_json::to_string(&hello)?).as_bytes())
        .await?;
    let (mut from_app, mut to_app) = stream.into_split();
    let (mut stdin, mut stdout) = (tokio::io::stdin(), tokio::io::stdout());
    tokio::select! {
        r = tokio::io::copy(&mut stdin, &mut to_app) => r.map(drop)?,
        r = tokio::io::copy(&mut from_app, &mut stdout) => r.map(drop)?,
    }
    Ok(())
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

async fn stub(why: &'static str) -> anyhow::Result<()> {
    Stub(why)
        .serve((tokio::io::stdin(), tokio::io::stdout()))
        .await?
        .waiting()
        .await?;
    Ok(())
}
