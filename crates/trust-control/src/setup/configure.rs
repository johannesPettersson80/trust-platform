//! The loop the step-by-step setup builds from its answers (specification 35, B1.3, B1.4, C1, C12 and C13;
//! `configureLoop.ts`). It checks the answers with the rules of their questions, assembles the loop as the TypeScript does,
//! applies the controller's adapter and the channel step, and checks the result with the loop loader and its validation.
//! It never writes a file: saving is the engine's job (`save`), and the sentence of the save confirmation is
//! `texts::configure::save_summary`.
//!
//! Differences from the TypeScript, each set out where it is made: the project id comes from the scope the caller passes,
//! not from the folder's URI; the revision is [`with_revision`]'s digest; the observation's duration is checked by
//! [`identification_duration`]; the adapter route asks no duration (B1.3); the proportional route writes no experiment,
//! as the TypeScript does, so its `control-studio` branch is not reachable and is not ported.

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    Action, Controller, ControllerLaw, Design, Direction, Execution, Experiment, ExperimentKind,
    Limits, LoopDefinition, LoopRejection, LoopScope, LoopSignal, ManualOutputType, ModeBinding,
    ModeWhenTrue, Parameter, ParameterBinding, Range, RealType, SampleTiming, Signals, Topology,
    LOOP_SCHEMA_VERSION,
};
use crate::setup::catalog::{CatalogAnswer, CompiledSignal, CompiledSymbol};
use crate::setup::configure_adapter;
use crate::setup::configure_channels;
use crate::setup::loop_bindings::{identification_duration, loop_id, real_type, with_revision};
use crate::setup::prompts::{
    accept_text, next_step, sole_task, workflows_offered, ControllerChoice, NumberRule,
    SetpointChoice, SetupStepAnswers, SetupWorkflow,
};
use crate::setup::texts;

/// The loop's name in the duration's refusal (`configureLoop.ts`, "Observation duration (seconds)").
const OBSERVATION_DURATION: &str = "Observation duration";

/// Builds the loop a step-by-step setup declares (`configureControlLoop`, before the save).
///
/// # Errors
/// The catalog has no process-image signal; a question is unanswered; a workflow, channel or actuator is parked or refused
/// (ON/OFF, multiple channels, ON/OFF actuator); an answer breaks its rule; the chosen symbols or the mapping do not fit the
/// catalog; or the loop breaks the loop contract.
pub fn configure_loop(
    catalog: &CatalogAnswer,
    scope: &LoopScope,
    answers: &SetupStepAnswers,
) -> Result<LoopDefinition, Failure> {
    let workflow = check_answered(catalog, answers)?;
    let name = accept_text(required(
        answers.name.as_deref(),
        texts::prompts::loop_name(),
    )?)
    .map_err(|reason| refusal(&reason))?;
    let identity = identity_of(answers)?;
    let period_ms = loop_period(answers)?;
    let task = sole_task(answers.controller.as_ref()).map(|task| task.name.clone());
    let process = required(
        answers.process_variable.as_ref(),
        texts::prompts::process_variable(),
    )?;
    let output = required(
        answers.controller_output.as_ref(),
        texts::prompts::controller_output(),
    )?;
    if output.address == process.address {
        return Err(refusal(&texts::configure::output_repeats_process(
            &output.symbol,
        )));
    }
    let setpoint = setpoint_of(answers, process, output)?;
    if !workflows_offered(setpoint.is_some()).contains(&workflow) {
        return Err(refusal(&texts::configure::workflow_needs_setpoint(
            workflow_label(workflow),
        )));
    }
    let process_range = bounds(
        answers.process_unit.as_deref(),
        answers.process_minimum,
        answers.process_maximum,
        [
            texts::prompts::process_unit(),
            texts::prompts::process_minimum(),
            texts::prompts::process_maximum(),
        ],
    )?;
    let output_range = bounds(
        answers.output_unit.as_deref(),
        answers.output_minimum,
        answers.output_maximum,
        [
            texts::prompts::output_unit(),
            texts::prompts::output_minimum(),
            texts::prompts::output_maximum(),
        ],
    )?;
    let proportional = match workflow {
        SetupWorkflow::Proportional => Some(proportional_parts(catalog, answers, task.as_deref())?),
        _ => None,
    };
    // B1.3 and C12: the observation records its duration; the other routes leave it to Identify and Limits.
    let duration = match workflow {
        SetupWorkflow::Observation => Some(identification_duration(
            OBSERVATION_DURATION,
            answers.observation_duration_seconds,
        )?),
        _ => None,
    };
    let parts = Assembly {
        name,
        identity,
        period_ms,
        process,
        output,
        setpoint,
        process_range,
        output_range,
        proportional,
        duration,
    };
    let mut loop_definition = assemble(scope, &parts);
    if workflow == SetupWorkflow::Adapter {
        let bytes = required(
            answers.adapter_mapping.as_deref(),
            texts::prompts::adapter_mapping(),
        )?;
        let mapping = configure_adapter::mapping_from_bytes(bytes)?;
        configure_adapter::apply_mapping(
            &mut loop_definition,
            &mapping,
            &catalog.control,
            task.as_deref(),
        )?;
    }
    configure_channels::apply_channels(&mut loop_definition, answers)?;
    checked(&with_revision(loop_definition), scope)
}

