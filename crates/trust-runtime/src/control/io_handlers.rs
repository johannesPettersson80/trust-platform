//! The host's `io.*` handlers: they read the published snapshot and the Live Values queues and
//! answer through the shared [`io_requests`] operations (specification 34, ARCH-11).
//!
//! The host keeps the lock around the snapshot slot and the queue state; every parameter check,
//! refusal, error text and reply is the shared code's.

use trust_runtime_services::live_values::io_requests::{self, IoQueuePort};
use trust_runtime_services::live_values::io_snapshot::IoSnapshot as PortableIoSnapshot;
use trust_runtime_services::live_values::RequestQueueFull;

use crate::io::IoAddress;
use crate::value::Value;

use super::{ControlResponse, ControlState};

#[cfg(test)]
#[path = "io_handlers/contract_tests.rs"]
mod contract_tests;

pub(super) fn handle_io_list(id: u64, state: &ControlState) -> ControlResponse {
    let snapshot = match state.io_snapshot.lock() {
        Ok(guard) => guard.as_ref().map(PortableIoSnapshot::from),
        Err(_) => return ControlResponse::error(id, "I/O snapshot unavailable".into()),
    };
    io_requests::io_list(id, snapshot.as_ref())
}

pub(super) fn handle_io_read(id: u64, state: &ControlState) -> ControlResponse {
    let snapshot = match state.io_snapshot.lock() {
        Ok(guard) => guard.as_ref().map(PortableIoSnapshot::from),
        Err(_) => return ControlResponse::error(id, "I/O snapshot unavailable".into()),
    };
    io_requests::io_read(id, snapshot.as_ref())
}

pub(super) fn handle_io_write(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    io_requests::io_write(id, params, &state.debug)
}

pub(super) fn handle_io_force(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    io_requests::io_force(id, params, &state.debug)
}

pub(super) fn handle_io_unforce(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    io_requests::io_unforce(id, params, &state.debug)
}

/// The host's Live Values queues, behind the one state lock `DebugControl` holds.
impl IoQueuePort for crate::debug::DebugControl {
    fn enqueue_io_write(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.try_enqueue_io_write(address, value)
    }

    fn force_io(&self, address: IoAddress, value: Value) -> Result<(), RequestQueueFull> {
        self.try_force_io(address, value)
    }

    fn release_io(&self, address: &IoAddress) {
        crate::debug::DebugControl::release_io(self, address);
    }
}
