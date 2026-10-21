//! The canvas tools: drawings, the canvas the user is looking at, and drawing on it.

use std::collections::BTreeMap;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::{Deserialize, Serialize};
use tauri_specta::Event as _;

use super::server::{Lgtm, Out, failed, json};
use crate::canvas::{CanvasKind, CanvasOp, FocusPart, Swatch, ViewCanvas};
use crate::events::{DesignChanged, DrawingChanged};

#[derive(Deserialize, JsonSchema)]
pub struct NewDrawing {
    pub name: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct CanvasEdit {
    pub kind: CanvasKind,
    /// The drawing's or design's id, from current_canvas, list_drawings or list_designs.
    pub id: String,
    /// A design's view: `context`, `containers`, `components/<container id>` or
    /// `interfaces/<component id>`. Drawings have one view and leave it out.
    pub view: Option<String>,
    /// Applied in order, as one step the user can undo. Shape ids start `s:`. A design's part
    /// ids: `c:<container id>`, `k:<component id>`, `p:<person name>`, `x:<external name>`,
    /// `i:<component id>#<n>` for a component's n-th interface (from 0), and `sys` for the
    /// system at the context level. Design parts can be moved, colored and grouped, never
    /// removed.
    pub ops: Vec<CanvasOp>,
}

#[derive(Serialize)]
struct DrawingItem {
    id: String,
    name: String,
}

#[derive(Serialize)]
struct Current {
    kind: CanvasKind,
    id: String,
    name: String,
    view: String,
    /// The user's numbered tags: digit to element id.
    tags: BTreeMap<String, String>,
    selection: Vec<String>,
    canvas: ViewCanvas,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    parts: Vec<FocusPart>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    part_colors: BTreeMap<String, Swatch>,
}

#[tool_router(router = canvas_router, vis = "pub(super)")]
impl Lgtm {
    #[tool(description = "The repo's drawings in lgtm, the last changed first.")]
    async fn list_drawings(&self) -> Out {
        let (_, ctx) = self.repo().map_err(failed)?;
        let all = ctx.drawings().map_err(failed)?;
        json(
            &all.into_iter()
                .map(|d| DrawingItem {
                    id: d.id,
                    name: d.name,
                })
                .collect::<Vec<_>>(),
        )
    }

    #[tool(description = "Make an empty drawing in lgtm and return its id.")]
    async fn create_drawing(&self, Parameters(p): Parameters<NewDrawing>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let d = ctx.create_drawing(&p.name).map_err(failed)?;
        DrawingChanged { id: d.id.clone() }
            .emit_to(self.app(), &label)
            .ok();
        Ok(format!("Made drawing `{}` with id `{}`.", d.name, d.id))
    }

    #[tool(
        description = "The canvas the user last looked at in lgtm: a drawing, or a view of a \
        design's diagram. Returns its elements with who made each, what the user selected, and \
        their tags. Tags are the user's numbered pointers (0 to 9) to elements: when they say \
        \"3\" or \"tag 3\", they mean the element under tag 3 here. Call it first whenever the \
        user talks about a drawing, a diagram, a selection or a tag."
    )]
    async fn current_canvas(&self) -> Out {
        let (_, ctx) = self.repo().map_err(failed)?;
        let f = ctx.canvas_focus(self.workspace()).ok_or_else(|| {
            "No drawing or design diagram is open in lgtm. list_drawings has the drawings."
                .to_string()
        })?;
        let (name, canvas, colors) = match f.kind {
            CanvasKind::Drawing => {
                let d = ctx.drawing(&f.id).map_err(failed)?;
                (d.name, d.canvas, BTreeMap::new())
            }
            CanvasKind::Design => {
                let d = ctx.design(&f.id).map_err(failed)?;
                (d.name, d.canvas.clone(), d.canvas.colors)
            }
        };
        let view = canvas.views.get(&f.view).cloned().unwrap_or_default();
        json(&Current {
            kind: f.kind,
            id: f.id,
            name,
            view: f.view,
            tags: view.tags.clone(),
            selection: f.selection,
            canvas: view,
            parts: f.parts,
            part_colors: colors,
        })
    }

    #[tool(
        description = "Draw on a drawing or a design's diagram in lgtm: add and change shapes, \
        text, sticky notes, arrows and lines, group things under labeled frames (groups nest), \
        color them, move them, and on designs style edges. The user draws there too; \
        current_canvas shows the canvas as it is, with who made each element. Your changes may \
        move or restyle the user's; they show as one agent pass the user can undo."
    )]
    async fn edit_canvas(&self, Parameters(p): Parameters<CanvasEdit>) -> Out {
        let (label, ctx) = self.repo().map_err(failed)?;
        let n = p.ops.len();
        match p.kind {
            CanvasKind::Drawing => {
                ctx.edit_drawing_canvas(&p.id, p.ops).map_err(failed)?;
                DrawingChanged { id: p.id.clone() }
                    .emit_to(self.app(), &label)
                    .ok();
                Ok(format!("Applied {n} changes to drawing `{}`.", p.id))
            }
            CanvasKind::Design => {
                let view = p.view.unwrap_or_default();
                ctx.edit_design_canvas(&p.id, &view, p.ops)
                    .map_err(failed)?;
                DesignChanged {
                    id: p.id.clone(),
                    workspace: self.workspace().map(String::from),
                }
                .emit_to(self.app(), &label)
                .ok();
                Ok(format!(
                    "Applied {n} changes to `{view}` of design `{}`.",
                    p.id
                ))
            }
        }
    }
}
