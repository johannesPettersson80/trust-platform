//! Tests of the Tune section of a study (specification 35, A8.1, A8.4, B4.1, B4.3, B4.7 and B10.4): its document, its
//! currency against the study's model, the evidence it gives the stages, the refusals that come before Tune starts, and the
//! words of the messages.

use serde_json::json;

use super::test_support::record;
use super::*;
use crate::experiment::ControlExperimentKind;
use crate::identification::evidence::tests::fitted_model;
use crate::identification::evidence::{ModelFitEvidence, ModelFitPoint, OperatingRange};
use crate::identification::identify_result::identify_signals;
use crate::identification::identify_result::tests::plant_loop;
use crate::identification::model_comparison::ComparisonPresentation;
use crate::identification::presentation::identified_model_presentation;
use crate::loop_definition::{ControllerLaw, Design, PidLaw, Topology, NO_CONTROLLER_LAW};
use crate::messages::{CandidateOutcome, TuneChoice, TuneMethod};
use crate::model::decode_general_model;
use crate::recording::EvidenceSource;
use crate::stages::{stage_availability, Stage, StageStatus};
use crate::study::identify::IdentifyMethod;
use crate::study::Study;
use crate::tuning::automatic_criteria::AutomaticCriteria;
use crate::tuning::candidate::{
    candidate_from_control_pid, AntiWindup, CandidateSource, ControlPidCandidate,
    ControlPidParameters,
};
use crate::tuning::objective::TuningObjective;
use crate::tuning::structure::Structure;

/// The recording file the identification's model was fitted from.
const RECORDING_ID: &str = "rec-00112233445566778899aabbccddeeff";

/// The fitted model of these tests, `gain · exp(−delay·s) / (2·s + 1)`, from a command in V to a position in m.
fn general_model(revision: &str, gain: f64, delay: f64) -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": revision, "source": "fitted",
        "validation": "unvalidated", "evidence": ["sha256:recording"], "delaySeconds": delay,
        "inputs": [{ "id": "command", "unit": "V", "operatingRange": { "minimum": -10.0, "maximum": 10.0 } }],
        "outputs": [{ "id": "position", "unit": "m", "operatingRange": { "minimum": -10.0, "maximum": 10.0 } }],
        "numerator": [gain], "denominator": [2.0, 1.0]
    }))
    .expect("a fitted model")
}

/// The provisional result of a step on the plant: the model of `general_model`, a first-order model with delay whose
/// normalized error is 0.04.
fn identify_section(revision: &str, gain: f64, delay: f64) -> IdentifySection {
    let fitted = IdentifiedModel {
        time_constant_seconds: Some(2.0),
        dead_time_seconds: Some(delay),
        normalized_error: Some(0.04),
        ..fitted_model(LowOrderModelKind::FirstOrderDeadTime, gain)
    };
    let fit = ModelFitEvidence {
        model_structure: None,
        points: vec![
            ModelFitPoint {
                time_seconds: 0.0,
                measured: 0.0,
                predicted: 0.0,
                residual: 0.0,
            },
            ModelFitPoint {
                time_seconds: 2.0,
                measured: 1.0,
                predicted: 0.9,
                residual: 0.1,
            },
        ],
        root_mean_square_error: 0.1,
        output_unit: "m".to_owned(),
        stable: true,
        final_change_fraction: None,
        delay_seconds: delay,
        operating_range: OperatingRange {
            input_minimum: 0.0,
            input_maximum: 1.0,
            output_minimum: 0.0,
            output_maximum: 1.0,
        },
        warnings: Vec::new(),
        step_response: None,
        step_response_measurement: None,
    };
    let presentation = ComparisonPresentation {
        presentation: identified_model_presentation(&fitted, &fit, "V", "m"),
        experiment: None,
        output_id: None,
        input_channels: None,
    };
    IdentifySection::new(
        IdentifyMethod::Live(ControlExperimentKind::MvStep),
        EvidenceSource::SimulatedPlant,
        identify_signals(&plant_loop(), ControlExperimentKind::MvStep),
        RECORDING_ID,
        None,
        general_model(revision, gain, delay),
        presentation,
    )
}

