//! `lgtm issues <command>`: what agents run.

use std::io::Read;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::folder::Folder;
use crate::issue::Asked;
use crate::workflow::Verb;
use crate::{guide, lgterm, ops, time};

pub const USAGE: &str = "usage: lgtm issues [-C <dir>] [--as <name>] [command]

With no command, opens the issues screen on this folder, or on one picked under it.

  init                         start an issues folder here
  new <title> [--body <text>] [--status <s>] [--<field> <value>]...
                               file an issue; prints its id
  list [--all] [--status <s>]  issues not done; --all adds done ones
  show <id>                    print an issue's file
  statuses                     this tracker's statuses, in order
  claim <id> [--force]         make it yours and move it to the claim status
  move <id> <status> [--force] [note]
                               move it to a status
  set <id> <field> [value]     set a header field, or clear it with no value
  log <id> <text>              add a note
  ask <id> <Qn> [--after <Qm>] [text]
                               ask the user; text comes from stdin when not given
  answer <id> <Qn> [--pick <n>] [text]
                               answer a question and send it to the issue's owner:
                               an option and notes, or with no --pick, a reply that
                               closes the question for a new one
  close <id> [--force] [note]  move it to the close status
  drop <id> [--force] [note]   move it to the drop status
  reopen <id> [--force] [note] move it to the reopen status
  guide                        how agents use this issues folder";

/// The arguments every command takes.
pub struct Args {
    pub dir: PathBuf,
    pub name: Option<String>,
    pub rest: Vec<String>,
}

impl Args {
    pub fn parse(argv: Vec<String>) -> Result<Args> {
        let mut dir = None;
        let mut name = None;
        let mut rest = Vec::new();
        let mut it = argv.into_iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "-C" if rest.is_empty() => dir = Some(it.next().context("-C needs a folder")?),
                "--as" => name = Some(it.next().context("--as needs a name")?),
                _ => rest.push(a),
            }
        }
        let dir = match dir {
            Some(d) => PathBuf::from(d),
            None => std::env::current_dir()?,
        };
        Ok(Args { dir, name, rest })
    }
}

fn id(arg: Option<&String>) -> Result<u32> {
    let arg = arg.context("which issue?")?;
    arg.trim_start_matches('#')
        .parse()
        .with_context(|| format!("{arg} isn't an issue id"))
}

fn flag(rest: &mut Vec<String>, name: &str) -> Result<Option<String>> {
    let Some(i) = rest.iter().position(|a| a == name) else {
        return Ok(None);
    };
    rest.remove(i);
    if i >= rest.len() {
        bail!("{name} needs a value");
    }
    Ok(Some(rest.remove(i)))
}

fn switch(rest: &mut Vec<String>, name: &str) -> bool {
    let before = rest.len();
    rest.retain(|a| a != name);
    rest.len() != before
}

fn stdin() -> Result<String> {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text)?;
    Ok(text)
}

pub fn run(args: Args) -> Result<()> {
    let mut rest = args.rest;
    if rest.is_empty() {
        bail!("{USAGE}");
    }
    let cmd = rest.remove(0);
    if cmd == "init" {
        let folder = Folder::init(&args.dir)?;
        println!("{}", folder.root.display());
        return Ok(());
    }
    if cmd == "guide" {
        match Folder::open(&args.dir) {
            Ok(folder) => print!("{}", guide::full(&folder.workflow()?)),
            Err(_) => print!("{}", guide::GUIDE),
        }
        return Ok(());
    }
    if matches!(cmd.as_str(), "help" | "-h" | "--help")
        || rest.iter().any(|a| a == "-h" || a == "--help")
    {
        println!("{USAGE}");
        return Ok(());
    }
    let folder = Folder::open(&args.dir)?;
    let who = || lgterm::who(args.name.clone());
    match cmd.as_str() {
        "new" => {
            let body = flag(&mut rest, "--body")?.unwrap_or_default();
            let status = flag(&mut rest, "--status")?;
            let mut with = Vec::new();
            for f in folder.workflow()?.fields {
                if let Some(v) = flag(&mut rest, &format!("--{}", f.name))? {
                    with.push((f.name, v));
                }
            }
            if let Some(f) = rest.iter().find(|a| a.starts_with("--")) {
                bail!("new has no {f} flag");
            }
            let title = rest.join(" ");
            let (id, d) = ops::file(&folder, &title, &body, &who(), status.as_deref(), with)?;
            println!("{id}");
            deliver(d);
        }
        "list" => {
            let all = switch(&mut rest, "--all");
            list(&folder, all, flag(&mut rest, "--status")?.as_deref())?
        }
        "show" => print!(
            "{}",
            std::fs::read_to_string(folder.path(id(rest.first())?)?)?
        ),
        "statuses" => statuses(&folder)?,
        "claim" => {
            let force = switch(&mut rest, "--force");
            let d = ops::claim(&folder, id(rest.first())?, &who(), lgterm::term(), force)?;
            deliver(d);
        }
        "move" => {
            let force = switch(&mut rest, "--force");
            let issue = id(rest.first())?;
            let status = rest.get(1).context("move it to which status?")?;
            let wf = folder.workflow()?;
            let label = wf.find(status)?.label.clone();
            let note = match rest[2..].join(" ") {
                n if n.trim().is_empty() => format!("Moved to {label}."),
                n => n,
            };
            let who = who();
            deliver(ops::move_to(
                &folder,
                issue,
                status,
                force,
                Some((&who, &note)),
            )?);
        }
        "set" => {
            let issue = id(rest.first())?;
            let name = rest.get(1).context("set which field?")?;
            ops::set(&folder, issue, &who(), name, &rest[2..].join(" "))?;
        }
        "log" => ops::note(&folder, id(rest.first())?, &who(), &rest[1..].join(" "))?,
        "ask" => {
            let after = flag(&mut rest, "--after")?;
            let issue = id(rest.first())?;
            let qid = rest.get(1).context("ask needs a question id, like Q1")?;
            let text = match rest[2..].join(" ") {
                t if t.trim().is_empty() => stdin()?,
                t => t,
            };
            ops::ask(&folder, issue, qid, after, &who(), lgterm::term(), &text)?;
        }
        "answer" => {
            let pick = flag(&mut rest, "--pick")?;
            let issue = id(rest.first())?;
            let qid = rest.get(1).context("answer needs a question id, like Q1")?;
            let text = rest[2..].join(" ");
            let response = match pick {
                Some(n) => ops::Response::Pick {
                    option: n
                        .parse()
                        .with_context(|| format!("--pick {n} isn't a number"))?,
                    notes: text,
                },
                None => ops::Response::Reply(text),
            };
            if let Some(d) = ops::answer(&folder, issue, qid, &who(), &response)? {
                lgterm::send(&d.to, &d.text)
                    .with_context(|| format!("answer saved, but not sent to {}", d.to))?;
            }
        }
        "close" | "drop" | "reopen" => {
            let verb = match cmd.as_str() {
                "close" => Verb::Close,
                "drop" => Verb::Drop,
                _ => Verb::Reopen,
            };
            let force = switch(&mut rest, "--force");
            let issue = id(rest.first())?;
            let note = rest[1..].join(" ");
            deliver(ops::verb(&folder, issue, verb, &who(), &note, force)?);
        }
        _ => bail!("unknown command {cmd}\n\n{USAGE}"),
    }
    Ok(())
}

