//! `lgtm`: terminal code review for jj, and the command Claude Code runs for lgtm's MCP server
//! and hooks.

mod agent;
mod mcp;
mod route;
mod snap;
mod tui;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("mcp") => route::mcp(),
        Some("hook") => route::hook(),
        Some("gc") => route::gc(&args[1..]),
        Some("snap") => snap::run(&args[1..]),
        Some("-h" | "--help") => {
            println!("usage: lgtm [revset]\n       lgtm snap <label>\n       lgtm mcp | hook | gc");
            0
        }
        Some(revset) => tui::run(Some(revset.to_string())),
        None => tui::run(None),
    };
    std::process::exit(code);
}
