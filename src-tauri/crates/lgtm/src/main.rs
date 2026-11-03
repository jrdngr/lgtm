//! `lgtm`: terminal code review for jj, and the command Claude Code runs for lgtm's MCP server
//! and hooks.

mod agent;
mod config;
mod edit;
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
                "usage: lgtm [--pr <number>] [revset]\n       lgtm <file>[:line]\n       lgtm snap <label>\n       lgtm mcp | hook | gc"
            );
            0
        }
        Some(arg) => tui::run(start(arg), pr),
        None => tui::run(tui::Start::Home, pr),
    };
    std::process::exit(code);
}

/// What an argument opens: a file there is, with `:line` after it or not, else a revset.
fn start(arg: &str) -> tui::Start {
    let (path, line) = match arg.rsplit_once(':') {
        Some((path, line)) if line.parse::<u32>().is_ok() => (path, line.parse().unwrap_or(1)),
        _ => (arg, 1),
    };
    if std::path::Path::new(path).is_file() {
        tui::Start::File(path.into(), line)
    } else {
        tui::Start::Revset(arg.to_string())
    }
}
