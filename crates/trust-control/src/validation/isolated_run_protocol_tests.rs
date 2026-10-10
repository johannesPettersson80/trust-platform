#![allow(clippy::float_cmp)] // The tests compare the grid's times and the run's values exactly, as the runtime writes them (E8.3).

use serde_json::{json, Value};

use super::*;

fn observation(symbol: &str, unit: &str) -> IsolatedRunObservation {
    IsolatedRunObservation {
        symbol: symbol.into(),
        unit: unit.into(),
    }
}

fn channels(count: usize) -> Vec<IsolatedRunObservation> {
    (0..count)
        .map(|index| observation(&format!("Main.Ch{index}"), "1"))
        .collect()
}

/// A valid request: 500 ms at 100 ms, two channels, no overrides, no relay and no selected model.
fn request() -> IsolatedRunRequest {
    IsolatedRunRequest {
        request_id: "run-1".into(),
        expected_project_revision: "sha256:project".into(),
        execution: IsolatedExecution::IsolatedSimulation,
        step_ms: 100,
        duration_ms: 500,
        seed: 7,
        max_total_cycles: 6,
        wall_budget_ms: 5_000,
        record_every: 1,
        initial_values: BTreeMap::new(),
        events: Vec::new(),
        observations: vec![observation("Main.PV", "degC"), observation("Main.Out", "%")],
        parameter_overrides: Vec::new(),
        simulation_override: None,
        feedback_relay: None,
    }
}

fn parameter(
    symbol: &str,
    iec_type: IsolatedParameterType,
    minimum: f64,
    maximum: f64,
    value: f64,
) -> IsolatedParameterOverride {
    IsolatedParameterOverride {
        symbol: symbol.into(),
        iec_type,
        unit: "%".into(),
        convention: "gain".into(),
        minimum,
        maximum,
        value,
    }
}

fn relay() -> IsolatedFeedbackRelay {
    IsolatedFeedbackRelay {
        process_variable: "Main.PV".into(),
        command: "Main.Out".into(),
        center: 50.0,
        hysteresis: 2.0,
        low: 0.0,
        high: 100.0,
        restore: 0.0,
        process_minimum: 0.0,
        process_maximum: 100.0,
        positive_gain: true,
        start_ms: 100,
        restore_ms: 400,
        minimum_high_ms: None,
        minimum_low_ms: None,
    }
}

/// The message of the refusal of `request`, which must be refused.
fn refusal(request: &IsolatedRunRequest) -> String {
    request.check().expect_err("the request is refused").message
}

/// The message of the refusal of a status answer for `request`, which must decode and be refused.
fn status_refusal(
    answer: Value,
    request: &IsolatedRunRequest,
    include_results: Option<bool>,
) -> String {
    decode_status(answer)
        .expect("the answer decodes")
        .check(request, include_results)
        .expect_err("the answer is refused")
        .message
}

/// The compact run the runtime sends for `request` with its first `samples` grid points (A7, E8.4): two values per
/// sample, with no quality array because every value is good.
fn compact_run(request: &IsolatedRunRequest, samples: usize) -> Value {
    let grid = recorded_sample_times(request.step_ms, request.duration_ms, request.record_every);
    let frames: Vec<Value> = grid
        .iter()
        .take(samples)
        .enumerate()
        .map(|(index, time)| json!({"sequence": index, "timeSeconds": time, "values": [1.5, 2.5]}))
        .collect();
    let observed: Vec<Value> = request
        .observations
        .iter()
        .map(|channel| json!({"id": channel.symbol, "unit": channel.unit}))
        .collect();
    json!({
        "id": "run-0",
        "evidence": "ST-executed",
        "parameters": {},
        "conditions": {
            "initialState": request.initial_values,
            "events": [],
            "durationSeconds": seconds_of_ms(request.duration_ms),
            "sampleTimesSeconds": grid,
            "seed": request.seed,
            "modelRevision": "sha256:model",
            "programRevision": "sha256:program",
            "parameterSchemaRevision": "sha256:schema"
        },
        "channels": observed,
        "samples": frames
    })
}

