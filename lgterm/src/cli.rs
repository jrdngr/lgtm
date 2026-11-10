//! `lgterm <command>`: drives the running app from one of its terminals.

use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;

use crate::ipc::{self, Place, Reply, Request};

const USAGE: &str = "\
usage:
  lgterm ls [--json]
  lgterm split [--right|--down] [--from T] [--cwd DIR] [--focus] [-- CMD...]
  lgterm tab [--from T] [--cwd DIR] [--focus] [-- CMD...]
  lgterm space [--right|--down] [--from T] [--cwd DIR] [--focus] [-- CMD...]
  lgterm layout [--cwd DIR] [--focus] [-- CMD...]
  lgterm send T MESSAGE...
  lgterm type T TEXT... [--enter]
  lgterm wait T [--timeout SECONDS]
  lgterm read T [--lines N]
  lgterm focus T
  lgterm close T
  lgterm hook [JSON]";

const COMMANDS: &[&str] = &[
    "ls", "split", "tab", "space", "layout", "send", "type", "wait", "read", "focus", "close",
    "hook", "help", "--help", "-h",
];

/// Whether these arguments are a command rather than a launch of the app.
pub fn is_command(args: &[String]) -> bool {
    args.first().is_some_and(|a| COMMANDS.contains(&a.as_str()))
}

/// Runs a command and returns the exit code.
pub fn run(args: &[String]) -> i32 {
    let hook = args[0] == "hook";
    match parse(args) {
        Ok(None) => {
            println!("{USAGE}");
            0
        }
        Ok(Some((request, timeout))) => send(&request, timeout, hook),
        Err(_) if hook => 0,
        Err(e) => {
            eprintln!("lgterm: {e}\n{USAGE}");
            2
        }
    }
}

fn send(request: &Request, timeout: Option<Duration>, quiet: bool) -> i32 {
    let Some(socket) = std::env::var_os(ipc::SOCKET_ENV) else {
        if quiet {
            return 0;
        }
        eprintln!("lgterm: not inside an lgterm terminal");
        return 1;
    };
    match ipc::call(&PathBuf::from(socket), request, timeout) {
        Ok(Reply::Ok(out)) => {
            if !out.is_empty() {
                println!("{out}");
            }
            0
        }
        Ok(Reply::Err(_)) if quiet => 0,
        Ok(Reply::Err(e)) => {
            eprintln!("lgterm: {e}");
            1
        }
        Err(_) if quiet => 0,
        Err(e) => {
            let timed_out = e.downcast_ref::<std::io::Error>().is_some_and(|e| {
                matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                )
            });
            if timed_out {
                eprintln!("lgterm: timed out");
                return 124;
            }
            eprintln!("lgterm: {e:#}");
            1
        }
    }
}

fn this_term() -> Option<String> {
    std::env::var("LGTERM_TERMINAL_ID")
        .ok()
        .filter(|t| !t.is_empty())
}

type Parsed = Option<(Request, Option<Duration>)>;

fn parse(args: &[String]) -> Result<Parsed, String> {
    let cmd = args[0].as_str();
    let (flags, rest) = match args[1..].iter().position(|a| a == "--") {
        Some(i) => (&args[1..i + 1], args[i + 2..].to_vec()),
        None => (&args[1..], vec![]),
    };
    let mut f = Flags::new(flags);
    let request = match cmd {
        "help" | "--help" | "-h" => return Ok(None),
        "ls" => Request::Ls {
            json: f.switch("--json"),
        },
        "split" | "tab" | "space" | "layout" => {
            let down = f.switch("--down");
            let right = f.switch("--right");
            if down && right {
                return Err("pick one of --right and --down".into());
            }
            let place = match (cmd, down) {
                ("split", false) => Place::Right,
                ("split", true) => Place::Down,
                ("space", false) => Place::SpaceRight,
                ("space", true) => Place::SpaceDown,
                ("tab", _) => Place::Tab,
                _ => Place::Layout,
            };
            Request::Open {
                from: f.value("--from")?.or_else(this_term),
                place,
                cwd: f.value("--cwd")?.map(absolute),
                focus: f.switch("--focus"),
                argv: rest,
            }
        }
        "send" => {
            let to = f.positional("terminal")?;
            Request::Send {
                from: this_term(),
                to,
                text: f.text("message", &rest)?,
            }
        }
        "type" => {
            let enter = f.switch("--enter");
            let to = f.positional("terminal")?;
            Request::Type {
                to,
                text: f.text("text", &rest)?,
                enter,
            }
        }
        "wait" => {
            let timeout = f
                .value("--timeout")?
                .map(|t| {
                    t.parse::<f64>()
                        .ok()
                        .and_then(|s| Duration::try_from_secs_f64(s).ok())
                        .filter(|d| !d.is_zero())
                        .ok_or_else(|| format!("bad timeout: {t}"))
                })
                .transpose()?;
            let term = f.positional("terminal")?;
            f.done()?;
            return Ok(Some((Request::Wait { term }, timeout)));
        }
        "read" => {
            let lines = f
                .value("--lines")?
                .map(|n| n.parse().map_err(|_| format!("bad line count: {n}")))
                .transpose()?;
            Request::Read {
                term: f.positional("terminal")?,
                lines,
            }
        }
        "focus" => Request::Focus {
            term: f.positional("terminal")?,
        },
        "close" => Request::Close {
            term: f.positional("terminal")?,
        },
        "hook" => {
            let raw = match f.args.first() {
                Some(arg) => arg.clone(),
                None => {
                    let mut s = String::new();
                    let _ = std::io::stdin().read_to_string(&mut s);
                    s
                }
            };
            f.args.clear();
            let event: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
            Request::Hook {
                term: this_term().ok_or("not inside an lgterm terminal")?,
                event,
            }
        }
        _ => return Err(format!("unknown command: {cmd}")),
    };
    f.done()?;
    Ok(Some((request, Some(Duration::from_secs(10)))))
}

