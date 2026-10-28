//! Tests of the wait for the starting point (`wait`; specification 35, B6.2, A3.2, A3.4 and A10.7) on scripted transfer readings. They
//! port the assertions of `control-studio-apply-settling.test.ts`, and those of `control-studio-apply-process-time.test.ts` and
//! `control-studio-commissioning-starting-point.test.ts` that test the engine's wait: the reading that continues, the plant input that
//! refuses at once, the cancel, the integrating output, the bound measured in process time, the connection loss and the progress shown
//! once. The decisions themselves are tested in trust-control (`starting_point_wait_tests`).

#![allow(clippy::float_cmp)] // The readings hold exact values, compared exactly on purpose.

use std::cell::Cell;

use serde_json::{json, Value};
use trust_control::commissioning::commissioning_result::{TransferState, TransferStatus};
use trust_control::commissioning::starting_point::{
    anchor_integrating_outputs, wait_bound_ms, wait_cancelled, StartingPoint,
};
use trust_control::commissioning::transfer::TransferValue;
use trust_control::failure::{Failure, NextAction};
use trust_control::loop_definition::LoopDefinition;
use trust_control::model::general::decode_general_model;
use trust_control::model::GeneralModel;
use trust_control::validation::engineer_verification_plan::{
    engineer_requirements, EngineerRequirements,
};

use super::await_starting_point;
use crate::workflow::apply::test_support::{
    instant, model, pid_loop, proportional_loop, task, Report,
};

/// The cold room of `control-studio-apply-settling.test.ts`: `P1.RoomC` in degC is the measured output, `P1.CompressorPct` in % the
/// plant input.
fn cold_room() -> LoopDefinition {
    let mut loop_definition = proportional_loop();
    loop_definition.signals.process_variable.symbol = "P1.RoomC".to_owned();
    loop_definition.signals.process_variable.unit = "degC".to_owned();
    loop_definition.signals.manipulated_variable.symbol = "P1.CompressorPct".to_owned();
    loop_definition.signals.manipulated_variable.unit = "%".to_owned();
    loop_definition
}

/// The room model of the settling test (`MODEL`), validated at 12 degC with the compressor at 0 %.
#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn room_model(numerator: Value, denominator: Value, revision: &str) -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": revision, "source": "provided", "validation": "validated",
        "evidence": ["bench"], "delaySeconds": 0, "numerator": numerator, "denominator": denominator,
        "inputs": [{ "id": "P1.CompressorPct", "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } }],
        "outputs": [{ "id": "P1.RoomC", "unit": "degC", "operatingRange": { "minimum": 2, "maximum": 12 } }],
        "operatingPoint": { "inputs": [0], "outputs": [12] }
    }))
    .expect("the room model decodes")
}

/// The requirements of the settling test (`REQUIREMENTS`): a 0.5 degC band, 90 s to settle in a 300 s test.
fn room_requirements() -> EngineerRequirements {
    engineer_requirements(&json!({
        "referenceChange": -7, "maximumError": 2, "settlingBand": 0.5, "settlingTimeSeconds": 90, "durationSeconds": 300
    }))
    .expect("the room requirements are valid")
}

/// One transfer reading of `input` and `output` at `values`, taken at `runtime_time_ns` of the runtime's clock (E6.3).
fn reading(values: &[(&str, f64)], runtime_time_ns: u64) -> TransferStatus {
    TransferStatus {
        program_revision: "program-1".into(),
        binding_revision: "transfer-1".into(),
        values: values
            .iter()
            .map(|(symbol, value)| ((*symbol).to_owned(), TransferValue::Number(*value)))
            .collect(),
        runtime_time_ns: Some(runtime_time_ns),
        status: TransferState::Idle,
        reason: None,
        supported_tracking: None,
        transaction: None,
    }
}

