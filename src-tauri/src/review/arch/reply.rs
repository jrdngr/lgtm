//! Assess's reply format, and turning a reply into an assessment the rest of the review can
//! trust: a score in range, and only components and files that exist.

use std::collections::HashSet;

use serde::Deserialize;

use super::assess::Assessed;
use super::{
    AssessmentDto, BoundaryDto, LayerDto, LookDto, LookSeverity, LookSource, RangeDto, RiskLevel,
};

pub(super) const SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "score": { "type": "integer", "minimum": 1, "maximum": 5 },
    "risk": { "enum": ["minimal", "low", "moderate", "high", "critical"] },
    "reason": { "type": "string" },
    "boundaries": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "name": { "type": "string" },
          "components": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["name", "components"],
        "additionalProperties": false
      }
    },
    "layers": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "title": { "type": "string" },
          "summary": { "type": "string" },
          "components": { "type": "array", "items": { "type": "string" } },
          "ranges": {
            "type": "array",
            "items": {
              "type": "object",
              "properties": {
                "path": { "type": "string" },
                "start": { "type": "integer" },
                "end": { "type": "integer" }
              },
              "required": ["path", "start", "end"],
              "additionalProperties": false
            }
          }
        },
        "required": ["title", "summary", "components", "ranges"],
        "additionalProperties": false
      }
    },
    "look": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "source": { "enum": ["agent", "security"] },
          "severity": { "enum": ["low", "medium", "high"] },
          "title": { "type": "string" },
          "detail": { "type": "string" },
          "component": { "type": "string" },
          "from": { "type": "string" },
          "to": { "type": "string" },
          "path": { "type": "string" },
          "line": { "type": "integer" }
        },
        "required": ["source", "severity", "title"],
        "additionalProperties": false
      }
    }
  },
  "required": ["score", "risk", "reason", "boundaries", "layers", "look"],
  "additionalProperties": false
}"#;

#[derive(Deserialize)]
pub(super) struct Reply {
    score: i64,
    risk: RiskLevel,
    reason: String,
    boundaries: Vec<BoundaryDto>,
    layers: Vec<LayerReply>,
    look: Vec<LookReply>,
}

#[derive(Deserialize)]
struct LayerReply {
    title: String,
    summary: String,
    components: Vec<String>,
    ranges: Vec<RangeReply>,
}

#[derive(Deserialize)]
struct RangeReply {
    path: String,
    start: i64,
    end: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum AgentSource {
    Agent,
    Security,
}

#[derive(Deserialize)]
struct LookReply {
    source: AgentSource,
    severity: LookSeverity,
    title: String,
    detail: Option<String>,
    component: Option<String>,
    from: Option<String>,
    to: Option<String>,
    path: Option<String>,
    line: Option<i64>,
}

/// What a reply may refer to.
pub(super) struct Known<'a> {
    pub components: HashSet<&'a str>,
    /// The review's file paths, by index.
    pub files: &'a [String],
}

