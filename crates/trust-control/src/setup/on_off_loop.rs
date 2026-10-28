//! The loop definition of a switching (ON/OFF) controller that the program runs (specification 35, C3 and B1.3; ported from
//! `loopSetup/onOffLoopGenerator.ts`, with the choices that `configureOnOffController.ts` asks). The generator names the controller's
//! three settings, its two output levels (D-11), its threshold policy (D-10) and its retained states, and it writes the switched output's
//! channel with the equipment's minimum times when the engineer declared them (D-9). Tune designs the settings; Validate runs them.
//!
//! The loop is built as a loop file and decoded by the loop contract, so every rule the loader checks holds for a generated loop too.

#![allow(clippy::module_name_repetitions)] // `OnOffLoopDraft` names the module's subject, as the TypeScript does.

use serde::Deserialize;
use serde_json::{json, Value};

use super::catalog::CompiledCatalog;
use super::loop_bindings::{
    binding, compiled, cyclic_task, explicit_experiment, form_number, loop_id, manual_output_type,
    measured_signal, measurement_binding, real_type, with_revision, FormSignal, MeasuredSignal,
};
use super::pid_bindings::startup_state;
use super::pid_loop::{setpoint_signal, ModeChoice, SignalChoice};
use super::state_feedback_loop::check_startup;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    Action, Direction, Execution, ExperimentKind, LoopDefinition, LoopScope, ModeWhenTrue,
    RealType, DWELL_TIMER_MAXIMUM_SECONDS, LOOP_SCHEMA_VERSION, MINIMUM_TIME_MAXIMUM_SECONDS,
};

/// The refusal of a latch that is not a BOOL: the latch holds whether the output is on (C3).
const LATCH_NOT_BOOL: &str = "The output latch must be a BOOL: it holds whether the output is on.";
/// The refusal of a draft without a name, an instance or a switched output, which the form asks for (B1.3).
const CHOICES_NEEDED: &str = "Name the loop, choose the program instance, and choose the measurement and the switched output.";

/// A switching controller of the program as the setup form confirms it (C3, §17.1): the instance, the switched output and the measurement,
/// the three settings, the output levels, the threshold policy and the retained states.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OnOffLoopDraft {
    /// The loop's name as the engineer reads it.
    pub name: String,
    /// Direct or reverse: the direction of the controller's action.
    pub action: Action,
    /// The compiled program or function block instance that runs the switching logic.
    pub instance: String,
    /// The measured process variable, with its unit and range.
    pub process_variable: FormSignal,
    /// The setpoint the program reads; its unit and range default to the measurement's.
    pub setpoint: SignalChoice,
    /// The numeric output that the program switches between its two levels.
    pub manipulated_variable: FormSignal,
    /// The Auto/Manual signal, and the mode its TRUE value means.
    pub mode: ModeChoice,
    /// The manual output the Manual mode writes.
    pub manual_output: String,
    /// The hysteresis setting's symbol, in the measurement's unit.
    pub hysteresis: String,
    /// `half-band` when the hysteresis is the half-width of the band, `full-band` when it is the full width.
    pub band_convention: String,
    /// The minimum ON time's symbol, in seconds.
    pub minimum_on: String,
    /// The minimum OFF time's symbol, in seconds.
    pub minimum_off: String,
    /// The BOOL that holds whether the output is on.
    pub latch: String,
    /// The dwell timer's symbol, when the program counts the seconds in the current state.
    pub dwell_timer: Option<String>,
    /// The switched equipment's own minimum ON time in seconds, zero when it has none; absent when the engineer has not declared it. The
    /// form sends the text typed (`form_number`), and a text that no number reads is refused under its field.
    #[serde(default, deserialize_with = "form_number")]
    pub equipment_minimum_on_seconds: Option<f64>,
    /// The switched equipment's own minimum OFF time in seconds; see `equipment_minimum_on_seconds`.
    #[serde(default, deserialize_with = "form_number")]
    pub equipment_minimum_off_seconds: Option<f64>,
    /// The lower output level; the output's minimum when absent (D-11). The form sends the text typed (`form_number`), and a text that
    /// no number reads is refused under its field.
    #[serde(default, deserialize_with = "form_number")]
    pub low_output: Option<f64>,
    /// The upper output level; the output's maximum when absent (D-11); see `low_output`.
    #[serde(default, deserialize_with = "form_number")]
    pub high_output: Option<f64>,
    /// The measurement's noise half-band, in its unit, zero or more.
    pub noise_band: Option<f64>,
    /// `at-or-beyond` (the default) or `beyond`: where the requested state changes at a threshold (D-10).
    pub threshold_equality: Option<String>,
    /// The identification step of the output, in its unit, when the engineer sets one.
    pub experiment_step: Option<f64>,
    /// The identification duration in seconds, when the engineer sets one.
    pub experiment_duration_seconds: Option<f64>,
    /// A startup flag the program sets once it has initialized; required when the program writes a switching setting itself.
    pub startup_flag: Option<String>,
}

