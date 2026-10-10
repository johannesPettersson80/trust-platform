//! Ports of the `TypeScript` tests of the automatic test criteria (`control-studio-automatic-requirements.test.ts`) that
//! exercise the fill policy of this module. The `TypeScript` derives the policy's evidence from its test model (a gain of
//! 1.5 with a 0.5 s dead time, operating point 0, operating ranges of +/-20) on the `pidLoop()` fixture: a task of 20 ms,
//! output and input ranges of +/-20 and no rate limits. [`pid_evidence`] gives the same numbers. A test that runs the Tune
//! flow (`EngineerWorkflowSession`) also asserts the flow's candidate and stored task; those assertions belong to the flow
//! and are listed as FLOW in the coverage table, while the policy values they rest on are asserted here.

use super::*;

/// The dead time of the test model, in seconds (`delaySeconds` of the `TypeScript` test model).
const DELAY_SECONDS: f64 = 0.5;

/// The keys that the `TypeScript` test enters after Automatic generated them.
const ENTERED_KEYS: [&str; 5] = [
    "referenceChange",
    "maximumError",
    "settlingBand",
    "settlingTimeSeconds",
    "durationSeconds",
];

/// The refusal sentence that the `TypeScript` test's out-of-range change produces, up to the number that follows it.
const RANGE_SENTENCE: &str =
    "The test change of 30 m for Main.PV leaves the range the model was identified in (";

/// The evidence that the `TypeScript` derives for the test model with the given time constant, on the `pidLoop()`
/// fixture with a task of `period_seconds`. The timescale is the time constant, or ten task periods or twice the dead time
/// when those are longer. The response at the test's low frequency is the static gain of 1.5 to within 1e-5, and there is
/// no recording, so the baseline noise is zero.
fn pid_evidence(time_constant: f64, period_seconds: f64) -> FillEvidence {
    FillEvidence {
        model_revision: format!("model-{time_constant}"),
        output_symbol: "Main.PV".to_string(),
        output_unit: "m".to_string(),
        input_symbol: "Main.MV".to_string(),
        period_seconds,
        delay_seconds: DELAY_SECONDS,
        timescale_seconds: time_constant
            .max(10.0 * period_seconds)
            .max(2.0 * DELAY_SECONDS),
        operating_output: 0.0,
        operating_input: 0.0,
        output_minimum: -20.0,
        output_maximum: 20.0,
        input_minimum: -20.0,
        input_maximum: 20.0,
        input_rate_up_per_s: f64::INFINITY,
        input_rate_down_per_s: f64::INFINITY,
        low_frequency_real: 1.5,
        low_frequency_magnitude: 1.5,
        baseline_noise: 0.0,
    }
}

/// Asserts that a value is the expected one within an absolute tolerance of 1e-9.
fn assert_close(value: f64, expected: f64) {
    assert!(
        (value - expected).abs() <= 1e-9,
        "{value} is not {expected}"
    );
}

/// Whether two requirement records design the same test: the five values agree within 1e-9 and the response is the same.
fn same_design(first: &TuningRequirements, second: &TuningRequirements) -> bool {
    let values = [
        (first.reference_change, second.reference_change),
        (first.maximum_error, second.maximum_error),
        (first.settling_band, second.settling_band),
        (first.settling_time_seconds, second.settling_time_seconds),
        (first.duration_seconds, second.duration_seconds),
    ];
    values
        .iter()
        .all(|&(left, right)| (left - right).abs() <= 1e-9)
        && first.tuning_objective == second.tuning_objective
}

// control-studio-automatic-requirements: "derives a bounded model-scaled task without explicit requirement fields"
// The policy part of the TypeScript test; its Tune flow run (the candidate and the stored task) is listed as FLOW.
#[test]
fn a_slower_model_gets_a_later_settling_deadline_and_a_bounded_test() {
    let fast = fill_requirements(
        &pid_evidence(2.0, 0.02),
        SuppliedRequirements::default(),
        None,
    )
    .expect("the fast model's test is filled");
    let slow = fill_requirements(
        &pid_evidence(8.0, 0.02),
        SuppliedRequirements::default(),
        None,
    )
    .expect("the slow model's test is filled");
    assert!(slow.settling_time_seconds > fast.settling_time_seconds);
    assert!(
        slow.duration_seconds > slow.settling_time_seconds
            && slow.duration_seconds <= maximum_duration_seconds(),
        "the validation is longer than the settling time and within the limit: {} s",
        slow.duration_seconds
    );
    assert!(fast.settling_band > 0.0 && fast.maximum_error > fast.settling_band);
    let criteria = fast
        .automatic_criteria
        .as_ref()
        .expect("the generated values are marked");
    assert!(criteria.fields.iter().any(|field| field == "maximumError"));
    assert_eq!(criteria.model_revision, "model-2");
}

