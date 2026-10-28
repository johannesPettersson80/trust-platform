//! The host's variable handlers: `eval` reads a variable of the published state of the program
//! through its symbol table (specification 44 §13.3). `set`, `var.force`, `var.unforce` and
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
    let Some(program) = state
        .debug
        .snapshot()
        .and_then(|snapshot| snapshot.compiled)
    else {
        return ControlResponse::error(id, "no snapshot available".into());
    };
    let value = program.value(params.expr.trim());
    match value {
        Some(value) => ControlResponse::ok(id, json!({ "value": format!("{value:?}") })),
        None => ControlResponse::error(id, "unknown identifier".into()),
    }
}