/// The status answer for `request` in `state`, with the captured frame count and `run` when it has one (E8.4).
fn answer(request: &IsolatedRunRequest, state: &str, captured: usize, run: Option<Value>) -> Value {
    let mut status = json!({
        "job_id": request.request_id,
        "project_revision": request.expected_project_revision,
        "status": state,
        "cancel_requested": false,
        "reason": null,
        "source_basis": "loaded-runtime-sources",
        "io_provenance": "isolated-simulation-no-physical-drivers",
        "captured_sample_count": captured
    });
    if let Some(run) = run {
        status["run"] = run;
    }
    status
}

#[test]
fn a_valid_request_passes_its_checks() {
    request().check().expect("the request is valid");
}

#[test]
fn the_step_is_one_to_one_thousand_milliseconds() {
    let mut fastest = request();
    fastest.step_ms = 1;
    fastest.max_total_cycles = 501;
    fastest.check().expect("a 1 ms step is valid");
    let mut slowest = request();
    slowest.step_ms = 1_000;
    slowest.duration_ms = 1_000;
    slowest.max_total_cycles = 2;
    slowest.check().expect("a 1,000 ms step is valid");
    let mut stopped = request();
    stopped.step_ms = 0;
    assert!(refusal(&stopped).contains("The step is 0 ms"));
    let mut too_slow = request();
    too_slow.step_ms = 1_001;
    too_slow.duration_ms = 1_001;
    assert!(refusal(&too_slow).contains("The step is 1001 ms"));
}

#[test]
fn the_duration_is_a_positive_multiple_of_the_step_up_to_a_day() {
    let mut day = request();
    day.step_ms = 1_000;
    day.duration_ms = 86_400_000;
    day.max_total_cycles = 86_401;
    day.check().expect("a day at 1 s is valid");
    let mut longer = day.clone();
    longer.duration_ms = 86_401_000;
    longer.max_total_cycles = 86_402;
    assert!(refusal(&longer).contains("The duration is 86401000 ms"));
    let mut off_grid = request();
    off_grid.duration_ms = 150;
    assert!(refusal(&off_grid).contains("The duration is 150 ms"));
    let mut empty = request();
    empty.duration_ms = 0;
    assert!(refusal(&empty).contains("The duration is 0 ms"));
}

#[test]
fn the_seed_is_at_most_two_to_the_fifty_three_minus_one() {
    let mut largest = request();
    largest.seed = 9_007_199_254_740_991;
    largest.check().expect("the largest integer is valid");
    let mut beyond = request();
    beyond.seed = 9_007_199_254_740_992;
    assert!(refusal(&beyond).contains("9,007,199,254,740,991"));
}

#[test]
fn the_wall_budget_is_one_to_six_hundred_thousand_milliseconds() {
    for (budget, valid) in [(0, false), (1, true), (600_000, true), (600_001, false)] {
        let mut candidate = request();
        candidate.wall_budget_ms = budget;
        assert_eq!(candidate.check().is_ok(), valid, "wall budget {budget} ms");
    }
}

#[test]
fn the_record_period_is_one_to_one_hundred_thousand_executions() {
    for (every, valid) in [(0, false), (1, true), (100_000, true), (100_001, false)] {
        let mut candidate = request();
        candidate.record_every = every;
        assert_eq!(candidate.check().is_ok(), valid, "record period {every}");
    }
}

#[test]
fn the_cycles_are_at_least_the_grid_points_and_at_most_ten_million() {
    request().check().expect("the grid's own scans are enough");
    let mut short = request();
    short.max_total_cycles = 5;
    assert!(refusal(&short).contains("the run needs 6 scans"));
    let mut largest = request();
    largest.step_ms = 1;
    largest.duration_ms = 9_999_999;
    largest.record_every = 100;
    largest.max_total_cycles = 10_000_000;
    largest
        .check()
        .expect("ten million scans, sampled every 100 executions, are valid");
    largest.max_total_cycles = 10_000_001;
    assert!(refusal(&largest).contains("at most 10,000,000"));
}

