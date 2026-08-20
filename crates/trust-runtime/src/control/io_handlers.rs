use crate::io::{validate_process_image_address, IoAddress};
use serde_json::json;

use super::types::{IoAddressParams, IoSnapshotJson, IoWriteParams};
use super::{parse_value, ControlResponse, ControlState};

#[cfg(test)]
#[path = "io_handlers/contract_tests.rs"]
mod contract_tests;

pub(super) fn handle_io_list(id: u64, state: &ControlState) -> ControlResponse {
    let snapshot = match state.io_snapshot.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => return ControlResponse::error(id, "I/O snapshot unavailable".into()),
    };
    match snapshot {
        Some(snapshot) => ControlResponse::ok(id, snapshot.into_json()),
        None => ControlResponse::error(id, "no snapshot available".into()),
    }
}

pub(super) fn handle_io_read(id: u64, state: &ControlState) -> ControlResponse {
    let snapshot = match state.io_snapshot.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => return ControlResponse::error(id, "I/O snapshot unavailable".into()),
    };
    ControlResponse::ok(
        id,
        json!({
            "snapshot": snapshot.map(|snap| snap.into_json())
        }),
    )
}

pub(super) fn handle_io_write(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: IoWriteParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let address = match IoAddress::parse(&params.address) {
        Ok(addr) => addr,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    if let Err(err) = validate_process_image_address(&address) {
        return ControlResponse::error(id, err.to_string());
    }
    let value = match parse_value(&params.value) {
        Ok(value) => value,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    state.debug.enqueue_io_write(address, value);
    ControlResponse::ok(id, json!({"status": "queued"}))
}

pub(super) fn handle_io_force(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: IoWriteParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let address = match IoAddress::parse(&params.address) {
        Ok(addr) => addr,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    if let Err(err) = validate_process_image_address(&address) {
        return ControlResponse::error(id, err.to_string());
    }
    let value = match parse_value(&params.value) {
        Ok(value) => value,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    state.debug.force_io(address, value);
    ControlResponse::ok(id, json!({"status": "forced"}))
}

pub(super) fn handle_io_unforce(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: IoAddressParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let address = match IoAddress::parse(&params.address) {
        Ok(addr) => addr,
        Err(err) => return ControlResponse::error(id, err.to_string()),
    };
    if let Err(err) = validate_process_image_address(&address) {
        return ControlResponse::error(id, err.to_string());
    }
    state.debug.release_io(&address);
    ControlResponse::ok(id, json!({"status": "released"}))
}