/// The rule of a question that is answered, and the parked workflow refused (`configureOnOffController.ts`).
fn check_answered(
    catalog: &CatalogAnswer,
    answers: &SetupStepAnswers,
) -> Result<SetupWorkflow, Failure> {
    if catalog.signals.is_empty() {
        return Err(Failure::new(
            texts::configure::no_process_signals(),
            Cause::Implementation,
            NextAction::None,
        ));
    }
    if let Some(step) = next_step(catalog, answers) {
        return Err(refusal(&texts::configure::unanswered(&step.title)));
    }
    let workflow = required(answers.workflow, texts::prompts::control_workflow())?;
    if workflow == SetupWorkflow::OnOff {
        return Err(parked(texts::configure::on_off_controller_not_available()));
    }
    Ok(workflow)
}

/// The controller's compiled or typed identity (`configureLoop.ts`, the controller pick and its instance and type).
struct Identity {
    instance: String,
    type_name: String,
}

fn identity_of(answers: &SetupStepAnswers) -> Result<Identity, Failure> {
    if let Some(ControllerChoice::Compiled(controller)) = &answers.controller {
        return Ok(Identity {
            instance: controller.instance.clone(),
            type_name: controller.type_name.clone(),
        });
    }
    let instance = accept_text(required(
        answers.custom_instance.as_deref(),
        texts::prompts::controller_instance(),
    )?)
    .map_err(|reason| refusal(&reason))?;
    let type_name = accept_text(required(
        answers.custom_type.as_deref(),
        texts::prompts::controller_type(),
    )?)
    .map_err(|reason| refusal(&reason))?;
    Ok(Identity {
        instance,
        type_name,
    })
}

/// The loop's period: the controller's sole task, or the typed period above zero (`configureLoop.ts`, `period`).
fn loop_period(answers: &SetupStepAnswers) -> Result<f64, Failure> {
    if let Some(task) = sole_task(answers.controller.as_ref()) {
        return Ok(task.interval_ms);
    }
    in_range(
        required(answers.task_period_ms, texts::prompts::task_period())?,
        NumberRule::Positive,
    )
}

/// The setpoint, or none for an open loop; a setpoint may be neither the process variable nor the output.
fn setpoint_of<'a>(
    answers: &'a SetupStepAnswers,
    process: &CompiledSignal,
    output: &CompiledSignal,
) -> Result<Option<&'a CompiledSignal>, Failure> {
    match required(answers.setpoint.as_ref(), texts::prompts::setpoint())? {
        SetpointChoice::OpenLoop => Ok(None),
        SetpointChoice::Signal(signal) => {
            if signal.address == process.address || signal.address == output.address {
                Err(refusal(&texts::configure::setpoint_repeats(&signal.symbol)))
            } else {
                Ok(Some(signal))
            }
        }
    }
}

