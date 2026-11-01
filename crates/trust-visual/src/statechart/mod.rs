//! Statecharts, `*.statechart.json` (specification 25 §11.4, specification 17 §3.1).
//!
//! A statechart file is an XState-style machine: an `id`, the `initial` state, the `states` (each
//! with `entry` and `exit` action names, a `type`, and `on`: the transitions by event, each with a
//! `target`, an optional `guard`, `actions` and an `after` delay in milliseconds) and the
//! `actionMappings` (what each action name writes or logs).
//!
//! Its companion is one `FUNCTION_BLOCK` that holds the active state as an index (`generate`).
//! The editor draws states and transitions; reading a file into the editor's graph and writing the
//! graph back as the file are `graph` and `file`. The delay and the mapping value typed into the
//! editor's panels are read by [`after_delay`] and [`mapping_value`].

mod generate;
mod graph;
#[cfg(test)]
mod tests;
mod values;

pub use generate::generate;
pub use graph::{action_report, file, graph, keep_identity, ActionReport};
pub use values::{add_mapping, after_delay, mapping_value};

use crate::Json;

/// The message when a file is not a statechart.
pub(crate) const FORMAT_MESSAGE: &str =
    "Invalid statechart format. Expected JSON object with id and states.";

/// Checks that `program` has the shape the companion reads: an object with a text `id` and a
/// `states` object.
///
/// # Errors
/// The program has another shape; the message says which shape is expected.
pub fn check_format(program: &Json) -> Result<(), String> {
    let has_id = matches!(program.get("id"), Some(Json::String(_)));
    let has_states = program.get("states").is_some_and(Json::is_object);
    if has_id && has_states {
        Ok(())
    } else {
        Err(FORMAT_MESSAGE.to_owned())
    }
}

/// The editor's Validate: whether the companion can be generated from `program` (the format, at
/// least one state, and action mappings the companion can read).
///
/// # Errors
/// The companion cannot be generated; the message says why, one problem per line.
pub fn validate(program: &Json) -> Result<(), String> {
    generate(program, "").map(|_| ()).map_err(|problems| {
        problems
            .iter()
            .map(|problem| problem.message.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    })
}
