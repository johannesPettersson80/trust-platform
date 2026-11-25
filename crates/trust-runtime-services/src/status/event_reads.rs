//! The event reads `events.tail`, `events` and `faults` (specification 11, §6.8.1), answered
//! whole, as trust-runtime answers them, or written event by event into a reply written in parts
//! (specification 34, PORT-05), so that a chip never builds a reply longer than its session holds
//! and never takes a block of its heap longer than a part.

use alloc::vec::Vec;

use trust_runtime_core::fault::FaultSite;

use super::params::{invalid_params, parse_bounded_limit, unavailable};
use crate::events::{
    newest_selected, runtime_event_to_json_placed, EventSelection, NamedPlace, RuntimeEvent,
};
use crate::ports::RecordsPort;
use crate::protocol::{ControlResponse, ReplyWriter};

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
    let place = |site| host.fault_place(site);
    let events = events
        .into_iter()
        .map(|event| runtime_event_to_json_placed(event, &place))
        .collect::<Vec<_>>();
    let mut result = serde_json::Map::new();
    result.insert(read.member().into(), serde_json::Value::Array(events));
    ControlResponse::ok(id, serde_json::Value::Object(result))
}

/// The newest fault event, as `faults` lists it, or none when the store holds no fault or cannot
/// be read: what `simulator.advance` compares before and after its cycles and adds to its answer
/// (specification 11 §6.8.1, "Simulator for machine tests").
pub(crate) fn newest_fault<H: RecordsPort>(host: &H) -> Option<serde_json::Value> {
    host.read_events(|store| {
        newest_selected(store, EventRead::Faults.selection(), 1)
            .next()
            .cloned()
    })
    .flatten()
    .map(|event| runtime_event_to_json_placed(event, &|site| host.fault_place(site)))
}

/// The read written into `reply` as the result of its successful response, event by event: the
/// text that [`answer_event_read`]'s result serializes to, as far as the reply's bound and the heap
/// reach. Writing stops at the first event that is refused, so no reply longer than the bound is
/// built and no part is taken for an event that cannot be written.
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
    reply: &mut ReplyWriter,
) -> Result<(), ControlResponse> {
    let limit = parse_limit(params).map_err(|error| invalid_params(id, error))?;
    let place = |site| host.fault_place(site);
    host.read_events(|store| {
        write_events(
            read.member(),
            newest_selected(store, read.selection(), limit),
            &place,
            reply,
        );
    })
    .ok_or_else(|| unavailable(id, "event store"))
}

/// Writes `{"<member>":[<event>,...]}`, one event at a time, until an event is refused.
fn write_events<'a>(
    member: &str,
    events: impl Iterator<Item = &'a RuntimeEvent>,
    place: &dyn Fn(FaultSite) -> Option<NamedPlace>,
    reply: &mut ReplyWriter,
) {
    reply.push("{\"");
    reply.push(member);
    reply.push("\":[");
    for (index, event) in events.enumerate() {
        if reply.refused() {
            return;
        }
        if index > 0 {
            reply.push(",");
        }
        reply.push_json(&runtime_event_to_json_placed(event.clone(), place));
    }
    reply.push("]}");
}
