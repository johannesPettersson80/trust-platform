//! The loop contract's rules for a project experiment and for the records and signals around it (specification 35, §17.1, A5, B1.4 and
//! C12): a project experiment's baseline, hold parameter, activation and abort variable, the parameter record of the controller or of
//! the experiment, the typed Boolean manual output, the further signals, and the keys of removed analyses. Each test pins the rules the
//! page's loop validator (`loopDefinition.ts`) held, which the engine applies when it opens the loop.

use serde_json::{json, Value};

use super::tests::{complete_loop, decode, with};
use super::*;

/// The activation variable of the project experiments below: a written BOOL with a direct address.
fn activation() -> Value {
    json!({ "symbol": "P1.Run", "address": "%MX64.0", "iecType": "BOOL", "direction": "write" })
}

/// The scope the complete loop is opened for.
fn scope() -> LoopScope {
    LoopScope {
        project_id: "project:sha256:abc".into(),
        runtime_id: "runtime:local".into(),
    }
}

/// Asserts that `value` is refused when the loop loads, with a reason that says `sentence`.
fn refusal_says(value: &Value, sentence: &str) {
    match decode(value) {
        Err(LoopRejection::Malformed(text)) => {
            assert!(text.contains(sentence), "{sentence:?} is not in {text:?}");
        }
        other => panic!("expected a refusal that says {sentence:?}, got {other:?}"),
    }
}

/// Asserts that `value` is refused when the loop loads, whatever the reason.
fn is_refused(value: &Value) {
    assert!(
        matches!(decode(value), Err(LoopRejection::Malformed(_))),
        "the loop is refused"
    );
}

/// The complete loop without its controller law, whose record the loops below replace.
fn lawless_loop() -> Value {
    let mut value = complete_loop();
    value["controller"]
        .as_object_mut()
        .expect("the controller")
        .remove("law");
    value
}

/// A project-scripted observation of 35.1 s with its activation and a 5 s baseline.
fn baseline_loop() -> Value {
    with(
        complete_loop(),
        &["experiment"],
        json!({
            "execution": "project-scripted", "kind": "observation", "amplitude": 0, "durationSeconds": 35.1,
            "seed": 1, "baselineSeconds": 5, "activation": activation()
        }),
    )
}

#[test]
fn a_baseline_belongs_to_an_activated_project_experiment_and_is_shorter_than_it() {
    decode(&baseline_loop()).expect("a 5 s baseline of a 35.1 s project experiment is accepted");
    for baseline in [json!(-1), json!(35.1), json!(40)] {
        let refused = with(
            baseline_loop(),
            &["experiment", "baselineSeconds"],
            baseline,
        );
        refusal_says(
            &refused,
            "baseline belongs to an activated project experiment",
        );
    }
    let mut unactivated = baseline_loop();
    unactivated["experiment"]
        .as_object_mut()
        .expect("the experiment")
        .remove("activation");
    refusal_says(
        &unactivated,
        "baseline belongs to an activated project experiment",
    );
    let studio = with(
        unactivated,
        &["experiment", "execution"],
        json!("control-studio"),
    );
    refusal_says(
        &studio,
        "baseline belongs to an activated project experiment",
    );
}

/// A project step test whose hold parameter is declared in seconds: 20 to 40 s, in a loop sampled every 100 ms.
fn hold_loop() -> Value {
    let mut value = lawless_loop();
    value["controller"]["taskPeriodMs"] = json!(100);
    value["observedSampleTiming"] = json!({ "nominalPeriodMs": 100 });
    value["experiment"] = json!({
        "execution": "project-scripted", "kind": "observation", "amplitude": 0, "durationSeconds": 45.1, "seed": 1,
        "baselineSeconds": 5, "holdParameter": "P1.HoldSeconds", "activation": activation()
    });
    value["parameterPurpose"] = json!("experiment");
    value["parameterBinding"] = json!({
        "loop_id": "loop:tank", "task_name": "MainTask",
        "parameters": [{ "symbol": "P1.HoldSeconds", "iec_type": "REAL", "unit": "s", "convention": "held step duration",
            "minimum": 20, "maximum": 40 }],
        "mode": { "symbol": "P1.Run", "manual_value": false }
    });
    value
}