#[test]
fn the_samples_of_a_run_are_at_most_one_hundred_thousand() {
    let mut over = request();
    over.step_ms = 1;
    over.duration_ms = 100_000;
    over.max_total_cycles = 100_001;
    assert!(refusal(&over).contains("Validation would record 100001 samples"));
    over.record_every = 2;
    over.check()
        .expect("averaging two scans per sample fits the sample limit");
}

#[test]
fn the_samples_times_channels_are_at_most_one_point_six_million() {
    let mut fits = request();
    fits.observations = channels(64);
    fits.step_ms = 100;
    fits.duration_ms = 2_499_900;
    fits.max_total_cycles = 25_000;
    fits.check()
        .expect("25,000 samples of 64 channels are 1.6 million values");
    let mut over = fits.clone();
    over.duration_ms = 2_500_000;
    over.max_total_cycles = 25_001;
    assert!(refusal(&over).contains("25001 samples × 64 channels"));
}

#[test]
fn the_observations_are_one_to_sixty_four_unique_symbols_with_units() {
    let mut none = request();
    none.observations.clear();
    assert!(refusal(&none).contains("observes no channel"));
    let mut many = request();
    many.observations = channels(65);
    assert!(refusal(&many).contains("records 65 channels"));
    let mut twice = request();
    twice.observations = vec![
        observation("Main.PV", "degC"),
        observation("Main.PV", "degC"),
    ];
    assert!(refusal(&twice).contains("listed twice"));
    let mut unitless = request();
    unitless.observations[0].unit.clear();
    assert!(refusal(&unitless).contains("observed unit"));
    let mut one = request();
    one.observations = channels(1);
    one.check().expect("one channel is valid");
    let mut sixty_four = request();
    sixty_four.observations = channels(64);
    sixty_four.check().expect("64 channels are valid");
}

#[test]
fn the_request_identifier_is_one_to_128_ascii_letters_digits_or_hyphens() {
    let mut candidate = request();
    for (id, valid) in [("run-1", true), ("", false), ("a b", false), ("é", false)] {
        candidate.request_id = id.into();
        assert_eq!(candidate.check().is_ok(), valid, "identifier {id:?}");
    }
    candidate.request_id = "a".repeat(128);
    candidate.check().expect("128 bytes are valid");
    candidate.request_id = "a".repeat(129);
    assert!(refusal(&candidate).contains("must be 1 to 128"));
}

#[test]
fn the_run_names_the_project_revision_it_was_reviewed_on() {
    let mut unnamed = request();
    unnamed.expected_project_revision.clear();
    assert!(refusal(&unnamed).contains("names no project revision"));
}

#[test]
fn the_parameter_overrides_are_at_most_sixty_four_with_unique_symbols() {
    let mut candidate = request();
    candidate.parameter_overrides = (0..64)
        .map(|index| {
            parameter(
                &format!("Main.P{index}"),
                IsolatedParameterType::Lreal,
                0.0,
                100.0,
                1.0,
            )
        })
        .collect();
    candidate.check().expect("64 overrides are valid");
    candidate.parameter_overrides.push(parameter(
        "Main.Extra",
        IsolatedParameterType::Lreal,
        0.0,
        100.0,
        1.0,
    ));
    assert!(refusal(&candidate).contains("overrides 65 parameters"));
    let mut twice = request();
    twice.parameter_overrides = vec![
        parameter("Main.Kp", IsolatedParameterType::Lreal, 0.0, 100.0, 1.0),
        parameter("main.kp", IsolatedParameterType::Lreal, 0.0, 100.0, 1.0),
    ];
    assert!(refusal(&twice).contains("overridden twice"));
}

