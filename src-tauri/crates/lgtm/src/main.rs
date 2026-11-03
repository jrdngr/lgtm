//! `lgtm`: terminal code review for jj, and the command Claude Code runs for lgtm's MCP server
//! and hooks.

mod route;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match args.first().map(String::as_str) {
        Some("mcp") => route::mcp(),
        Some("hook") => route::hook(),
        Some("gc") => route::gc(&args[1..]),
        _ => {
            eprintln!("lgtm: the review UI isn't built yet");
            2
        }
    };
    std::process::exit(code);
}
