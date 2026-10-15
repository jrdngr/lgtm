//! Persona reviews: personas review a revset headless, each in its own agent run, and return
//! structured findings. Synthesis, cross-review, and a minimal report are further runs over
//! those findings.

mod fix;
pub mod install;
pub mod output;
pub mod personas;
mod pipeline;
mod prompts;
mod reports;
mod schema;
mod submitted;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use specta::Type;

pub use fix::Fix;
pub use output::Output;
pub use personas::Persona;
pub use pipeline::{Event, Notify};
pub use prompts::describe;
pub(crate) use prompts::fence;

/// What a run does past the personas' own reviews.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BotOptions {
    /// One set of findings merged across every persona's.
    pub synthesize: bool,
    /// Personas challenge each other's findings before synthesis.
    pub cross_review: bool,
    /// Cross-review argues against every medium and higher finding.
    pub adversarial: bool,
    /// Only the findings that matter to what the change is for.
    pub minimal: bool,
}

impl BotOptions {
    /// Cross-review needs two personas to challenge each other, and a synthesis to rule on
    /// disputes. A minimal report from more than one persona needs a synthesis too.
    fn normalized(mut self, personas: usize) -> Self {
        self.cross_review &= personas > 1;
        self.adversarial &= self.cross_review;
        self.synthesize |= self.cross_review || (self.minimal && personas > 1);
        self
    }
}

/// The steps of a run, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Step {
    Reports,
    Challenges,
    Rebuttals,
    Synthesis,
    Minimal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Running,
    Done,
    Failed,
    Stopped,
}

/// One agent run within a review run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Part {
    pub step: Step,
    /// The persona's name. Absent for synthesis and the minimal report.
    pub persona: Option<String>,
    pub status: Status,
    /// Set once it's done.
    pub output: Option<Output>,
    /// Why it failed.
    pub error: Option<String>,
    /// The agent's session, which a rebuttal resumes.
    pub session: Option<String>,
    /// What the agent is doing while it runs. Only memory knows.
    #[serde(default)]
    pub activity: Option<String>,
}

impl Part {
    fn running(step: Step, persona: Option<String>) -> Self {
        Self {
            step,
            persona,
            status: Status::Running,
            output: None,
            error: None,
            session: None,
            activity: None,
        }
    }
}

/// A persona review of a revset, frozen at the commits it resolved to and the personas as
/// they were when it started.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BotRun {
    pub id: u32,
    /// What the user renamed it to. Its id is `R<id>` until then.
    #[serde(default)]
    pub name: Option<String>,
    pub workspace: Option<String>,
    pub revset: String,
    pub from: Option<String>,
    pub to: String,
    pub personas: Vec<Persona>,
    pub options: BotOptions,
    /// What the user told the reviewers about the change.
    #[serde(default)]
    pub context: String,
    pub parts: Vec<Part>,
    /// Steps whose parts all finished.
    pub done: Vec<Step>,
    pub created_at: f64,
    /// Agent runs that fixed picked findings, oldest first.
    #[serde(default)]
    pub fixes: Vec<Fix>,
    /// Ids of findings the user dismissed.
    #[serde(default)]
    pub dismissed: Vec<String>,
    /// Bumped each time synthesis is redone, which renumbers its findings.
    #[serde(default)]
    pub synthesis: u32,
    /// Whether it's running now. Only memory knows, so the stored value is ignored.
    #[serde(default)]
    pub active: bool,
}

/// Runs in progress and what their parts are doing.
#[derive(Default)]
pub struct Live {
    active: Mutex<HashSet<u32>>,
    /// Active runs the user stopped.
    stopping: Mutex<HashSet<u32>>,
    activity: Mutex<HashMap<(u32, usize), String>>,
    /// Running fixes' replies so far, by run and fix.
    fix_text: Mutex<HashMap<(u32, usize), String>>,
}

impl Live {
    /// Fills in what only memory knows: whether it's running and what its parts are doing.
    /// Parts and fixes left running by an app exit or a failed run show as stopped.
    pub fn fill(&self, mut run: BotRun) -> BotRun {
        run.active = self.is_active(run.id);
        let activity = self.activity.lock().unwrap();
        for (i, p) in run.parts.iter_mut().enumerate() {
            match (p.status, run.active) {
                (Status::Running, true) => p.activity = activity.get(&(run.id, i)).cloned(),
                (Status::Running, false) => p.status = Status::Stopped,
                _ => {}
            }
        }
        let text = self.fix_text.lock().unwrap();
        for (i, f) in run.fixes.iter_mut().enumerate() {
            if f.status == Status::Running {
                match text.get(&(run.id, i)) {
                    Some(t) => f.text = t.clone(),
                    None => f.status = Status::Stopped,
                }
            }
        }
        run
    }

    pub fn is_active(&self, run: u32) -> bool {
        self.active.lock().unwrap().contains(&run)
    }

    /// Marks `run` active. False when it already was.
    fn begin(&self, run: u32) -> bool {
        self.active.lock().unwrap().insert(run)
    }

    fn stop(&self, run: u32) {
        if self.is_active(run) {
            self.stopping.lock().unwrap().insert(run);
        }
    }

    fn stopping(&self, run: u32) -> bool {
        self.stopping.lock().unwrap().contains(&run)
    }

    fn end(&self, run: u32) {
        self.active.lock().unwrap().remove(&run);
        self.stopping.lock().unwrap().remove(&run);
        self.activity.lock().unwrap().retain(|(r, _), _| *r != run);
    }

    /// Whether any of the run's fixes is running.
    pub fn has_fix(&self, run: u32) -> bool {
        self.fix_text.lock().unwrap().keys().any(|(r, _)| *r == run)
    }

    fn begin_fix(&self, run: u32, fix: usize) {
        self.fix_text
            .lock()
            .unwrap()
            .insert((run, fix), String::new());
    }

    /// Adds to a fix's reply and returns the whole reply so far.
    fn append_fix(&self, run: u32, fix: usize, text: &str) -> String {
        let mut all = self.fix_text.lock().unwrap();
        let all = all.entry((run, fix)).or_default();
        all.push_str(text);
        all.clone()
    }

    fn end_fix(&self, run: u32, fix: usize) {
        self.fix_text.lock().unwrap().remove(&(run, fix));
    }

    fn set_activity(&self, run: u32, part: usize, what: String) {
        self.activity.lock().unwrap().insert((run, part), what);
    }
}

/// The key a part's agent run is registered under, to stop it.
fn run_key(run: u32, part: usize) -> String {
    format!("{}{part}", run_prefix(run))
}

fn run_prefix(run: u32) -> String {
    format!("bots-{run}-")
}
