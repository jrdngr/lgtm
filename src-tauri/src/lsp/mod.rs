//! Language servers from the user's PATH, for go to definition and hover. One runs per
//! workspace root and language, started the first time a file of that language asks. Files
//! are sent as they are in the editor, or as on disk.

mod hover;
mod rpc;
mod uri;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use specta::Type;

use rpc::Rpc;

/// A kind of server: the files it takes, as extension and language id, and the commands it
/// may run as, the first one found on PATH winning.
struct Kind {
    name: &'static str,
    langs: &'static [(&'static str, &'static str)],
    commands: &'static [(&'static str, &'static [&'static str])],
}

const KINDS: &[Kind] = &[
    Kind {
        name: "rust",
        langs: &[("rs", "rust")],
        commands: &[("rust-analyzer", &[])],
    },
    Kind {
        name: "python",
        langs: &[("py", "python"), ("pyi", "python")],
        commands: &[
            ("basedpyright-langserver", &["--stdio"]),
            ("pyright-langserver", &["--stdio"]),
        ],
    },
    Kind {
        name: "typescript",
        langs: &[
            ("ts", "typescript"),
            ("mts", "typescript"),
            ("cts", "typescript"),
            ("tsx", "typescriptreact"),
            ("js", "javascript"),
            ("mjs", "javascript"),
            ("cjs", "javascript"),
            ("jsx", "javascriptreact"),
        ],
        commands: &[("typescript-language-server", &["--stdio"])],
    },
    Kind {
        name: "go",
        langs: &[("go", "go")],
        commands: &[("gopls", &[])],
    },
    Kind {
        name: "swift",
        langs: &[("swift", "swift")],
        commands: &[("sourcekit-lsp", &[])],
    },
];

/// The server kind for `path` and the file's language id.
fn kind_of(path: &Path) -> Option<(&'static Kind, &'static str)> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    KINDS.iter().find_map(|k| {
        let (_, lang) = k.langs.iter().find(|(e, _)| *e == ext)?;
        Some((k, *lang))
    })
}

struct Server {
    rpc: Rpc,
    /// Each open file's version and the text the server has for it.
    docs: Mutex<HashMap<PathBuf, (i64, String)>>,
}

static SERVERS: LazyLock<tokio::sync::Mutex<HashMap<(PathBuf, &'static str), Arc<Server>>>> =
    LazyLock::new(Default::default);

/// Files outside every workspace that a definition led to, which the editor may show.
static EXTERNAL: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(Default::default);

/// Whether a definition led to `path`, a file outside the workspaces.
pub fn shown_external(path: &Path) -> bool {
    EXTERNAL.lock().unwrap().contains(path)
}

const START: Duration = Duration::from_secs(30);
const DEFINITION: Duration = Duration::from_secs(15);
const HOVER: Duration = Duration::from_secs(5);

/// What a server of `kind` gets when it asks for settings: for Python, the interpreter of the
/// root's virtualenv, so imports of its libraries resolve.
fn settings(root: &Path, kind: &Kind) -> Value {
    if kind.name != "python" {
        return json!({});
    }
    let venv = [".venv", "venv"]
        .iter()
        .map(|d| root.join(d).join("bin/python"))
        .find(|p| p.exists());
    match venv {
        Some(p) => json!({"python": {"pythonPath": p}}),
        None => json!({}),
    }
}

/// The running server of `kind` for `root`, started when there's none. `None` when none of the
/// kind's commands is on PATH.
async fn server(root: &Path, kind: &'static Kind) -> anyhow::Result<Option<Arc<Server>>> {
    let mut servers = SERVERS.lock().await;
    let key = (root.to_path_buf(), kind.name);
    if let Some(s) = servers.get(&key)
        && s.rpc.alive()
    {
        return Ok(Some(s.clone()));
    }
    let Some((program, args)) = kind
        .commands
        .iter()
        .find_map(|(name, args)| crate::tools::which(name).map(|p| (p, *args)))
    else {
        return Ok(None);
    };
    let rpc = Rpc::spawn(&program, args, root, settings(root, kind))?;
    let root_uri = uri::of(root);
    let name = root.file_name().map(|n| n.to_string_lossy().into_owned());
    let params = json!({
        "processId": std::process::id(),
        "rootUri": root_uri,
        "workspaceFolders": [{"uri": root_uri, "name": name.unwrap_or_default()}],
        "capabilities": {
            "general": {"positionEncodings": ["utf-16"]},
            "textDocument": {
                "synchronization": {"dynamicRegistration": false},
                "hover": {"contentFormat": ["markdown", "plaintext"]},
                "definition": {"linkSupport": true},
            },
            "workspace": {"configuration": true, "workspaceFolders": true},
        },
    });
    rpc.request("initialize", params, START).await?;
    rpc.notify("initialized", json!({}));
    let s = Arc::new(Server {
        rpc,
        docs: Mutex::default(),
    });
    servers.insert(key, s.clone());
    Ok(Some(s))
}

/// Stops the servers running for workspace root `root`.
pub async fn stop(root: &Path) {
    SERVERS.lock().await.retain(|(r, _), _| r != root);
}

impl Server {
    /// Gives the server `text` as `path`'s contents, unless it has them already.
    fn sync(&self, path: &Path, lang: &str, text: String) {
        let uri = uri::of(path);
        let mut docs = self.docs.lock().unwrap();
        match docs.get_mut(path) {
            Some((_, had)) if *had == text => {}
            Some((version, had)) => {
                *version += 1;
                let params = json!({
                    "textDocument": {"uri": uri, "version": *version},
                    "contentChanges": [{"text": text}],
                });
                *had = text;
                self.rpc.notify("textDocument/didChange", params);
            }
            None => {
                let params = json!({
                    "textDocument": {"uri": uri, "languageId": lang, "version": 1, "text": text},
                });
                docs.insert(path.to_path_buf(), (1, text));
                self.rpc.notify("textDocument/didOpen", params);
            }
        }
    }
}

/// Where in a file to ask about: a 0-based line and UTF-16 column, with the file's text when
/// it isn't as on disk.
pub struct At<'a> {
    pub root: &'a Path,
    pub file: &'a Path,
    pub text: Option<String>,
    pub line: u32,
    pub character: u32,
}