#[test]
fn a_real_override_is_exact_in_single_precision_and_inside_its_bounds() {
    let mut candidate = request();
    candidate.parameter_overrides = vec![parameter(
        "Main.Kp",
        IsolatedParameterType::Real,
        0.0,
        100.0,
        0.5,
    )];
    candidate.check().expect("0.5 is exact in single precision");
    candidate.parameter_overrides = vec![parameter(
        "Main.Kp",
        IsolatedParameterType::Real,
        0.0,
        100.0,
        0.1,
    )];
    assert!(refusal(&candidate).contains("not exactly representable as single precision"));
    candidate.parameter_overrides = vec![parameter(
        "Main.Kp",
        IsolatedParameterType::Lreal,
        0.0,
        100.0,
        0.1,
    )];
    candidate
        .check()
        .expect("a LREAL value is exact in double precision");
    candidate.parameter_overrides = vec![parameter(
        "Main.Kp",
        IsolatedParameterType::Lreal,
        0.0,
        100.0,
        101.0,
    )];
    assert!(refusal(&candidate).contains("outside its bounds 0 to 100"));
    candidate.parameter_overrides = vec![parameter(
        "Main.Kp",
        IsolatedParameterType::Lreal,
        0.0,
        100.0,
        f64::NAN,
    )];
    assert!(refusal(&candidate).contains("not a finite number"));
    candidate.parameter_overrides = vec![parameter(
        "Main.Kp",
        IsolatedParameterType::Lreal,
        50.0,
        10.0,
        20.0,
    )];
    assert!(refusal(&candidate).contains("minimum of 50 above its maximum of 10"));
}

#[test]
fn a_bool_override_is_a_fixed_zero_or_one_on_unit_one_with_bounds_zero_to_one() {
    let mut enable = parameter("Main.Enable", IsolatedParameterType::Bool, 0.0, 1.0, 1.0);
    enable.unit = "1".into();
    let mut candidate = request();
    candidate.parameter_overrides = vec![enable.clone()];
    candidate.check().expect("a fixed BOOL of 1 is valid");
    let mut half = enable.clone();
    half.value = 0.5;
    candidate.parameter_overrides = vec![half];
    assert!(refusal(&candidate).contains("it is 0 or 1"));
    let mut seconds = enable.clone();
    seconds.unit = "s".into();
    candidate.parameter_overrides = vec![seconds];
    assert!(refusal(&candidate).contains("its unit is 1"));
    let mut wide = enable;
    wide.maximum = 2.0;
    candidate.parameter_overrides = vec![wide];
    assert!(refusal(&candidate).contains("its bounds are 0 to 1"));
}

#[test]
fn initial_values_are_finite_and_never_set_a_parameter() {
    let mut candidate = request();
    candidate.initial_values.insert("Main.PV".into(), 20.0);
    candidate.check().expect("an initial value is valid");
    candidate.initial_values.insert("Main.NaN".into(), f64::NAN);
    assert!(refusal(&candidate).contains("not a finite number"));
    candidate.initial_values.remove("Main.NaN");
    candidate.parameter_overrides = vec![parameter(
        "Main.Kp",
        IsolatedParameterType::Lreal,
        0.0,
        100.0,
        1.0,
    )];
    candidate.initial_values.insert("main.KP".into(), 1.0);
    assert!(refusal(&candidate).contains("would set a parameter that the run overrides"));
}