fn absolute(path: String) -> PathBuf {
    let p = PathBuf::from(path);
    match std::env::current_dir() {
        Ok(dir) if p.is_relative() => dir.join(p),
        _ => p,
    }
}

/// Arguments left to parse.
struct Flags {
    args: Vec<String>,
}

impl Flags {
    fn new(args: &[String]) -> Self {
        Flags {
            args: args.to_vec(),
        }
    }

    fn switch(&mut self, name: &str) -> bool {
        let before = self.args.len();
        self.args.retain(|a| a != name);
        self.args.len() != before
    }

    fn value(&mut self, name: &str) -> Result<Option<String>, String> {
        let Some(i) = self.args.iter().position(|a| a == name) else {
            return Ok(None);
        };
        if i + 1 >= self.args.len() {
            return Err(format!("{name} needs a value"));
        }
        let v = self.args.remove(i + 1);
        self.args.remove(i);
        Ok(Some(v))
    }

    fn positional(&mut self, what: &str) -> Result<String, String> {
        let i = self
            .args
            .iter()
            .position(|a| !a.starts_with("--"))
            .ok_or(format!("missing {what}"))?;
        Ok(self.args.remove(i))
    }

    /// Everything left, then everything after `--`, as one string.
    fn text(&mut self, what: &str, rest: &[String]) -> Result<String, String> {
        let mut words = std::mem::take(&mut self.args);
        words.extend_from_slice(rest);
        if words.is_empty() {
            return Err(format!("missing {what}"));
        }
        Ok(words.join(" "))
    }

    fn done(&self) -> Result<(), String> {
        match self.args.first() {
            Some(a) => Err(format!("unexpected argument: {a}")),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Request {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        parse(&args).unwrap().unwrap().0
    }

    #[test]
    fn splits_take_a_side_and_a_command() {
        let Request::Open {
            place, argv, focus, ..
        } = p(&["split", "--down", "--focus", "--", "cargo", "test", "--all"])
        else {
            panic!("not an open")
        };
        assert_eq!(place, Place::Down);
        assert!(focus);
        assert_eq!(argv, ["cargo", "test", "--all"]);
    }

    #[test]
    fn messages_join_their_words() {
        let Request::Send { to, text, .. } = p(&["send", "t3", "tests", "pass"]) else {
            panic!("not a send")
        };
        assert_eq!((to.as_str(), text.as_str()), ("t3", "tests pass"));
        let Request::Type { enter, text, .. } = p(&["type", "--enter", "t3", "ls"]) else {
            panic!("not a type")
        };
        assert!(enter);
        assert_eq!(text, "ls");
    }

    #[test]
    fn text_after_a_double_dash_is_kept() {
        let Request::Send { text, .. } = p(&["send", "t2", "a", "--", "--b"]) else {
            panic!("not a send")
        };
        assert_eq!(text, "a --b");
        let Request::Type { text, enter, .. } = p(&["type", "t3", "--enter", "--", "--enter"])
        else {
            panic!("not a type")
        };
        assert!(enter);
        assert_eq!(text, "--enter");
        let Request::Type { text, .. } = p(&["type", "t3", "--", "-x"]) else {
            panic!("not a type")
        };
        assert_eq!(text, "-x");
    }

    #[test]
    fn bad_arguments_are_errors() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(parse(&args(&["split", "--right", "--down"])).is_err());
        assert!(parse(&args(&["read", "t1", "--lines"])).is_err());
        assert!(parse(&args(&["focus", "t1", "t2"])).is_err());
        assert!(parse(&args(&["send", "t1"])).is_err());
        for t in ["-1", "nan", "0", "1e-12", "inf"] {
            assert!(parse(&args(&["wait", "t1", "--timeout", t])).is_err());
        }
    }

    #[test]
    fn wait_has_no_timeout_unless_asked() {
        let args: Vec<String> = ["wait", "t2"].iter().map(|s| s.to_string()).collect();
        assert_eq!(parse(&args).unwrap().unwrap().1, None);
        let args: Vec<String> = ["wait", "t2", "--timeout", "1.5"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            parse(&args).unwrap().unwrap().1,
            Some(Duration::from_millis(1500))
        );
    }
}
