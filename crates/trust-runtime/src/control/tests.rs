include!("tests/helpers.rs");
include!("tests/ads_product_surfaces.rs");
include!("tests/debug_boundary.rs");
include!("tests/audit_durability.rs");
include!("tests/connectors.rs");
include!("tests/goldens.rs");
include!("tests/hmi.rs");
include!("tests/debug_mutation_lifecycle.rs");
include!("tests/authorization_matrix.rs");
include!("tests/authorization_trace.rs");
include!("tests/core.rs");
include!("tests/control_endpoint_source_contract.rs");
include!("tests/control_request_boundary_contract.rs");
include!("tests/control_studio_experiment.rs");
include!("tests/control_studio_pid_apply.rs");
include!("tests/plant_contract.rs");
include!("tests/plant_remove_contract.rs");
include!("tests/plant_scenario_contract.rs");
include!("tests/plant_metadata_contract.rs");
include!("tests/plant_trace_contract.rs");
include!("tests/plant_authorization_contract.rs");
include!("tests/plant_offline_contract.rs");
include!("tests/plant_full_roundtrip_contract.rs");

include!("tests/control_studio_recording.rs");

include!("tests/control_studio_parameters.rs");
include!("tests/control_studio_transfer.rs");

#[cfg(target_os = "linux")]
include!("tests/control_studio_search.rs");

include!("tests/control_studio_symbolic_acquisition.rs");

#[cfg(target_os = "linux")]
include!("tests/control_studio_isolated_run.rs");
include!("tests/control_studio_delayed_integrator.rs");

include!("tests/control_studio_experiment_abort.rs");
