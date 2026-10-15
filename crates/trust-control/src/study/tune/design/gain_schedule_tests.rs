//! Tests of the Tune of a gain schedule (specification 35, C10 Tune; D-19) on two operating points of the commissioning example. Each
//! point's loop is the example as a program's PI law (C2) whose seed names the point's gain variables, and each point's model is the
//! first-order model with delay `gain·exp(−2·s)/(4·s + 1)` from `P1.ManipulatedValue` to `P1.ProcessValue`, at the operating point 50 %
//! and 50 degC. Equal models give one PI that meets both points. A second model with a fifth of the gain makes the first point's PI
//! too slow there, so each point keeps its own PI in the same table.

#![allow(clippy::float_cmp)] // The table holds the numbers that the single-loop designs write, so the tests compare them exactly.

use std::collections::BTreeMap;

use serde_json::json;
use trust_control_math::model::{OperatingPoint, TransferFunction};

use super::*;
use crate::failure::Failure;
use crate::identification::identify_result::kept_candidate;
use crate::loop_definition::{
    ControllerProject, GainSchedule, LoopDefinition, Parameter, RealType, SchedulePoint,
    SchedulingVariable,
};
use crate::messages::{CandidateOutcome, ScheduleChoiceWord, TuneMethod, TuneRequest};
use crate::model::{
    GeneralModel, ModelChannel, ModelIdentity, ModelSource, ModelValidation, OperatingRange,
};
use crate::stages::StageEvidence;
use crate::study::current_model::{CurrentModel, CurrentModelSource};
use crate::study::tune::design::{run, run_single};
use crate::study::tune::TuneInputs;
use crate::tuning::objective::TuningObjective;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::structure::{ParallelGains, Structure};
use crate::tuning::structures::gain_schedule::{choose_schedule, ScheduleChoice};

/// The commissioning example's loop, from which the points' loops are made.
const EXAMPLE: &str =
    include_str!("../../../../../../examples/control-pid-commissioning/control-loop.json");

/// The schedule's operating points: the name, the value of `P1.ProcessValue` in degC, the loop, and the point's proportional and integral
/// gain variables.
const POINTS: [(&str, f64, &str, &str, &str); 2] = [
    (
        "Low",
        20.0,
        "loop:low",
        "P1.Low.ProportionalGain",
        "P1.Low.IntegralGain",
    ),
    (
        "High",
        60.0,
        "loop:high",
        "P1.High.ProportionalGain",
        "P1.High.IntegralGain",
    ),
];

/// The table of the two points, each with a tolerance of 5 degC on the scheduling variable (C10 Setup).
fn table() -> Vec<SchedulePoint> {
    POINTS
        .iter()
        .map(
            |&(name, value, loop_id, proportional, integral)| SchedulePoint {
                name: name.to_owned(),
                loop_id: loop_id.to_owned(),
                value,
                tolerance: 5.0,
                proportional_symbol: proportional.to_owned(),
                integral_symbol: integral.to_owned(),
            },
        )
        .collect()
}

/// The schedule that the loop of the point `own` declares (C10 Setup): the table, the scheduling variable and the record the PID is
/// configured from.
fn schedule(own: &str) -> GainSchedule {
    GainSchedule {
        parameter_record: "P1.Parameters".to_owned(),
        scheduling: SchedulingVariable {
            symbol: "P1.ProcessValue".to_owned(),
            unit: "degC".to_owned(),
            iec_type: RealType::Lreal,
        },
        point: own.to_owned(),
        points: table(),
    }
}

/// A parameter of the loop's binding for one gain variable, bounded to ±10⁶ as N10 gives (here written ±1e6).
fn bound(symbol: String) -> Parameter {
    Parameter {
        symbol,
        iec_type: RealType::Real,
        unit: "1".to_owned(),
        convention: "schedule gain".to_owned(),
        minimum: -1e6,
        maximum: 1e6,
    }
}

