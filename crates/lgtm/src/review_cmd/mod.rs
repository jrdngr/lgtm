//! `lgtm agent review`: agents read and mark a review, and read and leave comments, in the store
//! the app shows.

mod args;
mod guide;
mod load;
pub mod skill;

use lgtm_core::review::agent::{AddComment, AgentMark, CheckFiles, MarkLines, author};

use crate::agent::Request;
use args::Args;

const USAGE: &str = "usage: lgtm agent review [-r <revset>]
       lgtm agent review check [-r <revset>] [--uncheck] <path>...
       lgtm agent review mark [-r <revset>] <path> [<lines>] [--removed <lines>] --reviewed|--flagged|--clear
       lgtm agent review comments [-r <revset>]
       lgtm agent review comment [-r <revset>] <path>:<line> [--removed] [--as <name>] [<text>]
       lgtm agent review resolve|reopen <C3>...
       lgtm agent review guide";

/// Runs `lgtm agent review` with the arguments after it. `revset` is `-r`'s.
pub fn run(args: Vec<String>, revset: Option<String>) -> i32 {
    match run_args(args, revset) {
        Ok(out) => {
            println!("{out}");
            0
        }
        Err(e) => {
            eprintln!("lgtm: {e}");
            1
        }
    }
}

fn run_args(args: Vec<String>, revset: Option<String>) -> Result<String, String> {
    let mut args = Args::new(args);
    let cmd = args.next_word();
    if matches!(cmd.as_deref(), Some("guide")) {
        return Ok(guide::GUIDE.trim_end().to_string());
    }
    if matches!(cmd.as_deref(), Some("-h" | "--help" | "help")) {
        return Ok(USAGE.into());
    }
    let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let root = lgtm_core::repo::root_of(&cwd).ok_or("not in a jj or git repo")?;
    let mut store = crate::tui::store(&root);
    let req = match cmd.as_deref() {
        None => Request::GetReview,
        Some("comments") => Request::GetComments,
        Some("check") => {
            let checked = !args.flag("--uncheck");
            let paths = args.rest()?;
            if paths.is_empty() {
                return Err("name the files to check".into());
            }
            Request::CheckFiles(CheckFiles { paths, checked })
        }
        Some("mark") => Request::MarkLines(mark(&mut args)?),
        Some("comment") => Request::AddComment(comment(&mut args)?),
        Some(verb @ ("resolve" | "reopen")) => {
            let store = store.as_mut().ok_or("lgtm couldn't open its store")?;
            return resolve(store, args.rest()?, verb == "resolve");
        }
        Some(other) => return Err(format!("unknown command `{other}`\n{USAGE}")),
    };
    let shown = load::shown(&root, revset);
    let mut review = load::review(&root, shown, store.as_ref()).map_err(|e| format!("{e:#}"))?;
    review.answer(req, store.as_mut())
}

fn mark(args: &mut Args) -> Result<MarkLines, String> {
    let removed = args.value("--removed")?;
    let reviewed = args.flag("--reviewed");
    let flagged = args.flag("--flagged");
    let clear = args.flag("--clear");
    let mark = match (reviewed, flagged, clear) {
        (true, false, false) => Some(AgentMark::Reviewed),
        (false, true, false) => Some(AgentMark::Flagged),
        (false, false, true) => None,
        _ => return Err("give one of --reviewed, --flagged or --clear".into()),
    };
    let mut rest = args.rest()?.into_iter();
    let path = rest.next().ok_or("name the file to mark")?;
    let lines = rest.next();
    if rest.next().is_some() {
        return Err("give the lines as one list, like 12,20-30".into());
    }
    let list = |s: Option<String>| -> Vec<String> {
        s.iter()
            .flat_map(|s| s.split(','))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    };
    let (lines, removed_lines) = (list(lines), list(removed));
    if lines.is_empty() && removed_lines.is_empty() {
        return Err("give the lines to mark, like 12,20-30".into());
    }
    Ok(MarkLines {
        path,
        lines,
        removed_lines,
        mark,
    })
}

fn comment(args: &mut Args) -> Result<AddComment, String> {
    let removed = args.flag("--removed");
    let named = args.value("--as")?;
    let mut rest = args.rest()?.into_iter();
    let at = rest.next().ok_or("give the line as <path>:<line>")?;
    let (path, line) = at
        .rsplit_once(':')
        .and_then(|(p, l)| Some((p.to_string(), l.parse::<u32>().ok()?)))
        .ok_or("give the line as <path>:<line>")?;
    let text: Vec<String> = rest.collect();
    let body = if text.is_empty() {
        std::io::read_to_string(std::io::stdin()).map_err(|e| e.to_string())?
    } else {
        text.join(" ")
    };
    let claude_code = std::env::var_os("CLAUDECODE").is_some_and(|v| !v.is_empty());
    Ok(AddComment {
        path,
        line: (!removed).then_some(line),
        removed_line: removed.then_some(line),
        body,
        author: author(named.as_deref(), claude_code),
    })
}

fn resolve(
    store: &mut lgtm_core::store::Store,
    numbers: Vec<String>,
    done: bool,
) -> Result<String, String> {
    if numbers.is_empty() {
        return Err("name the comments, like C3".into());
    }
    let mut said = vec![];
    for n in numbers {
        let number: i64 = n
            .trim_start_matches(['C', 'c'])
            .parse()
            .map_err(|_| format!("`{n}` isn't a comment number like C3"))?;
        let found = store
            .set_agent_comment_done(number, done)
            .map_err(|e| e.to_string())?;
        if !found {
            return Err(format!("there's no comment C{number}"));
        }
        said.push(format!(
            "C{number} is {}.",
            if done { "done" } else { "open again" }
        ));
    }
    Ok(said.join("\n"))
}
