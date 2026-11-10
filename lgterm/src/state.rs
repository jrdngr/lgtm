//! Layouts saved on quit and restored on launch.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agents::{Kind, Message};
use crate::model::legacy::OldModel;
use crate::model::{Model, Tab, TermId};

#[derive(Debug, Serialize, Deserialize)]
pub struct SavedTerm {
    pub cwd: PathBuf,
    /// The agent running in it and its session, to resume.
    pub agent: Option<(Kind, String)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    #[serde(deserialize_with = "any_model")]
    pub model: Model,
    pub terms: BTreeMap<TermId, SavedTerm>,
    pub messages: Vec<Message>,
    pub collapsed: Vec<u64>,
    pub sidebar: bool,
    /// The sidebar's width, once the user drags it.
    #[serde(default)]
    pub sidebar_width: Option<f32>,
    /// The Messages tab's id, from states saved before tabs said what they show.
    #[serde(default, skip_serializing)]
    messages_tab: Option<TermId>,
}

/// A model as saved now, or as saved before spaces left layouts.
fn any_model<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Model, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Any {
        Now(Model),
        Old(OldModel),
    }
    Ok(match Any::deserialize(d)? {
        Any::Now(m) => m,
        Any::Old(m) => m.into(),
    })
}

/// The newest messages kept across a restart.
const SAVED_MESSAGES: usize = 200;

fn path() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("lgterm").join("state.json"))
}

impl State {
    pub fn new(
        model: Model,
        terms: BTreeMap<TermId, SavedTerm>,
        messages: &[Message],
        collapsed: Vec<u64>,
        sidebar: bool,
        sidebar_width: f32,
    ) -> Self {
        let from = messages.len().saturating_sub(SAVED_MESSAGES);
        State {
            model,
            terms,
            messages: messages[from..].to_vec(),
            collapsed,
            sidebar,
            sidebar_width: Some(sidebar_width),
            messages_tab: None,
        }
    }

    pub fn load() -> Option<State> {
        Self::parse(&std::fs::read(path()?).ok()?)
    }

    fn parse(bytes: &[u8]) -> Option<State> {
        let mut state: State = serde_json::from_slice(bytes).ok()?;
        if let Some(id) = state.messages_tab.take() {
            state.model.replace_tab(Tab::Term(id), Tab::Messages);
        }
        state.model.is_valid().then_some(state)
    }

    pub fn to_json(&self) -> Option<Vec<u8>> {
        serde_json::to_vec(self).ok()
    }

    pub fn write(json: &[u8]) {
        let Some(path) = path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(tmp, path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Axis;

    #[test]
    fn states_survive_a_save_and_load() {
        let (mut model, first) = Model::new();
        model.split_split(Axis::Row);
        model.show_messages();
        let terms = BTreeMap::from([(
            first,
            SavedTerm {
                cwd: "/tmp".into(),
                agent: Some((Kind::Claude, "abc".into())),
            },
        )]);
        let json = State::new(model, terms, &[], vec![3], true, 200.)
            .to_json()
            .unwrap();
        let back = State::parse(&json).unwrap();
        assert_eq!(back.model.tab(), Some(Tab::Messages));
        assert_eq!(back.terms[&first].agent, Some((Kind::Claude, "abc".into())));
        assert_eq!(back.collapsed, [3]);
    }

    #[test]
    fn an_older_messages_tab_id_becomes_the_messages_tab() {
        let (mut model, _) = Model::new();
        let old = model.new_tab();
        let mut json =
            serde_json::to_value(State::new(model, BTreeMap::new(), &[], vec![], true, 200.))
                .unwrap();
        json["messages_tab"] = old.0.into();
        let back = State::parse(json.to_string().as_bytes()).unwrap();
        assert_eq!(back.model.tab(), Some(Tab::Messages));
        assert_eq!(back.model.terms().len(), 1);
    }

    #[test]
    fn layouts_saved_with_their_spaces_become_slots() {
        let old = r#"{"layouts":[
            {"id":1,"name":"work","focus":3,"zoom":null,"spaces":{"Branch":{"axis":"Row","children":[
                {"weight":1.0,"tree":{"Leaf":{"id":2,"name":null,"focus":4,"splits":{"Leaf":{"id":4,"tabs":[5],"active":0}}}}},
                {"weight":1.0,"tree":{"Leaf":{"id":3,"name":"api","focus":6,"splits":{"Leaf":{"id":6,"tabs":[7],"active":0}}}}}
            ]}}},
            {"id":8,"name":null,"focus":9,"zoom":null,"spaces":{"Leaf":{"id":9,"name":null,"focus":10,"splits":{"Leaf":{"id":10,"tabs":[11],"active":0}}}}}
        ],"active":0,"next":12}"#;
        let json = format!(
            r#"{{"model":{old},"terms":{{}},"messages":[],"collapsed":[],"sidebar":true}}"#
        );
        let m = State::parse(json.as_bytes()).unwrap().model;
        assert_eq!(m.terms(), [TermId(5), TermId(7), TermId(11)]);
        assert_eq!(m.layouts[0].spaces().len(), 2);
        assert_eq!(m.layouts[1].spaces().len(), 1);
        assert_eq!(m.term(), Some(TermId(7)));
        let colors: Vec<usize> = m.spaces.iter().map(|s| s.color).collect();
        assert_eq!(colors, [0, 1, 2]);
    }
}