#[test]
fn a_hold_parameter_is_a_declared_experiment_parameter_in_seconds_the_duration_can_record() {
    decode(&hold_loop()).expect("a 45.1 s duration records a 5 s baseline and a 40 s hold");
    let short = with(hold_loop(), &["experiment", "durationSeconds"], json!(35.1));
    refusal_says(&short, "cannot record the longest permitted hold");
    let undeclared = with(
        hold_loop(),
        &["experiment", "holdParameter"],
        json!("P1.Other"),
    );
    refusal_says(
        &undeclared,
        "hold parameter is a declared experiment parameter in seconds",
    );
    let minutes = with(
        hold_loop(),
        &["parameterBinding", "parameters"],
        json!([{ "symbol": "P1.HoldSeconds", "iec_type": "REAL", "unit": "min", "convention": "held step duration",
            "minimum": 20, "maximum": 40 }]),
    );
    refusal_says(
        &minutes,
        "hold parameter is a declared experiment parameter in seconds",
    );
    let controller_record = with(hold_loop(), &["parameterPurpose"], json!("controller"));
    refusal_says(
        &controller_record,
        "hold parameter is a declared experiment parameter in seconds",
    );
    let mut studio = with(
        hold_loop(),
        &["experiment", "execution"],
        json!("control-studio"),
    );
    let experiment = studio["experiment"]
        .as_object_mut()
        .expect("the experiment");
    experiment.remove("activation");
    experiment.remove("baselineSeconds");
    refusal_says(
        &studio,
        "experiment parameters need a project experiment with an activation",
    );
}

/// A project experiment with an activation and a distinct abort variable that asserts TRUE.
fn abort_loop() -> Value {
    with(
        complete_loop(),
        &["experiment"],
        json!({
            "execution": "project-scripted", "kind": "observation", "amplitude": 0, "durationSeconds": 30, "seed": 1,
            "activation": { "symbol": "Main.Run", "address": "%MX64.0", "iecType": "BOOL", "direction": "write" },
            "abort": { "symbol": "Main.Abort", "address": "%MX64.1", "iecType": "BOOL", "direction": "write",
                "assertedValue": true }
        }),
    )
}

#[test]
fn an_abort_is_a_distinct_written_bool_with_its_asserted_value_beside_an_activation() {
    decode(&abort_loop()).expect("a distinct abort with its asserted value is accepted");
    let mut without_activation = abort_loop();
    without_activation["experiment"]
        .as_object_mut()
        .expect("the experiment")
        .remove("activation");
    refusal_says(&without_activation, "an abort needs an activation");
    let mut unasserted = abort_loop();
    unasserted["experiment"]["abort"]
        .as_object_mut()
        .expect("the abort")
        .remove("assertedValue");
    refusal_says(
        &unasserted,
        "an abort is a distinct variable with its asserted value",
    );
    let same_address = with(
        abort_loop(),
        &["experiment", "abort", "address"],
        json!("%MX64.0"),
    );
    refusal_says(
        &same_address,
        "an abort is a distinct variable with its asserted value",
    );
    let same_symbol = with(
        abort_loop(),
        &["experiment", "abort", "symbol"],
        json!("Main.Run"),
    );
    refusal_says(
        &same_symbol,
        "an abort is a distinct variable with its asserted value",
    );
    for (key, change) in [("iecType", json!("REAL")), ("direction", json!("read"))] {
        let wrong = with(abort_loop(), &["experiment", "abort", key], change);
        refusal_says(&wrong, "is a written BOOL with an address");
    }
}