/// A unit with its minimum and maximum (`configureLoop.ts`, phase 2): the unit is non-blank, the minimum is finite and the
/// maximum is above it.
fn bounds(
    unit: Option<&str>,
    minimum: Option<f64>,
    maximum: Option<f64>,
    titles: [&str; 3],
) -> Result<UnitRange, Failure> {
    let unit = accept_text(required(unit, titles[0])?).map_err(|reason| refusal(&reason))?;
    let minimum = in_range(required(minimum, titles[1])?, NumberRule::Finite)?;
    let maximum = in_range(
        required(maximum, titles[2])?,
        NumberRule::AboveBound(minimum),
    )?;
    Ok(UnitRange {
        unit,
        minimum,
        maximum,
    })
}

/// The proportional setup's answers, each checked as its question checks it (`proportionalSetup`, `configureLoop.ts`).
struct Proportional<'a> {
    gain: &'a CompiledSymbol,
    gain_type: RealType,
    gain_minimum: f64,
    gain_maximum: f64,
    manual_command: &'a CompiledSymbol,
    manual_output_type: ManualOutputType,
    manual_output_address: Option<String>,
    mode: &'a CompiledSymbol,
    manual_when_true: bool,
    task_name: String,
}

fn proportional_parts<'a>(
    catalog: &CatalogAnswer,
    answers: &'a SetupStepAnswers,
    task_name: Option<&str>,
) -> Result<Proportional<'a>, Failure> {
    let gain = required(answers.gain.as_ref(), texts::prompts::proportional_gain())?;
    let gain_type = numeric_type(gain)?;
    let gain_minimum = in_range(
        required(answers.gain_minimum, texts::prompts::gain_minimum())?,
        NumberRule::Finite,
    )?;
    let gain_maximum = in_range(
        required(answers.gain_maximum, texts::prompts::gain_maximum())?,
        NumberRule::AtLeast(gain_minimum),
    )?;
    let manual_command = required(
        answers.manual_command.as_ref(),
        texts::prompts::manual_command(),
    )?;
    let manual_output_type = manual_output_of(numeric_type(manual_command)?);
    let mode = required(answers.manual_mode.as_ref(), texts::prompts::manual_mode())?;
    if mode.iec_type != "BOOL" {
        return Err(refusal(&texts::configure::not_compiled_bool(&mode.symbol)));
    }
    let manual_when_true = required(
        answers.manual_when_true,
        texts::prompts::manual_mode_polarity(),
    )?;
    let task_name = match task_name {
        Some(name) => name.to_owned(),
        None => accept_text(required(
            answers.controller_task.as_deref(),
            texts::prompts::controller_task(),
        )?)
        .map_err(|reason| refusal(&reason))?,
    };
    let manual_output_address = catalog
        .signals
        .iter()
        .find(|signal| {
            signal.symbol == manual_command.symbol && signal.iec_type == manual_command.iec_type
        })
        .map(|signal| signal.address.clone());
    Ok(Proportional {
        gain,
        gain_type,
        gain_minimum,
        gain_maximum,
        manual_command,
        manual_output_type,
        manual_output_address,
        mode,
        manual_when_true,
        task_name,
    })
}

/// The REAL or LREAL type of a symbol, refused as the question refuses it (`configureLoop.ts`).
fn numeric_type(symbol: &CompiledSymbol) -> Result<RealType, Failure> {
    real_type(symbol).map_err(|_| refusal(&texts::configure::not_compiled_numeric(&symbol.symbol)))
}

const fn manual_output_of(value: RealType) -> ManualOutputType {
    match value {
        RealType::Real => ManualOutputType::Real,
        RealType::Lreal => ManualOutputType::Lreal,
    }
}

/// A unit and its range, as the loop's signals take them.
struct UnitRange {
    unit: String,
    minimum: f64,
    maximum: f64,
}

