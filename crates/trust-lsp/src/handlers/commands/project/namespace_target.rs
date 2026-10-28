//! The namespace move's target (specification 25 §1.1, Move Structured Text Namespace): whether the
//! cursor's line declares or uses a namespace, the name the move starts from, a new namespace path
//! and the file it moves to by default, and the editor's quick fix.
//!
//! A line is a namespace line when, after its indentation, it is `NAMESPACE` or `USING` alone or
//! followed by a space, in any letter case. The move starts from the dotted name under the cursor,
//! or else from the line's first dotted name after the keyword. Positions count UTF-16 code units,
//! as the editor's do. A new path is one or more identifiers joined by dots, none of them a
//! keyword, as the language server's move takes it; its default file is `<last part>.st` in one
//! folder per earlier part below the first workspace folder.

use serde::Deserialize;
use serde_json::{json, Value};
use tower_lsp::lsp_types::{
    CodeAction, CodeActionKind, CodeActionOrCommand, CodeActionParams, Command, Position, Url,
};
use trust_ide::refactor::parse_namespace_path;

use super::decode;
use crate::state::{uri_to_path, Document, ServerState};

/// The quick fix's title, which is also the command's (specification 25 §1.1).
const MOVE_TITLE: &str = "Move Structured Text Namespace";
/// The extension command the quick fix runs.
const MOVE_UI_COMMAND: &str = "trust-lsp.moveNamespace.ui";

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
enum Args {
    /// Whether the line at `position` is a namespace line, and the name the move starts from.
    Context {
        #[serde(rename = "textDocument")]
        text_document: TextDocument,
        position: Position,
    },
    /// The check of a new namespace path and its default target file.
    Path {
        #[serde(rename = "newPath")]
        new_path: String,
    },
}

#[derive(Deserialize)]
struct TextDocument {
    uri: Url,
}

/// `context`: `{ ok, onNamespaceLine, namePosition }`. `path`: `{ ok, problem, path, target }`, where
/// `problem` is the input check's message or `null`, `path` the path the move takes and `target`
/// the default file's URI, or `null` without a workspace folder.
pub(super) fn answer(state: &ServerState, argument: Value) -> Result<Value, String> {
    match decode::<Args>(argument)? {
        Args::Context {
            text_document,
            position,
        } => {
            let text = document_text(state, &text_document.uri);
            let line = text
                .as_deref()
                .and_then(|text| line_text(text, position.line));
            let on_namespace_line = line.is_some_and(is_namespace_or_using_line);
            let name_position = line
                .filter(|_| on_namespace_line)
                .and_then(|line| name_start(line, position.character))
                .map(|character| Position {
                    line: position.line,
                    character,
                });
            Ok(json!({
                "ok": true,
                "onNamespaceLine": on_namespace_line,
                "namePosition": name_position,
            }))
        }
        Args::Path { new_path } => {
            let path = new_path.trim();
            let Some(parts) = parse_namespace_path(path) else {
                let problem = if path.is_empty() {
                    "Namespace path is required."
                } else {
                    "Namespace path must use dot-separated identifiers."
                };
                return Ok(
                    json!({ "ok": true, "problem": problem, "path": Value::Null, "target": Value::Null }),
                );
            };
            // The language server's own default, which the move takes when the extension names no
            // target file.
            let target = super::super::derive_target_uri(state, &parts);
            Ok(json!({
                "ok": true,
                "problem": Value::Null,
                "path": path,
                "target": target.map(|target| target.to_string()),
            }))
        }
    }
}

/// The quick fix `Move Structured Text Namespace` on a namespace line, which starts the extension's
/// move from the name under the cursor.
pub(super) fn code_actions(doc: &Document, params: &CodeActionParams) -> Vec<CodeActionOrCommand> {
    if !allows_quick_fix(params.context.only.as_deref()) {
        return Vec::new();
    }
    let start = params.range.start;
    let Some(line) = line_text(&doc.content, start.line) else {
        return Vec::new();
    };
    if !is_namespace_or_using_line(line) {
        return Vec::new();
    }
    let Some(character) = name_start(line, start.character) else {
        return Vec::new();
    };
    let command = Command {
        title: MOVE_TITLE.to_owned(),
        command: MOVE_UI_COMMAND.to_owned(),
        arguments: Some(vec![json!({
            "uri": doc.uri.to_string(),
            "position": { "line": start.line, "character": character },
        })]),
    };
    vec![CodeActionOrCommand::CodeAction(CodeAction {
        title: MOVE_TITLE.to_owned(),
        kind: Some(CodeActionKind::QUICKFIX),
        command: Some(command),
        ..CodeAction::default()
    })]
}

