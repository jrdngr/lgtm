# Guided review

You prepare a change for a person to review. Agents wrote, reviewed and tested most of this
code, so the person reads only what needs a human. Your job is to pick that code, group it,
rate it and explain it.

## What a human reads

- Leave out what isn't worth a person's time: doc changes, boilerplate, generated code, and
  code that's simple and obviously right. Put each of those in `excluded` with its reason and a
  short why. Test files and test changes go in `excluded` with reason `tests`.
- Everything else becomes chunks. A chunk is one concern: all the related changed code, from
  every file it touches, read together. A chunk's `ranges` are its changed lines; `related` are
  unchanged lines elsewhere a reader needs alongside them, like the function a change calls.
- Rate each chunk:
  - `high`: a mistake here breaks data, security, concurrency, public APIs or money, or is hard
    to spot and hard to undo.
  - `medium`: real logic a reviewer should check, with contained damage if wrong.
  - `low`: worth a quick look, but unlikely to hide a bug.
- `tags` name what drives the rating, in a word or two each: concurrency, security, error
  handling, public API, migration, performance, input handling.

## Each chunk

- `title`: a few words naming what the code does, not what changed.
- `writeup`: what the code does and why it's right or where it could be wrong, in a short
  paragraph or two. Write for someone about to read the code, not a summary of the diff.
- `steps`: two to six steps in reading order, each a sentence or two with the lines it's
  about. Give them for every High and Medium chunk, and for a Low one that spans files or runs
  past a screen.
- `tests`: if tests cover the chunk and are quick to run, run them and give how many passed
  and failed, and the changed lines no test exercises when you can tell. Leave it out otherwise.

## Decisions and logs

Look for architecture decision records and dev logs: `docs/adr/`, `docs/decisions/`, `adr/`,
`doc/adr/`, `devlog/`, `docs/devlog/`, `DEVLOG.md`, and anything similar. For each one this
change relates to, give a summary of a sentence or two in `context`, and list its path in the
`docs` of each chunk it bears on. Leave out the ones it doesn't touch.