/// The loop of the point at `index` of the table (C10 Setup): the commissioning example as a program's PI law (C2), whose seed names the
/// point's own gain variables, with the schedule and the gain variables of both points bound.
fn point_loop(index: usize) -> LoopDefinition {
    let (name, _, loop_id, proportional, integral) = POINTS[index];
    let mut loop_definition =
        LoopDefinition::decode(EXAMPLE).expect("the commissioning example's loop");
    loop_definition.id = loop_id.to_owned();
    loop_definition.controller.law = None;
    loop_definition.controller.pid_initialization = None;
    let mut project = ControllerProject::from_value(&json!({
        "convention": "project-owned-native-controller",
        "family": "PI",
        "initialization": { "kind": "declared-state-v1", "states": [] },
        "seed": { "kind": "parallel-pid", "family": "PI",
            "proportionalSymbol": proportional, "integralSymbol": integral },
        "axes": [],
        "objective": { "requirementId": "target-error", "direction": "minimize" },
        "maximumCandidates": 4, "maximumCycles": 50000, "wallBudgetMs": 60000
    }))
    .expect("a served PI seed is a complete project block");
    project.schedule = Some(schedule(name));
    loop_definition.controller.project = Some(project);
    if let Some(binding) = &mut loop_definition.parameter_binding {
        binding.parameters = table()
            .iter()
            .flat_map(|point| {
                [
                    point.proportional_symbol.clone(),
                    point.integral_symbol.clone(),
                ]
            })
            .map(bound)
            .collect();
    }
    loop_definition
}

/// A channel of a model, from 0 to 100 in `unit`.
fn channel(id: &str, unit: &str) -> ModelChannel {
    ModelChannel {
        id: id.to_owned(),
        unit: unit.to_owned(),
        operating_range: OperatingRange {
            minimum: 0.0,
            maximum: 100.0,
        },
    }
}

/// The supplied model of a point: `gain·exp(−2·s)/(4·s + 1)` from `P1.ManipulatedValue` in % to `P1.ProcessValue` in degC, at the
/// operating point 50 % and 50 degC. A supplied model is not fitted, so it has no normalized error (B3.7).
fn point_model(gain: f64) -> CurrentModel {
    CurrentModel {
        source: CurrentModelSource::Supplied,
        model: GeneralModel::TransferFunction {
            identity: ModelIdentity {
                revision: format!("sha256:point-{gain}"),
                source: ModelSource::Fitted,
                validation: ModelValidation::Validated,
                evidence: vec!["sha256:recording".to_owned()],
                inputs: vec![channel("P1.ManipulatedValue", "%")],
                outputs: vec![channel("P1.ProcessValue", "degC")],
                delay_seconds: 2.0,
                operating_point: Some(OperatingPoint {
                    inputs: vec![50.0],
                    outputs: vec![50.0],
                }),
            },
            transfer: TransferFunction {
                numerator: vec![gain],
                denominator: vec![4.0, 1.0],
            },
        },
        identify: None,
    }
}

/// The requirements of the points: a unit setpoint change, 0.04 of allowed error and a band of 0.02, settled by 40 s of an 80 s test, and
/// the Balanced response, as the single-loop design tests state them.
fn requirements() -> TuningRequirements {
    TuningRequirements {
        reference_change: 1.0,
        maximum_error: 0.04,
        settling_band: 0.02,
        settling_time_seconds: 40.0,
        duration_seconds: 80.0,
        constant_input_disturbance: None,
        measurement_noise_band: None,
        gain_uncertainty_fraction: None,
        transfer_tolerance: None,
        maximum_overshoot_percent: None,
        tuning_method: None,
        tuning_objective: Some(TuningObjective::Balanced),
        automatic_criteria: None,
    }
}

/// Automatic's request of the schedule (B4.2) with the PI structure of the schedule's seed, which each point is designed to (C10): no method
/// and no pins.
fn automatic() -> TuneRequest {
    TuneRequest {
        project_root: "/projects/control-pid-commissioning".to_owned(),
        runtime_id: "simulator".to_owned(),
        loop_id: "loop:low".to_owned(),
        storage_folder: "/state/trust".to_owned(),
        method: None,
        choice: None,
        structure: Some(Structure::PI),
        pins: None,
        settings: None,
        connection: None,
    }
}

