//! The commissioning rules of Control Studio (specification 35: B6 Apply, Restore and Keep; A6 modes and the transfer;
//! A7 limits; A10 long processes; A11 failures; C0 and C1 the controller structures), as pure functions without I/O,
//! clocks or threads. The language server runs them against the runtime; each rule decides, and its sentence comes from
//! `texts`.

pub mod actuator_feedback;
pub mod adapters;
pub mod application;
pub mod application_records;
pub mod application_texts;
pub mod apply_texts;
mod cascade_adapter;
pub mod channel_mapping;
pub mod checkpoint;
pub mod checkpoint_restore;
pub mod coefficient_record;
pub mod commissioning_result;
pub mod confirmation;
pub mod declared_initialization;
pub mod entry_allowance;
#[cfg(test)]
pub mod fixture_loops;
pub mod found_mode;
pub mod gain_recovery;
pub mod gain_schedule_adapter;
mod group_adapter;
pub mod initial_state;
pub mod initialization_formulas;
pub mod initialization_plan;
pub mod keep;
pub mod keep_texts;
pub mod library_pid;
pub mod mode_binding;
pub mod noise_probe;
pub mod observation;
pub mod observation_assessment;
pub mod on_off_adapter;
pub mod parameter_application;
pub mod parameter_binding;
pub mod parameter_session;
pub mod parameter_status;
#[cfg(test)]
pub mod pid_apply_fixture;
pub mod pid_commissioning;
pub mod pid_ending;
pub mod pid_initialization;
pub mod pid_measurement;
pub mod pid_operating_state;
pub mod pid_recovery;
pub mod profile;
pub mod project_law;
pub mod proportional_commissioning;
pub mod proportional_recovery;
pub mod restore_availability;
pub mod reviewed_application;
pub mod search_adapter;
pub mod smith_adapter;
pub mod starting_point;
#[cfg(test)]
#[path = "commissioning/starting_point_wait_tests.rs"]
mod starting_point_wait_tests;
pub mod state_adapter;
pub mod texts;
pub mod transfer;
pub mod transfer_protocol;
pub mod transfer_tracking;