// control-studio-automatic-requirements: "derives a validation of a slow process longer than 100,000 task periods, which a record period keeps (A7, A4.2)"
#[test]
fn a_slow_process_gets_a_validation_longer_than_100000_periods_within_the_limit() {
    // A 75 min process at a 100 ms task: its validation needs more than the 10,000 s that 100,000 unaveraged periods would allow.
    // The limit is in seconds (specification 35 A7: 1 s to 24 h of process time), so the longer validation fits it.
    let slow = pid_evidence(4_500.0, 0.1);
    let filled = fill_requirements(&slow, SuppliedRequirements::default(), None)
        .expect("the slow process is tested");
    assert!(
        filled.duration_seconds > 10_000.0,
        "the test covers the slow process: {} s",
        filled.duration_seconds
    );
    assert!(filled.duration_seconds <= maximum_duration_seconds());
}

// control-studio-automatic-requirements: "keeps explicit process requirements and fills only missing settings"
// The policy part of the TypeScript test; its Tune flow run is listed as FLOW.
#[test]
fn explicit_process_requirements_are_kept_and_only_the_missing_settings_are_filled() {
    let supplied = SuppliedRequirements {
        reference_change: Some(2.0),
        maximum_error: Some(0.4),
        settling_band: Some(0.1),
        settling_time_seconds: Some(45.0),
        ..SuppliedRequirements::default()
    };
    let filled = fill_requirements(&pid_evidence(4.0, 0.02), supplied, None)
        .expect("the missing settings are filled");
    assert_close(filled.reference_change, 2.0);
    assert_close(filled.maximum_error, 0.4);
    assert_close(filled.settling_band, 0.1);
    assert_close(filled.settling_time_seconds, 45.0);
    assert!(
        filled.duration_seconds > 45.0,
        "{}",
        filled.duration_seconds
    );
    let fields = filled
        .automatic_criteria
        .expect("the duration and the response are generated")
        .fields;
    assert!(!fields.iter().any(|field| field == "maximumError"));
    assert!(fields.iter().any(|field| field == "durationSeconds"));
}

// control-studio-automatic-requirements: "fills Automatic's response choice as More robust also when every requirement was entered"
// The policy part of the TypeScript test: the entered record and the generated-then-entered record design the same
// requirements, and the response is More robust (Tune page, "Control requirements"). Its gain comparison is FLOW.
#[test]
fn a_fully_entered_record_and_a_generated_then_entered_one_design_the_same_requirements() {
    let evidence = pid_evidence(4.0, 0.02);
    let entered = SuppliedRequirements {
        reference_change: Some(2.0),
        maximum_error: Some(0.4),
        settling_band: Some(0.1),
        settling_time_seconds: Some(45.0),
        duration_seconds: Some(90.0),
        ..SuppliedRequirements::default()
    };
    let direct =
        fill_requirements(&evidence, entered, None).expect("the entered record is complete");
    assert_eq!(
        direct.tuning_objective,
        Some(TuningObjective::Robust),
        "the page declares no response choice, so Automatic fills More robust"
    );
    assert!(
        direct.automatic_criteria.is_none(),
        "a complete entered record generates nothing"
    );

    // The other order: Tune generates the criteria first, then every requirement is entered over them.
    let first_run = fill_requirements(&evidence, SuppliedRequirements::default(), None)
        .expect("the generated criteria");
    let generated = first_run
        .automatic_criteria
        .expect("the generated values are marked");
    let still_generated: Vec<String> = generated
        .fields
        .iter()
        .filter(|field| !ENTERED_KEYS.contains(&field.as_str()))
        .cloned()
        .collect();
    let previous = AutomaticCriteria {
        model_revision: generated.model_revision,
        fields: still_generated,
    };
    let after_generated = fill_requirements(&evidence, entered, Some(&previous))
        .expect("the entered values over the generated criteria");
    assert!(
        same_design(&direct, &after_generated),
        "the same requirements design the same controller in either order"
    );
    let still_marked = after_generated
        .automatic_criteria
        .expect("the response is still generated")
        .fields;
    assert_eq!(still_marked, ["tuningObjective"]);
}

