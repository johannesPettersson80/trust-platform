//! The edits the Statechart editor stores (specification 25 §11.4): a new state.
//!
//! The file names each state by its name (`states: { <name>: … }`) and starts in one of them
//! (`initial`), so two states never share a name and only one is initial. A new state gets the
//! smallest numbered name `State<n>` that no state has, and the id the editor gives a state of that
//! name (`state_<name>`, as [`super::graph`] reads a file).

use crate::Json;

/// The kinds a state has in the editor graph.
const STATE_KINDS: [&str; 4] = ["normal", "initial", "final", "compound"];

/// A new state of the kind `kind` for the editor graph `graph`: `{ id, label, type, entry, exit }`,
/// named `State<n>` with n the smallest number from 1 that no state's name or id uses, without
/// entry or exit actions. Where it is drawn is the editor's.
///
/// # Errors
/// `kind` is not a state kind, or it is `initial` while a state of `graph` is initial already.
pub fn add_state(graph: &Json, kind: &str) -> Result<Json, String> {
    if !STATE_KINDS.contains(&kind) {
        return Err(format!(
            "A state is normal, initial, final or compound, not \"{kind}\"."
        ));
    }
    let states = graph
        .get("states")
        .and_then(Json::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if kind == "initial"
        && let Some(initial) = states
            .iter()
            .find(|state| state.str_at("type") == Some("initial"))
    {
        return Err(format!(
            "The statechart starts in \"{}\" already; it starts in one state.",
            initial.str_at("label").unwrap_or_default()
        ));
    }
    let taken = |name: &str| {
        let id = format!("state_{name}");
        states.iter().any(|state| {
            state.str_at("label") == Some(name) || state.str_at("id") == Some(id.as_str())
        })
    };
    // Each state takes at most two names (its own and the one its id names), so one of the
    // first 2 × states + 1 is free.
    let label = (1..=2 * states.len() + 1)
        .map(|n| format!("State{n}"))
        .find(|name| !taken(name))
        .unwrap_or_default();
    Ok(Json::Object(vec![
        ("id".to_owned(), Json::str(format!("state_{label}"))),
        ("label".to_owned(), Json::str(label)),
        ("type".to_owned(), Json::str(kind)),
        ("entry".to_owned(), Json::Array(Vec::new())),
        ("exit".to_owned(), Json::Array(Vec::new())),
    ]))
}
