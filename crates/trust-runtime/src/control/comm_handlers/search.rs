//! `comm.search.start`, `comm.search.status` and `comm.search.cancel`: the one search inside this
//! runtime (specification 11, "One search, device identity, and presence"), and `comm.compare`,
//! which compares one of its searches with the devices of the runtime's own project (specification
//! 11, "Compare with the project"). The search itself is `crate::search`; this module reads the
//! parameters, lends the search what the runtime knows, and answers.

use std::sync::PoisonError;

use serde_json::{json, Value};
use smol_str::SmolStr;

use crate::search::compare::{compare, searched};
use crate::search::project_choices;
use crate::search::project_devices;
use crate::search::sources::ethercat::{
    is_ethercat_driver, MOCK_ETHERCAT_ADAPTER, UNKNOWN_ADAPTER_OWNER,
};
use crate::search::{
    CancelRequest, CompareRequest, RuntimeFacts, SearchError, SearchHost, SearchRequest,
    StatusRequest,
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

/// `comm.compare`: the devices of one search against the devices of this runtime's project
/// (specification 11, "Compare with the project"). The search is the one named, or the most recent one
/// the runtime keeps, and it must have ended on its own.
pub(super) fn handle_comm_compare(
    id: u64,
    params: Option<Value>,
    state: &ControlState,
) -> ControlResponse {
    let request = match CompareRequest::from_params(params) {
        Ok(request) => request,
        Err(error) => return refused(id, "comm.compare", &error),
    };
    let status = match state.search.finished_status(request.search_id.as_deref()) {
        Ok(status) => status,
        Err(error) => return refused(id, "comm.compare", &error),
    };
    let Some(root) = state.project_root.as_deref() else {
        return ControlResponse::error_with_code(
            id,
            "this runtime has no project folder to compare with".to_string(),
            "compare_no_project",
        );
    };
    let unreadable = |reason: String| {
        ControlResponse::error_with_code(
            id,
            format!("the project cannot be compared with: {reason}"),
            "compare_project_unreadable",
        )
    };
    let configured = match project_devices::load(root) {
        Ok(configured) => configured,
        Err(reason) => return unreadable(reason),
    };
    let memory = match project_choices::load(root) {
        Ok(memory) => memory,
        Err(reason) => return unreadable(reason),
    };
    let comparison = compare(
        &searched(&status),
        &status.devices,
        &configured,
        &memory.ignored,
    );
    ControlResponse::ok(id, comparison.report(&status.search_id))
}

/// What this runtime gives a search: its project folder, its own mDNS discovery, and the adapters
/// of its running EtherCAT drivers.
fn runtime_facts(state: &ControlState) -> RuntimeFacts {
    let (discovery_enabled, discovery_interfaces) = {
        let settings = state
            .settings
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        (
            settings.discovery.enabled,
            settings.discovery.interfaces.clone(),
        )
    };
    RuntimeFacts {
        project_root: state.project_root.clone(),
        discovery: state.discovery.clone(),
        discovery_enabled,
        discovery_interfaces,
        runtime_id: state.runtime_id.clone(),
        owned_adapters: owned_ethercat_adapters(state),
        mesh: state
            .mesh_topology
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone(),
    }
}

/// The adapters of this runtime's running EtherCAT drivers (specification 11, "Safety"): the
/// adapter of each enabled EtherCAT driver of the project's `io.toml` while that driver runs. A
/// driver without an adapter, or with the simulated bus, owns no hardware adapter.
///
/// The owner must be complete, because a search never disturbs a bus that a running driver owns.
/// When no adapter can be attached to a running EtherCAT driver — there is no project, `io.toml`
/// cannot be read, or it names no enabled EtherCAT driver — the result marks the owners as unknown,
/// and a search then scans no EtherCAT adapter.
fn owned_ethercat_adapters(state: &ControlState) -> Vec<SmolStr> {
    let running_ethercat = state
        .io_health
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .any(|driver| is_ethercat_driver(driver.name.as_str()));
    if !running_ethercat {
        return Vec::new();
    }
    let Some(root) = state.project_root.as_deref() else {
        tracing::warn!("search cannot place a running EtherCAT driver: the runtime has no project");
        return unknown_owned_adapters();
    };
    let path = root.join("io.toml");
    let config = match crate::config::IoConfig::load(path) {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(
                "search could not read io.toml to find owned EtherCAT adapters: {error}"
            );
            return unknown_owned_adapters();
        }
    };
    let mut enabled = 0usize;
    let mut owned = Vec::new();
    for driver in &config.drivers {
        if !driver.enabled || !is_ethercat_driver(driver.name.as_str()) {
            continue;
        }
        enabled += 1;
        if let Some(adapter) = driver_adapter(&driver.params) {
            owned.push(adapter);
        }
    }
    if enabled == 0 {
        tracing::warn!(
            "search could not match a running EtherCAT driver to an enabled io.toml entry"
        );
        return unknown_owned_adapters();
    }
    owned
}

/// The marker that a running EtherCAT driver owns an adapter that could not be determined: a search
/// then scans no EtherCAT adapter (specification 11, "Safety").
fn unknown_owned_adapters() -> Vec<SmolStr> {
    vec![SmolStr::new(UNKNOWN_ADAPTER_OWNER)]
}

/// The hardware adapter that an EtherCAT driver owns: its `adapter` parameter. The simulated bus
/// and an absent adapter own nothing.
fn driver_adapter(params: &toml::Value) -> Option<SmolStr> {
    let adapter = params
        .get("adapter")
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|adapter| !adapter.is_empty())?;
    if adapter.eq_ignore_ascii_case(MOCK_ETHERCAT_ADAPTER) {
        return None;
    }
    Some(SmolStr::new(adapter))
}

fn refused(id: u64, kind: &str, error: &SearchError) -> ControlResponse {
    match error.code() {
        Some(code) => ControlResponse::error_with_code(id, error.to_string(), code),
        None => ControlResponse::error(id, format!("invalid {kind} payload: {error}")),
    }
}
