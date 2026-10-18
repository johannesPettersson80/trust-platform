//! The host's variable handlers: `eval` reads the debugger snapshot and the retained names here,
//! because it needs the HIR expression evaluator. `set`, `var.force`, `var.unforce` and
//! `var.forced` are answered by the shared request pipeline
//! (`trust_runtime_services::live_values::var_requests`); their host wiring is tested by
//! `control/tests/live_values_pipeline.rs` (specification 11, "Live Values"; specification 34
//! §4.3.1).

use serde_json::json;

use super::types::EvalParams;
use super::{ControlResponse, ControlState};

#[cfg(test)]
#[path = "variable_handlers/contract_tests.rs"]
mod contract_tests;

pub(super) fn handle_eval(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: EvalParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let snapshot = match state.debug.snapshot() {
        Some(snapshot) => snapshot,
        None => return ControlResponse::error(id, "no snapshot available".into()),
    };
    let name = params.expr.trim();
    let value = if let Some(value) = snapshot.storage.get_global(name) {
        Some(value.clone())
    } else {
        let metadata = match state.metadata.lock() {
            Ok(metadata) => metadata,
            Err(_) => return ControlResponse::error(id, "metadata unavailable".into()),
        };
        metadata
            .retained_reference_exact(&snapshot.storage, name)
            .and_then(|reference| snapshot.storage.materialize_by_ref_ref(&reference))
            .or_else(|| snapshot.storage.get_retain(name).cloned())
    };
    match value {
        Some(value) => ControlResponse::ok(id, json!({ "value": format!("{value:?}") })),
        None => ControlResponse::error(id, "unknown identifier".into()),
    }
}
