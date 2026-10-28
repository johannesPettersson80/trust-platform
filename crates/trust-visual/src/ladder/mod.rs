//! Ladder programs, `*.ladder.json` (specification 15, specification 16).
//!
//! A Ladder program is the schema-v2 file the Ladder editor draws (specification 16 §2). This
//! module holds its rules: the schema check ([`validate`]), the program as the editor opens it
//! ([`load`]) and as it is saved ([`save_text`]), the edits the editor stores in it ([`edit`]) and
//! the Structured Text companion that executes it ([`generate`], specification 16 §4).

pub mod edit;
mod lower;
mod power;
pub mod references;
mod schema;
#[cfg(test)]
mod tests;

pub use lower::generate;
pub use schema::{validate, ELEMENT_TYPES, VARIABLE_TYPES};

use crate::js;
use crate::Json;

/// The name of an empty program opened from a blank file.
const NEW_PROGRAM_NAME: &str = "New Ladder Program";
/// The name of the empty program shown for a file that cannot be read.
pub const INVALID_PROGRAM_NAME: &str = "Invalid Ladder Program";
/// The description an empty program gets.
const DEFAULT_DESCRIPTION: &str = "Ladder logic program";

/// An empty schema-v2 program called `name`, created and modified at `now` (an ISO 8601 time).
#[must_use]
pub fn empty_program(name: &str, now: &str) -> Json {
    Json::Object(vec![
        ("schemaVersion".to_owned(), Json::Number(2.0)),
        ("networks".to_owned(), Json::Array(Vec::new())),
        ("variables".to_owned(), Json::Array(Vec::new())),
        (
            "metadata".to_owned(),
            Json::Object(vec![
                ("name".to_owned(), Json::str(name)),
                ("description".to_owned(), Json::str(DEFAULT_DESCRIPTION)),
                ("created".to_owned(), Json::str(now)),
                ("modified".to_owned(), Json::str(now)),
            ]),
        ),
    ])
}

/// The program the editor opens from the file text `text` at the time `now`.
///
/// A blank file opens as an empty program. Otherwise the file must pass the schema check; its
/// metadata is reduced to `name`, `description`, `created` and `modified` (the members of
/// specification 16 §2), and the implicit Boolean locals of its contacts and coils are reconciled
/// ([`edit::reconcile_declarations`]), as the editor shows them.
///
/// # Errors
/// The text is not JSON or the program breaks the schema; the editor then shows
/// [`empty_program`] called [`INVALID_PROGRAM_NAME`].
pub fn load(text: &str, now: &str) -> Result<Json, String> {
    if js::trim(text).is_empty() {
        return Ok(empty_program(NEW_PROGRAM_NAME, now));
    }
    let mut program = Json::parse(text)?;
    validate(&program)?;
    if let Some(metadata) = program.get("metadata") {
        let mut reduced = vec![
            (
                "name".to_owned(),
                metadata.get("name").cloned().unwrap_or(Json::Null),
            ),
            (
                "description".to_owned(),
                metadata.get("description").cloned().unwrap_or(Json::Null),
            ),
        ];
        for key in ["created", "modified"] {
            if let Some(value) = metadata.get(key) {
                reduced.push((key.to_owned(), value.clone()));
            }
        }
        program.set("metadata", Json::Object(reduced));
    }
    edit::reconcile_declarations(&mut program);
    Ok(program)
}

/// The file text of the program `program` saved at the time `now`: `schemaVersion` 2, the
/// networks in ascending `order` (equal orders keep their place), `metadata.modified` set to
/// `now`, written with two-space indentation and a final newline.
///
/// # Errors
/// The program breaks the schema; nothing is saved.
pub fn save_text(program: &Json, now: &str) -> Result<String, String> {
    validate(program)?;
    let mut saved = program.clone();
    saved.set("schemaVersion", Json::Number(2.0));
    if let Some(networks) = saved.get_mut("networks").and_then(Json::as_array_mut) {
        networks.sort_by(|left, right| {
            let order = |network: &Json| network.get("order").and_then(Json::as_f64).unwrap_or(0.0);
            order(left)
                .partial_cmp(&order(right))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    if let Some(metadata) = saved.get_mut("metadata") {
        metadata.set("modified", Json::str(now));
    }
    Ok(format!("{}\n", saved.to_pretty()))
}