/// Asks the file's server `method` at `at`. `None` when no server takes the file.
async fn ask(at: At<'_>, method: &str, wait: Duration) -> anyhow::Result<Option<Value>> {
    let Some((kind, lang)) = kind_of(at.file) else {
        return Ok(None);
    };
    let Some(server) = server(at.root, kind).await? else {
        return Ok(None);
    };
    let text = match at.text {
        Some(t) => t,
        None => std::fs::read_to_string(at.file)?,
    };
    server.sync(at.file, lang, text);
    let params = json!({
        "textDocument": {"uri": uri::of(at.file)},
        "position": {"line": at.line, "character": at.character},
    });
    Ok(Some(server.rpc.request(method, params, wait).await?))
}

/// A place a definition is: the path relative to the workspace root, or absolute when outside
/// it, and its 0-based line and UTF-16 column.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
pub struct Place {
    pub path: String,
    pub external: bool,
    pub line: u32,
    pub character: u32,
}

/// The definitions of the symbol at `at`. `None` when no server takes the file.
pub async fn definition(at: At<'_>) -> anyhow::Result<Option<Vec<Place>>> {
    let root = at.root.to_path_buf();
    let Some(v) = ask(at, "textDocument/definition", DEFINITION).await? else {
        return Ok(None);
    };
    let items = match v {
        Value::Array(a) => a,
        Value::Null => vec![],
        one => vec![one],
    };
    let places = items
        .iter()
        .filter_map(|l| {
            let (uri, range) = match l.get("targetUri") {
                Some(u) => (u, &l["targetSelectionRange"]),
                None => (&l["uri"], &l["range"]),
            };
            let path = uri::path(uri.as_str()?)?;
            let start = &range["start"];
            Some(place(
                &root,
                path,
                start["line"].as_u64()?,
                start["character"].as_u64()?,
            ))
        })
        .collect();
    Ok(Some(places))
}

fn place(root: &Path, path: PathBuf, line: u64, character: u64) -> Place {
    let (line, character) = (line as u32, character as u32);
    match path.strip_prefix(root) {
        Ok(rel) => Place {
            path: rel.to_string_lossy().into_owned(),
            external: false,
            line,
            character,
        },
        Err(_) => {
            let shown = path.to_string_lossy().into_owned();
            EXTERNAL.lock().unwrap().insert(path);
            Place {
                path: shown,
                external: true,
                line,
                character,
            }
        }
    }
}

/// The server's markdown about the symbol at `at`. `None` when there's nothing to show.
pub async fn hover(at: At<'_>) -> anyhow::Result<Option<String>> {
    let Some(v) = ask(at, "textDocument/hover", HOVER).await? else {
        return Ok(None);
    };
    Ok(hover::markdown(&v["contents"]))
}

#[cfg(test)]
mod tests;