/// Waits on scripted readings of one plant input and one measured output, with the allowance of the exact transfer check (1e-6 of the
/// output span) and the given bound; returns the result and the number of readings taken.
fn wait_on(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    readings: &dyn Fn(usize) -> TransferStatus,
    bound_ms: f64,
    report: &Report,
    wall_ms: &dyn Fn() -> u64,
) -> (Result<TransferStatus, Failure>, usize) {
    let inputs = [loop_definition.signals.manipulated_variable.symbol.clone()];
    let outputs = [loop_definition.signals.process_variable.symbol.clone()];
    let reads = Cell::new(0_usize);
    let mut read = || -> Result<TransferStatus, Failure> {
        let at = reads.get();
        reads.set(at + 1);
        Ok(readings(at))
    };
    let step = |status: &TransferStatus, process_ms: f64, inside_since_ms: Option<f64>| {
        StartingPoint {
            loop_definition,
            model,
            task,
            input_symbols: &inputs,
            output_symbols: &outputs,
            input_tolerance: 1e-6 * 40.0,
        }
        .wait_step(&status.values, process_ms, bound_ms, inside_since_ms)
    };
    let waited = await_starting_point(&mut read, &step, report, &instant(wall_ms));
    (waited, reads.get())
}

/// TS control-studio-apply-settling, "waits for the measured output to settle at the validated point, showing its progress, then
/// continues" (B6.2): the room reads 10.3, 11.2 and then 11.7 degC, one reading every 100 ms, and stays at 11.7. The first reading inside
/// 12 ± 0.5 degC, at 200 ms, starts the 9 s settle dwell; the Apply continues with the reading that completes it, and the first progress
/// names the output with its validated and its current value.
#[test]
fn the_wait_continues_once_the_first_reading_inside_the_band_has_held_for_the_dwell() {
    let room = cold_room();
    let report = Report::default();
    let readings = |at: usize| {
        let temperature = [10.3, 11.2, 11.7][at.min(2)];
        reading(
            &[("P1.CompressorPct", 0.0), ("P1.RoomC", temperature)],
            u64::try_from(at).unwrap_or(0) * 100_000_000,
        )
    };
    let task = room_requirements();
    let (waited, reads) = wait_on(
        &room,
        &room_model(json!([-0.1]), json!([60, 1]), "room"),
        &task,
        &readings,
        wait_bound_ms(&task),
        &report,
        &|| 0_u64,
    );
    let settled = waited.expect("the room settles");
    assert_eq!(settled.values["P1.RoomC"], TransferValue::Number(11.7));
    // Readings at 0, 100 and 200 ms; the dwell of 9 s that starts at 200 ms ends at 9.2 s, which is the 93rd reading.
    assert_eq!(reads, 93);
    assert_eq!(
        report.lines()[0],
        "Waiting for P1.RoomC to settle at 12 °C ± 0.5 °C (now 10.3 °C)…"
    );
}

/// B6.2, the defect of the 10 October acceptance: the room touches its band at 0 s, leaves it at 100 ms and is back inside from 200 ms.
/// The touch does not end the wait: the dwell starts again at 200 ms, so the wait ends 9 s later, with the 93rd reading.
#[test]
fn a_reading_that_touches_the_band_once_does_not_end_the_wait() {
    let room = cold_room();
    let task = room_requirements();
    let readings = |at: usize| {
        let temperature = match at {
            0 => 11.7,
            1 => 10.0,
            _ => 11.8,
        };
        reading(
            &[("P1.CompressorPct", 0.0), ("P1.RoomC", temperature)],
            u64::try_from(at).unwrap_or(0) * 100_000_000,
        )
    };
    let (waited, reads) = wait_on(
        &room,
        &room_model(json!([-0.1]), json!([60, 1]), "room"),
        &task,
        &readings,
        wait_bound_ms(&task),
        &Report::default(),
        &|| 0_u64,
    );
    let settled = waited.expect("the room settles");
    assert_eq!(settled.values["P1.RoomC"], TransferValue::Number(11.8));
    assert_eq!(reads, 93, "the dwell restarts at 200 ms, not at the touch");
}

