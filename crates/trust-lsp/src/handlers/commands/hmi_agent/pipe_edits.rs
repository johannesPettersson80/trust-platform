//! Pipe edits of agent drafts (specification 43, AT-05 and the 7 October repair "One pipe rule").
//!
//! Port of `agent/pipeEdits.ts`. A port counts only when the canvas offers it: a catalogue port of
//! an object on the page that the shared drawing places. A new pipe joins two different ports that
//! have no pipe yet; a branch goes through a junction. A refused connection fails with the
//! builder's own reason.

use serde_json::Value;
use trust_runtime::hmi::catalogue::Catalogue;
use trust_runtime::hmi::page_files::{HmiPage, HmiPipe};

use super::edits::js_text;
use super::error::{AgentError, AgentResult};
use super::pipe_rules::{add_pipe, delete_pipes};
use super::ports::port_point;

/// The `object.port` text of a port, when the canvas would offer it on this page.
///
/// # Errors
///
/// Returns `INVALID_ARGUMENT` when the value is not text or names a port the builder does not offer.
pub(super) fn offered_port(
    page: &HmiPage,
    value: Option<&Value>,
    catalogue: &Catalogue,
) -> AgentResult<String> {
    let text = value
        .and_then(Value::as_str)
        .ok_or_else(|| AgentError::invalid("a port is written object.port."))?;
    let (object_id, port) = match text.rfind('.') {
        Some(dot) if dot > 0 => (&text[..dot], &text[dot + 1..]),
        _ => ("", text),
    };
    let offered = page
        .objects
        .iter()
        .find(|object| object.id == object_id)
        .is_some_and(|object| {
            catalogue_ports(catalogue, &object.kind)
                .iter()
                .any(|name| name == port)
                && port_point(object, port).is_some()
        });
    if !offered {
        return Err(AgentError::invalid(format!(
            "unknown port {text}; the builder offers only the ports its catalogue names for an object on this page."
        )));
    }
    Ok(text.to_owned())
}

/// The ports a component type's catalogue entry names; none for an unknown type.
fn catalogue_ports<'a>(catalogue: &'a Catalogue, kind: &str) -> &'a [String] {
    match catalogue.component(kind) {
        Some(component) => component.ports.as_slice(),
        None => &[],
    }
}

/// Why the builder's addPipe refused a pipe between two ports (`pipeRefusal`). The builder joins
/// a pipe between two different ports, and only ports that have no pipe yet.
#[must_use]
pub(super) fn pipe_refusal(page: &HmiPage, from: &str, to: &str) -> AgentError {
    if from == to {
        return AgentError::invalid(format!(
            "the builder joins two different ports; {from} cannot connect to itself."
        ));
    }
    let taken = [from, to].into_iter().find(|port| {
        page.pipes
            .iter()
            .any(|pipe| pipe.from == *port || pipe.to == *port)
    });
    match taken {
        Some(port) => AgentError::conflict(format!(
            "the builder joins only free ports, and {port} already has a pipe; disconnect or reconnect that pipe, or branch through a junction."
        )),
        None => AgentError::conflict(format!(
            "the builder refuses a pipe from {from} to {to}."
        )),
    }
}

/// A new pipe, joined as the canvas joins one, or refused with the builder's reason
/// (`connectPorts`).
///
/// # Errors
///
/// Returns the refusal from `pipe_refusal` and `INVALID_ARGUMENT` for a port the builder does not
/// offer.
pub(super) fn connect_ports(
    page: &HmiPage,
    from: Option<&Value>,
    to: Option<&Value>,
    catalogue: &Catalogue,
) -> AgentResult<HmiPage> {
    let start = offered_port(page, from, catalogue)?;
    let end = offered_port(page, to, catalogue)?;
    add_pipe(page, &start, &end).ok_or_else(|| pipe_refusal(page, &start, &end))
}

/// The place of the connection the agent names by its two ends (`connectionIndex`).
///
/// # Errors
///
/// Returns `INVALID_ARGUMENT` when no pipe joins those two ends.
pub(super) fn connection_index(
    page: &HmiPage,
    from: Option<&Value>,
    to: Option<&Value>,
) -> AgentResult<usize> {
    let from_text = from.and_then(Value::as_str);
    let to_text = to.and_then(Value::as_str);
    page.pipes
        .iter()
        .position(|pipe| from_text == Some(pipe.from.as_str()) && to_text == Some(pipe.to.as_str()))
        .ok_or_else(|| {
            AgentError::invalid(format!(
                "unknown connection {} -> {}.",
                js_text(from),
                js_text(to)
            ))
        })
}

/// The pipe at `index` moved onto new ports (`reconnectPipe`). It keeps its place, binding,
/// style, bends and the instruments attached to it. The new ports must be ones the builder joins
/// once the moved pipe is set aside.
///
/// # Errors
///
/// Returns `INVALID_ARGUMENT` or `CONFLICT` with the builder's reason, or the port refusal of
/// `offered_port`.
pub(super) fn reconnect_pipe(
    page: &HmiPage,
    index: usize,
    new_from: Option<&Value>,
    new_to: Option<&Value>,
    catalogue: &Catalogue,
) -> AgentResult<HmiPage> {
    let start = offered_port(page, new_from, catalogue)?;
    let end = offered_port(page, new_to, catalogue)?;
    let mut others = page.clone();
    others.pipes = page
        .pipes
        .iter()
        .enumerate()
        .filter(|(at, _)| *at != index)
        .map(|(_, pipe)| pipe.clone())
        .collect();
    if add_pipe(&others, &start, &end).is_none() {
        return Err(pipe_refusal(&others, &start, &end));
    }
    let mut result = page.clone();
    result.pipes = page
        .pipes
        .iter()
        .enumerate()
        .map(|(at, pipe)| {
            if at == index {
                HmiPipe {
                    from: start.clone(),
                    to: end.clone(),
                    ..pipe.clone()
                }
            } else {
                pipe.clone()
            }
        })
        .collect();
    Ok(result)
}

/// Removes the named connection, every pipe that joins those two ends (`disconnectPorts`). The
/// attachments of the pipes after it stay on their own pipes.
///
/// # Errors
///
/// Returns `INVALID_ARGUMENT` when no pipe joins those two ends.
pub(super) fn disconnect_ports(
    page: &HmiPage,
    from: Option<&Value>,
    to: Option<&Value>,
) -> AgentResult<HmiPage> {
    connection_index(page, from, to)?;
    let from_text = from.and_then(Value::as_str);
    let to_text = to.and_then(Value::as_str);
    let ids: Vec<String> = page
        .pipes
        .iter()
        .enumerate()
        .filter(|(_, pipe)| {
            from_text == Some(pipe.from.as_str()) && to_text == Some(pipe.to.as_str())
        })
        .map(|(index, _)| format!("pipe:{index}"))
        .collect();
    Ok(delete_pipes(page, &ids))
}

#[cfg(test)]
#[path = "pipe_edits_tests.rs"]
mod tests;
