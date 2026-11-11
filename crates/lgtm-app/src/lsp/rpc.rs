//! JSON-RPC with a language server over its stdin and stdout, framed by `Content-Length`.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, bail};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot};

type Reply = Result<Value, String>;

/// A running server: requests go out in order, and each answer finds its request by id.
pub struct Rpc {
    out: mpsc::UnboundedSender<Vec<u8>>,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<Reply>>>>,
    next: AtomicI64,
    alive: Arc<AtomicBool>,
    _child: Child,
}

fn frame(msg: &Value) -> Vec<u8> {
    let body = msg.to_string();
    format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
}

/// What lgtm answers a server's own request with: each setting section asked for from
/// `settings`, null for those it lacks, else null.
fn answer(method: &str, params: &Value, settings: &Value) -> Value {
    match method {
        "workspace/configuration" => {
            let items = params["items"].as_array().map_or(&[][..], Vec::as_slice);
            let section = |i: &Value| i["section"].as_str().and_then(|s| settings.get(s)).cloned();
            Value::Array(
                items
                    .iter()
                    .map(|i| section(i).unwrap_or(Value::Null))
                    .collect(),
            )
        }
        _ => Value::Null,
    }
}

impl Rpc {
    /// Starts `program`, which gets `settings`, by section, when it asks for its configuration.
    pub fn spawn(
        program: &Path,
        args: &[&str],
        cwd: &Path,
        settings: Value,
    ) -> anyhow::Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("start {}", program.display()))?;
        let mut stdin = child.stdin.take().context("no stdin")?;
        let stdout = child.stdout.take().context("no stdout")?;
        let (out, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        tokio::spawn(async move {
            while let Some(bytes) = rx.recv().await {
                if stdin.write_all(&bytes).await.is_err() || stdin.flush().await.is_err() {
                    break;
                }
            }
        });
        let pending: Arc<Mutex<HashMap<i64, oneshot::Sender<Reply>>>> = Arc::default();
        let alive = Arc::new(AtomicBool::new(true));
        let (p, a, o) = (pending.clone(), alive.clone(), out.clone());
        tokio::spawn(async move {
            let mut r = BufReader::new(stdout);
            while let Ok(Some(msg)) = read(&mut r).await {
                if let Some(method) = msg["method"].as_str() {
                    if !msg["id"].is_null() {
                        let reply = json!({"jsonrpc": "2.0", "id": msg["id"], "result": answer(method, &msg["params"], &settings)});
                        o.send(frame(&reply)).ok();
                    }
                    continue;
                }
                let Some(id) = msg["id"].as_i64() else {
                    continue;
                };
                let reply = match msg.get("error") {
                    Some(e) => Err(e["message"].as_str().unwrap_or("error").to_string()),
                    None => Ok(msg["result"].clone()),
                };
                if let Some(tx) = p.lock().unwrap().remove(&id) {
                    tx.send(reply).ok();
                }
            }
            a.store(false, Ordering::SeqCst);
            p.lock().unwrap().clear();
        });
        Ok(Self {
            out,
            pending,
            next: AtomicI64::new(1),
            alive,
            _child: child,
        })
    }

    pub fn alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    pub async fn request(
        &self,
        method: &str,
        params: Value,
        wait: Duration,
    ) -> anyhow::Result<Value> {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, tx);
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.out.send(frame(&msg)).ok();
        match tokio::time::timeout(wait, rx).await {
            Ok(Ok(Ok(v))) => Ok(v),
            Ok(Ok(Err(e))) => bail!("{method}: {e}"),
            Ok(Err(_)) => bail!("the language server stopped"),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                bail!("{method} took over {}s", wait.as_secs())
            }
        }
    }

    pub fn notify(&self, method: &str, params: Value) {
        let msg = json!({"jsonrpc": "2.0", "method": method, "params": params});
        self.out.send(frame(&msg)).ok();
    }
}

/// The next message, or `None` at the end of the stream.
async fn read<R: tokio::io::AsyncBufRead + Unpin>(r: &mut R) -> anyhow::Result<Option<Value>> {
    let mut len = None;
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).await? == 0 {
            return Ok(None);
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(n) = line.strip_prefix("Content-Length:") {
            len = Some(n.trim().parse::<usize>()?);
        }
    }
    let mut body = vec![0; len.context("no Content-Length")?];
    r.read_exact(&mut body).await?;
    Ok(Some(serde_json::from_slice(&body)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_requests_get_the_sections_lgtm_has() {
        let settings = json!({"python": {"pythonPath": "/r/.venv/bin/python"}});
        let params = json!({"items": [{"section": "python"}, {"section": "python.analysis"}, {}]});
        assert_eq!(
            answer("workspace/configuration", &params, &settings),
            json!([{"pythonPath": "/r/.venv/bin/python"}, null, null])
        );
    }

    #[tokio::test]
    async fn reads_framed_messages_one_after_another() {
        let a = frame(&json!({"id": 1, "result": "x"}));
        let b = frame(&json!({"method": "m"}));
        let bytes = [a, b].concat();
        let mut r = BufReader::new(&bytes[..]);
        assert_eq!(read(&mut r).await.unwrap().unwrap()["result"], "x");
        assert_eq!(read(&mut r).await.unwrap().unwrap()["method"], "m");
        assert!(read(&mut r).await.unwrap().is_none());
    }
}
