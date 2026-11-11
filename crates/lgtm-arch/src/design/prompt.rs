//! The prompt asking a terminal agent to write a design's map from its spec files.

use std::fmt::Write;

use lgtm_canvas::{Canvas, ViewCanvas};

/// Past this many, the prompt lists only the first files and says how many more.
pub const MAX_FILES: usize = 200;

pub const INSTRUCTIONS: &str = "Write a C4 map of the design these files describe, for lgtm's \
Architecture tab. Read every file first; don't edit anything.

- people and externals: who uses the designed system and what outside systems it talks to.
- containers: the deployable parts the design has. Reuse a code container's id, and set \
`code` to it, when the design keeps or changes one; give a new one a short kebab-case id.
- components: the parts inside each container, with ids like `<container>/<name>`, and \
`code` set to the code component they are or change.
- interfaces: the APIs, events and functions the spec names for each component, with the \
signature as written there.
- relations: who uses what, by person or external name and container or component id.
- change on each: `new`, `changed`, `removed` or `same` against the code's system below \
(everything is `new` when there is no code). A new container or component that takes over \
from removed ones lists their ids in `replaces`.
- spec: one to three lines of the spec files (path, 1-based line, what it says in a \
sentence) behind each container, component and interface.
- built: compare with the code. `built` when the code has it as designed, `planned` when it \
doesn't yet, `differs` when the code has it otherwise. Leave it null for `removed`.
- differences: each place the code differs from the design, or has parts the spec doesn't \
mention, under the design node it concerns, with a file and line where there is one.

Keep descriptions under a dozen words. Then call lgtm's set_design tool with id";

/// The canvas drawn over the design so far, and how to draw on it.
pub fn canvas_text(c: &Canvas) -> String {
    let drawn = !c.colors.is_empty() || c.views.values().any(|v| *v != ViewCanvas::default());
    let mut out = String::from(
        "\n\nAfterwards you may draw on the diagram with edit_canvas (kind `design`): arrange a \
         view, group parts under labels, color them, style edges, and add shapes, text and \
         arrows.",
    );
    if drawn {
        let mut c = c.clone();
        c.agent = None;
        write!(
            out,
            " The user and earlier passes drew this, each element marked by who made it; keep \
             it unless the user asks otherwise, and use it to see how they think about the \
             design:\n\n```json\n{}\n```",
            serde_json::to_string(&c).unwrap_or_default()
        )
        .unwrap();
    }
    out
}