#[test]
fn events_lie_on_the_scan_grid_in_time_order_and_never_set_a_parameter() {
    let event = |at_ms: u64, symbol: &str, value: f64| IsolatedRunEvent {
        at_ms,
        symbol: symbol.into(),
        value,
    };
    let mut candidate = request();
    candidate.events = vec![event(200, "Main.Setpoint", 30.0)];
    candidate.check().expect("an event on a scan is valid");
    candidate.events = vec![event(250, "Main.Setpoint", 30.0)];
    assert!(refusal(&candidate).contains("is not on a scan of 100 ms"));
    candidate.events = vec![event(600, "Main.Setpoint", 30.0)];
    assert!(refusal(&candidate).contains("lies after the end of the run at 500 ms"));
    candidate.events = vec![event(300, "Main.A", 1.0), event(200, "Main.B", 1.0)];
    assert!(refusal(&candidate).contains("not in time order"));
    candidate.events = vec![event(200, "Main.A", 1.0), event(200, "main.a", 2.0)];
    assert!(refusal(&candidate).contains("two events on main.a at 200 ms"));
    candidate.events = vec![event(200, "Main.A", f64::INFINITY)];
    assert!(refusal(&candidate).contains("not a finite number"));
    candidate.parameter_overrides = vec![parameter(
        "Main.A",
        IsolatedParameterType::Lreal,
        0.0,
        100.0,
        1.0,
    )];
    candidate.events = vec![event(200, "Main.A", 1.0)];
    assert!(refusal(&candidate).contains("would set a parameter that the run overrides"));
}

#[test]
fn the_feedback_relay_keeps_its_bounds_and_observes_its_signals() {
    let mut candidate = request();
    candidate.feedback_relay = Some(relay());
    candidate.check().expect("the relay is valid");
    let mut same = candidate.clone();
    same.feedback_relay.as_mut().expect("a relay").command = "main.pv".into();
    assert!(refusal(&same).contains("must be different signals"));
    let mut no_hysteresis = candidate.clone();
    no_hysteresis
        .feedback_relay
        .as_mut()
        .expect("a relay")
        .hysteresis = 0.0;
    assert!(refusal(&no_hysteresis).contains("needs a hysteresis above 0"));
    let mut outside = candidate.clone();
    outside.feedback_relay.as_mut().expect("a relay").restore = 150.0;
    assert!(refusal(&outside).contains("needs a hysteresis above 0"));
    let mut band = candidate.clone();
    band.feedback_relay.as_mut().expect("a relay").center = 1.0;
    assert!(refusal(&band).contains("switching band must lie inside"));
    let mut unaligned = candidate.clone();
    unaligned.feedback_relay.as_mut().expect("a relay").start_ms = 150;
    assert!(refusal(&unaligned).contains("multiples of the step of 100 ms"));
    let mut late = candidate.clone();
    late.feedback_relay.as_mut().expect("a relay").restore_ms = 600;
    assert!(refusal(&late).contains("starts at 100 ms and restores at 600 ms"));
    let mut reversed = candidate.clone();
    reversed.feedback_relay.as_mut().expect("a relay").start_ms = 400;
    assert!(refusal(&reversed).contains("starts at 400 ms and restores at 400 ms"));
    let mut dwell = candidate.clone();
    dwell
        .feedback_relay
        .as_mut()
        .expect("a relay")
        .minimum_high_ms = Some(300);
    assert!(refusal(&dwell).contains("minimum dwell times must be shorter"));
    let mut unobserved = candidate.clone();
    unobserved.observations = vec![observation("Main.PV", "degC")];
    assert!(refusal(&unobserved).contains("must observe its measurement and its command"));
    let mut commanded_parameter = candidate;
    commanded_parameter.parameter_overrides = vec![parameter(
        "Main.Out",
        IsolatedParameterType::Lreal,
        0.0,
        100.0,
        1.0,
    )];
    assert!(refusal(&commanded_parameter).contains("cannot be a parameter override or an event"));
}

fn selected(revision: &str, provenance: &str, configuration: &str) -> SimulationOverride {
    SimulationOverride {
        model_revision: revision.into(),
        model_provenance: provenance.into(),
        simulation_toml: configuration.into(),
    }
}

