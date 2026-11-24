//! What `lgtm --help` prints.

pub const HELP: &str = "lgtm: code review for jj, in the terminal

Usage:
  lgtm                      Open the editor in the current repo
  lgtm <file>[:<line>]      Open a file in the editor, at a line
  lgtm <folder>             Open the editor's file list in a folder
  lgtm review               Review the working copy (@)
  lgtm review -r <revset>   Review the changes in <revset>
  lgtm review --pr <n|url>  Review a GitHub PR's revisions, with its threads
  lgtm resolve [<file>]     Resolve conflicts in the working copy, or in one file
  lgtm issues [-C <dir>]    Open the issues screen
  lgtm issues init          Start an issues folder here
  lgtm issues answer ...    Answer an issue's question (lgtm issues --help)
  lgtm agent <command>      Commands agents run (lgtm agent --help)
  lgtm -h, --help           Show this help

`--pr` with `-r` reviews the revset and pulls the PR's threads into it.
Inside lgtm, `?` lists the keys for the screen you're on.";
