//! The wait for the starting point of an Apply (specification 35, B6.2, B6.7). These are the assertions of the `TypeScript` test
//! `control-studio-apply-settling.test.ts`, on the decision of each step (`StartingPoint::wait_step`): the reads of the runtime, the
//! process-time clock and the progress display are transport. The settle dwell (B6.2) has its own tests: a touch of the band does not
//! settle an output, a dwell that holds does, a reading that leaves the band restarts the dwell, and the bound still refuses. The V4.6
//! regression, a Manual Apply accepts a plant input within the Auto-entry allowance and refuses one beyond it, is also here.

#![allow(clippy::float_cmp)] // The validated points and the readings are exact decimals, compared exactly on purpose.

use std::collections::BTreeMap;

use serde_json::json;

use super::entry_allowance::auto_entry_tolerance;
use super::fixture_loops::{first_order_model, proportional_loop, requirements};
use super::starting_point::{
    anchor_integrating_outputs, settle_dwell_ms, wait_bound_ms, wait_cancelled, StartingPoint,
    WaitStep,
};
use super::transfer::TransferValue;
use crate::loop_definition::LoopDefinition;
use crate::model::general::decode_general_model;
use crate::model::GeneralModel;
use crate::validation::engineer_verification_plan::{engineer_requirements, EngineerRequirements};

/// The loop of the cold room: `P1.RoomC` is the measured output (`degC`), and `P1.CompressorPct` the plant input (%).
fn cold_room() -> LoopDefinition {
    let mut loop_definition = proportional_loop();
    loop_definition.signals.process_variable.symbol = "P1.RoomC".to_owned();
    loop_definition.signals.process_variable.unit = "degC".to_owned();
    loop_definition.signals.manipulated_variable.symbol = "P1.CompressorPct".to_owned();
    loop_definition
}

/// A transfer-function room model with the validated point of 12 `degC` at 0 % (`MODEL` of the `TypeScript` test).
#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn room_model(
    numerator: serde_json::Value,
    denominator: serde_json::Value,
    revision: &str,
) -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": revision, "source": "provided", "validation": "validated",
        "evidence": ["bench"], "delaySeconds": 0, "numerator": numerator, "denominator": denominator,
        "inputs": [{ "id": "P1.CompressorPct", "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } }],
        "outputs": [{ "id": "P1.RoomC", "unit": "degC", "operatingRange": { "minimum": 2, "maximum": 12 } }],
        "operatingPoint": { "inputs": [0], "outputs": [12] }
    }))
    .expect("the room model decodes")
}

/// The self-regulating room model: the output returns to its validated 12 `degC`.
fn room() -> GeneralModel {
    room_model(json!([-0.1]), json!([60, 1]), "room")
}

/// The integrating tank: the output rests wherever its inflow balances.
fn tank() -> GeneralModel {
    room_model(json!([0.01]), json!([1, 0]), "integrating")
}

/// The requirements of the cold room: a 7 `degC` change, a 0.5 `degC` settling band, 90 s to settle in a 300 s test.
fn room_requirements() -> EngineerRequirements {
    engineer_requirements(&json!({
        "referenceChange": -7, "maximumError": 2, "settlingBand": 0.5, "settlingTimeSeconds": 90, "durationSeconds": 300
    }))
    .expect("the room requirements are valid")
}

/// The start of the settle dwell that a waiting step hands on. A settled step has no start to hand on.
fn inside_since(step: &WaitStep) -> Option<f64> {
    match step {
        WaitStep::Waiting {
            inside_since_ms, ..
        } => *inside_since_ms,
        WaitStep::Settled => panic!("the wait has settled: {step:?}"),
    }
}

/// One reading of the runtime: the plant input and the measured output.
fn reading(compressor: f64, room_temperature: f64) -> BTreeMap<String, TransferValue> {
    BTreeMap::from([
        (
            "P1.CompressorPct".to_owned(),
            TransferValue::Number(compressor),
        ),
        (
            "P1.RoomC".to_owned(),
            TransferValue::Number(room_temperature),
        ),
    ])
}