/// Whether the client's `only` filter lets a quick fix through: no filter, or a kind that is
/// `quickfix` or a parent of it.
fn allows_quick_fix(only: Option<&[CodeActionKind]>) -> bool {
    only.is_none_or(|kinds| {
        kinds.iter().any(|kind| {
            let kind = kind.as_str();
            kind.is_empty() || kind == CodeActionKind::QUICKFIX.as_str()
        })
    })
}

/// The document's text: the language server's copy, else the file's.
fn document_text(state: &ServerState, uri: &Url) -> Option<String> {
    if let Some(document) = state.get_document(uri) {
        return Some(document.content);
    }
    std::fs::read_to_string(uri_to_path(uri)?).ok()
}

/// Line `line` (zero-based) of `text`, without its line break.
fn line_text(text: &str, line: u32) -> Option<&str> {
    let line = text.split('\n').nth(usize::try_from(line).ok()?)?;
    Some(line.strip_suffix('\r').unwrap_or(line))
}

/// Whether `line` is `NAMESPACE` or `USING`, alone or followed by a space, after its indentation.
fn is_namespace_or_using_line(line: &str) -> bool {
    let upper = line.trim_start().to_uppercase();
    ["NAMESPACE", "USING"].into_iter().any(|keyword| {
        upper
            .strip_prefix(keyword)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    })
}

/// The UTF-16 column where the move's name starts on a namespace line: the dotted name that holds
/// the cursor column `cursor`, else the first dotted name after the keyword and its blanks.
fn name_start(line: &str, cursor: u32) -> Option<u32> {
    let trimmed = line.trim_start();
    let upper = trimmed.to_uppercase();
    let keyword = ["NAMESPACE", "USING"]
        .into_iter()
        .find(|keyword| upper.starts_with(*keyword))?;
    let indent = utf16_len(line) - utf16_len(trimmed);
    let after_keyword = skip_utf16(trimmed, utf16_len(keyword))?;
    let names = after_keyword.trim_start();
    let blanks = utf16_len(after_keyword) - utf16_len(names);
    if blanks == 0 {
        return None;
    }
    let base = indent + utf16_len(keyword) + blanks;
    let spans = dotted_names(names);
    let (first, _) = *spans.first()?;
    let hit = spans
        .iter()
        .find(|(start, end)| (base + start..=base + end).contains(&cursor))
        .map_or(first, |(start, _)| *start);
    Some(base + hit)
}

/// The UTF-16 spans `[start, end)` of the dotted names in `text`: an identifier start
/// (`A-Z`, `a-z`, `_`) with identifier characters, then `.` and another such identifier, repeated.
fn dotted_names(text: &str) -> Vec<(u32, u32)> {
    let chars: Vec<char> = text.chars().collect();
    let mut offsets = Vec::with_capacity(chars.len() + 1);
    let mut offset = 0u32;
    for ch in &chars {
        offsets.push(offset);
        offset += u32::try_from(ch.len_utf16()).unwrap_or(2);
    }
    offsets.push(offset);
    let is_start = |ch: char| ch.is_ascii_alphabetic() || ch == '_';
    let is_part = |ch: char| ch.is_ascii_alphanumeric() || ch == '_';
    let mut spans = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if !is_start(chars[index]) {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < chars.len() && is_part(chars[index]) {
            index += 1;
        }
        while index + 1 < chars.len() && chars[index] == '.' && is_start(chars[index + 1]) {
            index += 2;
            while index < chars.len() && is_part(chars[index]) {
                index += 1;
            }
        }
        spans.push((offsets[start], offsets[index]));
    }
    spans
}

fn utf16_len(text: &str) -> u32 {
    u32::try_from(text.encode_utf16().count()).unwrap_or(u32::MAX)
}

/// `text` after its first `units` UTF-16 code units, when they end at a character boundary.
fn skip_utf16(text: &str, units: u32) -> Option<&str> {
    let mut count = 0u32;
    for (index, ch) in text.char_indices() {
        if count == units {
            return Some(&text[index..]);
        }
        count += u32::try_from(ch.len_utf16()).unwrap_or(2);
    }
    (count == units).then_some("")
}

#[cfg(test)]
#[path = "namespace_target_tests.rs"]
mod tests;
