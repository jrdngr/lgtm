//! The workflow a new or upgraded folder starts with: open, working, done and dropped.

use std::fs;
use std::path::Path;

use anyhow::Result;

use super::{FILE, STATUS_FILE};

const WORKFLOW: &str = "---
order: open, working, done, dropped
new: open
claim: working
close: done
drop: dropped
reopen: open
fields: severity, revision, blocked-by, duplicate-of
severity: low:muted, medium:fg, high:amber, critical:red
---

Agents file issues into open, claim one to work on it, and close it when it's done. Ask the
user when a decision needs them.
";

const STATUSES: [(&str, &str); 4] = [
    (
        "open",
        "---
color: muted
glyph: ○
sort: id
---

Filed and not started. Claim an issue before working on it.
",
    ),
    (
        "working",
        "---
color: accent
glyph: ●
---

Claimed by its owner, who logs progress as they go. Set its revision once code is written.
",
    ),
    (
        "done",
        "---
color: green
glyph: ✓
done: true
---

Finished. Reopen it if the problem comes back.
",
    ),
    (
        "dropped",
        "---
color: faint
glyph: ×
done: true
---

Won't be done: a duplicate, out of scope, or no longer wanted.
",
    ),
];

/// Writes the default workflow into the `issues` folder, leaving any file already there.
pub fn write(issues: &Path) -> Result<()> {
    let put = |path: &Path, text: &str| -> Result<()> {
        if !path.exists() {
            fs::write(path, text)?;
        }
        Ok(())
    };
    fs::create_dir_all(issues)?;
    put(&issues.join(FILE), WORKFLOW)?;
    for (id, text) in STATUSES {
        let dir = issues.join(id);
        fs::create_dir_all(&dir)?;
        put(&dir.join(STATUS_FILE), text)?;
    }
    Ok(())
}