#[test]
fn the_wait_shows_its_progress_and_continues_once_the_output_settles() {
    // The TypeScript test: "waits for the measured output to settle at the validated point, showing its progress, then continues".
    let loop_definition = cold_room();
    let model = room();
    let task = room_requirements();
    let inputs = ["P1.CompressorPct".to_owned()];
    let outputs = ["P1.RoomC".to_owned()];
    let start = StartingPoint {
        loop_definition: &loop_definition,
        model: &model,
        task: &task,
        input_symbols: &inputs,
        output_symbols: &outputs,
        input_tolerance: 1e-4,
    };
    let bound = wait_bound_ms(&task);
    assert_eq!(
        bound, 900_000.0,
        "the longer of 60 s and three times the 300 s test (B6.2)"
    );

    let first = start
        .wait_step(&reading(0.0, 10.3), 0.0, bound, None)
        .expect("the first reading waits");
    let WaitStep::Waiting { progress, .. } = first else {
        panic!("the output is not settled yet: {first:?}");
    };
    assert_eq!(
        progress, "Waiting for P1.RoomC to settle at 12 °C ± 0.5 °C (now 10.3 °C)…",
        "the progress names the validated point and the current value"
    );
    let second = start
        .wait_step(&reading(0.0, 11.2), 100.0, bound, None)
        .expect("the second reading waits");
    assert!(matches!(
        second,
        WaitStep::Waiting {
            inside_since_ms: None,
            ..
        }
    ));
    // 11.7 is inside the band 12 +- 0.5: the first reading inside the band starts the settle dwell (B6.2); it does not end the wait.
    let entered = start
        .wait_step(&reading(0.0, 11.7), 200.0, bound, None)
        .expect("the third reading waits for the dwell");
    assert_eq!(
        entered,
        WaitStep::Waiting {
            progress: "P1.RoomC settled at 12 °C ± 0.5 °C (now 11.7 °C)…".into(),
            inside_since_ms: Some(200.0),
        }
    );
    // The cold room's dwell is 9 s: a tenth of its 90 s settling time, longer than twenty 10 ms executions (B6.2).
    assert_eq!(settle_dwell_ms(&loop_definition, &task), 9_000.0);
    let running = start
        .wait_step(&reading(0.0, 11.7), 9_199.0, bound, Some(200.0))
        .expect("the dwell has not run out");
    assert!(matches!(running, WaitStep::Waiting { .. }));
    let settled = start
        .wait_step(&reading(0.0, 11.7), 9_200.0, bound, Some(200.0))
        .expect("the dwell has run out");
    assert_eq!(settled, WaitStep::Settled);
}

#[test]
fn an_output_that_touches_its_band_once_and_leaves_it_is_not_settled() {
    // B6.2: a reading inside the band starts the dwell; a reading outside it clears the dwell, however long ago the touch was.
    let loop_definition = cold_room();
    let model = room();
    let task = room_requirements();
    let inputs = ["P1.CompressorPct".to_owned()];
    let outputs = ["P1.RoomC".to_owned()];
    let start = StartingPoint {
        loop_definition: &loop_definition,
        model: &model,
        task: &task,
        input_symbols: &inputs,
        output_symbols: &outputs,
        input_tolerance: 1e-4,
    };
    let bound = wait_bound_ms(&task);
    let touched = start
        .wait_step(&reading(0.0, 11.7), 0.0, bound, None)
        .expect("the touch waits");
    assert_eq!(inside_since(&touched), Some(0.0));
    let left = start
        .wait_step(&reading(0.0, 10.3), 100.0, bound, Some(0.0))
        .expect("the room leaves its band");
    assert_eq!(inside_since(&left), None);
    // Past the 9 s that the touch alone would have run, the room is still outside its band: it has not settled.
    let later = start
        .wait_step(&reading(0.0, 10.3), 9_500.0, bound, None)
        .expect("the room is still outside its band");
    assert_eq!(inside_since(&later), None);
}

#[test]
fn an_output_that_stays_inside_its_band_for_the_dwell_is_settled() {
    // B6.2: the wait ends with the reading that completes the 9 s dwell of the cold room, and not one reading earlier.
    let loop_definition = cold_room();
    let model = room();
    let task = room_requirements();
    let inputs = ["P1.CompressorPct".to_owned()];
    let outputs = ["P1.RoomC".to_owned()];
    let start = StartingPoint {
        loop_definition: &loop_definition,
        model: &model,
        task: &task,
        input_symbols: &inputs,
        output_symbols: &outputs,
        input_tolerance: 1e-4,
    };
    let bound = wait_bound_ms(&task);
    let dwell = settle_dwell_ms(&loop_definition, &task);
    let entered = start
        .wait_step(&reading(0.0, 11.7), 1_000.0, bound, None)
        .expect("the room enters its band");
    assert_eq!(inside_since(&entered), Some(1_000.0));
    let running = start
        .wait_step(
            &reading(0.0, 11.8),
            1_000.0 + dwell - 100.0,
            bound,
            Some(1_000.0),
        )
        .expect("the dwell has not run out");
    assert!(matches!(running, WaitStep::Waiting { .. }));
    let settled = start
        .wait_step(&reading(0.0, 11.8), 1_000.0 + dwell, bound, Some(1_000.0))
        .expect("the dwell has run out");
    assert_eq!(settled, WaitStep::Settled);
}