/// Requirements that hold their rules, two of them generated.
fn valid_requirements() -> TuningRequirements {
    TuningRequirements {
        channel_requirements: None,
        reference_change: 2.0,
        maximum_error: 0.4,
        settling_band: 0.1,
        settling_time_seconds: 45.0,
        duration_seconds: 90.0,
        constant_input_disturbance: None,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: Some(10.0),
        tuning_method: Some(TuningMethod::Simc),
        tuning_objective: Some(TuningObjective::Balanced),
        automatic_criteria: Some(AutomaticCriteria {
            model_revision: "model:fitted:1".to_owned(),
            fields: vec![
                "referenceChange".to_owned(),
                "settlingTimeSeconds".to_owned(),
            ],
        }),
    }
}

/// A PI record that holds the rules of a `Control.PID` record.
fn parameters() -> ControlPidParameters {
    ControlPidParameters {
        proportional_gain: 0.8,
        integral_gain: 0.2,
        derivative_gain: 0.0,
        proportional_setpoint_weight: 1.0,
        derivative_setpoint_weight: 0.0,
        bias: 0.0,
        output_minimum: -10.0,
        output_maximum: 10.0,
        rate_minimum: -50.0,
        rate_maximum: 50.0,
        tracking_time: 4.0,
        anti_windup: AntiWindup::BackCalculation,
    }
}

/// The plant loop with the parallel PID law of a `Control.PID` record, which Tune designs for (B4.7). A loop without a
/// controller law is not one Tune designs for: its Tune stage is Not applicable (C12).
fn pid_loop() -> LoopDefinition {
    let mut loop_definition = plant_loop();
    loop_definition.controller.law = Some(ControllerLaw::Pid(Box::new(PidLaw {
        parameter_record_symbol: "Plant.Parameters".to_owned(),
        parameter_revision_symbol: None,
        proportional_gain_symbol: "Plant.Parameters.Kp".to_owned(),
        integral_gain_symbol: "Plant.Parameters.Ki".to_owned(),
        derivative_gain_symbol: "Plant.Parameters.Kd".to_owned(),
        proportional_setpoint_weight_symbol: None,
        derivative_setpoint_weight_symbol: None,
        measurement_filter_time_symbol: None,
        anti_windup: None,
    })));
    loop_definition
}

/// The `Control.PID` record of a section, for the tests that break it.
fn record_mut(section: &mut TuneSection) -> &mut ControlPidCandidate {
    section
        .candidate
        .as_mut()
        .expect("a Control.PID section keeps a record")
}

/// A SIMC section designed on the model of the given revision.
pub(crate) fn valid_section(revision: &str) -> TuneSection {
    TuneSection {
        method: Some(TuneMethod::Simc),
        model_revision: revision.to_owned(),
        requirements: valid_requirements(),
        candidate: Some(
            candidate_from_control_pid(parameters(), CandidateSource::Simc)
                .expect("a valid record"),
        ),
        project_candidate: None,
        state_candidate: None,
        search_candidate: None,
        on_off_candidate: None,
        group_candidate: None,
        reason: "The SIMC formula result for the declared response.".to_owned(),
        closed_loop_time_seconds: Some(8.0),
        attempts: Vec::new(),
        pins: None,
    }
}

/// What `section.check()` refuses, after `edit` broke a rule of a valid section.
fn refusal_of(edit: impl FnOnce(&mut TuneSection)) -> Failure {
    let mut broken = valid_section("model:fitted:1");
    edit(&mut broken);
    broken.check().expect_err("a section that breaks a rule")
}

fn tune_request(method: Option<TuneMethod>) -> TuneRequest {
    TuneRequest {
        project_root: "/project".to_owned(),
        runtime_id: "runtime".to_owned(),
        loop_id: "tank".to_owned(),
        storage_folder: "/storage".to_owned(),
        method,
        choice: None,
        structure: None,
        pins: None,
        settings: None,
        connection: None,
    }
}

fn tune_status(evidence: &StageEvidence) -> StageStatus {
    stage_availability(evidence)
        .into_iter()
        .find(|item| item.stage == Stage::Tune)
        .map(|item| item.status)
        .expect("every stage has a status")
}