// control-studio-automatic-requirements: "explains a test change beyond the identified range and how to fix it"
#[test]
fn a_test_change_beyond_the_identified_range_names_the_range_and_the_remedy() {
    let supplied = SuppliedRequirements {
        reference_change: Some(30.0),
        ..SuppliedRequirements::default()
    };
    let refusal = fill_requirements(&pid_evidence(4.0, 0.02), supplied, None)
        .expect_err("a change of 30 m leaves the range of +/-20 m");
    let sentence = refusal.0;
    assert!(sentence.contains(RANGE_SENTENCE), "{sentence}");
    assert!(
        sentence.contains("). Identify with a larger step, or choose a change of at most 20 m."),
        "{sentence}"
    );
}

// control-studio-automatic-requirements: "limits the automatically selected test step to actuator headroom"
// The policy part of the TypeScript test: its actuator limits of +/-0.5 are the input range of the fill.
#[test]
fn the_automatic_test_step_stays_within_the_actuator_headroom() {
    let limited = FillEvidence {
        input_minimum: -0.5,
        input_maximum: 0.5,
        ..pid_evidence(4.0, 0.02)
    };
    let filled = fill_requirements(&limited, SuppliedRequirements::default(), None)
        .expect("the step fits the headroom");
    assert!(
        filled.reference_change > 0.0 && filled.reference_change <= 0.075 + 1e-9,
        "{}",
        filled.reference_change
    );
}

// control-studio-automatic-requirements: "recalculates generated values for a new model while preserving manual accuracy"
// The policy part of the TypeScript test: every stored value is passed on, and only the generated ones follow the model.
#[test]
fn a_new_model_recalculates_the_generated_values_and_keeps_the_entered_accuracy() {
    let entered = SuppliedRequirements {
        maximum_error: Some(0.4),
        ..SuppliedRequirements::default()
    };
    let original =
        fill_requirements(&pid_evidence(2.0, 0.02), entered, None).expect("the first model's test");
    let stored = original
        .automatic_criteria
        .clone()
        .expect("the generated values are marked");
    // The policy fills again only the values that the record marks as generated (control-studio-tune-page.md, "Control requirements").
    let stored_values = SuppliedRequirements {
        reference_change: Some(original.reference_change),
        maximum_error: Some(original.maximum_error),
        settling_band: Some(original.settling_band),
        settling_time_seconds: Some(original.settling_time_seconds),
        duration_seconds: Some(original.duration_seconds),
        tuning_objective: original.tuning_objective,
        ..SuppliedRequirements::default()
    };
    let result = fill_requirements(&pid_evidence(8.0, 0.02), stored_values, Some(&stored))
        .expect("the new model's test");
    assert_close(result.maximum_error, 0.4);
    assert!(result.settling_time_seconds > original.settling_time_seconds);
    let revision = result
        .automatic_criteria
        .expect("the values stay generated")
        .model_revision;
    assert_eq!(revision, "model-8");
}

// control-studio-automatic-requirements: "uses available downward movement when the actuator is at its upper limit"
// The policy part of the TypeScript test: the operating input is at the top of its range, so only a downward test fits.
#[test]
fn the_test_steps_downward_when_the_actuator_is_at_its_upper_limit() {
    let at_limit = FillEvidence {
        operating_input: 0.5,
        input_minimum: -0.5,
        input_maximum: 0.5,
        ..pid_evidence(4.0, 0.02)
    };
    let filled = fill_requirements(&at_limit, SuppliedRequirements::default(), None)
        .expect("a downward test is feasible");
    assert!(
        filled.reference_change < 0.0,
        "the test picks a feasible direction instead of requesting unavailable upward actuation: {}",
        filled.reference_change
    );
}

/// B4.3: an overshoot row left blank declares no overshoot limit, and Tune never generates one; an overshoot the engineer
/// declared is kept as it is.
#[test]
fn tune_never_generates_an_overshoot_limit_and_keeps_a_declared_one() {
    let evidence = pid_evidence(4.0, 0.02);
    let filled = fill_requirements(&evidence, SuppliedRequirements::default(), None)
        .expect("the generated test");
    assert_eq!(filled.maximum_overshoot_percent, None);
    let fields = filled
        .automatic_criteria
        .expect("the generated values are marked")
        .fields;
    assert!(
        !fields
            .iter()
            .any(|field| field == "maximumOvershootPercent"),
        "no overshoot limit is generated: {fields:?}"
    );
    let declared = SuppliedRequirements {
        maximum_overshoot_percent: Some(5.0),
        ..SuppliedRequirements::default()
    };
    let kept =
        fill_requirements(&evidence, declared, None).expect("the test with a declared overshoot");
    assert_eq!(kept.maximum_overshoot_percent, Some(5.0));
}
