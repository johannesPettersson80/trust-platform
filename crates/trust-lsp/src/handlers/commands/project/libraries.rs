//! `trust-lsp.projectLibraries` (specification 25 §7, §11.6): the Libraries panel's entries and
//! every edit it makes to a project, and the quick fix that adds a curated library whose symbol is
//! under the cursor.
//!
//! - `curated {}` → `{ curated }`: the curated libraries' cards.
//! - `view { root, bundled, resolved? }` → `{ libraries, curated }`: the panel's entries (`resolved`
//!   is what `trust-lsp.projectInfo` reports for the project) and the curated cards.
//! - `add { root, bundled, source }` → `{ message }`, or `{ ok: false, needsManifest: true }` when a
//!   local folder has no truST library manifest and the user has not been asked.
//! - `remove { root, name }`, `fixPath { root, name, folder }` → `{ message }`.
//! - `updatePlan { root, bundled, id }` → `{ label, current, next }` for the confirmation.
//! - `insertion { text, snippet }` → `{ insertion: { line, text } | null }`.
//!
//! `root` is the project folder, `bundled` the extension's `media/libraries` folder.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{json, Value};
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, Command, Position,
};

use super::decode;
use crate::handlers::lsp_utils::position_to_offset;
use crate::state::{Document, ServerState};

mod catalog;
mod edits;
pub(crate) mod manifest;
mod symbols;
mod view;

use edits::Added;
use view::ResolvedLibrary;

/// The extension's command that adds a library; the quick fix runs it.
const ADD_LIBRARY_COMMAND: &str = "trust-lsp.libraries.add";

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
enum Args {
    /// The curated libraries, for the palette's quick pick.
    Curated {},
    View {
        root: PathBuf,
        bundled: PathBuf,
        #[serde(default)]
        resolved: Vec<ResolvedLibrary>,
    },
    Add {
        root: PathBuf,
        bundled: PathBuf,
        source: Source,
    },
    Remove {
        root: PathBuf,
        name: String,
    },
    UpdatePlan {
        root: PathBuf,
        bundled: PathBuf,
        id: String,
    },
    FixPath {
        root: PathBuf,
        name: String,
        folder: PathBuf,
    },
    Insertion {
        text: String,
        snippet: String,
    },
}

/// Where a library to add comes from.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Source {
    Curated {
        id: String,
    },
    Local {
        path: PathBuf,
        #[serde(default, rename = "createManifest")]
        create_manifest: Option<bool>,
    },
    Git {
        name: String,
        url: String,
        #[serde(default)]
        pin: Option<String>,
        #[serde(default)]
        version: Option<String>,
    },
}

/// The answer of `trust-lsp.projectLibraries`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    match decode::<Args>(argument)? {
        Args::Curated {} => Ok(json!({ "ok": true, "curated": view::curated_cards() })),
        Args::View {
            root,
            bundled,
            resolved,
        } => Ok(json!({
            "ok": true,
            "libraries": view::entries(&root, &bundled, &resolved)?,
            "curated": view::curated_cards(),
        })),
        Args::Add {
            root,
            bundled,
            source,
        } => {
            let added = match source {
                Source::Curated { id } => edits::add_curated(&root, &bundled, &id)?,
                Source::Local {
                    path,
                    create_manifest,
                } => edits::add_local(&root, &path, create_manifest)?,
                Source::Git {
                    name,
                    url,
                    pin,
                    version,
                } => edits::add_git(&root, &name, &url, pin.as_deref(), version)?,
            };
            Ok(match added {
                Added::Done(message) => json!({ "ok": true, "message": message }),
                Added::NeedsManifest => json!({
                    "ok": false,
                    "needsManifest": true,
                    "error": "No library added — the folder needs a truST library manifest.",
                }),
            })
        }
        Args::Remove { root, name } => {
            Ok(json!({ "ok": true, "message": edits::remove(&root, &name)? }))
        }
        Args::UpdatePlan { root, bundled, id } => {
            let (label, current, next) = edits::update_plan(&root, &bundled, &id)?;
            Ok(json!({ "ok": true, "label": label, "current": current, "next": next }))
        }
        Args::FixPath { root, name, folder } => {
            Ok(json!({ "ok": true, "message": edits::fix_path(&root, &name, &folder)? }))
        }
        Args::Insertion { text, snippet } => {
            let insertion = edits::insertion(&text, &snippet)
                .map(|(line, text)| json!({ "line": line, "text": text }));
            Ok(json!({ "ok": true, "insertion": insertion }))
        }
    }
}

/// The quick fix `Add <library> to this project` for a curated library's symbol under the cursor,
/// when the project's `trust-lsp.toml` does not already name the library (specification 25
/// §11.6).
pub(super) fn code_actions(
    state: &ServerState,
    doc: &Document,
    params: &CodeActionParams,
) -> Vec<CodeActionOrCommand> {
    if !offers_quick_fixes(params.context.only.as_deref()) {
        return Vec::new();
    }
    let Some(symbol) = word_at(&doc.content, params.range.start) else {
        return Vec::new();
    };
    if catalog::library_for_symbol(&symbol).is_none() {
        return Vec::new();
    }
    let Some(config) = state.workspace_config_for_uri(&params.text_document.uri) else {
        return Vec::new();
    };
    let names: Vec<&str> = config
        .dependencies
        .iter()
        .map(|dependency| dependency.name.as_str())
        .collect();
    catalog::quick_fix_library(&symbol, &names)
        .map(quick_fix_action)
        .into_iter()
        .collect()
}

/// Whether a request limited to `only` takes quick fixes.
fn offers_quick_fixes(only: Option<&[CodeActionKind]>) -> bool {
    only.is_none_or(|kinds| {
        kinds
            .iter()
            .any(|kind| kind.as_str().is_empty() || *kind == CodeActionKind::QUICKFIX)
    })
}

fn quick_fix_action(library: &catalog::CuratedLibrary) -> CodeActionOrCommand {
    let title = format!("Add {} to this project", library.label);
    CodeActionOrCommand::CodeAction(CodeAction {
        title: title.clone(),
        kind: Some(CodeActionKind::QUICKFIX),
        command: Some(Command {
            title,
            command: ADD_LIBRARY_COMMAND.to_owned(),
            arguments: Some(vec![
                json!({ "source": { "kind": "curated", "id": library.id } }),
            ]),
        }),
        ..CodeAction::default()
    })
}

/// The identifier under `position` (on it or just after it), as the editor names a word: letters,
/// digits and `_`, not starting with a digit.
fn word_at(text: &str, position: Position) -> Option<String> {
    let offset = usize::try_from(position_to_offset(text, position)?).ok()?;
    let bytes = text.as_bytes();
    let is_word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let mut start = offset.min(bytes.len());
    while start > 0 && is_word(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = offset.min(bytes.len());
    while end < bytes.len() && is_word(bytes[end]) {
        end += 1;
    }
    let identifier_start = start
        + bytes[start..end]
            .iter()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
    (identifier_start < end && offset >= identifier_start)
        .then(|| text[identifier_start..end].to_owned())
}

#[cfg(test)]
#[path = "libraries/libraries_tests.rs"]
mod tests;
