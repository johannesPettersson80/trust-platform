//! Validation evidence (specification 35, B5 and N8): the model-predicted engineering plots of a candidate, which
//! carry the response of the candidate and of the current settings, the curves of the open loop, the margins and
//! the evaluation windows of the step; and the records and checks of the isolated test runs that execute the actual
//! Structured Text (protocol annex E8.4); and the rules of Validate (B5, N8): the verification plan and its scenarios, the
//! campaign, the run assessment and Apply's eligibility, the independent model check, the plant an isolated run runs
//! against, the sample grid, the commissioning assessment and the terminal evidence of a test. The evidence is kept apart
//! by its class (N8.8): a prediction is never merged with a run on a plant.

pub mod commissioning_assessment;
pub mod configured_plant_verification;
pub mod engineer_envelope_scenarios;
pub mod engineer_scenario_effects;
pub mod engineer_verification_campaign;
pub mod engineer_verification_plan;
pub mod engineering_plots;
pub mod failure_sentence;
pub mod feedforward_case;
pub mod gain_schedule_case;
pub mod group_case;
pub mod initialization_case;
pub mod initialization_texts;
pub mod isolated_run_protocol;
pub mod isolated_run_sampling;
pub mod model_prediction_runs;
pub mod model_route;
pub mod model_validation_session;
pub mod on_off_case;
pub mod on_off_criteria;
pub mod plant_choice;
pub mod requirement_record;
pub mod run_requests;
pub mod run_validation;
pub mod scenario_contract;
pub mod scenario_form;
pub mod search_case;
pub mod smith_case;
pub mod state_case;
pub mod table_rows;
pub mod test_terminal_evidence;
pub mod texts;
