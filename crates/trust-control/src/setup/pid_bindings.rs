//! The parameter record and the controller states of a library `Control.PID` (specification 35, C0 and C1, §17.1). Ported from
//! `pidBindings.ts`: the bounds of each record field, the seven retained states, the startup-done flag and the manual output that
//! follows the output. The startup-done rule (C1 Setup) is here as well, because the PID and the program's own law share it.

use super::catalog::{pid_candidates, CompiledCatalog, PID_PARAMETERS};
use super::loop_bindings::MeasuredSignal;
use super::texts::generators as texts;
use crate::loop_definition::{
    InitialSource, InitialValue, InitializedState, Parameter, PostScan, RealType, StateType,
};

/// The parameter record of a `Control.PID`: its eleven numeric fields, each with its unit, its convention and the bounds Tune may
/// choose from (C0; `pidParameters`). The units are the stated units of the signals (see `stated_signal`).
#[must_use]
pub fn pid_parameters(
    record: &str,
    process_variable: &MeasuredSignal,
    manipulated_variable: &MeasuredSignal,
) -> Vec<Parameter> {
    PID_PARAMETERS
        .iter()
        .map(|field| {
            let (unit, minimum, maximum) =
                field_bounds(field, process_variable, manipulated_variable);
            Parameter {
                symbol: format!("{record}.{field}"),
                iec_type: RealType::Lreal,
                unit,
                convention: format!("Control.PID {field}"),
                minimum,
                maximum,
            }
        })
        .collect()
}

/// The unit and bounds of one record field (`pidParameters`). Gains are signed, since their sign follows the action; the bias is an
/// offset that may shift the output by its span either way; the rate limits admit the Plant library's ±1e9, which Control.PID takes
/// as no rate limit.
fn field_bounds(
    field: &str,
    process_variable: &MeasuredSignal,
    manipulated_variable: &MeasuredSignal,
) -> (String, f64, f64) {
    let output = manipulated_variable.unit.as_str();
    let process = process_variable.unit.as_str();
    let span = manipulated_variable.maximum - manipulated_variable.minimum;
    match field {
        "ProportionalGain" => (format!("{output}/{process}"), -1e6, 1e6),
        "IntegralGain" => (format!("{output}/({process}*s)"), -1e6, 1e6),
        "DerivativeGain" => (format!("{output}*s/{process}"), -1e6, 1e6),
        "ProportionalSetpointWeight" | "DerivativeSetpointWeight" => ("1".to_string(), 0.0, 1.0),
        "Bias" => (output.to_string(), -span, span),
        "OutputMinimum" | "OutputMaximum" => (
            output.to_string(),
            manipulated_variable.minimum,
            manipulated_variable.maximum,
        ),
        "RateMinimum" => (format!("{output}/s"), -1e9, 0.0),
        "RateMaximum" => (format!("{output}/s"), 0.0, 1e9),
        // The eleventh field, TrackingTime.
        _ => ("s".to_string(), 0.001, 1e4),
    }
}