/// The loop definition of a switching controller (C3), generated from the draft for the scope and checked by the loop contract.
///
/// # Errors
/// The name, the instance or a signal is missing; the instance is not a compiled controller with one cyclic task; the latch is not a
/// BOOL; a setting is not compiled; the program writes a switching setting without a startup flag; or the generated loop breaks a rule
/// of the loop contract.
pub fn on_off_loop_definition(
    catalog: &CompiledCatalog,
    scope: &LoopScope,
    draft: &OnOffLoopDraft,
) -> Result<LoopDefinition, Failure> {
    if draft.name.trim().is_empty()
        || draft.instance.trim().is_empty()
        || draft.process_variable.symbol.trim().is_empty()
        || draft.manipulated_variable.symbol.trim().is_empty()
        || draft.setpoint.symbol.trim().is_empty()
    {
        return Err(settings(CHOICES_NEEDED));
    }
    let pv = measured_signal(
        &draft.process_variable,
        "measurement",
        Some("onoff.processVariable"),
    )?;
    let mv = measured_signal(
        &draft.manipulated_variable,
        "output",
        Some("onoff.manipulatedVariable"),
    )?;
    check_equipment_times(draft)?;
    check_output_levels(draft)?;
    let controller = catalog
        .controllers
        .iter()
        .find(|entry| entry.instance == draft.instance)
        .ok_or_else(|| {
            settings(&format!(
                "{} is not a compiled program or function block instance.",
                draft.instance
            ))
        })?;
    let task = cyclic_task(controller)?;
    let latch = compiled(catalog, &draft.latch)?;
    if latch.iec_type != "BOOL" {
        return Err(settings(LATCH_NOT_BOOL));
    }
    let seeded = [
        draft.setpoint.symbol.clone(),
        draft.manual_output.clone(),
        draft.mode.symbol.clone(),
        draft.hysteresis.clone(),
        draft.minimum_on.clone(),
        draft.minimum_off.clone(),
    ];
    check_startup(
        catalog,
        &draft.instance,
        &seeded,
        draft.startup_flag.as_deref(),
    )?;
    let setpoint = setpoint_signal(&draft.setpoint, &pv);
    let when_true_manual = draft.mode.when_true == ModeWhenTrue::Manual;
    let (low, high) = (
        draft.low_output.unwrap_or(mv.minimum),
        draft.high_output.unwrap_or(mv.maximum),
    );
    let text = json!({
        "schemaVersion": LOOP_SCHEMA_VERSION,
        "id": loop_id(&draft.name),
        "name": draft.name,
        "revision": "sha256:pending",
        "projectId": scope.project_id,
        "runtimeId": scope.runtime_id,
        "topology": "closed-loop",
        "controller": controller_json(draft, controller.type_name.as_str(), task.interval_ms, low, high, catalog)?,
        "signals": {
            "setpoint": serde_json::to_value(binding(catalog, &setpoint, Direction::Read)?).unwrap_or_default(),
            "processVariable": serde_json::to_value(measurement_binding(catalog, &pv)?).unwrap_or_default(),
            "manipulatedVariable": serde_json::to_value(binding(catalog, &mv, Direction::Write)?).unwrap_or_default(),
        },
        "limits": { "outputMinimum": mv.minimum, "outputMaximum": mv.maximum },
        "observedSampleTiming": { "nominalPeriodMs": task.interval_ms },
        "channels": channels_json(draft, &pv, &mv),
        "parameterBinding": parameter_binding_json(catalog, draft, &pv, &task.name, when_true_manual)?,
    });
    Ok(with_revision(with_experiment(draft, text)?))
}