/// Tells a status's terminal that an issue moved there. The move stands either way.
fn deliver(d: Option<ops::Delivery>) {
    if let Some(d) = d
        && let Err(e) = lgterm::send(&d.to, &d.text)
    {
        eprintln!("lgtm issues: moved, but couldn't tell {}: {e:#}", d.to);
    }
}

fn statuses(folder: &Folder) -> Result<()> {
    let wf = folder.workflow()?;
    for s in &wf.statuses {
        let mut tags = Vec::new();
        if s.done {
            tags.push("done".to_owned());
        }
        if !s.requires.is_empty() {
            tags.push(format!("requires {}", s.requires.join(", ")));
        }
        if let Some(n) = &s.notify {
            tags.push(format!("notifies {n}"));
        }
        println!(
            "{}  {:<14} {:<16} {}",
            s.key,
            s.id,
            s.label,
            tags.join("; ")
        );
        if let Some(line) = s.body.lines().next() {
            println!("   {line}");
        }
    }
    Ok(())
}

fn list(folder: &Folder, all: bool, only: Option<&str>) -> Result<()> {
    let wf = folder.workflow()?;
    let only = only.map(|s| wf.find(s)).transpose()?;
    let (issues, bad) = folder.load()?;
    let now = time::now();
    for status in &wf.statuses {
        if only.is_some_and(|o| o.id != status.id) || (only.is_none() && status.done && !all) {
            continue;
        }
        for i in issues.iter().filter(|i| i.status == status.id) {
            println!(
                "{:<4} {:<14} {:<14} {:>4}  {}",
                i.id,
                status.id,
                i.owner.as_deref().unwrap_or("-"),
                time::age(i.touched(), now),
                i.title
            );
            let set: Vec<String> = wf
                .fields
                .iter()
                .filter(|f| !matches!(f.name.as_str(), "blocked-by" | "duplicate-of"))
                .filter_map(|f| Some(format!("{} {}", f.name, i.field(&f.name)?)))
                .collect();
            if !set.is_empty() {
                println!("       {}", set.join(" · "));
            }
            let blockers = i.open_blockers(&issues, |s| wf.is_done(s));
            if !blockers.is_empty() {
                let ids: Vec<String> = blockers.iter().map(|b| format!("#{b}")).collect();
                println!("       blocked by {}", ids.join(", "));
            }
            if let Some(d) = i.duplicate_of {
                println!("       duplicate of #{d}");
            }
            for (q, asked) in i.questions() {
                if asked != Asked::Answered {
                    let state = if asked == Asked::Waiting {
                        "waiting"
                    } else {
                        "queued"
                    };
                    println!(
                        "       {} {state}: {}",
                        q.qid().unwrap_or_default(),
                        q.subject()
                    );
                }
            }
        }
    }
    for b in bad {
        eprintln!("unreadable: {b}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_in(folder: &Folder, argv: &[&str]) -> Result<()> {
        run(Args {
            dir: folder.root.clone(),
            name: Some("test".into()),
            rest: argv.iter().map(|a| a.to_string()).collect(),
        })
    }

    #[test]
    fn help_after_a_command_and_unknown_flags_file_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let folder = Folder::init(dir.path()).unwrap();
        run_in(&folder, &["new", "--help"]).unwrap();
        run_in(&folder, &["claim", "-h"]).unwrap();
        let err = run_in(&folder, &["new", "Title", "--sevrity", "high"]).unwrap_err();
        assert_eq!(err.to_string(), "new has no --sevrity flag");
        assert!(folder.load().unwrap().0.is_empty());
    }
}
