//! What `lgtm --help` prints.

pub const HELP: &str = "lgtm: code review for jj, in the terminal

Usage:
  lgtm                      Open the editor in the current repo
  lgtm -r <revset>          Review the changes in <revset> (lgtm -r @ for the working copy)
  lgtm <file>[:<line>]      Open a file in the editor, at a line
  lgtm <folder>             Open the editor's file list in a folder
  lgtm resolve [<file>]     Resolve conflicts in the working copy, or in one file
  lgtm issues [<command>]   Open the issues screen, or run an issues command (lgtm issues --help)
  lgtm snap <label>         Start a named step in the workspace's agent turns
  lgtm review [<command>]   Commands agents run to read and mark the open review (lgtm review --help)
  lgtm hook                 Claude Code's hook command; lgtm registers it itself

Options:
  -r <revset>               The revisions to review
  --pr <number|url>         The GitHub PR whose threads `p` pulls into the review
  -h, --help                Show this help

Inside lgtm, `?` lists the keys for the screen you're on.";
