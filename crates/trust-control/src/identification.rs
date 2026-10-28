//! Identification: the rules of Identify (specification 35, A6.4, A10.3, B3.4, B5.2, B5.5), calling the step-response
//! mathematics of `trust_control_math::identify`. Each module holds one rule set; the sentences the rules produce are in
//! `texts`.

pub mod acquisition_settings;
pub mod automatic;
pub mod bounded_step_review;
pub mod campaign;
pub mod campaign_review;
pub mod connected_record;
pub mod connected_run;
pub mod data;
pub mod evidence;
pub mod excitation;
pub mod experiment_limits;
pub mod experiment_trace;
pub mod form_draft;
pub mod frequency_identification;
pub mod frequency_texts;
pub mod identify_result;
pub mod independent_validation;
pub mod loop_sample;
pub mod measured_frequency_rows;
pub mod method_applicability;
pub mod method_offers;
pub mod mode;
pub mod mode_transfer;
pub mod model_comparison;
pub mod model_compatibility;
pub mod multichannel_fit;
pub mod plant;
pub mod policy;
pub mod presentation;
pub mod process_noise;
pub mod project_baseline;
pub mod project_test;
pub mod reconcile;
#[cfg(test)]
pub(crate) mod record_fixtures;
pub mod recorded_limits;
pub mod recorded_model;
pub mod recording_import;
pub mod refusal;
pub mod review;
pub mod saved_result;
pub mod selected_model_analysis;
pub mod selected_recording;
pub mod selection;
pub mod sensor_lag;
pub mod sensor_lag_comparison;
pub mod sensor_lag_review;
pub mod sensor_lag_texts;
pub mod settle;
pub mod state;
pub mod step_response;
pub mod test_lifecycle;
pub mod texts;
pub mod workflow;
