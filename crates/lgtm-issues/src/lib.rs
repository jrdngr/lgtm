//! lgtm issues: an issue tracker for agents, kept as markdown files in one folder. lgtm runs
//! its commands as `lgtm issues <command>` and shows it as a screen.

mod cli;
mod fields;
pub mod folder;
mod guide;
mod header;
mod issue;
mod lgterm;
mod ops;
pub mod skill;
mod time;
pub mod tui;
mod workflow;

pub use cli::USAGE;

/// Runs `lgtm issues <args>`, a command other than opening the screen. Returns the exit code.
pub fn command(args: Vec<String>) -> i32 {
    let result = cli::Args::parse(args).and_then(cli::run);
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("lgtm issues: {e:#}");
            1
        }
    }
}

/// Who answers from this terminal: the login name.
pub fn user() -> String {
    std::env::var("USER").unwrap_or_else(|_| "user".into())
}