#[test]
fn a_tune_section_round_trips_through_its_document_and_the_study() {
    let section = valid_section("model:fitted:1");
    let text = section.encode().expect("encoded");
    let decoded = TuneSection::decode(&text).expect("decoded");
    assert_eq!(decoded, section);
    assert_eq!(
        decoded.encode().expect("again"),
        text,
        "the same section gives the same canonical JSON"
    );

    let mut study = Study::new("tank", "sha256:loop-1");
    study.set_section(&section).expect("kept");
    assert!(
        study.recordings.is_empty(),
        "a candidate refers to no recording"
    );
    let document = study.encode().expect("encoded");
    let reopened = Study::decode(&document, "tank").expect("decoded");
    assert_eq!(
        reopened.section::<TuneSection>().expect("read"),
        Some(section)
    );
}

#[test]
fn the_section_is_stored_under_the_words_of_the_study() {
    let value = serde_json::to_value(valid_section("model:fitted:1")).expect("json");
    assert_eq!(value["method"], json!("SIMC"));
    assert_eq!(value["modelRevision"], json!("model:fitted:1"));
    assert_eq!(value["candidate"]["applied"], json!(false));
    assert_eq!(value["candidate"]["source"], json!("SIMC"));
    assert_eq!(value["candidate"]["structure"], json!("PI"));
    assert_eq!(value["requirements"]["tuningObjective"], json!("balanced"));
    assert!(value.get("pins").is_none(), "no pins, no field");

    let automatic = TuneSection {
        method: None,
        ..valid_section("model:fitted:1")
    };
    let value = serde_json::to_value(automatic).expect("json");
    assert!(value.get("method").is_none(), "Automatic names no method");

    let mut extra = serde_json::to_value(valid_section("model:fitted:1")).expect("json");
    extra["extra"] = json!(true);
    assert!(
        TuneSection::decode(&extra.to_string()).is_err(),
        "a document with an unknown field is not this section"
    );
}

#[test]
fn a_candidate_is_current_only_for_the_model_it_was_designed_on() {
    let section = valid_section("model:fitted:1");
    assert!(section.is_current_for(&general_model("model:fitted:1", 0.5, 1.0)));
    assert!(
        !section.is_current_for(&general_model("model:fitted:2", 0.5, 1.0)),
        "a new or replaced model leaves the candidate behind (A8.4)"
    );
}

#[test]
fn a_section_that_breaks_a_rule_is_neither_kept_nor_read() {
    let applied = refusal_of(|section| record_mut(section).applied = true);
    assert!(
        applied.message.contains("marked applied"),
        "{}",
        applied.message
    );
    assert_eq!(
        (applied.cause, applied.next_action),
        (Cause::Settings, NextAction::None)
    );

    let unnamed = refusal_of(|section| section.model_revision = "  ".to_owned());
    assert!(
        unnamed.message.contains("no model revision"),
        "{}",
        unnamed.message
    );

    let requirements = refusal_of(|section| section.requirements.settling_band = 0.4);
    assert!(
        requirements
            .message
            .contains("greater than the settling band"),
        "{}",
        requirements.message
    );
    assert_eq!(
        (requirements.cause, requirements.next_action),
        (Cause::Settings, NextAction::OpenRequirements)
    );

    let criteria = refusal_of(|section| {
        section.requirements.automatic_criteria = Some(AutomaticCriteria {
            model_revision: "model:fitted:1".to_owned(),
            fields: vec!["unknownKey".to_owned()],
        });
    });
    assert_eq!(
        (criteria.cause, criteria.next_action),
        (Cause::Settings, NextAction::OpenRequirements)
    );

    let lambda = refusal_of(|section| section.closed_loop_time_seconds = Some(0.0));
    assert!(
        lambda.message.contains("closed-loop time constant"),
        "{}",
        lambda.message
    );

    let record = refusal_of(|section| record_mut(section).parameters.tracking_time = 0.0);
    assert!(
        record.message.contains("tracking time"),
        "{}",
        record.message
    );
    assert_eq!(
        (record.cause, record.next_action),
        (Cause::Settings, NextAction::None)
    );

    let mut broken = valid_section("model:fitted:1");
    record_mut(&mut broken).applied = true;
    let mut study = Study::new("tank", "sha256:loop-1");
    assert!(broken.encode().is_err());
    assert!(study.set_section(&broken).is_err());
    assert!(study.sections.is_empty(), "nothing was kept");

    let mut stored = serde_json::to_value(valid_section("model:fitted:1")).expect("json");
    stored["candidate"]["applied"] = json!(true);
    study.sections.insert("tune".to_owned(), stored);
    assert!(
        study.section::<TuneSection>().is_err(),
        "a stored section that breaks a rule is not read (B10.4)"
    );
}

