//! `lgtm`: terminal code review for jj, the commands agents run to review with it, and the
//! command Claude Code runs for lgtm's hooks.

mod agent;
mod edit;
mod help;
mod open_review;
mod register;
mod review_cmd;
mod route;
mod snap;
mod tui;
mod turn_hook;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let pr = take_flag(&mut args, "--pr", "a PR number or URL");
    let revset = take_flag(&mut args, "-r", "a revset");
    // lgterm marks a copy it opens beside another, which runs like any other.
    args.retain(|a| a != tui::SPLIT_FLAG);
    let code = match args.first().map(String::as_str) {
        Some("review") => review_cmd::run(args[1..].to_vec(), revset.clone()),
        Some("hook") => route::hook(),
        Some("snap") => snap::run(&args[1..]),
        Some("issues") if args.len() > 1 => lgtm_issues::command(args[1..].to_vec()),
        Some("issues") => tui::run(tui::Start::Issues, None),
        Some("resolve") => match args.get(1) {
            Some(path) => tui::run(tui::Start::File(path.into(), 1), None),
            None => tui::run(tui::Start::Conflicts, None),
        },
        Some("-h" | "--help") => {
            println!("{}", help::HELP);
            0
        }
        _ if revset.is_some() && !args.is_empty() => {
            eprintln!("lgtm: -r opens a review, not a file");
            2
        }
        Some(arg) => tui::run(file(arg), pr),
        None => match revset {
            Some(r) => tui::run(tui::Start::Revset(r), pr),
            None => tui::run(tui::Start::Editor, pr),
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