#[test]
fn a_selected_model_has_bounded_revision_provenance_and_configuration() {
    let mut candidate = request();
    candidate.simulation_override = Some(selected("sha256:model", "fit", "[simulation]"));
    candidate
        .check()
        .expect("a bounded selected model is valid");
    candidate.simulation_override = Some(selected("", "fit", "[simulation]"));
    assert!(refusal(&candidate).contains("revision is 0 bytes"));
    candidate.simulation_override = Some(selected(&"r".repeat(257), "fit", "[simulation]"));
    assert!(refusal(&candidate).contains("revision is 257 bytes"));
    candidate.simulation_override = Some(selected("r", &"p".repeat(4_097), "[simulation]"));
    assert!(refusal(&candidate).contains("provenance is 4097 bytes"));
    candidate.simulation_override = Some(selected("r", "p", &"t".repeat(262_145)));
    assert!(refusal(&candidate).contains("configuration is 262145 bytes"));
}

#[test]
fn a_start_answers_for_the_request_identifier_and_revision() {
    let asked = request();
    let accepted = IsolatedRunStart {
        job_id: "run-1".into(),
        status: IsolatedJobState::Queued,
        project_revision: "sha256:project".into(),
    };
    accepted.check(&asked).expect("the start names the request");
    let other_job = IsolatedRunStart {
        job_id: "run-2".into(),
        ..accepted.clone()
    };
    assert_eq!(
        other_job.check(&asked).expect_err("refused").message,
        "Runtime returned an invalid verification identity."
    );
    let other_revision = IsolatedRunStart {
        project_revision: "sha256:other".into(),
        ..accepted
    };
    assert!(other_revision.check(&asked).is_err());
}

#[test]
fn a_status_answers_for_the_request_job_and_revision_only() {
    let asked = request();
    let mut other_job = answer(&asked, "running", 0, None);
    other_job["job_id"] = json!("run-2");
    assert!(status_refusal(other_job, &asked, Some(false))
        .contains("Verification identity or revision changed during execution."));
    let mut other_revision = answer(&asked, "running", 0, None);
    other_revision["project_revision"] = json!("sha256:other");
    assert!(
        status_refusal(other_revision, &asked, Some(false)).contains("changed during execution")
    );
}

#[test]
fn a_status_keeps_isolated_simulation_provenance() {
    let asked = request();
    let mut source = answer(&asked, "running", 0, None);
    source["source_basis"] = json!("physical-target");
    assert!(status_refusal(source, &asked, Some(false)).contains("untrusted verification evidence"));
    let mut physical = answer(&asked, "running", 0, None);
    physical["io_provenance"] = json!("physical-drivers");
    assert!(
        status_refusal(physical, &asked, Some(false)).contains("untrusted verification evidence")
    );
}

#[test]
fn a_thin_status_carries_no_run_and_a_run_waits_for_the_end() {
    let asked = request();
    let run = compact_run(&asked, 6);
    assert!(status_refusal(
        answer(&asked, "completed", 6, Some(run.clone())),
        &asked,
        Some(false)
    )
    .contains("did not ask for results"));
    assert!(
        status_refusal(answer(&asked, "running", 2, Some(run)), &asked, Some(true))
            .contains("before the job ended")
    );
    decode_status(answer(&asked, "queued", 0, None))
        .expect("the answer decodes")
        .check(&asked, Some(false))
        .expect("a queued status without a run is valid");
    decode_status(answer(&asked, "completed", 6, None))
        .expect("the answer decodes")
        .check(&asked, Some(false))
        .expect("a thin answer of a completed job carries no run by design");
    assert!(
        status_refusal(answer(&asked, "completed", 6, None), &asked, None)
            .contains("incomplete or untrusted verification evidence")
    );
}

#[test]
fn a_completed_run_keeps_the_requested_grid_conditions_and_channels() {
    let asked = request();
    let status = decode_status(answer(&asked, "completed", 6, Some(compact_run(&asked, 6))))
        .expect("the answer decodes");
    status
        .check(&asked, Some(true))
        .expect("the run is the request's run");
    assert!(status.evidence().validates());
    let IsolatedRunEvidence::Complete(run) = status.evidence() else {
        panic!("a completed run is complete evidence")
    };
    assert_eq!(run.samples.len(), 6);
    assert_eq!(run.samples[3].values["Main.Out"].value, Some(2.5));
    assert_eq!(run.samples[3].values["Main.Out"].quality, Quality::Good);
    assert_eq!(status.captured_sample_count, 6);
}

