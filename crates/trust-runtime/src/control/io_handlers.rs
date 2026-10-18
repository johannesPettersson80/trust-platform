//! The host's `io.*` handlers: they answer through the shared [`io_requests`] operations, which
//! read the host's published snapshot and queue into the host's Live Values queues
//! (specification 34, ARCH-11).
//!
//! The host keeps the lock around the snapshot slot and the queue state; every parameter check,
//! refusal, error text and reply is the shared code's.

use trust_runtime_services::live_values::io_requests::{self, IoQueuePort};

use super::{ControlResponse, ControlState};

#[cfg(test)]
#[path = "io_handlers/contract_tests.rs"]
mod contract_tests;

pub(super) fn handle_io_list(id: u64, state: &ControlState) -> ControlResponse {
    io_requests::io_list(id, state.io_snapshot())
}

pub(super) fn handle_io_read(id: u64, state: &ControlState) -> ControlResponse {
    io_requests::io_read(id, state.io_snapshot())
}

pub(super) fn handle_io_write(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    io_requests::io_write(id, params, state)
}

pub(super) fn handle_io_force(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    io_requests::io_force(id, params, state)
}

pub(super) fn handle_io_unforce(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    io_requests::io_unforce(id, params, state)
}