/// The equipment's minimum ON and OFF times are zero or more seconds, or blank when the equipment declares none (`equipmentSeconds` of
/// `loopSetupForm.ts`). A negative time, or a text that no number reads, is refused under its field.
///
/// # Errors
/// A declared time is not a number of zero or more seconds.
fn check_equipment_times(draft: &OnOffLoopDraft) -> Result<(), Failure> {
    let times = [
        (
            draft.equipment_minimum_on_seconds,
            "equipment minimum ON time",
            "onoff.equipmentMinimumOn",
        ),
        (
            draft.equipment_minimum_off_seconds,
            "equipment minimum OFF time",
            "onoff.equipmentMinimumOff",
        ),
    ];
    for (seconds, label, field) in times {
        if seconds.is_some_and(|seconds| !seconds.is_finite() || seconds < 0.0) {
            let text = format!(
                "The {label} must be zero or more seconds; leave it blank when the equipment has none."
            );
            return Err(settings(&text).with_field(field));
        }
    }
    Ok(())
}

/// The output levels are numbers, or blank for the output's limits (D-11; B1.3: the form sends the text typed). A text that no number
/// reads is refused under its field.
///
/// # Errors
/// A level is given as a text that no number reads.
fn check_output_levels(draft: &OnOffLoopDraft) -> Result<(), Failure> {
    let levels = [
        (draft.low_output, "low", "minimum", "onoff.lowOutput"),
        (draft.high_output, "high", "maximum", "onoff.highOutput"),
    ];
    for (level, which, limit, field) in levels {
        if level.is_some_and(|level| !level.is_finite()) {
            let text = format!(
                "The {which} output level must be a number; leave it blank to use the output's {limit}."
            );
            return Err(settings(&text).with_field(field));
        }
    }
    Ok(())
}

/// The controller block: the three settings, the levels, the threshold policy and the declared states, with the controller's own fields.
fn controller_json(
    draft: &OnOffLoopDraft,
    type_name: &str,
    period_ms: f64,
    low: f64,
    high: f64,
    catalog: &CompiledCatalog,
) -> Result<Value, Failure> {
    let manual = compiled(catalog, &draft.manual_output)?;
    let manual_real = real_type(manual)?;
    let mut states = vec![latch_state(&draft.latch, high)];
    if let Some(dwell) = &draft.dwell_timer {
        states.push(dwell_state(compiled(catalog, dwell)?)?);
    }
    if let Some(flag) = draft
        .startup_flag
        .as_deref()
        .filter(|flag| !flag.is_empty())
    {
        states.push(serde_json::to_value(startup_state(flag)).unwrap_or_default());
    }
    let mut block = json!({
        "hysteresisSymbol": draft.hysteresis,
        "minimumOnTimeSymbol": draft.minimum_on,
        "minimumOffTimeSymbol": draft.minimum_off,
        "bandConvention": draft.band_convention,
        "lowOutput": low,
        "highOutput": high,
        "initialization": { "kind": "declared-state-v1", "states": states },
    });
    if let Some(band) = draft.noise_band {
        block["noiseBand"] = json!(band);
    }
    if let Some(policy) = &draft.threshold_equality {
        block["thresholdEquality"] = json!(policy);
    }
    let mut controller = json!({
        "instance": draft.instance,
        "type": type_name,
        "revision": "sha256:unverified",
        "action": draft.action,
        "taskPeriodMs": period_ms,
        "design": "project",
        "modeSignal": draft.mode.symbol,
        "modeWhenTrue": draft.mode.when_true,
        "manualOutputSignal": manual.symbol,
        "manualOutputType": manual_output_type(manual_real),
        "onOff": block,
    });
    if let Some(address) = manual
        .address
        .as_deref()
        .filter(|address| !address.is_empty())
    {
        controller["manualOutputAddress"] = json!(address);
    }
    Ok(controller)
}

/// The latch: a BOOL that starts ON exactly when the output starts at its high level (C3, Auto entry).
fn latch_state(symbol: &str, high: f64) -> Value {
    json!({
        "symbol": symbol, "iec_type": "BOOL", "unit": "1", "minimum": 0.0, "maximum": 1.0,
        "initial": { "source": "model-input-is", "index": 0, "value": high }, "postScan": "bounded",
    })
}

/// The dwell timer: seconds in the current state, starting at zero, bounded by the timer's maximum (`DWELL_TIMER_MAXIMUM_SECONDS`).
fn dwell_state(entry: &super::catalog::CompiledSymbol) -> Result<Value, Failure> {
    let real = real_type(entry)?;
    Ok(json!({
        "symbol": entry.symbol, "iec_type": real_word(real), "unit": "s", "minimum": 0.0,
        "maximum": DWELL_TIMER_MAXIMUM_SECONDS, "initial": 0.0, "postScan": "bounded",
    }))
}