#[test]
fn an_activation_is_a_written_bool_with_a_direct_address() {
    let sequence = with(
        complete_loop(),
        &["experiment"],
        json!({
            "execution": "project-scripted", "kind": "observation", "amplitude": 0, "durationSeconds": 40, "seed": 20_260_829,
            "activation": { "symbol": "P1.ExperimentEnabled", "address": "%MX96.0", "iecType": "BOOL", "direction": "write" }
        }),
    );
    decode(&sequence).expect("one typed Boolean authority is accepted");
    let typed_real = with(
        sequence.clone(),
        &["experiment", "activation", "iecType"],
        json!("LREAL"),
    );
    refusal_says(&typed_real, "is a written BOOL with an address");
    let read = with(
        sequence.clone(),
        &["experiment", "activation", "direction"],
        json!("read"),
    );
    refusal_says(&read, "is a written BOOL with an address");
    let unaddressed = with(
        sequence,
        &["experiment", "activation", "address"],
        json!("P1.ExperimentEnabled"),
    );
    refusal_says(&unaddressed, "is a written BOOL with an address");
}

/// A loop with a generic parameter record of one real field, and no controller law that would name its fields.
fn record_loop() -> Value {
    let mut value = lawless_loop();
    value["parameterBinding"] = json!({
        "loop_id": "loop:tank", "task_name": "MainTask",
        "parameters": [{ "symbol": "P1.Gain", "iec_type": "REAL", "unit": "1", "convention": "proportional gain",
            "minimum": 0, "maximum": 20 }],
        "mode": { "symbol": "P1.Enabled", "manual_value": false }
    });
    value
}

#[test]
fn a_parameter_record_names_its_loop_and_declares_distinct_bounded_real_fields() {
    decode(&record_loop()).expect("a record of one bounded real field is accepted");
    refusal_says(
        &with(
            record_loop(),
            &["parameterBinding", "loop_id"],
            json!("other"),
        ),
        "belongs to another loop",
    );
    is_refused(&with(
        record_loop(),
        &["parameterBinding", "unknown"],
        json!(true),
    ));
    refusal_says(
        &with(
            record_loop(),
            &["parameterBinding", "parameters"],
            json!([]),
        ),
        "1 to 64 parameters",
    );
    let gain = record_loop()["parameterBinding"]["parameters"][0].clone();
    refusal_says(
        &with(
            record_loop(),
            &["parameterBinding", "parameters"],
            json!([gain.clone(), gain.clone()]),
        ),
        "is bound twice",
    );
    refusal_says(
        &with(
            record_loop(),
            &["parameterBinding", "mode", "symbol"],
            json!("P1.Gain"),
        ),
        "is bound twice",
    );
    for iec_type in ["DINT", "UNKNOWN"] {
        let mut typed = gain.clone();
        typed["iec_type"] = json!(iec_type);
        is_refused(&with(
            record_loop(),
            &["parameterBinding", "parameters"],
            json!([typed]),
        ));
    }
    let mut reversed = gain;
    reversed["minimum"] = json!(21);
    refusal_says(
        &with(
            record_loop(),
            &["parameterBinding", "parameters"],
            json!([reversed]),
        ),
        "needs a range",
    );
}

/// A project experiment whose parameters are the experiment's, bound to the activation as their mode.
fn purpose_loop() -> Value {
    let mut value = record_loop();
    value["parameterPurpose"] = json!("experiment");
    value["parameterBinding"]["mode"] =
        json!({ "symbol": "P1.ExperimentEnabled", "manual_value": false });
    value["experiment"] = json!({
        "execution": "project-scripted", "kind": "observation", "amplitude": 0, "durationSeconds": 30, "seed": 1,
        "activation": { "symbol": "P1.ExperimentEnabled", "address": "%MX80.0", "iecType": "BOOL", "direction": "write" }
    });
    value
}

#[test]
fn experiment_parameters_belong_to_an_activated_project_experiment_and_are_bound_to_its_activation()
{
    decode(&purpose_loop()).expect("experiment parameters of an activated project experiment");
    is_refused(&with(
        purpose_loop(),
        &["parameterPurpose"],
        json!("tuning"),
    ));
    let mut unactivated = purpose_loop();
    unactivated["experiment"]
        .as_object_mut()
        .expect("the experiment")
        .remove("activation");
    refusal_says(
        &unactivated,
        "experiment parameters need a project experiment with an activation",
    );
    refusal_says(
        &with(
            purpose_loop(),
            &["parameterBinding", "mode", "manual_value"],
            json!(true),
        ),
        "whose Manual value is FALSE",
    );
    refusal_says(
        &with(
            purpose_loop(),
            &["parameterBinding", "mode", "symbol"],
            json!("P1.Other"),
        ),
        "bound to the activation",
    );
    is_refused(&with(
        purpose_loop(),
        &["experiment", "execution"],
        json!("host"),
    ));
}

