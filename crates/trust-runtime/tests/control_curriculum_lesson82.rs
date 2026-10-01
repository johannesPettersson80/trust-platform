use trust_runtime::io::IoAddress;
use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

const LESSON82: &str = "14-integrated-capstones/84-independent";

fn flag(runtime: &trust_runtime::Runtime, address: &str) -> bool {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::Bool(value) => value,
        other => panic!("expected BOOL at {address}, got {other:?}"),
    }
}

/// Lesson 89: the production line's heater, 10 kW/°C with 1 s delivery delay, under the real
/// Control.PID with a feedforward of the measured draw-off. Control Studio identifies, tunes, validates and applies the
/// design; the program starts from candidate A, the PI with Kp = 2.5 %/°C and Ki = 0.5 %/(°C·s) and no feedforward.
#[test]
fn lesson82_the_heater_rests_at_35_c_with_75_percent_and_no_draw_off_under_candidate_a() {
    let (mut runtime, mut simulation) = setup(LESSON82);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..10 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
        assert_eq!(
            number(&runtime, "%QL0"),
            75.0,
            "Manual holds the holding command 7.5 · 10 = 75 %"
        );
        assert_eq!(number(&runtime, "%QL8"), 0.0, "no draw-off");
        assert!(
            (number(&runtime, "%IL0") - 35.0).abs() < 1e-9,
            "the heater rests at 35 °C"
        );
    }
    assert_eq!(
        program_record_number(&runtime, program, "Parameters", "ProportionalGain"),
        2.5
    );
    assert_eq!(
        program_record_number(&runtime, program, "Parameters", "IntegralGain"),
        0.5
    );
    assert_eq!(
        program_number(&runtime, program, "FeedforwardGain"),
        0.0,
        "candidate A has no feedforward"
    );
    assert!(
        (controller_state(&runtime, program, "SaturatedOutput") - 75.0).abs() < 1e-9,
        "Control.PID holds 75 %"
    );
}

/// Starts the heater in Auto with the feedforward gain Control Studio applied, then measures a 10 kW draw-off: returns the
/// command before and in the update the draw-off is measured, the heater's extraction, and the command 6 s later.
fn lesson82_draw_off(feedforward_gain: f64) -> (f64, f64, f64, f64) {
    let (mut runtime, mut simulation) = setup(LESSON82);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    scan(&mut runtime, &mut simulation, &debug, 0, &[]);
    debug.enqueue_instance_write(program, "FeedforwardGain", Value::LReal(feedforward_gain));
    debug.enqueue_instance_write(program, "Auto", Value::Bool(true));
    scan(&mut runtime, &mut simulation, &debug, 100, &[]);
    let before = number(&runtime, "%QL0");
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        200,
        &[("%ML8", Value::LWord(10.0_f64.to_bits()))],
    );
    let measured = number(&runtime, "%QL0");
    let extraction = number(&runtime, "%QL8");
    for step in 3..=62 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
    }
    (before, measured, extraction, number(&runtime, "%QL0"))
}

#[test]
fn lesson82_candidate_b_feeds_the_measured_draw_off_forward_at_once() {
    let (before, measured, extraction, _) = lesson82_draw_off(0.5);
    assert_eq!(extraction, 10.0, "the measured draw-off reaches the heater");
    assert!(
        (measured - before - 5.0).abs() < 1e-9,
        "0.5 %/kW · 10 kW adds 5 points in the same update: {before} → {measured}"
    );
}

#[test]
fn lesson82_candidate_a_waits_for_the_temperature_to_fall() {
    let (before, measured, extraction, later) = lesson82_draw_off(0.0);
    assert!(
        (before - 75.0).abs() < 1e-9,
        "Auto at rest keeps 75 %, got {before}"
    );
    assert_eq!(extraction, 10.0, "the measured draw-off reaches the heater");
    assert!(
        (measured - before).abs() < 1e-9,
        "the PI alone does not move before the water cools, got {measured}"
    );
    assert!(
        later > before + 1.0,
        "6 s later the PI has raised the command, got {later}"
    );
}

#[test]
fn lesson82_an_invalid_measurement_latches_zero_heat_and_ends_the_draw_off() {
    let (mut runtime, mut simulation) = setup(LESSON82);
    let debug = runtime.enable_debug();
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        0,
        &[("%ML8", Value::LWord(10.0_f64.to_bits()))],
    );
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        100,
        &[("%IL0", Value::LWord(1000.0_f64.to_bits()))],
    );
    assert_eq!(
        number(&runtime, "%QL0"),
        0.0,
        "an invalid measurement removes heat"
    );
    assert_eq!(number(&runtime, "%QL8"), 0.0, "and ends the draw-off");
    assert!(flag(&runtime, "%MX32.0"), "the abort latches");
    scan(&mut runtime, &mut simulation, &debug, 200, &[]);
    assert_eq!(
        number(&runtime, "%QL0"),
        0.0,
        "the latched abort keeps the heat off with a valid measurement"
    );
}