/// TS control-studio-apply-settling, "refuses at once when a plant input differs from the validated point, because the process cannot
/// settle by itself" (B6.2, B6.7: none until the plant input is back): one reading is taken, and nothing waits.
#[test]
fn the_wait_refuses_a_plant_input_away_from_its_point_after_one_reading() {
    let room = cold_room();
    let task = room_requirements();
    let readings = |_: usize| reading(&[("P1.CompressorPct", 100.0), ("P1.RoomC", 12.0)], 0);
    let (waited, reads) = wait_on(
        &room,
        &room_model(json!([-0.1]), json!([60, 1]), "room"),
        &task,
        &readings,
        wait_bound_ms(&task),
        &Report::default(),
        &|| 0_u64,
    );
    let refused = waited.expect_err("the input cannot settle by itself");
    assert!(
        refused
            .message
            .starts_with("P1.CompressorPct is 100 %, but the settings were validated from 0 %"),
        "{}",
        refused.message
    );
    assert_eq!(refused.next_action, NextAction::None);
    assert_eq!(reads, 1);
}

/// TS control-studio-apply-settling, "stops when cancelled, before any setting is written": the cancel is the engineer's own, reported
/// as "Apply cancelled while waiting for the process to settle; nothing was written.", and no reading is taken.
#[test]
fn a_cancelled_wait_stops_before_any_reading() {
    let room = cold_room();
    let task = room_requirements();
    let report = Report::default();
    report.cancel();
    let readings = |_: usize| reading(&[("P1.CompressorPct", 0.0), ("P1.RoomC", 9.0)], 0);
    let (waited, reads) = wait_on(
        &room,
        &room_model(json!([-0.1]), json!([60, 1]), "room"),
        &task,
        &readings,
        wait_bound_ms(&task),
        &report,
        &|| 0_u64,
    );
    let cancelled = waited.expect_err("cancelled");
    assert_eq!(cancelled, wait_cancelled());
    assert!(cancelled
        .detail
        .unwrap_or(cancelled.message)
        .contains("nothing was written"));
    assert_eq!(reads, 0);
}

/// TS control-studio-apply-settling, "an integrating output rests wherever it is, so Apply starts from its present value": with the
/// plant input at its validated value, the integrating tank's output at 9 degC needs no wait, and the validated step starts from 9 degC;
/// a self-regulating output keeps its validated 12 degC.
#[test]
fn an_integrating_output_needs_no_wait_and_anchors_the_start() {
    let room = cold_room();
    let task = room_requirements();
    let integrating = room_model(json!([0.01]), json!([1, 0]), "integrating");
    let readings = |_: usize| reading(&[("P1.CompressorPct", 0.0), ("P1.RoomC", 9.0)], 0);
    let (waited, reads) = wait_on(
        &room,
        &integrating,
        &task,
        &readings,
        20.0,
        &Report::default(),
        &|| 0_u64,
    );
    let settled = waited.expect("no wait");
    assert_eq!(reads, 1);
    let outputs = ["P1.RoomC".to_owned()];
    let anchored = anchor_integrating_outputs(&integrating, &settled.values, &outputs);
    assert_eq!(
        anchored
            .identity()
            .operating_point
            .as_ref()
            .map(|point| point.outputs.clone()),
        Some(vec![9.0])
    );
    let settling = room_model(json!([-0.1]), json!([60, 1]), "room");
    let kept = anchor_integrating_outputs(&settling, &settled.values, &outputs);
    assert_eq!(
        kept.identity()
            .operating_point
            .as_ref()
            .map(|point| point.outputs.clone()),
        Some(vec![12.0])
    );
}

/// TS control-studio-apply-settling, "reports an output that has not settled within the bound, with its current value": with a 20 ms bound
/// and readings 100 ms apart, the second reading ends the wait, naming the band and the current 9 degC.
#[test]
fn an_output_that_does_not_settle_within_the_bound_ends_the_wait() {
    let room = cold_room();
    let task = room_requirements();
    let readings = |at: usize| {
        reading(
            &[("P1.CompressorPct", 0.0), ("P1.RoomC", 9.0)],
            u64::try_from(at).unwrap_or(0) * 100_000_000,
        )
    };
    let (waited, reads) = wait_on(
        &room,
        &room_model(json!([-0.1]), json!([60, 1]), "room"),
        &task,
        &readings,
        20.0,
        &Report::default(),
        &|| 0_u64,
    );
    let refused = waited.expect_err("not settled");
    let text = refused.detail.clone().unwrap_or(refused.message.clone());
    assert!(
        text.starts_with("P1.RoomC did not settle at 12 °C ± 0.5 °C"),
        "{text}"
    );
    assert!(
        text.contains("it is 9 °C. Apply again once it has settled"),
        "{text}"
    );
    assert_eq!(reads, 2);
}