/// The answers of the loop, checked and ready to assemble.
struct Assembly<'a> {
    name: String,
    identity: Identity,
    period_ms: f64,
    process: &'a CompiledSignal,
    output: &'a CompiledSignal,
    setpoint: Option<&'a CompiledSignal>,
    process_range: UnitRange,
    output_range: UnitRange,
    proportional: Option<Proportional<'a>>,
    duration: Option<f64>,
}

/// The loop definition the answers declare, before the adapter and the channels are applied.
fn assemble(scope: &LoopScope, parts: &Assembly<'_>) -> LoopDefinition {
    LoopDefinition {
        schema_version: LOOP_SCHEMA_VERSION,
        id: loop_id(&parts.name),
        name: parts.name.clone(),
        revision: "sha256:pending".into(),
        project_id: scope.project_id.clone(),
        runtime_id: scope.runtime_id.clone(),
        controller: controller(parts),
        signals: signals(parts),
        limits: Limits {
            output_minimum: Some(parts.output_range.minimum),
            output_maximum: Some(parts.output_range.maximum),
            rate_up_per_s: None,
            rate_down_per_s: None,
            expected_settling_seconds: None,
        },
        observed_sample_timing: SampleTiming {
            nominal_period_ms: parts.period_ms,
            maximum_jitter_ms: None,
        },
        parameter_binding: parts
            .proportional
            .as_ref()
            .map(|proportional| parameter_binding(parts, proportional)),
        parameter_purpose: None,
        topology: Some(if parts.setpoint.is_some() {
            Topology::ClosedLoop
        } else {
            Topology::OpenLoop
        }),
        experiment: parts.duration.map(observation),
        plant_id: None,
        operating_point: None,
        identification_policy: None,
        verification_scenarios: None,
        channels: None,
        analysis: None,
        analysis_parameter_symbols: None,
        cascade: None,
    }
}

/// The controller. A `Control.PID` with a setpoint is the PID design; any other controller is the project's (`configureLoop.ts`).
fn controller(parts: &Assembly<'_>) -> Controller {
    let proportional = parts.proportional.as_ref();
    let design = if parts.identity.type_name == "Control.PID" && parts.setpoint.is_some() {
        Design::Pid
    } else {
        Design::Project
    };
    Controller {
        instance: parts.identity.instance.clone(),
        type_name: parts.identity.type_name.clone(),
        revision: "sha256:unverified".into(),
        action: Action::Direct,
        task_period_ms: parts.period_ms,
        design: Some(design),
        law: proportional.map(|proportional| ControllerLaw::Proportional {
            gain_symbol: proportional.gain.symbol.clone(),
        }),
        mode_signal: proportional.map(|proportional| proportional.mode.symbol.clone()),
        mode_when_true: proportional.map(|proportional| {
            if proportional.manual_when_true {
                ModeWhenTrue::Manual
            } else {
                ModeWhenTrue::Auto
            }
        }),
        tracking_signal: None,
        manual_output_signal: proportional
            .map(|proportional| proportional.manual_command.symbol.clone()),
        manual_output_address: proportional
            .and_then(|proportional| proportional.manual_output_address.clone()),
        manual_output_type: proportional.map(|proportional| proportional.manual_output_type),
        manual_output_follows_output: None,
        pid_initialization: None,
        state_feedback: None,
        on_off: None,
        project: None,
        group: None,
    }
}

/// The signals: the process variable and the setpoint in the process unit, the output in its own unit. A proportional
/// controller's output is written; every other signal is read (`configureLoop.ts`).
fn signals(parts: &Assembly<'_>) -> Signals {
    let output_direction = if parts.proportional.is_some() {
        Direction::Write
    } else {
        Direction::Read
    };
    Signals {
        setpoint: parts
            .setpoint
            .map(|setpoint| loop_signal(setpoint, &parts.process_range, Direction::Read)),
        process_variable: loop_signal(parts.process, &parts.process_range, Direction::Read),
        manipulated_variable: loop_signal(parts.output, &parts.output_range, output_direction),
        actuator: None,
        additional: None,
    }
}