impl Known<'_> {
    fn component(&self, id: Option<String>) -> Option<String> {
        id.filter(|id| self.components.contains(id.as_str()))
    }

    fn components(&self, ids: Vec<String>) -> Vec<String> {
        ids.into_iter()
            .filter(|id| self.components.contains(id.as_str()))
            .collect()
    }

    fn range(&self, r: RangeReply) -> RangeDto {
        let start = r.start.clamp(1, u32::MAX as i64) as u32;
        let end = r.end.clamp(start as i64, u32::MAX as i64) as u32;
        let file = self
            .files
            .iter()
            .position(|f| *f == r.path)
            .map(|i| i as u32);
        RangeDto {
            path: r.path,
            file,
            start,
            end,
        }
    }
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// The reply as stored, with what it names checked against `known`. Layers and boundaries
/// left with nothing in them are dropped.
pub(super) fn validate(reply: Reply, known: &Known, created_at: f64) -> Assessed {
    let boundaries = reply
        .boundaries
        .into_iter()
        .map(|b| BoundaryDto {
            name: b.name,
            components: known.components(b.components),
        })
        .filter(|b| !b.components.is_empty())
        .collect();
    let layers = reply
        .layers
        .into_iter()
        .map(|l| (l.title, l.summary, known.components(l.components), l.ranges))
        .filter(|(_, _, comps, ranges)| !comps.is_empty() || !ranges.is_empty())
        .zip(1..)
        .map(|((title, summary, components, ranges), n)| LayerDto {
            n,
            title,
            summary,
            components,
            ranges: ranges.into_iter().map(|r| known.range(r)).collect(),
        })
        .collect();
    let (mut agent, mut security) = (0, 0);
    let look = reply
        .look
        .into_iter()
        .map(|l| {
            let (source, key) = match l.source {
                AgentSource::Agent => {
                    agent += 1;
                    (LookSource::Agent, format!("agent:{agent}"))
                }
                AgentSource::Security => {
                    security += 1;
                    (LookSource::Security, format!("security:{security}"))
                }
            };
            let edge = known.component(l.from).zip(known.component(l.to));
            let path = non_empty(l.path);
            let line = l
                .line
                .filter(|&n| n >= 1 && path.is_some())
                .map(|n| n as u32);
            LookDto {
                key,
                source,
                severity: l.severity,
                title: l.title,
                detail: non_empty(l.detail),
                component: known.component(l.component),
                edge,
                path,
                line,
                checked: false,
            }
        })
        .collect();
    let assessment = AssessmentDto {
        score: reply.score.clamp(1, 5) as u8,
        risk: reply.risk,
        reason: reply.reason.lines().next().unwrap_or("").trim().to_string(),
        boundaries,
        created_at,
    };
    Assessed {
        assessment,
        layers,
        look,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(files: &[String]) -> Known<'_> {
        Known {
            components: HashSet::from(["src/api", "src/db"]),
            files,
        }
    }

    fn reply(json: serde_json::Value) -> Reply {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn score_is_clamped_and_reason_kept_to_one_line() {
        let files = [];
        for (score, want) in [(0, 1), (9, 5), (3, 3)] {
            let r = reply(serde_json::json!({
                "score": score, "risk": "low", "reason": "Fine.\nMore detail.",
                "boundaries": [], "layers": [], "look": []
            }));
            let a = validate(r, &known(&files), 7.0).assessment;
            assert_eq!((a.score, a.reason.as_str()), (want, "Fine."));
        }
    }

    #[test]
    fn unknown_components_are_dropped_and_paths_mapped_to_review_files() {
        let files = ["src/api/a.rs".to_string(), "src/db/b.rs".to_string()];
        let r = reply(serde_json::json!({
            "score": 2, "risk": "high", "reason": "r",
            "boundaries": [
                { "name": "public", "components": ["src/api", "src/ghost"] },
                { "name": "gone", "components": ["src/ghost"] }
            ],
            "layers": [
                { "title": "Ghost", "summary": "s", "components": ["src/ghost"], "ranges": [] },
                { "title": "Schema", "summary": "s", "components": ["src/db"],
                  "ranges": [{ "path": "src/db/b.rs", "start": 0, "end": -4 }] },
                { "title": "Docs", "summary": "s", "components": [],
                  "ranges": [{ "path": "README.md", "start": 3, "end": 9 }] }
            ],
            "look": [
                { "source": "security", "severity": "high", "title": "Injection",
                  "detail": "api -> db", "from": "src/api", "to": "src/db",
                  "path": "src/db/b.rs", "line": 12 },
                { "source": "agent", "severity": "low", "title": "Coupling",
                  "component": "src/ghost", "from": "src/api", "to": "src/ghost", "line": 4 },
                { "source": "security", "severity": "medium", "title": "Secrets",
                  "component": "src/api", "detail": "  " }
            ]
        }));
        let a = validate(r, &known(&files), 0.0);
        assert_eq!(
            a.assessment.boundaries,
            vec![BoundaryDto {
                name: "public".into(),
                components: vec!["src/api".into()]
            }]
        );
        let layers: Vec<_> = a
            .layers
            .iter()
            .map(|l| (l.n, l.title.as_str(), l.ranges.clone()))
            .collect();
        let range = |path: &str, file, start, end| RangeDto {
            path: path.into(),
            file,
            start,
            end,
        };
        assert_eq!(
            layers,
            vec![
                (1, "Schema", vec![range("src/db/b.rs", Some(1), 1, 1)]),
                (2, "Docs", vec![range("README.md", None, 3, 9)]),
            ]
        );
        let look: Vec<_> = a
            .look
            .iter()
            .map(|l| {
                (
                    l.key.as_str(),
                    l.component.as_deref(),
                    l.edge.clone(),
                    l.line,
                    l.detail.as_deref(),
                )
            })
            .collect();
        let edge = Some(("src/api".to_string(), "src/db".to_string()));
        assert_eq!(
            look,
            vec![
                ("security:1", None, edge, Some(12), Some("api -> db")),
                ("agent:1", None, None, None, None),
                ("security:2", Some("src/api"), None, None, None),
            ]
        );
    }
}