#[test]
fn a_completed_run_with_a_different_grid_or_conditions_is_refused() {
    let asked = request();
    let mut moved = compact_run(&asked, 6);
    moved["conditions"]["sampleTimesSeconds"][1] = json!(0.2);
    assert!(status_refusal(
        answer(&asked, "completed", 6, Some(moved)),
        &asked,
        Some(true)
    )
    .contains("sample times do not match"));
    let mut longer = compact_run(&asked, 6);
    longer["conditions"]["durationSeconds"] = json!(0.6);
    assert!(status_refusal(
        answer(&asked, "completed", 6, Some(longer)),
        &asked,
        Some(true)
    )
    .contains("does not match the frozen verification request"));
    let mut reseeded = compact_run(&asked, 6);
    reseeded["conditions"]["seed"] = json!(8);
    assert!(status_refusal(
        answer(&asked, "completed", 6, Some(reseeded)),
        &asked,
        Some(true)
    )
    .contains("does not match the frozen verification request"));
    let mut reordered = compact_run(&asked, 6);
    reordered["channels"] =
        json!([{"id": "Main.Out", "unit": "%"}, {"id": "Main.PV", "unit": "degC"}]);
    assert!(status_refusal(
        answer(&asked, "completed", 6, Some(reordered)),
        &asked,
        Some(true)
    )
    .contains("does not match the frozen verification request"));
    let mut initialised = compact_run(&asked, 6);
    initialised["conditions"]["initialState"] = json!({"Main.PV": 3.0});
    assert!(status_refusal(
        answer(&asked, "completed", 6, Some(initialised)),
        &asked,
        Some(true)
    )
    .contains("does not match the frozen verification request"));
    let mut averaged = compact_run(&asked, 6);
    averaged["conditions"]["recordEvery"] = json!(2);
    assert!(status_refusal(
        answer(&asked, "completed", 6, Some(averaged)),
        &asked,
        Some(true)
    )
    .contains("averages a different number of scans"));
    let mut predicted = compact_run(&asked, 6);
    predicted["evidence"] = json!("model-predicted");
    assert!(status_refusal(
        answer(&asked, "completed", 6, Some(predicted)),
        &asked,
        Some(true)
    )
    .contains("requires ST-executed runtime evidence"));
}

#[test]
fn a_completed_run_must_hold_every_grid_point() {
    let asked = request();
    let short = answer(&asked, "completed", 3, Some(compact_run(&asked, 3)));
    assert!(status_refusal(short, &asked, Some(true))
        .contains("holds 3 samples; its time grid has 6 points"));
    let without_run = answer(&asked, "completed", 0, None);
    assert!(status_refusal(without_run, &asked, Some(true))
        .contains("incomplete or untrusted verification evidence"));
}

#[test]
fn a_run_that_ended_early_is_prefix_evidence_that_never_validates() {
    let asked = request();
    let failed = decode_status(answer(&asked, "failed", 2, Some(compact_run(&asked, 2))))
        .expect("the answer decodes");
    failed
        .check(&asked, Some(true))
        .expect("the prefix is the request's run");
    assert!(matches!(
        failed.evidence(),
        IsolatedRunEvidence::Prefix(run) if run.samples.len() == 2
    ));
    assert!(!failed.evidence().validates());
    let cancelled = decode_status(answer(&asked, "cancelled", 1, Some(compact_run(&asked, 1))))
        .expect("the answer decodes");
    cancelled
        .check(&asked, Some(true))
        .expect("the cancelled prefix is the request's run");
    assert!(!cancelled.evidence().validates());
    let before_first = answer(&asked, "failed", 0, Some(compact_run(&asked, 0)));
    assert!(status_refusal(before_first, &asked, Some(true)).contains("holds no complete sample"));
    let ended_early = decode_status(answer(&asked, "failed", 0, None)).expect("the answer decodes");
    ended_early
        .check(&asked, Some(true))
        .expect("a run that ended before its first sample has no run to check");
    assert_eq!(ended_early.evidence(), IsolatedRunEvidence::NoRun);
    let running = decode_status(answer(&asked, "running", 0, None)).expect("the answer decodes");
    assert_eq!(running.evidence(), IsolatedRunEvidence::Pending);
    let thin = decode_status(answer(&asked, "completed", 6, None)).expect("the answer decodes");
    assert_eq!(thin.evidence(), IsolatedRunEvidence::NotRequested);
    assert!(!thin.evidence().validates());
}