/// The seven retained states of `Control.PID` (C1, the `control-pid-v1` table), each bound to its operating-point coordinate on the
/// model's `channel` (`pidStates`). The setpoint and the measurement start at the model output, the rates and the feedforward at
/// zero, and the two outputs at the model input. Each is an `LREAL` with bounds, bounded after the first scan.
#[must_use]
pub fn pid_states(
    instance: &str,
    process_variable: &MeasuredSignal,
    manipulated_variable: &MeasuredSignal,
    channel: usize,
) -> Vec<InitializedState> {
    let process_span = process_variable.maximum - process_variable.minimum;
    let span = manipulated_variable.maximum - manipulated_variable.minimum;
    let process_unit = process_variable.unit.as_str();
    let output_unit = manipulated_variable.unit.as_str();
    let state = |field: &str, unit: String, minimum: f64, maximum: f64, initial: InitialValue| {
        InitializedState {
            symbol: format!("{instance}.StateValue.{field}"),
            iec_type: StateType::Lreal,
            unit,
            minimum,
            maximum,
            initial,
            post_scan: PostScan::Bounded,
        }
    };
    let model_output = || InitialValue::From(InitialSource::ModelOutput { index: channel });
    let model_input = || InitialValue::From(InitialSource::ModelInput { index: channel });
    vec![
        state(
            "Setpoint",
            process_unit.to_string(),
            process_variable.minimum,
            process_variable.maximum,
            model_output(),
        ),
        state(
            "SetpointRate",
            format!("{process_unit}/s"),
            -1000.0 * process_span,
            1000.0 * process_span,
            InitialValue::Constant(0.0),
        ),
        state(
            "ProcessValue",
            process_unit.to_string(),
            process_variable.minimum,
            process_variable.maximum,
            model_output(),
        ),
        state(
            "ProcessValueRate",
            format!("{process_unit}/s"),
            -1000.0 * process_span,
            1000.0 * process_span,
            InitialValue::Constant(0.0),
        ),
        state(
            "FeedForward",
            output_unit.to_string(),
            -span,
            span,
            InitialValue::Constant(0.0),
        ),
        state(
            "NominalOutput",
            output_unit.to_string(),
            manipulated_variable.minimum - 10.0 * span,
            manipulated_variable.maximum + 10.0 * span,
            model_input(),
        ),
        state(
            "SaturatedOutput",
            output_unit.to_string(),
            manipulated_variable.minimum,
            manipulated_variable.maximum,
            model_input(),
        ),
    ]
}

/// The startup-done flag: a BOOL the program sets when its startup code has run. Validation starts it TRUE, so that the startup code
/// keeps the seeded values (C1 Setup; `startupState`).
#[must_use]
pub fn startup_state(flag: &str) -> InitializedState {
    InitializedState {
        symbol: flag.to_string(),
        iec_type: StateType::Bool,
        unit: "1".to_string(),
        minimum: 0.0,
        maximum: 1.0,
        initial: InitialValue::Constant(1.0),
        post_scan: PostScan::Unchanged,
    }
}

/// Whether the code copies the PID's output into `manual_output`, so that Manual starts where Auto left off (`manualOutputTracking`,
/// C1 Setup). After an Auto scan that manual output is the output, not the value a transfer wrote.
#[must_use]
pub fn manual_output_tracking(
    catalog: &CompiledCatalog,
    instance: &str,
    manual_output: &str,
) -> bool {
    pid_candidates(catalog)
        .iter()
        .find(|candidate| candidate.instance == instance)
        .is_some_and(|candidate| {
            candidate.manual_output_follows_output
                && candidate.manual_output.as_deref() == Some(manual_output)
        })
}

/// The reason the loop's program needs the startup-done flag (C1 Setup, B1.3), or `None` when it does not. The program needs one when
/// it calls `Initialize` on the loop's PID (`initialized_instance`, a `Control.PID` only), or when it writes a symbol that validation
/// seeds: the first of `seeded` that it writes, else the first write into one of the parameter `records`. The TypeScript form decided
/// this in its webview (`loopSetupForm.ts`, `seededWrite`); the rule is Rust's now.
#[must_use]
pub fn startup_reason(
    catalog: &CompiledCatalog,
    initialized_instance: Option<&str>,
    seeded: &[String],
    records: &[String],
) -> Option<String> {
    if let Some(instance) = initialized_instance {
        let initialized = catalog
            .links
            .iter()
            .any(|link| link.instance == instance && link.method.to_uppercase() == "INITIALIZE");
        if initialized {
            return Some(texts::program_initializes(instance));
        }
    }
    let written = |symbol: &str| catalog.writes.iter().any(|write| write == symbol);
    if let Some(symbol) = seeded.iter().find(|symbol| written(symbol)) {
        return Some(texts::program_writes(symbol));
    }
    catalog
        .writes
        .iter()
        .find(|write| {
            records
                .iter()
                .any(|record| write.starts_with(format!("{record}.").as_str()))
        })
        .map(|write| texts::program_writes(write))
}