#[test]
fn the_tune_stage_has_a_candidate_only_while_the_study_holds_the_model_it_was_designed_on() {
    let loop_definition = plant_loop();
    let identify = CurrentModel::identified(identify_section("model:fitted:1", 0.5, 1.0));
    let designed = valid_section("model:fitted:1");

    let evidence = stage_evidence(&loop_definition, Some(&identify), Some(&designed), true);
    assert!(evidence.has_model && evidence.has_candidate && evidence.has_recording);
    assert_eq!(tune_status(&evidence), StageStatus::Complete);

    let replaced = CurrentModel::identified(identify_section("model:fitted:2", 0.5, 1.0));
    let stale = stage_evidence(&loop_definition, Some(&replaced), Some(&designed), true);
    assert!(stale.has_model, "the new model is current for the loop");
    assert!(
        !stale.has_candidate,
        "the candidate was designed on the replaced model (A8.4)"
    );
    assert_eq!(tune_status(&stale), StageStatus::Ready);

    let untuned = stage_evidence(&loop_definition, Some(&identify), None, false);
    assert!(untuned.has_model && !untuned.has_candidate);
    let unidentified = stage_evidence(&loop_definition, None, Some(&designed), false);
    assert!(!unidentified.has_model && !unidentified.has_candidate);

    let mut renamed = plant_loop();
    renamed.signals.process_variable.symbol = "Plant.Level".to_owned();
    let not_current = stage_evidence(&renamed, Some(&identify), Some(&designed), true);
    assert!(!not_current.has_model && !not_current.has_candidate);
    assert_eq!(tune_status(&not_current), StageStatus::NeedsData);
}

#[test]
fn tune_refuses_a_loop_without_a_controller_law_before_it_looks_for_a_model() {
    // C12: Tune is Not applicable to such a loop, so the refusal comes before the model check and needs no action.
    let mut observed = plant_loop();
    observed.controller.law = None;
    observed.controller.design = Some(Design::Project);
    observed.topology = Some(Topology::OpenLoop);
    let request = tune_request(None);
    let refused = check_before_start(&TuneInputs {
        loop_definition: &observed,
        model: None,
        previous: None,
        request: &request,
        relay: None,
        pins: None,
    })
    .expect_err("no controller law to tune");
    assert_eq!(refused.message, NO_CONTROLLER_LAW);
    assert_eq!(
        (refused.cause, refused.next_action),
        (Cause::Implementation, NextAction::None)
    );
}

#[test]
fn tune_is_refused_at_once_without_a_model_that_is_current_for_the_loop() {
    let loop_definition = plant_loop();
    let request = tune_request(None);
    let without_model = TuneInputs {
        loop_definition: &loop_definition,
        model: None,
        previous: None,
        request: &request,
        relay: None,
        pins: None,
    };
    let refused = check_before_start(&without_model).expect_err("no model");
    assert_eq!(
        (refused.cause, refused.next_action),
        (Cause::ModelOrData, NextAction::Identify)
    );
    assert!(refused.message.contains("no model"), "{}", refused.message);

    let identify = CurrentModel::identified(identify_section("model:fitted:1", 0.5, 1.0));
    let mut moved = plant_loop();
    moved.signals.manipulated_variable.symbol = "Plant.Valve".to_owned();
    let not_current = TuneInputs {
        loop_definition: &moved,
        model: Some(&identify),
        ..without_model
    };
    let refused = check_before_start(&not_current).expect_err("the signals changed");
    assert_eq!(
        (refused.cause, refused.next_action),
        (Cause::ModelOrData, NextAction::Identify)
    );
}

#[test]
fn a_model_that_every_method_takes_is_accepted() {
    let loop_definition = pid_loop();
    let identify = CurrentModel::identified(identify_section("model:fitted:1", 0.5, 1.0));
    for method in [None, Some(TuneMethod::Simc), Some(TuneMethod::Amigo)] {
        let request = tune_request(method);
        let inputs = TuneInputs {
            loop_definition: &loop_definition,
            model: Some(&identify),
            previous: None,
            request: &request,
            relay: None,
            pins: None,
        };
        assert_eq!(check_before_start(&inputs), Ok(()), "{method:?}");
    }
}

