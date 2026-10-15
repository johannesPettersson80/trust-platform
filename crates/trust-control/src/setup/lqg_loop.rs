//! The loop definition of an LQG law (specification 35, C4, B1.4 and §17.1): the program estimates the states from the controlled
//! output with a fixed model, and feeds the estimates back through `K` and `N`. Ported from `lqgLoopGenerator.ts` and from the LQG
//! choices of `loopSetupForm.ts`. Tune designs `K`, `L` and `N` on the selected model (C4); the loop declares the estimator's model,
//! sampled at the task period, and the symbols of its gains, estimates and noise.

#![allow(clippy::module_name_repetitions)] // `lqg_loop_definition` names the module's subject, as the TypeScript does.

use serde_json::{json, Value};

use super::catalog::CompiledCatalog;
use super::loop_bindings::{
    binding, compiled, cyclic_task, explicit_experiment, manual_output_type, measurement_binding,
    real_type, MeasuredSignal,
};
use super::pid_loop::setpoint_signal;
use super::state_feedback_loop::{
    assemble, check_startup, gain_parameter, identification_policy, instance_program, json_of,
    manipulated_input, mode_and_manual, mode_binding, output_limits, settings, startup_states,
    zero_started_state, EstimatorState, StateFeedbackLoopDraft, StateLawParts, CONVENTION,
};
use super::texts::save as texts;
use crate::failure::Failure;
use crate::loop_definition::{
    ChannelOutput, Controller, Design, Direction, Execution, ExperimentKind, InitializedState,
    LoopChannels, LoopDefinition, LoopScope, Parameter, Signals,
};
use crate::model::GeneralModel;
use crate::tuning::state_controller::sampling::{sample_model, state_channels, SampledModel};

/// The most states an LQG estimator's model has: the setup offers three estimator rows (C4, D-B4).
pub const MAXIMUM_ESTIMATOR_STATES: usize = 3;

/// The validated estimator of an LQG law: its model, the identifiers of the model's input and output, one checked row per model state,
/// and the measurement-noise variance (C4).
struct Estimator<'a> {
    model: &'a GeneralModel,
    input_id: String,
    output_id: String,
    rows: Vec<EstimatorRow>,
    measurement_noise: f64,
}

/// One state of the estimator, checked: its estimate's variable, the gain and observer gain on it, and its process-noise variance.
struct EstimatorRow {
    estimate: String,
    gain: String,
    observer_gain: String,
    process_noise: f64,
}

/// The loop definition of an LQG law (`lqgLoopDefinition`): the estimator's model and its rows, the estimate's start, the gains, and
/// the channels of the model's one input and one output. `model` is the model file the estimator implements, read by the engine.
///
/// # Errors
/// The reference gain or the model file is not chosen, the model is not a state-space model with one input and one output, the rows
/// do not match the model's states, a row or a noise variance is missing or not positive, the mode or the manual output is not chosen,
/// the program needs a startup-done flag the draft does not name, or the instance, a symbol or a signal breaks a rule of the loop.
pub(super) fn lqg_loop_definition(
    catalog: &CompiledCatalog,
    scope: &LoopScope,
    draft: &StateFeedbackLoopDraft,
    manipulated: &MeasuredSignal,
    process: &MeasuredSignal,
    model: Option<&GeneralModel>,
) -> Result<LoopDefinition, Failure> {
    let estimator = estimator_choices(draft, model)?;
    mode_and_manual("feedback", &draft.mode, &draft.manual_output)?;
    let seeded = [
        draft.setpoint.symbol.clone(),
        draft.reference_gain.clone(),
        draft.manual_output.clone(),
        draft.mode.symbol.clone(),
    ];
    check_startup(
        catalog,
        &draft.instance,
        &seeded,
        draft.startup_flag.as_deref(),
    )?;
    lqg_law(catalog, scope, draft, manipulated, process, &estimator)
}

