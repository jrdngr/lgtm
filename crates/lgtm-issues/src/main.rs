//! lgtm-issues: an issue tracker for agents, kept as markdown files in one folder.

mod cli;
mod folder;
mod guide;
mod issue;
mod lgterm;
mod ops;
mod time;
mod tui;

fn main() {
    let args = match cli::Args::parse(std::env::args().skip(1).collect()) {
        Ok(a) => a,
        Err(e) => fail(e),
    };
    let result = if args.rest.is_empty() {
        folder::Folder::open(&args.dir).and_then(|f| tui::run(f, args.name))
    } else {
        cli::run(args)
    };
    if let Err(e) = result {
        fail(e);
    }
}

fn fail(e: anyhow::Error) -> ! {
    eprintln!("lgtm-issues: {e:#}");
    std::process::exit(1);
}
