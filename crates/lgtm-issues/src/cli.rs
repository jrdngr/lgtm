//! `lgtm issues <command>`: what agents run.

use std::io::Read;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::folder::Folder;
use crate::guide::GUIDE;
use crate::issue::{Asked, Status};
use crate::{lgterm, ops, time};

pub const USAGE: &str = "usage: lgtm issues [-C <dir>] [--as <name>] [command]

With no command, opens the issues screen on this folder, or on one picked under it.

  init                         start an issues folder here
  new <title> [--body <text>]  file an issue; prints its id
  list [--all]                 open issues; --all adds done and dropped
  show <id>                    print an issue's file
  claim <id>                   make it yours and mark it working
  log <id> <text>              add a note
  ask <id> <Qn> [--after <Qm>] [text]
                               ask the user; text comes from stdin when not given
  answer <id> <Qn> [--pick <n>] [text]
                               answer a question and send it to the issue's owner:
                               an option and notes, or with no --pick, a reply that
                               closes the question for a new one
  close <id> [note]            mark it done
  drop <id> [note]             mark it dropped
  reopen <id> [note]           mark it open
  guide                        how agents use an issues folder";

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
        print!("{GUIDE}");
        return Ok(());
    }
    if matches!(cmd.as_str(), "help" | "-h" | "--help") {
        println!("{USAGE}");
        return Ok(());
    }
    let folder = Folder::open(&args.dir)?;
    let who = || lgterm::who(args.name.clone());
    match cmd.as_str() {
        "new" => {
            let body = flag(&mut rest, "--body")?.unwrap_or_default();
            println!("{}", ops::file(&folder, &rest.join(" "), &body, &who())?);
        }
        "list" => list(&folder, switch(&mut rest, "--all"))?,
        "show" => print!(
            "{}",
            std::fs::read_to_string(folder.path(id(rest.first())?)?)?
        ),
        "claim" => ops::claim(&folder, id(rest.first())?, &who(), lgterm::term())?,
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
            let status = match cmd.as_str() {
                "close" => Status::Done,
                "drop" => Status::Dropped,
                _ => Status::Open,
            };
            let issue = id(rest.first())?;
            ops::set_status(&folder, issue, status, &who(), &rest[1..].join(" "))?;
        }
        _ => bail!("unknown command {cmd}\n\n{USAGE}"),
    }
    Ok(())
}

fn list(folder: &Folder, all: bool) -> Result<()> {
    let (issues, bad) = folder.load()?;
    let now = time::now();
    for i in issues {
        if !all && matches!(i.status, Status::Done | Status::Dropped) {
            continue;
        }
        println!(
            "{:<4} {:<11} {:<14} {:>4}  {}",
            i.id,
            i.status.name(),
            i.owner.as_deref().unwrap_or("-"),
            time::age(i.touched(), now),
            i.title
        );
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
    for b in bad {
        eprintln!("unreadable: {b}");
    }
    Ok(())
}