/// The estimator the form has chosen, checked in the order of `loopSetupForm.ts` and the generator (`lqgLoopGenerator.ts`): the
/// reference gain, the model file, the model's shape, one row per model state, and the measurement-noise variance.
///
/// # Errors
/// One of those choices is missing or breaks its rule.
fn estimator_choices<'a>(
    draft: &StateFeedbackLoopDraft,
    model: Option<&'a GeneralModel>,
) -> Result<Estimator<'a>, Failure> {
    if draft.reference_gain.trim().is_empty() {
        return Err(settings(texts::reference_gain_needed()).with_field("feedback.referenceGain"));
    }
    let estimator = draft.estimator.as_ref();
    let model_file = estimator.map_or("", |entry| entry.model_file.as_str());
    if model_file.trim().is_empty() {
        return Err(settings(texts::estimator_model_needed()).with_field("feedback.estimatorModel"));
    }
    let model = model.ok_or_else(|| {
        settings(texts::estimator_model_shape()).with_field("feedback.estimatorModel")
    })?;
    let (input_id, output_id) = estimator_channels(model)?;
    let rows_given: &[EstimatorState] = match estimator {
        Some(entry) => &entry.states,
        None => &[],
    };
    let states = state_channels(model);
    if rows_given.len() != states.len() || states.len() > MAXIMUM_ESTIMATOR_STATES {
        return Err(settings(texts::estimator_rows()));
    }
    let rows = estimator_rows(rows_given)?;
    let measurement_noise = estimator
        .and_then(|entry| positive(entry.measurement_noise))
        .ok_or_else(|| {
            settings(texts::measurement_noise_needed()).with_field("feedback.measurementNoise")
        })?;
    Ok(Estimator {
        model,
        input_id,
        output_id,
        rows,
        measurement_noise,
    })
}

/// The identifiers of a model's one input and its one output, for a state-space model that has exactly those (`lqgLoopGenerator.ts`).
///
/// # Errors
/// The model is a transfer function, or it does not have one input and one output.
fn estimator_channels(model: &GeneralModel) -> Result<(String, String), Failure> {
    let identity = model.identity();
    let state_space = !matches!(model, GeneralModel::TransferFunction { .. });
    match (identity.inputs.first(), identity.outputs.first()) {
        (Some(input), Some(output))
            if state_space && identity.inputs.len() == 1 && identity.outputs.len() == 1 =>
        {
            Ok((input.id.clone(), output.id.clone()))
        }
        _ => Err(settings(texts::estimator_model_shape()).with_field("feedback.estimatorModel")),
    }
}

/// The rows of the estimator, each with its three variables and a positive process-noise variance (`loopSetupForm.ts`).
///
/// # Errors
/// A row has no estimate, gain or observer gain, or its process-noise variance is not positive.
fn estimator_rows(rows: &[EstimatorState]) -> Result<Vec<EstimatorRow>, Failure> {
    let mut checked = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let number = index + 1;
        let missing = [
            ("estimate", "", &row.estimate),
            ("gain", ".gain", &row.gain),
            ("observer gain", ".observerGain", &row.observer_gain),
        ]
        .into_iter()
        .find(|(_, _, value)| value.trim().is_empty());
        if let Some((part, suffix, _)) = missing {
            let field = format!("feedback.estimate.{index}{suffix}");
            return Err(settings(&texts::estimator_state_needed(part, number)).with_field(&field));
        }
        let Some(process_noise) = positive(row.process_noise) else {
            let field = format!("feedback.estimate.{index}.processNoise");
            return Err(settings(&texts::process_noise_needed(number)).with_field(&field));
        };
        checked.push(EstimatorRow {
            estimate: row.estimate.clone(),
            gain: row.gain.clone(),
            observer_gain: row.observer_gain.clone(),
            process_noise,
        });
    }
    Ok(checked)
}

/// The value when it is a finite positive number, which a variance must be (`lqgLoopGenerator.ts`).
fn positive(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value > 0.0)
}