#[test]
fn the_dwell_restarts_when_a_reading_leaves_the_band() {
    // B6.2: a reading that leaves the band clears the dwell, and the room must then stay inside for a whole dwell again.
    let loop_definition = cold_room();
    let model = room();
    let task = room_requirements();
    let inputs = ["P1.CompressorPct".to_owned()];
    let outputs = ["P1.RoomC".to_owned()];
    let start = StartingPoint {
        loop_definition: &loop_definition,
        model: &model,
        task: &task,
        input_symbols: &inputs,
        output_symbols: &outputs,
        input_tolerance: 1e-4,
    };
    let bound = wait_bound_ms(&task);
    let dwell = settle_dwell_ms(&loop_definition, &task);
    let first = start
        .wait_step(&reading(0.0, 11.7), 0.0, bound, None)
        .expect("the room enters its band");
    let left = start
        .wait_step(&reading(0.0, 10.3), 5_000.0, bound, inside_since(&first))
        .expect("the room leaves its band");
    assert_eq!(inside_since(&left), None);
    let back = start
        .wait_step(&reading(0.0, 11.8), 6_000.0, bound, inside_since(&left))
        .expect("the room is back inside its band");
    assert_eq!(inside_since(&back), Some(6_000.0));
    // The first entry would have settled at 9 s. This run has to hold for a whole dwell from 6 s, so 100 ms short is still running.
    let running = start
        .wait_step(
            &reading(0.0, 11.8),
            6_000.0 + dwell - 100.0,
            bound,
            Some(6_000.0),
        )
        .expect("the dwell has not run out");
    assert!(matches!(running, WaitStep::Waiting { .. }));
    let settled = start
        .wait_step(&reading(0.0, 11.8), 6_000.0 + dwell, bound, Some(6_000.0))
        .expect("the dwell has run out");
    assert_eq!(settled, WaitStep::Settled);
}

#[test]
fn a_plant_input_away_from_the_validated_point_is_refused_at_once() {
    // The TypeScript test: "refuses at once when a plant input differs from the validated point, because the process cannot settle by itself".
    let loop_definition = cold_room();
    let model = room();
    let task = room_requirements();
    let inputs = ["P1.CompressorPct".to_owned()];
    let outputs = ["P1.RoomC".to_owned()];
    let start = StartingPoint {
        loop_definition: &loop_definition,
        model: &model,
        task: &task,
        input_symbols: &inputs,
        output_symbols: &outputs,
        input_tolerance: 1e-4,
    };
    let refused = start
        .wait_step(&reading(100.0, 12.0), 0.0, wait_bound_ms(&task), None)
        .expect_err("the plant input is 100 % against 0 %");
    assert!(
        refused
            .message
            .contains("P1.CompressorPct is 100 %, but the settings were validated from 0 %"),
        "{}",
        refused.message
    );
}

#[test]
fn a_cancelled_wait_is_reported_as_cancelled_and_writes_nothing() {
    // The TypeScript test: "stops when cancelled, before any setting is written" (B6.2): the refusal says nothing was written.
    let cancelled = wait_cancelled();
    assert!(
        cancelled.message.contains("cancelled"),
        "{}",
        cancelled.message
    );
    assert!(
        cancelled.message.contains("nothing was written"),
        "{}",
        cancelled.message
    );
}

#[test]
fn an_integrating_output_starts_from_its_present_value() {
    // The TypeScript test: "an integrating output rests wherever it is, so Apply starts from its present value" (B6.2).
    let loop_definition = cold_room();
    let model = tank();
    let task = room_requirements();
    let inputs = ["P1.CompressorPct".to_owned()];
    let outputs = ["P1.RoomC".to_owned()];
    let start = StartingPoint {
        loop_definition: &loop_definition,
        model: &model,
        task: &task,
        input_symbols: &inputs,
        output_symbols: &outputs,
        input_tolerance: 1e-4,
    };
    let values = reading(0.0, 9.0);
    let step = start
        .wait_step(&values, 0.0, wait_bound_ms(&task), None)
        .expect("no wait: the output rests");
    assert_eq!(
        step,
        WaitStep::Settled,
        "the plant input is at its validated value, and the output rests where it is"
    );

    let anchored = anchor_integrating_outputs(&tank(), &values, &outputs);
    assert_eq!(
        anchored
            .identity()
            .operating_point
            .as_ref()
            .map(|point| point.outputs.clone()),
        Some(vec![9.0]),
        "the validated step and the Auto entry start from the present value"
    );
    let kept = anchor_integrating_outputs(&room(), &values, &outputs);
    assert_eq!(
        kept.identity()
            .operating_point
            .as_ref()
            .map(|point| point.outputs.clone()),
        Some(vec![12.0]),
        "a self-regulating output keeps its validated point"
    );
}

