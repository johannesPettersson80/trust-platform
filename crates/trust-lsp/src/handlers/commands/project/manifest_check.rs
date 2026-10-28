//! Whether a project's `trust-lsp.toml` can be read (specification 25 §6.2). The sidebar and the
//! status bar treat a folder as an open truST project only when its manifest can be read; a manifest
//! the language server cannot read is a project that needs repair.

use serde::Deserialize;
use serde_json::{json, Value};

use super::decode;
use crate::config::ProjectConfig;

#[derive(Deserialize)]
struct Args {
    /// The manifest's text.
    text: String,
}

/// `{ ok: true, issue }`: `issue` is `null` for a readable manifest, else what cannot be read.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    let args: Args = decode(argument)?;
    Ok(json!({ "ok": true, "issue": manifest_issue(&args.text) }))
}

/// What keeps `text` from being read as a project manifest: the language server reads it exactly
/// as it loads a project (`ProjectConfig::from_contents`), so the sidebar never calls a project
/// open that the language server cannot load, nor one it loads broken.
fn manifest_issue(text: &str) -> Option<String> {
    ProjectConfig::read_problem(text)
        .map(|(line, message)| format!("Project settings on line {line} cannot be read: {message}"))
}

#[cfg(test)]
#[path = "manifest_check_tests.rs"]
mod tests;
