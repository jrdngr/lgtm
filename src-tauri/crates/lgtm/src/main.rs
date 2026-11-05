//! `lgtm`: terminal code review for jj, and the command Claude Code runs for lgtm's MCP server
//! and hooks.

mod agent;
mod edit;
mod mcp;
mod route;
mod snap;
mod tui;
mod turn_hook;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let pr = take_flag(&mut args, "--pr", "a PR number or URL");
    let revset = take_flag(&mut args, "-r", "a revset");
    let split = match args.iter().position(|a| a == tui::SPLIT_FLAG) {
        Some(i) => {
            args.remove(i);
            true
        }
        None => false,
    };
    let code = match args.first().map(String::as_str) {
        Some("mcp") => route::mcp(),
        Some("hook") => route::hook(),
        Some("gc") => route::gc(&args[1..]),
        Some("snap") => snap::run(&args[1..]),
        Some("-h" | "--help") => {
            println!(
                "usage: lgtm [--pr <number>] [-r <revset>]\n       lgtm <file>[:line] | <folder>\n       lgtm snap <label>\n       lgtm mcp | hook | gc"
            );
            0
        }
        _ if revset.is_some() && !args.is_empty() => {
            eprintln!("lgtm: -r opens a review, not a file");
            2
        }
        Some(arg) => tui::run(file(arg), pr, split),
        None => match revset {
            Some(r) => tui::run(tui::Start::Revset(r), pr, split),
            None => tui::run(tui::Start::Home, pr, split),
        },
    };
    std::process::exit(code);
}

/// Takes `flag` and the value after it out of `args`, exiting when the value is missing.
fn take_flag(args: &mut Vec<String>, flag: &str, needs: &str) -> Option<String> {
    let i = args.iter().position(|a| a == flag)?;
    let value = args.get(i + 1).cloned();
    args.drain(i..(i + 2).min(args.len()));
    if value.is_none() {
        eprintln!("lgtm: {flag} needs {needs}");
        std::process::exit(2);
    }
    value
}

/// The file an argument opens, with `:line` after it or not.
fn file(arg: &str) -> tui::Start {
    match arg.rsplit_once(':') {
        Some((path, line)) if line.parse::<u32>().is_ok() => {
            tui::Start::File(path.into(), line.parse().unwrap_or(1))
        }
        _ => tui::Start::File(arg.into(), 1),
    }
}
