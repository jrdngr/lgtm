//! Which repo a command is for. A project window shows each of its repos in a frame of its
//! own, and a frame's commands carry its repo's label in a header. Commands from a window
//! without frames, like the start window, act as the window.

use std::ops::Deref;

use tauri::ipc::{CommandArg, CommandItem, InvokeError};
use tauri::{WebviewWindow, Wry};

/// The header naming the repo a frame shows, by its label.
pub const HEADER: &str = "lgtm-repo";

/// The window a command came from, and the label of the repo it's for.
pub struct Frame {
    window: WebviewWindow,
    label: String,
}

impl Frame {
    /// The repo's label, which keys its state, terminals and events.
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl Deref for Frame {
    type Target = WebviewWindow;

    fn deref(&self) -> &WebviewWindow {
        &self.window
    }
}

impl<'de> CommandArg<'de, Wry> for Frame {
    fn from_command(command: CommandItem<'de, Wry>) -> Result<Self, InvokeError> {
        let label = command
            .message
            .headers()
            .get(HEADER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let window = WebviewWindow::from_command(command)?;
        let label = label.unwrap_or_else(|| window.label().to_string());
        Ok(Self { window, label })
    }
}

impl specta::function::FunctionArg for Frame {
    fn to_datatype(_: &mut specta::Types) -> Option<specta::datatype::DataType> {
        None
    }
}