/// The members of the table over the points' loops and models, each held to the same requirements and asked the same request.
fn members<'a>(
    loops: &'a [LoopDefinition],
    models: &'a [CurrentModel],
    request: &'a TuneRequest,
    requirements: &'a TuningRequirements,
) -> Vec<ScheduleMember<'a>> {
    loops
        .iter()
        .zip(models)
        .map(|(loop_definition, model)| ScheduleMember {
            inputs: TuneInputs {
                loop_definition,
                model: Some(model),
                previous: None,
                request,
                relay: None,
            },
            held: Some(requirements),
            running: None,
        })
        .collect()
}

/// The PI that a point's own single-loop design gives (B4.4): the gains the table holds at that point when the fixed design does not.
fn own_gains(member: &ScheduleMember<'_>) -> ParallelGains {
    run_single(&member.inputs, member.held, member.running)
        .expect("the point designs its own PI")
        .section
        .project_candidate
        .expect("the point's program candidate")
        .gains
}

/// The values of the table's variables for the gains of each point, in the order of the points (C2, C10).
fn table_values(gains: &[ParallelGains]) -> BTreeMap<String, f64> {
    table()
        .iter()
        .zip(gains)
        .flat_map(|(point, gain)| {
            [
                (point.proportional_symbol.clone(), gain.proportional_gain),
                (point.integral_symbol.clone(), gain.integral_gain),
            ]
        })
        .collect()
}

/// The refusal of the schedule for a request, on two points whose models are equal.
fn refusal_of(request: &TuneRequest) -> Failure {
    let loops = [point_loop(0), point_loop(1)];
    let models = [point_model(1.5), point_model(1.5)];
    let requirements = requirements();
    let points = members(&loops, &models, request, &requirements);
    run_schedule(&points, 0).expect_err("the schedule refuses the request")
}

#[test]
fn a_fixed_design_that_meets_every_point_gives_one_pi_to_every_point() {
    // Equal models: the first point's PI meets the second point's requirements too, so the table holds that PI at both points.
    let loops = [point_loop(0), point_loop(1)];
    let models = [point_model(1.5), point_model(1.5)];
    let request = automatic();
    let requirements = requirements();
    let points = members(&loops, &models, &request, &requirements);
    let first = own_gains(&points[0]);
    let outcome = run_schedule(&points, 1)
        .expect("the schedule is designed")
        .own;
    assert!(
        outcome.section.reason.starts_with("Fixed design"),
        "{}",
        outcome.section.reason
    );
    let candidate = outcome
        .section
        .project_candidate
        .as_ref()
        .expect("the program candidate");
    assert_eq!(candidate.gains, first);
    assert_eq!(candidate.values, table_values(&[first, first]));
    assert_eq!(outcome.result.settings, candidate.values);
    assert_eq!(outcome.result.reason, outcome.section.reason);
    assert_eq!(outcome.result.outcome, CandidateOutcome::Meets);
}

#[test]
fn a_fixed_design_that_misses_one_point_gives_each_point_its_own_pi_in_one_table() {
    // The second point has a fifth of the gain, so the first point's PI responds too slowly there and misses the settling requirement:
    // each point keeps its own PI.
    let loops = [point_loop(0), point_loop(1)];
    let models = [point_model(1.5), point_model(0.3)];
    let request = automatic();
    let requirements = requirements();
    let points = members(&loops, &models, &request, &requirements);
    let own = [own_gains(&points[0]), own_gains(&points[1])];
    assert_ne!(own[0], own[1], "the two points have different own PIs");
    let low = run_schedule(&points, 0)
        .expect("the schedule is designed for the first point")
        .own;
    let high = run_schedule(&points, 1)
        .expect("the schedule is designed for the second point")
        .own;
    assert!(
        low.section.reason.starts_with("Scheduled design"),
        "{}",
        low.section.reason
    );
    assert!(
        low.section.reason.contains("operating point High"),
        "{}",
        low.section.reason
    );
    let low_candidate = low
        .section
        .project_candidate
        .as_ref()
        .expect("the program candidate");
    assert_eq!(low_candidate.gains, own[0]);
    assert_eq!(low_candidate.values, table_values(&own));
    let high_candidate = high
        .section
        .project_candidate
        .as_ref()
        .expect("the program candidate");
    assert_eq!(high_candidate.gains, own[1]);
    assert_eq!(
        high_candidate.values, low_candidate.values,
        "both points' candidates hold the same table"
    );
    assert_eq!(high.section.reason, low.section.reason);
}