/// TS control-studio-apply-process-time, "the bound is measured in process time from the runtime's timestamps, not from the wall clock"
/// (B6.2, A3.2): readings one second apart on the runtime's clock, with the wall clock standing still, end the wait after 60 s of process
/// time (the longer of 60 s and three times the 4 s test), which takes at least 61 readings; the next action is Validate.
#[test]
fn the_bound_of_the_wait_is_measured_in_process_time() {
    let loop_definition = pid_loop();
    let readings = |at: usize| {
        reading(
            &[("Main.MV", 0.0), ("Main.PV", 9.0)],
            u64::try_from(at).unwrap_or(0) * 1_000_000_000,
        )
    };
    let task = task();
    let (waited, reads) = wait_on(
        &loop_definition,
        &model(),
        &task,
        &readings,
        wait_bound_ms(&task),
        &Report::default(),
        &|| 0_u64,
    );
    let refused = waited.expect_err("the bound ends the wait");
    let text = refused.detail.clone().unwrap_or(refused.message.clone());
    assert!(
        text.contains("did not settle at 2 m ± 0.02 m within 60 s of process time; it is 9 m."),
        "{text}"
    );
    assert_eq!(refused.next_action, NextAction::Validate);
    assert!(
        reads >= 61,
        "60 s at one second per reading: {reads} readings"
    );
}

/// TS control-studio-apply-process-time, "the wait ends with 'The runtime stopped sending values.' when no newer runtime value arrives for
/// 5 s" (A3.4): the runtime's clock stands still while this receiver's clock advances 2 s per look.
#[test]
fn the_wait_ends_when_the_runtime_stops_sending_values() {
    let loop_definition = pid_loop();
    let readings = |_: usize| reading(&[("Main.MV", 0.0), ("Main.PV", 9.0)], 0);
    let wall = Cell::new(0_u64);
    let clock = || {
        wall.set(wall.get() + 2000);
        wall.get()
    };
    let task = task();
    let (waited, _) = wait_on(
        &loop_definition,
        &model(),
        &task,
        &readings,
        wait_bound_ms(&task),
        &Report::default(),
        &clock,
    );
    let refused = waited.expect_err("the runtime stopped sending values");
    assert_eq!(refused, Failure::stopped_sending());
    assert_eq!(refused.next_action, NextAction::ConnectRuntime);
}

/// TS control-studio-commissioning-starting-point, "keeps progress messages light: ... only a changed reading is posted again" and
/// "continues to the Apply review without another click once the process settles" (B6.2, A10.7): three equal readings outside the band
/// show one progress; the readings inside the band show one more, and the wait continues by itself once they have held for the dwell.
#[test]
fn the_wait_shows_only_a_changed_progress_and_continues_by_itself() {
    let loop_definition = pid_loop();
    let report = Report::default();
    let readings = |at: usize| {
        let process = if at < 3 { 2.5 } else { 2.004 };
        reading(
            &[("Main.MV", 0.0), ("Main.PV", process)],
            u64::try_from(at).unwrap_or(0) * 100_000_000,
        )
    };
    let task = task();
    let (waited, reads) = wait_on(
        &loop_definition,
        &model(),
        &task,
        &readings,
        wait_bound_ms(&task),
        &report,
        &|| 0_u64,
    );
    assert!(waited.is_ok());
    // The readings from 300 ms are inside the band; their dwell is 400 ms (20 executions of 20 ms, a tenth of the 2 s settling time),
    // so the 8th reading, at 700 ms, ends the wait.
    assert_eq!(reads, 8);
    assert_eq!(
        report.lines(),
        [
            "Waiting for Main.PV to settle at 2 m ± 0.02 m (now 2.5 m)…",
            "Main.PV settled at 2 m ± 0.02 m (now 2 m)…",
        ]
    );
}