/// The switched output's channel: the one ON/OFF input on the manipulated variable with the equipment's declared minimum times, and the
/// measured process variable as its output with the loop's setpoint (C3, D-9).
fn channels_json(draft: &OnOffLoopDraft, pv: &MeasuredSignal, mv: &MeasuredSignal) -> Value {
    let mut actuator = json!({
        "kind": "on-off", "minimum": mv.minimum, "maximum": mv.maximum,
    });
    if let Some(seconds) = draft.equipment_minimum_on_seconds {
        actuator["minimumOnSeconds"] = json!(seconds);
    }
    if let Some(seconds) = draft.equipment_minimum_off_seconds {
        actuator["minimumOffSeconds"] = json!(seconds);
    }
    json!({
        "inputs": [{
            "modelChannelId": mv.symbol,
            "signalId": "manipulatedVariable",
            "actuator": actuator,
        }],
        "outputs": [{
            "modelChannelId": pv.symbol,
            "signalId": "processVariable",
            "setpointSignalId": "setpoint",
        }],
    })
}

/// The parameter record: the hysteresis in the measurement's unit (at most the measurement's span), and the two minimum times in seconds
/// (at most `MINIMUM_TIME_MAXIMUM_SECONDS`), with the Auto/Manual mode that the record's binding names.
fn parameter_binding_json(
    catalog: &CompiledCatalog,
    draft: &OnOffLoopDraft,
    pv: &MeasuredSignal,
    task_name: &str,
    when_true_manual: bool,
) -> Result<Value, Failure> {
    let span = pv.maximum - pv.minimum;
    let hysteresis = setting(
        catalog,
        &draft.hysteresis,
        &pv.unit,
        "ON/OFF hysteresis",
        span,
    )?;
    let minimum_on = setting(
        catalog,
        &draft.minimum_on,
        "s",
        "ON/OFF minimum ON time",
        MINIMUM_TIME_MAXIMUM_SECONDS,
    )?;
    let minimum_off = setting(
        catalog,
        &draft.minimum_off,
        "s",
        "ON/OFF minimum OFF time",
        MINIMUM_TIME_MAXIMUM_SECONDS,
    )?;
    Ok(json!({
        "loop_id": loop_id(&draft.name),
        "task_name": task_name,
        "parameters": [hysteresis, minimum_on, minimum_off],
        "mode": { "symbol": draft.mode.symbol, "manual_value": when_true_manual },
    }))
}

/// One bound setting: its type from the compiled variable, its unit, its convention and its bounds `0` to `maximum`.
fn setting(
    catalog: &CompiledCatalog,
    symbol: &str,
    unit: &str,
    convention: &str,
    maximum: f64,
) -> Result<Value, Failure> {
    let real = real_type(compiled(catalog, symbol)?)?;
    Ok(json!({
        "symbol": symbol, "iec_type": real_word(real), "unit": unit, "convention": convention,
        "minimum": 0.0, "maximum": maximum,
    }))
}

/// Adds the experiment the draft names, when it names one, to the loop text.
fn with_experiment(draft: &OnOffLoopDraft, mut text: Value) -> Result<LoopDefinition, Failure> {
    if let Some(experiment) = explicit_experiment(
        &draft.name,
        Execution::ControlStudio,
        ExperimentKind::MvStep,
        draft.experiment_step,
        draft.experiment_duration_seconds,
    )? {
        text["experiment"] = serde_json::to_value(experiment).unwrap_or_default();
    }
    LoopDefinition::decode(&text.to_string()).map_err(|rejection| {
        Failure::new(
            &rejection.to_string(),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        )
    })
}

/// The JSON word of a real type, as the loop file writes a parameter's `iec_type`.
fn real_word(real: RealType) -> &'static str {
    match real {
        RealType::Real => "REAL",
        RealType::Lreal => "LREAL",
    }
}

/// A refusal of the draft's choices, with the cause of a setting that the engineer must change.
fn settings(text: &str) -> Failure {
    Failure::new(text, Cause::Settings, NextAction::OpenLoopSetup)
}

#[cfg(test)]
#[path = "on_off_loop_tests.rs"]
mod tests;