#[test]
fn a_named_method_outside_its_domain_is_refused_with_choose_another_method() {
    let loop_definition = pid_loop();
    // The dead time of 3 s is longer than the time constant of 2 s: outside AMIGO's domain (N5.5), inside SIMC's.
    let identify = CurrentModel::identified(identify_section("model:fitted:1", 0.5, 3.0));
    let amigo = tune_request(Some(TuneMethod::Amigo));
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&identify),
        previous: None,
        request: &amigo,
        relay: None,
        pins: None,
    };
    let refused = check_before_start(&inputs).expect_err("L/T above 1");
    assert_eq!(
        (refused.cause, refused.next_action),
        (Cause::ModelOrData, NextAction::ChooseMethod)
    );
    assert_eq!(
        refused.message,
        "AMIGO delay ratio is outside the qualified delay domain L/T <= 1."
    );

    let simc = tune_request(Some(TuneMethod::Simc));
    let simc_inputs = TuneInputs {
        request: &simc,
        ..inputs
    };
    assert_eq!(check_before_start(&simc_inputs), Ok(()));
}

#[test]
fn a_gain_that_contradicts_the_controller_action_is_refused_with_the_loop_setup() {
    let mut loop_definition = pid_loop();
    loop_definition.controller.action = Action::Reverse;
    let direct_plant = CurrentModel::identified(identify_section("model:fitted:1", 0.5, 1.0));
    for method in [None, Some(TuneMethod::Simc)] {
        let request = tune_request(method);
        let inputs = TuneInputs {
            loop_definition: &loop_definition,
            model: Some(&direct_plant),
            previous: None,
            request: &request,
            relay: None,
            pins: None,
        };
        let refused = check_before_start(&inputs).expect_err("the gain contradicts the action");
        assert_eq!(
            (refused.cause, refused.next_action),
            (Cause::ModelOrData, NextAction::OpenLoopSetup)
        );
        assert!(
            refused
                .message
                .contains("the measurement rises when the output rises"),
            "{}",
            refused.message
        );
    }

    // A reverse-acting loop expects a negative gain (N6.2).
    let reverse_plant = CurrentModel::identified(identify_section("model:fitted:1", -0.5, 1.0));
    let request = tune_request(Some(TuneMethod::Simc));
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&reverse_plant),
        previous: None,
        request: &request,
        relay: None,
        pins: None,
    };
    assert_eq!(check_before_start(&inputs), Ok(()));
}

#[test]
fn the_messages_use_the_words_of_the_command_line() {
    for (word, choice, objective) in [
        ("faster", TuneChoice::Faster, TuningObjective::Fast),
        ("balanced", TuneChoice::Balanced, TuningObjective::Balanced),
        (
            "more-robust",
            TuneChoice::MoreRobust,
            TuningObjective::Robust,
        ),
    ] {
        assert_eq!(serde_json::to_value(choice).expect("json"), json!(word));
        assert_eq!(
            serde_json::from_value::<TuneChoice>(json!(word)).expect("decoded"),
            choice
        );
        assert_eq!(choice.objective(), objective);
    }

    let mut document = json!({
        "projectRoot": "/project", "runtimeId": "runtime", "loopId": "tank", "storageFolder": "/storage"
    });
    let minimal: TuneRequest = crate::json::decode_value(document.clone()).expect("a request");
    assert_eq!(
        (
            minimal.method,
            minimal.choice,
            minimal.structure,
            minimal.pins
        ),
        (None, None, None, None),
        "Automatic names no method, response, structure or pin"
    );
    document["method"] = json!("AMIGO");
    document["choice"] = json!("more-robust");
    document["structure"] = json!("PID");
    let named: TuneRequest = crate::json::decode_value(document.clone()).expect("a request");
    assert_eq!(named.method, Some(TuneMethod::Amigo));
    assert_eq!(named.choice, Some(TuneChoice::MoreRobust));
    assert_eq!(named.structure, Some(Structure::PID));
    document["connection"] = json!({});
    assert!(
        crate::json::decode_value::<TuneRequest>(document).is_err(),
        "Tune never reaches the runtime, so a request carries no connection"
    );

    assert_eq!(
        serde_json::to_value(CandidateOutcome::Meets).expect("json"),
        json!({ "status": "meets" })
    );
    let misses = CandidateOutcome::Misses {
        shortfall: "The settling time is 3 s too long.".to_owned(),
    };
    assert_eq!(
        serde_json::to_value(misses).expect("json"),
        json!({ "status": "misses", "shortfall": "The settling time is 3 s too long." })
    );
    let unpredicted = CandidateOutcome::Unpredicted {
        reason: "No model prediction is available.".to_owned(),
    };
    assert_eq!(
        serde_json::to_value(unpredicted).expect("json"),
        json!({ "status": "unpredicted", "reason": "No model prediction is available." })
    );
}