/// The LQG loop of a checked estimator: its program, its gains, its record and its channels (C4).
///
/// # Errors
/// The instance is not a compiled program or function block, it does not run in exactly one cyclic task, the model cannot be sampled at
/// the task period, a symbol is not compiled or is not `REAL` or `LREAL`, or a signal is computed by the program.
fn lqg_law(
    catalog: &CompiledCatalog,
    scope: &LoopScope,
    draft: &StateFeedbackLoopDraft,
    manipulated: &MeasuredSignal,
    process: &MeasuredSignal,
    estimator: &Estimator<'_>,
) -> Result<LoopDefinition, Failure> {
    let program = instance_program(catalog, &draft.instance)?;
    let task = cyclic_task(program)?;
    let period_seconds = task.interval_ms / 1000.0;
    let sampled = sample_model(estimator.model, period_seconds)?;
    let manual = compiled(catalog, &draft.manual_output)?;
    let setpoint = setpoint_signal(&draft.setpoint, process);
    let estimates = estimate_states(catalog, estimator)?;
    let initial: Vec<InitializedState> = estimates
        .into_iter()
        .chain(startup_states(draft.startup_flag.as_deref()))
        .collect();
    let controller = Controller {
        instance: draft.instance.clone(),
        type_name: program.type_name.clone(),
        revision: "sha256:unverified".to_string(),
        action: draft.action,
        task_period_ms: task.interval_ms,
        design: Some(Design::Project),
        law: None,
        mode_signal: Some(draft.mode.symbol.clone()),
        mode_when_true: Some(draft.mode.when_true),
        tracking_signal: None,
        manual_output_signal: Some(manual.symbol.clone()),
        manual_output_address: manual.address.clone().filter(|address| !address.is_empty()),
        manual_output_type: Some(manual_output_type(real_type(manual)?)),
        manual_output_follows_output: None,
        pid_initialization: None,
        state_feedback: Some(lqg_record(
            draft,
            estimator,
            &sampled,
            period_seconds,
            &initial,
        )),
        on_off: None,
        project: None,
        group: None,
    };
    let signals = Signals {
        setpoint: Some(binding(catalog, &setpoint, Direction::Read)?),
        process_variable: measurement_binding(catalog, process)?,
        manipulated_variable: binding(catalog, manipulated, Direction::Write)?,
        actuator: None,
        additional: None,
    };
    let experiment = explicit_experiment(
        &draft.name,
        Execution::ControlStudio,
        ExperimentKind::MvStep,
        draft.experiment_step,
        draft.experiment_duration_seconds,
    )?;
    let parameters = lqg_parameters(catalog, draft, manipulated, process, &setpoint, estimator)?;
    Ok(assemble(
        scope,
        StateLawParts {
            name: draft.name.clone(),
            controller,
            signals,
            channels: Some(lqg_channels(manipulated, estimator)),
            identification_policy: Some(identification_policy()),
            limits: output_limits(manipulated),
            period_ms: task.interval_ms,
            task_name: task.name.clone(),
            mode: mode_binding(&draft.mode),
            parameters,
            experiment,
        },
    ))
}

/// The record of an LQG law, as `tuning::state_controller` decodes it (C4): the gains and the estimator's sampled model, the noise
/// variances, the estimates' declared initialization and the symbols of the estimates.
fn lqg_record(
    draft: &StateFeedbackLoopDraft,
    estimator: &Estimator<'_>,
    sampled: &SampledModel,
    period_seconds: f64,
    initial: &[InitializedState],
) -> Value {
    let gains: Vec<&str> = estimator.rows.iter().map(|row| row.gain.as_str()).collect();
    let observers: Vec<[&str; 1]> = estimator
        .rows
        .iter()
        .map(|row| [row.observer_gain.as_str()])
        .collect();
    let estimates: Vec<&str> = estimator
        .rows
        .iter()
        .map(|row| row.estimate.as_str())
        .collect();
    json!({
        "family": "LQG",
        "convention": CONVENTION,
        "gainSymbols": [gains],
        "referenceGainSymbols": [[draft.reference_gain.as_str()]],
        "observerGainSymbols": observers,
        "estimatorModel": {
            "a": sampled.system.a,
            "b": sampled.system.b,
            "c": sampled.system.c,
            "sampleTimeSeconds": period_seconds,
        },
        "processNoise": process_noise_matrix(&estimator.rows),
        "measurementNoise": [[estimator.measurement_noise]],
        "initialization": { "kind": "declared-state-v1", "states": json_of(initial) },
        "estimatorStateSymbols": estimates,
    })
}