fn loop_signal(signal: &CompiledSignal, range: &UnitRange, direction: Direction) -> LoopSignal {
    LoopSignal {
        symbol: signal.symbol.clone(),
        address: Some(signal.address.clone()),
        iec_type: signal.iec_type.clone(),
        direction,
        unit: range.unit.clone(),
        range: Range {
            minimum: range.minimum,
            maximum: range.maximum,
        },
    }
}

/// The parameter record of a proportional controller: its gain, in output units per process unit, and its Manual mode
/// (`configureLoop.ts`, `parameterBinding`).
fn parameter_binding(parts: &Assembly<'_>, proportional: &Proportional<'_>) -> ParameterBinding {
    ParameterBinding {
        loop_id: loop_id(&parts.name),
        task_name: proportional.task_name.clone(),
        parameters: vec![Parameter {
            symbol: proportional.gain.symbol.clone(),
            iec_type: proportional.gain_type,
            unit: format!("{}/{}", parts.output_range.unit, parts.process_range.unit),
            convention: "proportional gain".into(),
            minimum: proportional.gain_minimum,
            maximum: proportional.gain_maximum,
        }],
        mode: ModeBinding {
            symbol: proportional.mode.symbol.clone(),
            manual_value: proportional.manual_when_true,
        },
    }
}

/// The observation experiment: the project runs its scenario and Control Studio records it for the duration, with no
/// excitation (C12; `configureLoop.ts`).
fn observation(duration_seconds: f64) -> Experiment {
    Experiment {
        execution: Execution::ProjectScripted,
        kind: ExperimentKind::Observation,
        amplitude: 0.0,
        relay_direction: None,
        duration_seconds: Some(duration_seconds),
        baseline_seconds: None,
        hold_parameter: None,
        seed: 1,
        activation: None,
        abort: None,
    }
}

/// The loop contract's loader and its validation, run on the loop as the engine reads it: the loop is serialised and decoded
/// by [`LoopDefinition::decode`], which checks its fields, and then validated against the scope.
fn checked(loop_definition: &LoopDefinition, scope: &LoopScope) -> Result<LoopDefinition, Failure> {
    let text =
        serde_json::to_string(loop_definition).map_err(|error| incompatible(&error.to_string()))?;
    let decoded =
        LoopDefinition::decode(&text).map_err(|rejection| incompatible(&rejection.to_string()))?;
    decoded
        .validate(scope)
        .map_err(|rejection: LoopRejection| incompatible(&rejection.to_string()))?;
    Ok(decoded)
}

fn workflow_label(workflow: SetupWorkflow) -> &'static str {
    match workflow {
        SetupWorkflow::Observation => texts::prompts::observation(),
        SetupWorkflow::Adapter => texts::prompts::adapter_workflow(),
        SetupWorkflow::OnOff => texts::prompts::on_off_workflow(),
        SetupWorkflow::Proportional => texts::prompts::proportional_workflow(),
    }
}

/// A value must pass its rule, or the question's own sentence refuses it (`number`, `configureLoop.ts`).
fn in_range(value: f64, rule: NumberRule) -> Result<f64, Failure> {
    if rule.accepts(value) {
        Ok(value)
    } else {
        Err(refusal(texts::prompts::invalid_number()))
    }
}

fn required<T>(value: Option<T>, title: &str) -> Result<T, Failure> {
    value.ok_or_else(|| refusal(&texts::configure::unanswered(title)))
}

/// A refusal of an answer, which the setup shows with the loop setup as its next action (A11.2).
fn refusal(text: &str) -> Failure {
    Failure::new(text, Cause::Settings, NextAction::OpenLoopSetup)
}

/// A refusal of a part the Rust engine does not run yet (checklist V7), with no action to offer (A11.2).
fn parked(text: &str) -> Failure {
    Failure::new(text, Cause::ControllerCapability, NextAction::None)
}

/// A loop that breaks the loop contract (`validateControlLoopDefinition`'s reason in the TypeScript's sentence).
fn incompatible(reason: &str) -> Failure {
    refusal(&texts::configure::incompatible_bindings(reason))
}

#[cfg(test)]
#[path = "configure_tests.rs"]
mod tests;