#[test]
fn a_controller_the_project_designs_loads_and_validates() {
    let project_owned = with(
        with(
            complete_loop(),
            &["controller", "type"],
            json!("ColdStorageOnOff"),
        ),
        &["controller", "design"],
        json!("project"),
    );
    decode(&project_owned)
        .expect("a project-owned non-PID controller loads")
        .validate(&scope())
        .expect("it validates against the loop's scope");
}

/// A further signal of a channel, as the loop file declares it.
fn auxiliary(
    id: &str,
    channel: &str,
    role: &str,
    address: &str,
    unit: &str,
    direction: &str,
) -> Value {
    json!({ "id": id, "channel": channel, "role": role, "symbol": id, "address": address, "iecType": "LREAL",
        "direction": direction, "unit": unit, "range": { "minimum": -1000, "maximum": 1000 } })
}

#[test]
fn one_typed_set_of_further_signals_carries_mimo_state_disturbance_schedule_and_quality_evidence() {
    let mut quality = auxiliary(
        "zone-b-quality",
        "zone-b",
        "quality",
        "%IX64.0",
        "1",
        "read",
    );
    quality["iecType"] = json!("BOOL");
    let value = with(
        complete_loop(),
        &["signals", "additional"],
        json!([
            auxiliary("zone-b-sp", "zone-b", "SP", "%ML32", "degC", "read"),
            auxiliary("zone-b-pv", "zone-b", "PV", "%IL32", "degC", "read"),
            auxiliary("zone-b-mv", "zone-b", "MV", "%QL32", "%", "write"),
            auxiliary("jacket-state", "reactor", "state", "%IL40", "degC", "read"),
            auxiliary(
                "feed-flow",
                "reactor",
                "disturbance",
                "%QL48",
                "kg/s",
                "write"
            ),
            auxiliary("level-schedule", "tank", "schedule", "%IL56", "m", "read"),
            quality
        ]),
    );
    decode(&value)
        .expect("the set loads")
        .validate(&scope())
        .expect("the set validates");
}

#[test]
fn a_boolean_state_of_the_controller_is_a_further_signal_with_a_zero_to_one_range() {
    let mut automatic = auxiliary(
        "automatic-mode",
        "controller",
        "state",
        "%MX72.0",
        "1",
        "read",
    );
    automatic["iecType"] = json!("BOOL");
    automatic["range"] = json!({ "minimum": 0, "maximum": 1 });
    let value = with(
        complete_loop(),
        &["signals", "additional"],
        json!([automatic]),
    );
    decode(&value)
        .expect("a Boolean state loads")
        .validate(&scope())
        .expect("a Boolean state validates");
}

#[test]
fn a_boolean_manual_output_needs_an_on_off_controller_and_a_boolean_manipulated_variable() {
    let value = with(
        with(
            complete_loop(),
            &["controller", "manualOutputSignal"],
            json!("Main.ManualCommand"),
        ),
        &["controller", "manualOutputType"],
        json!("BOOL"),
    );
    let loaded = decode(&value).expect("the declaration loads");
    assert_eq!(
        loaded.validate(&scope()),
        Err(LoopRejection::IncompatibleSignalType(
            "manipulatedVariable".into()
        )),
        "a PID with a REAL output has no Boolean manual output"
    );
}

#[test]
fn the_keys_of_the_removed_saturation_and_anti_windup_analyses_are_refused_by_name() {
    for key in ["saturationEvidence", "antiWindupVerification"] {
        let legacy = with(
            complete_loop(),
            &["controller", key],
            json!({ "requestedOutputId": "requested", "integralStateId": "integral" }),
        );
        is_refused(&legacy);
        is_refused(&with(
            complete_loop(),
            &["controller", key],
            json!("not an adapter"),
        ));
    }
}
