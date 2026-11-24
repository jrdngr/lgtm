//! `lgtm`: terminal code review for jj, and under `lgtm agent`, the commands agents run.

mod agent;
mod agent_cmd;
mod edit;
mod help;
mod open_review;
mod pr_start;
mod register;
mod review_cmd;
mod route;
mod snap;
mod tui;
mod turn_hook;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // lgterm marks a copy it opens beside another, which runs like any other.
    args.retain(|a| a != tui::SPLIT_FLAG);
    let code = match args.first().map(String::as_str) {
        Some("agent") => agent_cmd::run(args[1..].to_vec()),
        Some("review") => review(args[1..].to_vec()),
        Some("issues") => issues(args[1..].to_vec()),
        Some("resolve") => match args.get(1) {
            Some(path) => tui::run(tui::Start::File(path.into(), 1), None),
            None => tui::run(tui::Start::Conflicts, None),
        },
        Some("-h" | "--help") => {
            println!("{}", help::HELP);
            0
        }
        Some(arg) if args.len() == 1 && !arg.starts_with('-') => tui::run(file(arg), None),
        Some(_) => {
            eprintln!("{}", help::HELP);
            2
        }
        None => tui::run(tui::Start::Editor, None),
    };
    std::process::exit(code);
}

/// `lgtm review [-r <revset>] [--pr <pr>]`: the working copy, a revset, or a PR's revisions.
fn review(mut args: Vec<String>) -> i32 {
    let pr = take_flag(&mut args, "--pr", "a PR number or URL");
    let revset = take_flag(&mut args, "-r", "a revset");
    if let Some(extra) = args.first() {
        eprintln!(
            "lgtm: review takes -r and --pr, not `{extra}`\n\n{}",
            help::HELP
        );
        return 2;
    }
    let revset = match (revset, &pr) {
        (Some(r), _) => r,
        (None, Some(pr)) => {
            let cwd = std::env::current_dir().unwrap_or_default();
            let Some(root) = lgtm_core::repo::root_of(&cwd) else {
                eprintln!("lgtm: not in a jj or git repo");
                return 1;
            };
            match pr_start::revset(&root, pr) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("lgtm: {e:#}");
                    return 1;
                }
            }
        }
        (None, None) => "@".into(),
    };
    tui::run(tui::Start::Revset(revset), pr)
}

/// `lgtm issues [-C <dir>] [init | answer ...]`: the issues screen, or the user's commands.
fn issues(args: Vec<String>) -> i32 {
    let screen = match args.as_slice() {
        [] => true,
        [c, dir] if c == "-C" => {
            if let Err(e) = std::env::set_current_dir(dir) {
                eprintln!("lgtm: {dir}: {e}");
                return 1;
            }
            true
        }
        _ => false,
    };
    if screen {
        return tui::run(tui::Start::Issues, None);
    }
    lgtm_issues::command(args, lgtm_issues::Side::User)
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
