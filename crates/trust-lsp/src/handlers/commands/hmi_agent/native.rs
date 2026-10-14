//! The existing HMI commands the authoring service calls in-process (specification 43, §3).
//!
//! The service parses no page, formats none, resolves no binding, checks no faceplate and reads no
//! catalogue itself. It asks the command that owns the rule, through the command table with the same
//! argument the extension sent, and reads the same answer. A refusal is a typed error here: a command
//! that must succeed is `VALIDATION_FAILED` (or the code the caller names), as the TypeScript engine
//! reported it.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use tower_lsp::lsp_types::ExecuteCommandParams;
use trust_runtime::hmi::page_files::HmiPage;

use crate::handlers::commands::{
    execute_read_command, HMI_ADD_MISSING_VARIABLES_COMMAND, HMI_BINDINGS_COMMAND,
    HMI_CATALOGUE_COMMAND, HMI_FACEPLATES_COMMAND, HMI_FILE_CHECK_COMMAND, HMI_FILE_FORMAT_COMMAND,
    HMI_FILE_PARSE_COMMAND, HMI_PLACED_FACEPLATES_COMMAND, HMI_TEXTS_COMMAND,
};
use crate::state::ServerState;

use super::documents::OpenDocument;
use super::error::{AgentError, AgentResult, COMPILATION_FAILED, VALIDATION_FAILED};

/// Calls the command `name` with one argument object and returns its answer (the `native` of the
/// TypeScript engine).
///
/// # Errors
///
/// `VALIDATION_FAILED` when the command is not in the table.
fn call(state: &ServerState, name: &str, argument: Value) -> AgentResult<Value> {
    let params: ExecuteCommandParams =
        serde_json::from_value(json!({ "command": name, "arguments": [argument] }))
            .map_err(|error| AgentError::validation(format!("{name} cannot be called: {error}")))?;
    execute_read_command(state, params)
        .ok_or_else(|| AgentError::validation(format!("{name} is not available.")))
}

/// The answer of a command that must succeed for a read: `ok` true and no variables error.
///
/// # Errors
///
/// `code` with the command's own message when it refuses.
fn checked(
    state: &ServerState,
    name: &str,
    argument: Value,
    code: &'static str,
) -> AgentResult<Value> {
    let answer = call(state, name, argument)?;
    let variables_failed = answer
        .get("variablesError")
        .is_some_and(|error| !error.is_null());
    if answer.get("ok").and_then(Value::as_bool) != Some(true) || variables_failed {
        return Err(AgentError::new(code, refusal_text(&answer)));
    }
    Ok(answer)
}

/// The answer of a command that must succeed for an edit or a check: `ok` true.
///
/// # Errors
///
/// `VALIDATION_FAILED` with the command's own message when it refuses.
fn command(state: &ServerState, name: &str, argument: Value) -> AgentResult<Value> {
    let answer = call(state, name, argument)?;
    if answer.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(AgentError::validation(refusal_text(&answer)));
    }
    Ok(answer)
}

/// The sentence of a refused answer: its variables error, its error, or its messages.
fn refusal_text(answer: &Value) -> String {
    ["variablesError", "error", "errors"]
        .iter()
        .filter_map(|key| answer.get(*key))
        .find(|value| !value.is_null())
        .map_or_else(
            || "Native HMI service unavailable".to_owned(),
            |value| {
                value
                    .as_str()
                    .map_or_else(|| value.to_string(), str::to_owned)
            },
        )
}

/// A text field of a JSON answer or model, or the empty text when it is absent.
#[must_use]
pub(super) fn text_of<'value>(value: &'value Value, key: &str) -> &'value str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

/// The unsaved HMI page texts, by project-relative path: what the HMI commands read in place of the
/// disk (`open` in `queries.ts`). Only dirty editor buffers under `hmi/` are passed.
#[must_use]
pub(super) fn unsaved_pages(open: &BTreeMap<String, OpenDocument>) -> Value {
    Value::Object(
        open.iter()
            .filter(|(file, document)| {
                document.dirty && file.starts_with("hmi/") && file.ends_with(".toml")
            })
            .map(|(file, document)| (file.clone(), Value::String(document.text.clone())))
            .collect(),
    )
}

/// The program's bindings from its compiled program (`trust-lsp.hmiBindings`). The declarations-only mode
/// (`authoring: true`) of the TypeScript engine never compiles, so a broken program would pass; spec 43 §3
/// and §10.1 require the compiled program, so a failed compilation is `COMPILATION_FAILED`.
///
/// # Errors
///
/// `COMPILATION_FAILED` when the program does not compile.
pub(super) fn bindings(state: &ServerState, root_uri: &str) -> AgentResult<Value> {
    checked(
        state,
        HMI_BINDINGS_COMMAND,
        json!({ "root_uri": root_uri }),
        COMPILATION_FAILED,
    )
}

