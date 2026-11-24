//! `lgtm agent <command>`: what agents, and Claude Code's hooks, run.

const USAGE: &str = "usage: lgtm agent <command>

  review [<command>]           read and mark the review open in lgtm (lgtm agent review --help)
  issues [<command>]           file, claim and work on issues (lgtm agent issues --help)
  snap <label>                 start a named step in the workspace's agent turns
  hook                         Claude Code's hook command; lgtm registers it itself";

pub fn run(mut args: Vec<String>) -> i32 {
    if args.is_empty() {
        eprintln!("{USAGE}");
        return 2;
    }
    let cmd = args.remove(0);
    match cmd.as_str() {
        "review" => {
            let revset = crate::take_flag(&mut args, "-r", "a revset");
            crate::review_cmd::run(args, revset)
        }
        "issues" => lgtm_issues::command(args, lgtm_issues::Side::Agent),
        "snap" => crate::snap::run(&args),
        "hook" => crate::route::hook(),
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            0
        }
        other => {
            eprintln!("lgtm: unknown agent command `{other}`\n{USAGE}");
            2
        }
    }
}
