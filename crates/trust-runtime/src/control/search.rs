//! Bounded actual-ST search in disposable simulation processes.
#![allow(missing_docs)]
mod contracts;
mod execution_plan;
mod execution_settings;
mod feedback_relay;
mod limits;
mod process_inputs;
mod program;
mod record_period;
mod run_contracts;
mod run_handlers;
mod service;
mod snapshot;
mod worker;
mod worker_checkpoint;
mod workload;
use super::{ControlResponse, ControlState};
pub(crate) use record_period::bounded_mean;
use serde_json::{json, Value};
pub use service::SearchService;
pub fn worker_stdio() -> Result<(), String> {
    worker::stdio()
}
pub(super) fn dispatch(
    id: u64,
    operation: &str,
    params: Option<Value>,
    state: &ControlState,
) -> Option<ControlResponse> {
    if let Some(response) = run_handlers::dispatch(id, operation, params.clone(), state) {
        return Some(response);
    }
    if !matches!(
        operation,
        "control.search.status" | "control.search.start" | "control.search.cancel"
    ) {
        return None;
    }
    let result = (|| -> Result<Value, String> {
        let params = params.unwrap_or_else(|| json!({}));
        if operation == "control.search.status" && params.as_object().is_some_and(|v| v.is_empty())
        {
            return Ok(match snapshot::load(state) {
                Ok(snapshot) => {
                    json!({"ready":true,"project_revision":snapshot.project_revision,"source_basis":"loaded-runtime-sources","execution":"isolated-simulation","limitations":["reload edited ST before search","REAL/LREAL symbolic gains only","hard bounds and integrated squared error"]})
                }
                Err(reason) => json!({"ready":false,"reason":reason}),
            });
        }
        if operation == "control.search.start" {
            let request: contracts::Request =
                serde_json::from_value(params).map_err(|e| e.to_string())?;
            request.validate()?;
            return state.searches.start(snapshot::load(state)?, request);
        }
        let identity: contracts::Identity =
            serde_json::from_value(params).map_err(|e| e.to_string())?;
        let project = state
            .project_root
            .as_ref()
            .ok_or("active project unavailable")?
            .canonicalize()
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into_owned();
        state.searches.inspect(
            &identity.job_id,
            &identity.expected_project_revision,
            &project,
            operation == "control.search.cancel",
            identity.include_results,
        )
    })();
    Some(match result {
        Ok(value) => ControlResponse::ok(id, value),
        Err(reason) => ControlResponse::error_with_code(id, reason, "invalid_search_request"),
    })
}
#[cfg(test)]
mod limit_contract_tests;

#[cfg(test)]
mod tests {
    /// Protocol annex E8.1: off Linux only the program's allocator limits a worker, and this test
    /// program's allocator is the system's, so it runs no worker. Catches a worker that claims a
    /// memory limit nothing enforces.
    #[cfg(not(target_os = "linux"))]
    #[test]
    fn a_program_without_a_worker_memory_limit_runs_no_worker() {
        assert!(!super::limits::available());
        assert_eq!(
            super::limits::apply(),
            Err(String::from(crate::memory_budget::NO_LIMIT))
        );
    }
    #[test]
    fn finite_search_worker_entry() {
        if std::env::var_os("TRUST_SEARCH_TEST_WORKER").is_some() {
            super::worker_stdio().unwrap();
        }
    }
}