/// The process-noise covariance: diagonal, with each state's variance on its own row (`lqgLoopGenerator.ts`).
fn process_noise_matrix(rows: &[EstimatorRow]) -> Vec<Vec<f64>> {
    rows.iter()
        .enumerate()
        .map(|(row, state)| {
            rows.iter()
                .enumerate()
                .map(|(column, _)| {
                    if row == column {
                        state.process_noise
                    } else {
                        0.0
                    }
                })
                .collect()
        })
        .collect()
}

/// The model states as the estimator's signals: each estimate with its model state's unit and operating range.
fn estimate_signals(estimator: &Estimator<'_>) -> Vec<MeasuredSignal> {
    estimator
        .rows
        .iter()
        .zip(state_channels(estimator.model))
        .map(|(row, state)| MeasuredSignal {
            symbol: row.estimate.clone(),
            unit: state.unit.clone(),
            minimum: state.operating_range.minimum,
            maximum: state.operating_range.maximum,
        })
        .collect()
}

/// The estimates as declared states: each starts at zero, the operating point, and is bounded by its model state's range (C4).
///
/// # Errors
/// An estimate is not compiled, or it is not `REAL` or `LREAL`.
fn estimate_states(
    catalog: &CompiledCatalog,
    estimator: &Estimator<'_>,
) -> Result<Vec<InitializedState>, Failure> {
    estimator
        .rows
        .iter()
        .zip(estimate_signals(estimator))
        .map(|(row, signal)| {
            zero_started_state(
                catalog,
                &row.estimate,
                signal.unit,
                (signal.minimum, signal.maximum),
            )
        })
        .collect()
}

/// The parameters of an LQG law: the estimator's gains on the estimates, the reference gain, then the observer gains (the order of
/// `lqgLoopGenerator.ts`).
///
/// # Errors
/// A variable is not compiled, or it is not `REAL` or `LREAL`.
fn lqg_parameters(
    catalog: &CompiledCatalog,
    draft: &StateFeedbackLoopDraft,
    manipulated: &MeasuredSignal,
    process: &MeasuredSignal,
    setpoint: &MeasuredSignal,
    estimator: &Estimator<'_>,
) -> Result<Vec<Parameter>, Failure> {
    let estimates = estimate_signals(estimator);
    let mut parameters = Vec::new();
    for (row, signal) in estimator.rows.iter().zip(&estimates) {
        parameters.push(gain_parameter(
            catalog,
            &row.gain,
            signal,
            manipulated,
            format!("LQG gain on {}", row.estimate),
        )?);
    }
    parameters.push(gain_parameter(
        catalog,
        &draft.reference_gain,
        process,
        manipulated,
        format!("Reference gain on {}", setpoint.symbol),
    )?);
    for (row, signal) in estimator.rows.iter().zip(&estimates) {
        parameters.push(gain_parameter(
            catalog,
            &row.observer_gain,
            process,
            signal,
            format!("Observer gain on {}", row.estimate),
        )?);
    }
    Ok(parameters)
}

/// The channels of an LQG law: the model's one input driven by the plant input, and its one output as the controlled output with its
/// setpoint (C4; `lqgLoopGenerator.ts`: the supplied model maps by its own channel names).
fn lqg_channels(manipulated: &MeasuredSignal, estimator: &Estimator<'_>) -> LoopChannels {
    LoopChannels {
        inputs: vec![manipulated_input(manipulated, estimator.input_id.clone())],
        outputs: vec![ChannelOutput {
            model_channel_id: estimator.output_id.clone(),
            signal_id: "processVariable".to_string(),
            setpoint_signal_id: Some("setpoint".to_string()),
            requirements: None,
        }],
        initial_state: None,
    }
}

#[cfg(test)]
#[path = "lqg_loop_tests.rs"]
mod tests;