#[test]
fn an_output_that_does_not_settle_within_its_bound_is_reported_with_its_current_value() {
    // The TypeScript test: "reports an output that has not settled within the bound, with its current value".
    let loop_definition = cold_room();
    let model = room();
    let task = room_requirements();
    let inputs = ["P1.CompressorPct".to_owned()];
    let outputs = ["P1.RoomC".to_owned()];
    let start = StartingPoint {
        loop_definition: &loop_definition,
        model: &model,
        task: &task,
        input_symbols: &inputs,
        output_symbols: &outputs,
        input_tolerance: 1e-4,
    };
    let bound = wait_bound_ms(&task);
    let timed_out = start
        .wait_step(&reading(0.0, 9.0), bound + 1.0, bound, None)
        .expect_err("the output is still 3 degC away after the bound");
    assert!(
        timed_out
            .message
            .contains("P1.RoomC did not settle at 12 °C ± 0.5 °C"),
        "{}",
        timed_out.message
    );
    // A11: the message is the first sentence, and the detail keeps the whole text.
    let full = timed_out.detail.as_deref().unwrap_or_default();
    assert!(
        full.contains("it is 9 °C. Apply again once it has settled"),
        "{full}"
    );
    // Inside the band but not yet held for the dwell when the bound passes: the same refusal.
    let not_held = start
        .wait_step(&reading(0.0, 11.7), bound + 1.0, bound, Some(bound))
        .expect_err("the dwell has not run out when the bound has passed");
    assert!(
        not_held
            .message
            .contains("P1.RoomC did not settle at 12 °C ± 0.5 °C"),
        "{}",
        not_held.message
    );
}

#[test]
fn a_manual_apply_accepts_a_plant_input_within_the_auto_entry_allowance_and_refuses_one_beyond_it()
{
    // V4.6: a Manual Apply accepts a plant input within the Auto-entry allowance and refuses one beyond it (A6.4, B6.2).
    let loop_definition = proportional_loop();
    let model = first_order_model();
    let task = requirements();
    // The allowance of the proportional gain 2 with a measured noise of 0.05: 2 |Kp| n = 0.2 (A6.4).
    let gains = BTreeMap::from([("Main.Parameters.Kp".to_owned(), 2.0)]);
    let tolerance = auto_entry_tolerance(&loop_definition, &task, &gains, Some(0.05), None)
        .expect("the allowance");
    assert!((tolerance - 0.2).abs() < 1e-12);
    let inputs = ["ValvePosition".to_owned()];
    let outputs = ["LevelPV".to_owned()];
    let start = StartingPoint {
        loop_definition: &loop_definition,
        model: &model,
        task: &task,
        input_symbols: &inputs,
        output_symbols: &outputs,
        input_tolerance: tolerance,
    };
    let within = BTreeMap::from([
        ("ValvePosition".to_owned(), TransferValue::Number(0.1)),
        ("LevelPV".to_owned(), TransferValue::Number(2.0)),
    ]);
    // The output is inside its band at once, but the wait holds it for the settle dwell first (B6.2); the input is accepted.
    let held = start
        .wait_step(&within, 0.0, wait_bound_ms(&task), None)
        .expect("within the allowance");
    assert_eq!(inside_since(&held), Some(0.0));
    assert_eq!(
        start
            .wait_step(
                &within,
                settle_dwell_ms(&loop_definition, &task),
                wait_bound_ms(&task),
                Some(0.0)
            )
            .expect("within the allowance"),
        WaitStep::Settled
    );
    let beyond = BTreeMap::from([
        ("ValvePosition".to_owned(), TransferValue::Number(0.5)),
        ("LevelPV".to_owned(), TransferValue::Number(2.0)),
    ]);
    assert!(
        start
            .wait_step(&beyond, 0.0, wait_bound_ms(&task), None)
            .is_err(),
        "a plant input beyond the allowance cannot settle by itself"
    );
}
