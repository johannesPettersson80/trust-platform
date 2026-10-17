//! `comm.search.start`, `comm.search.status` and `comm.search.cancel`: the one search inside this
//! runtime (specification 11, "One search, device identity, and presence"). The search itself is
//! `crate::search`; this module reads the parameters, lends the search what the runtime knows,
//! and answers.

use std::sync::PoisonError;

use serde_json::{json, Value};

use crate::search::{
    CancelRequest, RuntimeFacts, SearchError, SearchHost, SearchRequest, StatusRequest,
};

use super::super::{ControlResponse, ControlState};

pub(super) fn handle_comm_search_start(
    id: u64,
    params: Option<Value>,
    state: &ControlState,
) -> ControlResponse {
    let started = SearchRequest::from_params(params).and_then(|request| {
        state
            .search
            .start(request, SearchHost::Runtime(runtime_facts(state)))
    });
    match started {
        Ok(search_id) => ControlResponse::ok(id, json!({ "search_id": search_id })),
        Err(error) => refused(id, "comm.search.start", &error),
    }
}

pub(super) fn handle_comm_search_status(
    id: u64,
    params: Option<Value>,
    state: &ControlState,
) -> ControlResponse {
    let status = StatusRequest::from_params(params)
        .and_then(|request| state.search.status(&request.search_id, request.since));
    match status {
        Ok(status) => match serde_json::to_value(status) {
            Ok(value) => ControlResponse::ok(id, value),
            Err(error) => ControlResponse::error(
                id,
                format!("comm.search.status serialization failed: {error}"),
            ),
        },
        Err(error) => refused(id, "comm.search.status", &error),
    }
}

pub(super) fn handle_comm_search_cancel(
    id: u64,
    params: Option<Value>,
    state: &ControlState,
) -> ControlResponse {
    let cancelled = CancelRequest::from_params(params).and_then(|request| {
        let search_state = state.search.cancel(&request.search_id)?;
        Ok(json!({ "search_id": request.search_id, "state": search_state }))
    });
    match cancelled {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => refused(id, "comm.search.cancel", &error),
    }
}

/// What this runtime gives a search: its project folder and its own mDNS discovery.
fn runtime_facts(state: &ControlState) -> RuntimeFacts {
    let settings = state
        .settings
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    RuntimeFacts {
        project_root: state.project_root.clone(),
        discovery: state.discovery.clone(),
        discovery_enabled: settings.discovery.enabled,
        discovery_interfaces: settings.discovery.interfaces.clone(),
    }
}

fn refused(id: u64, kind: &str, error: &SearchError) -> ControlResponse {
    match error.code() {
        Some(code) => ControlResponse::error_with_code(id, error.to_string(), code),
        None => ControlResponse::error(id, format!("invalid {kind} payload: {error}")),
    }
}