#[test]
fn a_control_pid_section_keeps_its_bytes_and_holds_no_project_candidate() {
    // C1: a Control.PID section names no project candidate, and reads and writes back unchanged.
    let section = valid_section("model:fitted:1");
    let text = section.encode().expect("the section encodes");
    assert!(
        !text.contains("projectCandidate"),
        "the record's section has no program candidate"
    );
    let decoded = TuneSection::decode(&text).expect("the section decodes");
    assert_eq!(decoded, section);
    assert_eq!(decoded.encode().expect("the section encodes again"), text);
}

#[test]
fn a_constrained_proportional_recommendation_is_kept_and_read_back_whole() {
    // control-studio-validate-apply: "round-trips a retained constrained proportional recommendation" (B10.2, B10.4; C0 "Families and
    // zero terms": a P writes its integral and derivative gains as explicit zeros). The candidate keeps its source and its structure.
    let mut section = valid_section("model:fitted:1");
    section.method = None;
    section.closed_loop_time_seconds = None;
    section.candidate = Some(
        candidate_from_control_pid(
            ControlPidParameters {
                proportional_gain: 0.8,
                integral_gain: 0.0,
                derivative_gain: 0.0,
                ..parameters()
            },
            CandidateSource::ConstrainedComparison,
        )
        .expect("a valid record"),
    );
    let text = section.encode().expect("encoded");
    let decoded = TuneSection::decode(&text).expect("decoded");
    assert_eq!(decoded, section);
    let kept = record(decoded.candidate.as_ref());
    assert_eq!(kept.source, CandidateSource::ConstrainedComparison);
    assert_eq!(kept.structure, Structure::P);
    assert!(!kept.applied, "a candidate is never applied by Tune");

    let value = serde_json::to_value(&section).expect("json");
    assert_eq!(
        value["candidate"]["source"],
        json!("Constrained comparison")
    );
    assert_eq!(value["candidate"]["structure"], json!("P"));
}

#[test]
fn the_automatic_comparison_keeps_every_attempt_and_refuses_an_unknown_status_or_lambda() {
    // control-studio-validate-apply: "round-trips and validates the complete automatic tuning comparison" (B4.4, Part C "Candidates and
    // results": every attempt stays with its reason; B10.4: a record that breaks a rule is rejected as a whole).
    use crate::tuning::recommendation::AttemptStatus;
    let mut section = valid_section("model:fitted:1");
    section.method = None;
    section.closed_loop_time_seconds = Some(1.5);
    section.attempts = vec![
        Attempt {
            structure: Structure::P,
            status: AttemptStatus::Rejected,
            reason: "Failed: Noise response.".to_owned(),
        },
        Attempt {
            structure: Structure::PI,
            status: AttemptStatus::Feasible,
            reason: "A compatible candidate on the model.".to_owned(),
        },
    ];
    let text = section.encode().expect("encoded");
    assert_eq!(
        TuneSection::decode(&text).expect("decoded"),
        section,
        "every attempt comes back with its structure, status and reason"
    );

    let mut unknown = serde_json::to_value(&section).expect("json");
    unknown["attempts"][0]["status"] = json!("Winner");
    assert!(
        TuneSection::decode(&unknown.to_string()).is_err(),
        "an unknown attempt status does not reopen as qualified evidence"
    );

    let lambda = refusal_of(|broken| broken.closed_loop_time_seconds = Some(-1.0));
    assert!(
        lambda.message.contains("closed-loop time constant"),
        "{}",
        lambda.message
    );
}