#[test]
fn a_scheduled_loop_is_refused_by_the_single_loop_design() {
    let loop_definition = point_loop(0);
    let model = point_model(1.5);
    let request = automatic();
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&model),
        previous: None,
        request: &request,
        relay: None,
    };
    let refusal = run(&inputs, None, None).expect_err("a scheduled loop is tuned with its points");
    assert_eq!(
        refusal.message,
        "A gain schedule is tuned with all its operating points together."
    );
}

#[test]
fn a_schedule_takes_only_automatic_and_the_pi_structure_of_its_seed() {
    let mut named = automatic();
    named.method = Some(TuneMethod::Simc);
    assert_eq!(
        refusal_of(&named).message,
        "A gain schedule is designed by Automatic."
    );
    let mut proportional = automatic();
    proportional.structure = Some(Structure::P);
    assert_eq!(
        refusal_of(&proportional).message,
        "A gain schedule's points are PI."
    );
}

#[test]
fn the_scheduled_reason_names_the_first_point_that_the_fixed_design_misses() {
    let points = table();
    assert_eq!(
        decision_reason(ScheduleChoice::Fixed, &[true, true], &points).expect("the fixed design"),
        FIXED_REASON
    );
    let high = decision_reason(ScheduleChoice::Scheduled, &[true, false], &points)
        .expect("a scheduled design");
    assert!(high.contains("operating point High"), "{high}");
    let low = decision_reason(ScheduleChoice::Scheduled, &[false, false], &points)
        .expect("a scheduled design");
    assert!(low.contains("operating point Low"), "{low}");
}

#[test]
fn the_table_is_stored_in_each_siblings_section() {
    // The second point's own section holds the same table as its program candidate, and no record of its own (C10 Tune).
    let loops = [point_loop(0), point_loop(1)];
    let models = [point_model(1.5), point_model(0.3)];
    let request = automatic();
    let requirements = requirements();
    let points = members(&loops, &models, &request, &requirements);
    let own = [own_gains(&points[0]), own_gains(&points[1])];
    let designed = run_schedule(&points, 0).expect("the schedule is designed");
    assert_eq!(designed.siblings.len(), 1, "one other point");
    let sibling = &designed.siblings[0];
    assert_eq!(sibling.index, 1);
    assert!(
        sibling.section.candidate.is_none(),
        "a program's law keeps no record"
    );
    let candidate = sibling
        .section
        .project_candidate
        .as_ref()
        .expect("the sibling's program candidate");
    assert!(!candidate.applied, "Tune never applies");
    assert_eq!(candidate.gains, own[1]);
    assert_eq!(candidate.values, table_values(&own));
    assert_eq!(sibling.section.reason, designed.own.section.reason);
    assert!(
        sibling.section.reason.starts_with("Scheduled design"),
        "{}",
        sibling.section.reason
    );
}

#[test]
fn an_unpredicted_fixed_design_is_not_a_failure_so_the_table_stays_fixed() {
    // A predicted response that misses a requirement fails the fixed design; without a prediction nothing is shown to fail (C10 Tune).
    let unpredicted = CandidateOutcome::Unpredicted {
        reason: "No model prediction is available.".to_owned(),
    };
    let misses = CandidateOutcome::Misses {
        shortfall: "The settling time is 20 s too long.".to_owned(),
    };
    assert!(!fails(&unpredicted));
    assert!(!fails(&CandidateOutcome::Meets));
    assert!(fails(&misses));
    assert_eq!(
        choose_schedule(&[!fails(&unpredicted), !fails(&CandidateOutcome::Meets)]),
        ScheduleChoice::Fixed
    );
    assert_eq!(
        choose_schedule(&[!fails(&unpredicted), !fails(&misses)]),
        ScheduleChoice::Scheduled
    );
}

