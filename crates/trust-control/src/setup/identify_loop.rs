//! The identify-only loop (specification 35, C12): an open loop that records a plant input and its measured output, while the program
//! keeps its authority and runs its own experiment. Ported from `identifyLoopGenerator.ts`. It has no controller law, no setpoint and no
//! Tune or Apply stage.

#![allow(clippy::module_name_repetitions)] // `IdentifyLoopDraft` names the module's subject, as the TypeScript does.

use serde::{Deserialize, Deserializer};
use serde_json::Value;

use super::catalog::{CompiledCatalog, CompiledController};
use super::loop_bindings::{
    binding, cyclic_task, duration_seconds_of_text, identification_duration, loop_id,
    measured_signal, measurement_binding, required_choice, stated_signal, with_revision,
    FormSignal,
};
use super::texts::generators as texts;
use super::texts::save;
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    Action, Controller, Design, Direction, Execution, Experiment, ExperimentKind, Limits,
    LoopDefinition, LoopScope, SampleTiming, Signals, Topology, LOOP_SCHEMA_VERSION,
};

/// The identify-only draft: the plant input, its measured output, and how long the program's experiment is recorded (`IdentifyLoopDraft`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IdentifyLoopDraft {
    /// The loop's name, from which its id is made.
    pub name: String,
    /// The plant input the program drives.
    pub manipulated_variable: FormSignal,
    /// The measured output.
    pub process_variable: FormSignal,
    /// How long Studio records the project's experiment: the form's text as typed (`15 min`, `900`), or a number of seconds. The
    /// engine reads it with `duration_of_form`; a text that is not a duration, or a duration outside 1 s to 24 h, is refused (C12).
    #[serde(default, deserialize_with = "duration_of_form")]
    pub experiment_duration_seconds: Option<f64>,
}

/// The recording duration the form posts (`durationField.ts`): a number of seconds, or text with its unit words. Text that is not a duration
/// reads as absent, and the setup refuses it with its sentence.
fn duration_of_form<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<f64>, D::Error> {
    Ok(match Option::<Value>::deserialize(deserializer)? {
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(text)) => duration_seconds_of_text(&text),
        _ => None,
    })
}

/// The identify-only loop definition (`identifyLoopDefinition`): an open loop on the program that owns the input, whose experiment the
/// project runs and Control Studio only observes, with zero external amplitude (C12 Setup).
///
/// # Errors
/// The input does not belong to a compiled program instance, or that program does not run in exactly one cyclic task; a signal is not
/// compiled, is not `REAL` or `LREAL`, or is a measurement the program computes; or the recording duration is missing or outside 1 s to
/// 24 h.
pub fn identify_loop_definition(
    catalog: &CompiledCatalog,
    scope: &LoopScope,
    draft: &IdentifyLoopDraft,
) -> Result<LoopDefinition, Failure> {
    // The form's questions in the order it asks them: the name, the plant input, the measured output, the recording duration.
    required_choice(&draft.name, save::name_needed(), "name")?;
    let input = stated_signal(&measured_signal(
        &draft.manipulated_variable,
        "plant input",
        Some("identify.manipulatedVariable"),
    )?);
    let process_variable = stated_signal(&measured_signal(
        &draft.process_variable,
        "measured output",
        Some("identify.processVariable"),
    )?);
    let duration = identification_duration(&draft.name, draft.experiment_duration_seconds)
        .map_err(|failure| failure.with_field("identify.durationSeconds"))?;
    let owner = draft
        .manipulated_variable
        .symbol
        .split('.')
        .next()
        .unwrap_or_default();
    let program = catalog
        .controllers
        .iter()
        .find(|entry| entry.instance == owner)
        .ok_or_else(|| {
            Failure::new(
                &texts::not_compiled_owner(&draft.manipulated_variable.symbol),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })?;
    let task = cyclic_task(program)?;
    let process_signal = measurement_binding(catalog, &process_variable)?;
    let input_signal = binding(catalog, &input, Direction::Read)?;
    Ok(with_revision(LoopDefinition {
        schema_version: LOOP_SCHEMA_VERSION,
        id: loop_id(&draft.name),
        name: draft.name.clone(),
        revision: "sha256:pending".to_string(),
        project_id: scope.project_id.clone(),
        runtime_id: scope.runtime_id.clone(),
        controller: project_controller(program, owner, task.interval_ms),
        signals: Signals {
            setpoint: None,
            process_variable: process_signal,
            manipulated_variable: input_signal,
            actuator: None,
            additional: None,
        },
        limits: Limits {
            output_minimum: Some(input.minimum),
            output_maximum: Some(input.maximum),
            rate_up_per_s: None,
            rate_down_per_s: None,
            expected_settling_seconds: None,
        },
        observed_sample_timing: SampleTiming {
            nominal_period_ms: task.interval_ms,
            maximum_jitter_ms: None,
        },
        parameter_binding: None,
        parameter_purpose: None,
        topology: Some(Topology::OpenLoop),
        experiment: Some(Experiment {
            execution: Execution::ProjectScripted,
            kind: ExperimentKind::Observation,
            amplitude: 0.0,
            relay_direction: None,
            duration_seconds: Some(duration),
            baseline_seconds: None,
            hold_parameter: None,
            seed: 1,
            activation: None,
            abort: None,
        }),
        plant_id: None,
        operating_point: None,
        identification_policy: None,
        verification_scenarios: None,
        channels: None,
        analysis: None,
        analysis_parameter_symbols: None,
        cascade: None,
    }))
}

/// The controller of an identify-only loop: the program that owns the input, designed by the project, with no law (C12).
fn project_controller(program: &CompiledController, owner: &str, period_ms: f64) -> Controller {
    Controller {
        instance: owner.to_string(),
        type_name: program.type_name.clone(),
        revision: "sha256:unverified".to_string(),
        action: Action::Direct,
        task_period_ms: period_ms,
        design: Some(Design::Project),
        law: None,
        mode_signal: None,
        mode_when_true: None,
        tracking_signal: None,
        manual_output_signal: None,
        manual_output_address: None,
        manual_output_type: None,
        manual_output_follows_output: None,
        pid_initialization: None,
        state_feedback: None,
        on_off: None,
        project: None,
        group: None,
    }
}

#[cfg(test)]
#[path = "identify_loop_tests.rs"]
mod tests;
