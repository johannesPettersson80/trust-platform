//! Identification: the rules of Identify (specification 35, A6.4, A10.3, B3.4, B5.2, B5.5), calling the step-response
//! mathematics of `trust_control_math::identify`. Each module holds one rule set; the sentences the rules produce are in
//! `texts`.

pub mod acquisition_settings;
pub mod automatic;
pub mod bounded_step_review;
pub mod campaign;
pub mod connected_record;
pub mod connected_run;
pub mod data;
pub mod evidence;
pub mod excitation;
pub mod experiment_trace;
pub mod form_draft;
pub mod identify_result;
pub mod independent_validation;
pub mod loop_sample;
pub mod method_applicability;
pub mod mode;
pub mod model_comparison;
pub mod model_compatibility;
pub mod plant;
pub mod policy;
pub mod presentation;
pub mod process_noise;
pub mod reconcile;
#[cfg(test)]
pub(crate) mod record_fixtures;
pub mod recorded_limits;
pub mod recorded_model;
pub mod recording_import;
pub mod refusal;
pub mod review;
pub mod selected_model_analysis;
pub mod selected_recording;
pub mod selection;
pub mod settle;
pub mod state;
pub mod step_response;
pub mod test_lifecycle;
pub mod texts;
pub mod workflow;
