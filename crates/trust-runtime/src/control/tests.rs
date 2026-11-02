//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

include!("tests/helpers.rs");
include!("tests/ads_product_surfaces.rs");
include!("tests/debug_boundary.rs");
include!("tests/audit_durability.rs");
include!("tests/connectors.rs");
include!("tests/goldens.rs");
include!("tests/debug_mutation_lifecycle.rs");
include!("tests/authorization_matrix.rs");
include!("tests/installed_write_debug_set_authorization.rs");
include!("tests/checked_admission_access_policy_tests.rs");
include!("tests/authorization_trace.rs");
include!("tests/core.rs");
include!("tests/control_endpoint_source_contract.rs");
include!("tests/control_request_boundary_contract.rs");
include!("tests/io_write_integer_literals.rs");
include!("tests/variable_write_integer_literals.rs");
include!("tests/live_values_pipeline.rs");
include!("tests/request_queue_bounds.rs");
include!("tests/rejected_force_removal.rs");
include!("tests/production_step_and_debug_hold.rs");
include!("tests/pause_resume_resource_state.rs");
include!("tests/concurrency_requests.rs");
include!("tests/reload_withdrawal.rs");
include!("tests/resource_queue_bounds.rs");
include!("tests/config_set_lock_order.rs");
include!("tests/capabilities.rs");

// The tests of `comm.search.*` keep a module of their own: their `search_request` builds a control
// request, and Control Studio's search tests below name their search parameters so.
mod comm_search {
    use super::*;

    include!("tests/search.rs");
}

include!("tests/identify.rs");
include!("tests/control_studio_experiment.rs");
include!("tests/control_studio_pid_apply.rs");
include!("tests/plant_contract.rs");
include!("tests/plant_save_validation_contract.rs");
include!("tests/plant_remove_contract.rs");
include!("tests/plant_scenario_contract.rs");
include!("tests/plant_metadata_contract.rs");
include!("tests/plant_trace_contract.rs");
include!("tests/plant_authorization_contract.rs");
include!("tests/plant_offline_contract.rs");
include!("tests/plant_full_roundtrip_contract.rs");

include!("tests/control_studio_recording.rs");
include!("tests/control_studio_runtime_facts.rs");

include!("tests/control_studio_parameters.rs");
include!("tests/control_studio_transfer.rs");
include!("tests/control_studio_transfer_formulas.rs");

// The isolated runs below use this test program as their worker. Its allocator is the system's,
// so only Linux limits that worker (protocol annex E8.1); the trust-runtime and trust-debug
// programs run their workers on every platform.
#[cfg(target_os = "linux")]
include!("tests/control_studio_search.rs");

include!("tests/control_studio_symbolic_acquisition.rs");
include!("tests/control_studio_signal_paths.rs");
include!("tests/control_studio_experiment_symbol.rs");

#[cfg(target_os = "linux")]
include!("tests/control_studio_isolated_run.rs");
#[cfg(target_os = "linux")]
include!("tests/control_studio_isolated_live_inputs.rs");
// The delayed integrator runs as an isolated run, with the search fixture and the run's channel readers above.
#[cfg(target_os = "linux")]
include!("tests/control_studio_delayed_integrator.rs");

include!("tests/control_studio_experiment_abort.rs");
include!("tests/control_studio_experiment_boundary.rs");
include!("tests/control_studio_experiment_scan.rs");