#[test]
fn the_scheduled_answer_names_the_table_and_repeats_the_reason() {
    // The two points hold different PIs, so the answer is scheduled, and each row shows its point's own gains (C10 Tune).
    let loops = [point_loop(0), point_loop(1)];
    let models = [point_model(1.5), point_model(0.3)];
    let request = automatic();
    let requirements = requirements();
    let points = members(&loops, &models, &request, &requirements);
    let own = [own_gains(&points[0]), own_gains(&points[1])];
    let designed = run_schedule(&points, 0)
        .expect("the schedule is designed")
        .own;
    let answer = designed
        .result
        .schedule
        .as_ref()
        .expect("the answer names the schedule");
    assert_eq!(answer.choice, ScheduleChoiceWord::Scheduled);
    assert_eq!(answer.reason, designed.section.reason);
    assert_eq!(answer.scheduling.symbol, "P1.ProcessValue");
    assert_eq!(answer.points.len(), 2);
    for (row, gains) in answer.points.iter().zip(&own) {
        assert_eq!(
            row.proportional_gain, gains.proportional_gain,
            "{}",
            row.name
        );
        assert_eq!(row.integral_gain, gains.integral_gain, "{}", row.name);
    }
    assert_eq!(answer.points[1].name, "High");
}

#[test]
fn the_fixed_answer_shows_the_first_points_gains_on_every_row() {
    // Equal models give one PI, so the answer is fixed, and every row shows the first point's gains (C10 Tune).
    let loops = [point_loop(0), point_loop(1)];
    let models = [point_model(1.5), point_model(1.5)];
    let request = automatic();
    let requirements = requirements();
    let points = members(&loops, &models, &request, &requirements);
    let first = own_gains(&points[0]);
    let designed = run_schedule(&points, 1)
        .expect("the schedule is designed")
        .own;
    let answer = designed
        .result
        .schedule
        .as_ref()
        .expect("the answer names the schedule");
    assert_eq!(answer.choice, ScheduleChoiceWord::Fixed);
    assert_eq!(answer.reason, designed.section.reason);
    assert_eq!(answer.points.len(), 2);
    for row in &answer.points {
        assert_eq!(
            row.proportional_gain, first.proportional_gain,
            "{}",
            row.name
        );
        assert_eq!(row.integral_gain, first.integral_gain, "{}", row.name);
    }
}

#[test]
fn the_reopened_answer_equals_the_live_one() {
    // A kept section is answered from its candidate's values, which hold the table, so its schedule is the live schedule (C10).
    let loops = [point_loop(0), point_loop(1)];
    let models = [point_model(1.5), point_model(0.3)];
    let request = automatic();
    let requirements = requirements();
    let points = members(&loops, &models, &request, &requirements);
    let live = run_schedule(&points, 0)
        .expect("the schedule is designed")
        .own;
    let reopened = kept_candidate(
        &loops[0],
        Some(&models[0]),
        Some(&live.section),
        StageEvidence::default(),
    )
    .expect("the kept candidate is answered");
    assert!(
        reopened.schedule.is_some(),
        "a kept schedule names its table"
    );
    assert_eq!(reopened.schedule, live.result.schedule);
}

#[test]
fn the_choice_word_serializes_as_fixed_or_scheduled_and_an_absent_schedule_has_no_key() {
    assert_eq!(
        serde_json::to_value(ScheduleChoiceWord::Fixed).expect("json"),
        json!("fixed")
    );
    assert_eq!(
        serde_json::to_value(ScheduleChoiceWord::Scheduled).expect("json"),
        json!("scheduled")
    );
    let loops = [point_loop(0), point_loop(1)];
    let models = [point_model(1.5), point_model(0.3)];
    let request = automatic();
    let requirements = requirements();
    let points = members(&loops, &models, &request, &requirements);
    let mut result = run_schedule(&points, 0)
        .expect("the schedule is designed")
        .own
        .result;
    let with_key = serde_json::to_value(&result).expect("json");
    assert_eq!(with_key["schedule"]["choice"], json!("scheduled"));
    result.schedule = None;
    let without_key = serde_json::to_value(&result).expect("json");
    assert!(
        without_key.get("schedule").is_none(),
        "an absent schedule has no key"
    );
}