#[test]
fn the_captured_count_is_the_number_of_frames_received() {
    let asked = request();
    let captured = answer(&asked, "failed", 3, Some(compact_run(&asked, 2)));
    assert!(status_refusal(captured, &asked, Some(true))
        .contains("counts 3 captured samples, but its run holds 2"));
}

#[test]
fn compact_samples_expand_into_values_with_their_quality() {
    let asked = request();
    let mut run = compact_run(&asked, 2);
    run["samples"][1] =
        json!({"sequence": 1, "timeSeconds": 0.1, "values": [null, 2.5], "quality": [3, 0]});
    let failed = decode_status(answer(&asked, "failed", 2, Some(run))).expect("the answer decodes");
    failed
        .check(&asked, Some(true))
        .expect("the prefix holds the request's grid");
    let IsolatedRunEvidence::Prefix(record) = failed.evidence() else {
        panic!("a failed run with samples is a prefix")
    };
    let missing = record.samples[1].values["Main.PV"];
    assert_eq!(missing.value, None);
    assert_eq!(missing.quality, Quality::Missing);
}

#[test]
fn the_readiness_answer_names_the_revision_only_when_isolated_runs_are_ready() {
    let ready = json!({
        "ready": true,
        "project_revision": "sha256:project",
        "source_basis": "loaded-runtime-sources",
        "execution": "isolated-simulation",
        "supports_request_id": true
    });
    assert_eq!(
        ready_project_revision(ready.clone()).expect("the runtime is ready"),
        "sha256:project"
    );
    let waiting = json!({"ready": false, "reason": "Start the local analysis runtime."});
    assert_eq!(
        ready_project_revision(waiting)
            .expect_err("the runtime is not ready")
            .message,
        "Start the local analysis runtime."
    );
    let unnamed = json!({"ready": false});
    assert!(ready_project_revision(unnamed)
        .expect_err("the runtime is not ready")
        .message
        .contains("not ready for verification"));
    let mut without_identity = ready.clone();
    without_identity["supports_request_id"] = json!(false);
    assert!(ready_project_revision(without_identity)
        .expect_err("the identity support is missing")
        .message
        .contains("recoverable isolated-operation identity support"));
    let mut other_basis = ready;
    other_basis["source_basis"] = json!("physical-target");
    assert!(ready_project_revision(other_basis)
        .expect_err("the basis is not loaded sources")
        .message
        .contains("not ready for verification"));
}

#[test]
fn the_grid_is_the_scan_times_at_the_record_period() {
    assert_eq!(scan_count(100, 500), 6);
    assert_eq!(
        recorded_sample_times(100, 500, 1),
        vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5]
    );
    // Three scans per sample: the means of scans 0 to 2 and 3 to 5. Four scans per sample: the last sample averages
    // the two scans that remain.
    assert_eq!(recorded_sample_times(100, 500, 3), vec![0.1, 0.4]);
    assert_eq!(recorded_sample_times(100, 500, 4), vec![0.15, 0.45]);
}

#[test]
fn only_a_job_that_has_ended_is_terminal() {
    assert!(!IsolatedJobState::Queued.is_terminal());
    assert!(!IsolatedJobState::Running.is_terminal());
    assert!(IsolatedJobState::Completed.is_terminal());
    assert!(IsolatedJobState::Cancelled.is_terminal());
    assert!(IsolatedJobState::Failed.is_terminal());
}
