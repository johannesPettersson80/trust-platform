//! The event reads `events.tail`, `events` and `faults` (specification 11, §6.8.1), answered
//! whole, as trust-runtime answers them, or written event by event within a link's frame
//! (specification 34, PORT-05), so that a chip never builds a reply longer than its frame.

use alloc::vec::Vec;

use super::params::{invalid_params, parse_bounded_limit, unavailable};
use crate::events::{newest_selected, runtime_event_to_json, EventSelection, RuntimeEvent};
use crate::ports::RecordsPort;
use crate::protocol::{BoundedReply, ControlResponse};

/// A read of the event store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EventRead {
    /// `events.tail` and its alias `events`: every event.
    Events,
    /// `faults`: only fault events.
    Faults,
}

impl EventRead {
    /// The read that operation `kind` names, if it names one.
    pub(crate) fn of(kind: &str) -> Option<Self> {
        match kind {
            "events.tail" | "events" => Some(Self::Events),
            "faults" => Some(Self::Faults),
            _ => None,
        }
    }

    /// The member of the result that holds the events.
    const fn member(self) -> &'static str {
        match self {
            Self::Events => "events",
            Self::Faults => "faults",
        }
    }

    const fn selection(self) -> EventSelection {
        match self {
            Self::Events => EventSelection::All,
            Self::Faults => EventSelection::Faults,
        }
    }
}

/// The `limit` parameter of an event read: 50 when absent, otherwise 1 to 1,000.
fn parse_limit(params: Option<&serde_json::Value>) -> Result<usize, &'static str> {
    parse_bounded_limit(params, 50, 1_000, &["limit"])
}

/// The read answered whole: the newest selected events, newest first, at most `limit` of them.
pub(super) fn answer_event_read<H: RecordsPort>(
    read: EventRead,
    id: u64,
    params: Option<&serde_json::Value>,
    host: &H,
) -> ControlResponse {
    let limit = match parse_limit(params) {
        Ok(limit) => limit,
        Err(error) => return invalid_params(id, error),
    };
    let Some(events) = host.read_events(|store| {
        newest_selected(store, read.selection(), limit)
            .cloned()
            .collect::<Vec<_>>()
    }) else {
        return unavailable(id, "event store");
    };
    let events = events
        .into_iter()
        .map(runtime_event_to_json)
        .collect::<Vec<_>>();
    let mut result = serde_json::Map::new();
    result.insert(read.member().into(), serde_json::Value::Array(events));
    ControlResponse::ok(id, serde_json::Value::Object(result))
}

/// The read written into `reply` as the result of its successful response, event by event: the
/// text that [`answer_event_read`]'s result serializes to, as far as the reply's bound reaches.
/// Writing stops at the first event that does not fit, so no reply longer than the bound is
/// built.
///
/// # Errors
///
/// The response to a read that is refused, as [`answer_event_read`] gives it: parameters that are
/// not valid, or an event store that cannot be read.
pub(crate) fn write_event_read<H: RecordsPort>(
    read: EventRead,
    id: u64,
    params: Option<&serde_json::Value>,
    host: &H,
    reply: &mut BoundedReply,
) -> Result<(), ControlResponse> {
    let limit = parse_limit(params).map_err(|error| invalid_params(id, error))?;
    host.read_events(|store| {
        write_events(
            read.member(),
            newest_selected(store, read.selection(), limit),
            reply,
        );
    })
    .ok_or_else(|| unavailable(id, "event store"))
}

/// Writes `{"<member>":[<event>,...]}`, one event at a time, until an event does not fit.
fn write_events<'a>(
    member: &str,
    events: impl Iterator<Item = &'a RuntimeEvent>,
    reply: &mut BoundedReply,
) {
    reply.push("{\"");
    reply.push(member);
    reply.push("\":[");
    for (index, event) in events.enumerate() {
        if index > 0 {
            reply.push(",");
        }
        reply.push_json(&runtime_event_to_json(event.clone()));
        if reply.overflowed() {
            return;
        }
    }
    reply.push("]}");
}
