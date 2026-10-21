//! Drawings: named canvases of the repo the user sketches on and talks over with a terminal
//! agent, which can change them through MCP.

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::canvas::{Canvas, CanvasOp};
use crate::ctx::{RepoCtx, now_ms};
use crate::store::Store;

/// A drawing's one view.
pub const VIEW: &str = "main";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DrawingDto {
    pub id: String,
    pub name: String,
    pub created_at: f64,
    pub updated_at: f64,
    pub canvas: Canvas,
    /// The canvas before the last agent pass, while that pass can be undone.
    pub canvas_undo: Option<Canvas>,
}

impl DrawingDto {
    /// Takes the user's `canvas`. One without an agent pass ends the last pass's undo.
    fn user_save(&mut self, canvas: Canvas) {
        if canvas.agent.is_none() {
            self.canvas_undo = None;
        }
        self.canvas = canvas;
        self.updated_at = now_ms() as f64;
    }

    /// Applies an agent's `ops` as one pass the user can undo.
    fn agent_edit(&mut self, ops: Vec<CanvasOp>) -> anyhow::Result<()> {
        let mut canvas = self.canvas.clone();
        canvas.apply(VIEW, ops)?;
        let mut before = std::mem::replace(&mut self.canvas, canvas);
        before.agent = None;
        self.canvas_undo = Some(before);
        self.updated_at = now_ms() as f64;
        Ok(())
    }

    fn undo_agent(&mut self) -> anyhow::Result<()> {
        self.canvas = self.canvas_undo.take().context("nothing to undo")?;
        self.updated_at = now_ms() as f64;
        Ok(())
    }
}

fn load(store: &Store, id: &str) -> anyhow::Result<DrawingDto> {
    let data = store
        .drawing(id)?
        .with_context(|| format!("no drawing `{id}`"))?;
    Ok(serde_json::from_str(&data)?)
}

fn save(store: &Store, d: &DrawingDto) -> anyhow::Result<()> {
    store.save_drawing(&d.id, &serde_json::to_string(d)?)?;
    Ok(())
}

impl RepoCtx {
    /// Every drawing, the last changed first.
    pub fn drawings(&self) -> anyhow::Result<Vec<DrawingDto>> {
        let store = self.store.lock().unwrap();
        let mut all = store
            .drawings()?
            .iter()
            .filter_map(|d| serde_json::from_str::<DrawingDto>(d).ok())
            .collect::<Vec<_>>();
        all.sort_by(|a, b| b.updated_at.total_cmp(&a.updated_at));
        Ok(all)
    }

    pub fn drawing(&self, id: &str) -> anyhow::Result<DrawingDto> {
        load(&self.store.lock().unwrap(), id)
    }

    pub fn create_drawing(&self, name: &str) -> anyhow::Result<DrawingDto> {
        let name = name.trim();
        if name.is_empty() {
            bail!("a drawing needs a name");
        }
        let store = self.store.lock().unwrap();
        let now = now_ms();
        let mut id = format!("w{now}");
        while store.drawing(&id)?.is_some() {
            id.push('x');
        }
        let d = DrawingDto {
            id,
            name: name.to_string(),
            created_at: now as f64,
            updated_at: now as f64,
            canvas: Canvas::default(),
            canvas_undo: None,
        };
        save(&store, &d)?;
        Ok(d)
    }

    pub fn rename_drawing(&self, id: &str, name: &str) -> anyhow::Result<DrawingDto> {
        let name = name.trim();
        if name.is_empty() {
            bail!("a drawing needs a name");
        }
        let store = self.store.lock().unwrap();
        let mut d = load(&store, id)?;
        d.name = name.to_string();
        save(&store, &d)?;
        Ok(d)
    }

    pub fn delete_drawing(&self, id: &str) -> anyhow::Result<()> {
        self.store.lock().unwrap().delete_drawing(id)?;
        Ok(())
    }

    pub fn save_drawing_canvas(&self, id: &str, canvas: Canvas) -> anyhow::Result<DrawingDto> {
        let store = self.store.lock().unwrap();
        let mut d = load(&store, id)?;
        d.user_save(canvas);
        save(&store, &d)?;
        Ok(d)
    }

    pub fn edit_drawing_canvas(&self, id: &str, ops: Vec<CanvasOp>) -> anyhow::Result<DrawingDto> {
        let store = self.store.lock().unwrap();
        let mut d = load(&store, id)?;
        d.agent_edit(ops)?;
        save(&store, &d)?;
        Ok(d)
    }

    pub fn undo_drawing_agent(&self, id: &str) -> anyhow::Result<DrawingDto> {
        let store = self.store.lock().unwrap();
        let mut d = load(&store, id)?;
        d.undo_agent()?;
        save(&store, &d)?;
        Ok(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drawing() -> DrawingDto {
        DrawingDto {
            id: "w1".into(),
            name: "Auth".into(),
            created_at: 0.0,
            updated_at: 0.0,
            canvas: Canvas::default(),
            canvas_undo: None,
        }
    }

    fn shape(id: &str) -> Vec<CanvasOp> {
        serde_json::from_value(serde_json::json!([{"op": "shape", "id": id}])).unwrap()
    }

    #[test]
    fn undoing_an_agent_pass_restores_the_users_canvas_until_the_user_changes_it() {
        let mut d = drawing();
        d.agent_edit(shape("a")).unwrap();
        d.agent_edit(shape("b")).unwrap();
        d.undo_agent().unwrap();
        assert_eq!(d.canvas.views[VIEW].shapes.len(), 1);
        assert_eq!(d.canvas.agent, None, "the restored canvas shows no pass");
        assert!(d.undo_agent().is_err(), "a pass undoes once");

        d.agent_edit(shape("c")).unwrap();
        let mut moved = d.canvas.clone();
        moved.agent = None;
        d.user_save(moved);
        assert!(d.undo_agent().is_err(), "the user's change ends the pass's undo");
    }
}
