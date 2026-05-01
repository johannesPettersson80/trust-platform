//! Minimal terminal styling for developer/workbench commands.

use owo_colors::OwoColorize;

pub(crate) fn error(message: impl AsRef<str>) -> String {
    message.as_ref().red().bold().to_string()
}
