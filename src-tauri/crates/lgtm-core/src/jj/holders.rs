//! What has a jj lock: the processes with its file open, less the app's own jj processes
//! known to be waiting on it. `lsof` can't tell a holder from another waiter, so the list
//! can name several.

use tokio::process::Command;

use super::child;

/// One line per process: PID, how long it has run, and its command. Empty when none is found.
pub(super) async fn describe(lock: &str) -> String {
    let own = child::own_waiters(lock);
    let pids: Vec<String> = match output("lsof", &["-t", lock]).await {
        Some(out) => out
            .lines()
            .filter_map(|l| l.trim().parse::<u32>().ok())
            .filter(|p| !own.contains(p))
            .map(|p| p.to_string())
            .collect(),
        None => return String::new(),
    };
    if pids.is_empty() {
        return String::new();
    }
    let list = pids.join(",");
    let out = output("ps", &["-o", "pid=,etime=,command=", "-p", &list])
        .await
        .unwrap_or_default();
    out.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The command's stdout, or `None` when it can't run. `lsof` and `ps` exit 1 when they find
/// nothing, so the status isn't checked.
async fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program)
        .args(args)
        .kill_on_drop(true)
        .output()
        .await
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}