/// The component catalogue with the project's symbols and formats (`trust-lsp.hmiCatalogue`).
///
/// # Errors
///
/// `VALIDATION_FAILED` when the catalogue cannot be read.
pub(super) fn catalogue(state: &ServerState, root_uri: &str, open: &Value) -> AgentResult<Value> {
    checked(
        state,
        HMI_CATALOGUE_COMMAND,
        json!({ "root_uri": root_uri, "open": open }),
        VALIDATION_FAILED,
    )
}

/// The faceplates of the project and its libraries (`trust-lsp.hmiFaceplates`).
///
/// # Errors
///
/// `VALIDATION_FAILED` when the faceplates cannot be read.
pub(super) fn faceplates(state: &ServerState, root_uri: &str, open: &Value) -> AgentResult<Value> {
    checked(
        state,
        HMI_FACEPLATES_COMMAND,
        json!({ "root_uri": root_uri, "open": open }),
        VALIDATION_FAILED,
    )
}

/// The project's texts and languages (`trust-lsp.hmiTexts`).
///
/// # Errors
///
/// `VALIDATION_FAILED` when the texts cannot be read.
pub(super) fn texts(state: &ServerState, root_uri: &str, open: &Value) -> AgentResult<Value> {
    checked(
        state,
        HMI_TEXTS_COMMAND,
        json!({ "root_uri": root_uri, "open": open }),
        VALIDATION_FAILED,
    )
}

/// The model of one HMI file, read by its kind (`trust-lsp.hmiFileParse`).
///
/// # Errors
///
/// `VALIDATION_FAILED` with the file's own errors when it does not load.
pub(super) fn parse_model(
    state: &ServerState,
    kind: &str,
    text: &str,
    file: &str,
) -> AgentResult<Value> {
    let answer = command(
        state,
        HMI_FILE_PARSE_COMMAND,
        json!({ "kind": kind, "text": text, "file": file }),
    )?;
    answer
        .get("model")
        .cloned()
        .ok_or_else(|| AgentError::validation(format!("{file} has no model.")))
}

/// A page read into the runtime's page model.
///
/// # Errors
///
/// `VALIDATION_FAILED` when the page does not load.
pub(super) fn page(state: &ServerState, text: &str, file: &str) -> AgentResult<HmiPage> {
    let model = parse_model(state, "page", text, file)?;
    serde_json::from_value(model)
        .map_err(|error| AgentError::validation(format!("{file}: {error}")))
}

/// A page model written as canonical TOML (`trust-lsp.hmiFileFormat`), read back by the writer.
///
/// # Errors
///
/// `VALIDATION_FAILED` when the page cannot be written as a valid file.
pub(super) fn format_page(state: &ServerState, page: &HmiPage) -> AgentResult<String> {
    let model = serde_json::to_value(page)
        .map_err(|error| AgentError::validation(format!("the page cannot be encoded: {error}")))?;
    let answer = command(
        state,
        HMI_FILE_FORMAT_COMMAND,
        json!({ "kind": "page", "model": model }),
    )?;
    answer
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| AgentError::validation("writer returned no document."))
}

/// The binding and program problems of one page (`trust-lsp.hmiFileCheck` with `authoring`).
///
/// # Errors
///
/// `VALIDATION_FAILED` when the check cannot run.
pub(super) fn file_problems(
    state: &ServerState,
    root_uri: &str,
    file: &str,
    text: &str,
    remote: &Value,
) -> AgentResult<Vec<Value>> {
    let answer = command(
        state,
        HMI_FILE_CHECK_COMMAND,
        json!({
            "authoring": true,
            "root_uri": root_uri,
            "file": file,
            "text": text,
            "remote": remote,
        }),
    )?;
    Ok(answer
        .get("problems")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// The faceplate instances a page places, by object (`trust-lsp.hmiPlacedFaceplates`); `{}` when none.
///
/// # Errors
///
/// `VALIDATION_FAILED` when the faceplates cannot be resolved.
pub(super) fn placed(
    state: &ServerState,
    root_uri: &str,
    file: &str,
    text: &str,
    open: &Value,
) -> AgentResult<Value> {
    let answer = command(
        state,
        HMI_PLACED_FACEPLATES_COMMAND,
        json!({ "root_uri": root_uri, "file": file, "text": text, "open": open }),
    )?;
    Ok(answer.get("placed").cloned().unwrap_or_else(|| json!({})))
}

/// The PLC variables a page needs that the program does not declare, as a separate proposal
/// (`trust-lsp.hmiAddMissingVariables`). It never applies the proposal.
///
/// # Errors
///
/// `VALIDATION_FAILED` when the proposal cannot be built.
pub(super) fn missing_variables(
    state: &ServerState,
    root_uri: &str,
    file: &str,
    text: &str,
) -> AgentResult<Value> {
    command(
        state,
        HMI_ADD_MISSING_VARIABLES_COMMAND,
        json!({ "root_uri": root_uri, "file": file, "text": text }),
    )
}
