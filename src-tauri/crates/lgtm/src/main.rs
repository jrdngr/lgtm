//! `lgtm`: terminal code review for jj, and the command Claude Code runs for lgtm's MCP server
//! and hooks.

mod agent;
mod config;
mod mcp;
mod route;
mod snap;
mod tui;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let pr = args.iter().position(|a| a == "--pr").map(|i| {
        let pr = args.get(i + 1).cloned();
        args.drain(i..(i + 2).min(args.len()));
        pr
    });
    if let Some(None) = pr {
        eprintln!("lgtm: --pr needs a PR number or URL");
        std::process::exit(2);
    }
    let pr = pr.flatten();
    let code = match args.first().map(String::as_str) {
        Some("mcp") => route::mcp(),
        Some("hook") => route::hook(),
        Some("gc") => route::gc(&args[1..]),
        Some("snap") => snap::run(&args[1..]),
        Some("-h" | "--help") => {
            println!(
                "usage: lgtm [--pr <number>] [revset]\n       lgtm snap <label>\n       lgtm mcp | hook | gc"
            );
            0
        }
        Some(revset) => tui::run(Some(revset.to_string()), pr),
        None => tui::run(None, pr),
    };
    std::process::exit(code);
}
