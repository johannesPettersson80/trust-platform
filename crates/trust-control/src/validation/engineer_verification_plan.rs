//! The verification plan of a loop (specification 35, B5.3, C0 and C1, A6.4 and B4.3; numerical annex N8.2 and N8.3;
//! `TypeScript` `engineerVerificationPlan.ts`): the task's requirements as the run checks them, the run's sample grid, its
//! initial values and the symbols it observes. This port covers one plant input and one measured output. The requirement
//! record, the model compatibility guard and the criteria checks are the existing Rust rules that the plan calls.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::failure::{Cause, Failure, NextAction};
use crate::identification::model_compatibility::{
    assert_engineer_model_compatibility, CompatibilityRefusal,
};
use crate::identification::texts::compatibility_text;
use crate::json::decode_value;
use crate::limits::{
    record_period_executions, ISOLATED_SIMULATION_CHANNEL_LIMIT,
    ISOLATED_SIMULATION_DURATION_LIMIT_MS, ISOLATED_SIMULATION_SAMPLE_LIMIT,
    ISOLATED_SIMULATION_SCAN_LIMIT,
};
use crate::loop_definition::{LoopDefinition, LoopSignal, Parameter, RealType};
use crate::model::general::js_text;
use crate::model::{GeneralModel, ModelChannel};
use crate::tuning::automatic_criteria::{validate_criteria, AutomaticCriteria};
use crate::tuning::objective::{TuningMethod, TuningObjective};
use crate::tuning::requirements::{response_deadline_seconds, TuningRequirements};
use crate::tuning::settings_pins::single_precision;
use crate::validation::isolated_run_sampling::{isolated_record_every, scan_count};
use crate::validation::run_validation::{
    ErrorReference, ErrorStatistic, Movement, RunCheck, RunRequirement, Window,
};
use crate::validation::texts;

pub use crate::tuning::requirements::MAXIMUM_OVERSHOOT_PERCENT;

/// The longest task period the runtime adapter runs, in whole milliseconds (`TypeScript` `engineerVerificationPlan`).
const MAXIMUM_TASK_PERIOD_MS: f64 = 1_000.0;

/// The task requirements of a verification (`TypeScript` `EngineerRequirements`). It is the tuning requirement record, which
/// holds the same values and checks; the channel requirements of several outputs are not part of this port.
pub type EngineerRequirements = TuningRequirements;

/// The plan of one verification run (`TypeScript` `engineerVerificationPlan` without its runtime request): the sample grid,
/// the values the run starts from, the symbols it observes and the requirements it checks.
#[derive(Debug, Clone, PartialEq)]
pub struct VerificationPlan {
    /// The task period, in ms: the time between two scans of the run.
    pub step_ms: u64,
    /// The run's duration, in ms: a whole number of task periods.
    pub duration_ms: u64,
    /// The scans the run executes: one at time zero and one per period to the end.
    pub max_total_cycles: u64,
    /// The scans per recorded sample; 1 records every scan (A4.2).
    pub record_every: u64,
    /// The values the run starts from, by symbol: the mode, the plant input at its operating point and the setpoint target.
    pub initial_values: BTreeMap<String, f64>,
    /// The symbols the run observes, each once, in the order the plan lists them.
    pub observed_symbols: Vec<String>,
    /// The requirements the run checks, each condition once, in the order the plan lists them.
    pub requirements: Vec<RunRequirement>,
}

/// Decodes the task requirements of a verification (`TypeScript` `engineerRequirements`): the five required values as finite
/// numbers, the optional values with their own sentences, and the checks of the requirement record
/// ([`TuningRequirements::validate`]). Keys that the record does not hold are not read.
///
/// # Errors
/// Returns a [`Failure`] with the sentence of the first value that is missing, mistyped or out of its range.
pub fn engineer_requirements(value: &Value) -> Result<EngineerRequirements, Failure> {
    let object = value.as_object().ok_or_else(requirements_missing)?;
    let number = |key: &str| object.get(key).and_then(Value::as_f64);
    let (
        Some(reference_change),
        Some(maximum_error),
        Some(settling_band),
        Some(settling_time_seconds),
        Some(duration_seconds),
    ) = (
        number("referenceChange"),
        number("maximumError"),
        number("settlingBand"),
        number("settlingTimeSeconds"),
        number("durationSeconds"),
    )
    else {
        return Err(requirements_missing());
    };
    let bounds = texts::disturbance_noise_bounds_invalid();
    let requirements = TuningRequirements {
        reference_change,
        maximum_error,
        settling_band,
        settling_time_seconds,
        duration_seconds,
        constant_input_disturbance: optional_number(
            object.get("constantInputDisturbance"),
            &bounds,
        )?,
        measurement_noise_band: optional_number(object.get("measurementNoiseBand"), &bounds)?,
        gain_uncertainty_fraction: optional_number(object.get("gainUncertaintyFraction"), &bounds)?,
        transfer_tolerance: optional_number(
            object.get("transferTolerance"),
            &texts::transfer_tolerance_invalid(),
        )?,
        maximum_overshoot_percent: maximum_overshoot_percent(
            object.get("maximumOvershootPercent"),
        )?,
        tuning_method: tuning_method(object.get("tuningMethod"))?,
        tuning_objective: tuning_objective(object.get("tuningObjective"))?,
        automatic_criteria: automatic_criteria(object.get("automaticCriteria"))?,
    };
    requirements
        .validate()
        .map_err(|refusal| invalid_requirement(&refusal.0))?;
    Ok(requirements)
}

/// Plans the verification run of a loop tuned on `model` (`TypeScript` `engineerVerificationPlan`, single-channel form). The
/// task's requirements become the run's checks (C0, C1, B4.3): the response deadline and the target error after it, an
/// overshoot limit when the task declares one and the run applies a step, and the bounds of the output, the input, the
/// rates and the fixed parameters. The run takes the sample grid of its duration (A4.2). `values` are the controller's
/// parameter values; `controller_initial` are the controller's initial values, which the caller computes (commissioning).
///
/// # Errors
/// Returns the failure of [`LoopDefinition::rust_engine_support`] first, when the loop uses a feature the Rust engine does
/// not run (state feedback, an ON/OFF controller, the program's own law, a PID group, channels, a declared analysis, a
/// cascade) or has no controller law. Then it returns a [`Failure`] when the loop has no parameter or setpoint binding, the
/// model is not compatible with the loop (a model that is not one input and one output is refused with
/// `channel_bindings_unmapped`), the task period is not a whole number of milliseconds from 1 to 1000, the run is too long,
/// does not pass its response deadline or needs too many scans, too many channels are observed, a parameter is out of its
/// bounds or not representable, or the target lies outside the declared scope.
pub fn engineer_verification_plan(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    controller_initial: &BTreeMap<String, f64>,
) -> Result<VerificationPlan, Failure> {
    // Before anything else, the Rust engine's gate refuses the parked features (checklist V7) and a loop without a law.
    loop_definition.rust_engine_support()?;
    let (Some(binding), Some(setpoint)) = (
        &loop_definition.parameter_binding,
        &loop_definition.signals.setpoint,
    ) else {
        return Err(binding_missing());
    };
    assert_engineer_model_compatibility(loop_definition, model)
        .map_err(|refusal| compatibility_failure(&refusal))?;
    let channels = plant_connection(loop_definition, model, setpoint)?;
    let period_ms = task_period_ms(loop_definition)?;
    let duration_f = run_duration_ms(task, period_ms)?;
    let step_ms = whole_ms(period_ms);
    let duration_ms = whole_ms(duration_f);
    let scans = scan_count(step_ms, duration_ms);
    if scans > ISOLATED_SIMULATION_SCAN_LIMIT {
        return Err(Failure::new(
            &texts::too_many_scans(scans, ISOLATED_SIMULATION_SCAN_LIMIT),
            Cause::Settings,
            NextAction::OpenRequirements,
        ));
    }
    let observed = observed_symbols(loop_definition, &channels, &binding.parameters);
    if u64::try_from(observed.len()).unwrap_or(u64::MAX) > ISOLATED_SIMULATION_CHANNEL_LIMIT {
        return Err(Failure::new(
            &texts::too_many_observed_channels(ISOLATED_SIMULATION_CHANNEL_LIMIT),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    let parameters = checked_parameters(&binding.parameters, values)?;
    let window = Window {
        start: 0.0,
        end: duration_f / 1_000.0,
    };
    // The run starts in Auto: the mode holds the value that is not its Manual value (`Number(!manual_value)`).
    let mut initial_values = controller_initial.clone();
    initial_values.insert(
        binding.mode.symbol.clone(),
        f64::from(u8::from(!binding.mode.manual_value)),
    );
    initial_values.insert(channels.manipulated.symbol.clone(), channels.input_origin);
    let mut requirements =
        output_requirements(&channels, task, period_ms, window, &mut initial_values)?;
    requirements.extend(input_requirements(&channels, window));
    requirements.extend(parameter_requirements(&parameters, window));
    Ok(VerificationPlan {
        step_ms,
        duration_ms,
        max_total_cycles: scans,
        record_every: record_every(
            step_ms,
            duration_ms,
            observed.len(),
            task.settling_time_seconds,
        ),
        initial_values,
        observed_symbols: observed,
        requirements: unique_conditions(requirements),
    })
}

/// A requirement that is missing, mistyped or out of its range: the sentence, with the cause and the action that fixes it.
fn invalid_requirement(text: &str) -> Failure {
    Failure::new(text, Cause::Settings, NextAction::OpenRequirements)
}

/// The task requirements are missing or not finite.
fn requirements_missing() -> Failure {
    invalid_requirement(&texts::task_requirements_missing())
}

/// The loop has no parameter binding or no setpoint, so no plan can be made.
fn binding_missing() -> Failure {
    Failure::new(
        &texts::parameter_or_reference_binding_missing(),
        Cause::Implementation,
        NextAction::OpenLoopSetup,
    )
}

/// A declared number: an absent key stays undefined, and a value that is not a number is refused with `invalid`.
fn optional_number(value: Option<&Value>, invalid: &str) -> Result<Option<f64>, Failure> {
    match value {
        None => Ok(None),
        Some(value) => value
            .as_f64()
            .map(Some)
            .ok_or_else(|| invalid_requirement(invalid)),
    }
}

/// A declared overshoot limit in percent of the setpoint change. An absent value, `null` and a blank text declare no limit;
/// any other value must be a number, and its range is checked by the requirement record (`TypeScript`
/// `decodeMaximumOvershootPercent`).
fn maximum_overshoot_percent(value: Option<&Value>) -> Result<Option<f64>, Failure> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if !js_text(text) => Ok(None),
        Some(value) => value.as_f64().map(Some).ok_or_else(|| {
            invalid_requirement(&texts::overshoot_limit_invalid(
                "Maximum overshoot",
                MAXIMUM_OVERSHOOT_PERCENT,
            ))
        }),
    }
}

/// The formula method the engineer chose: `SIMC` or `AMIGO`; an absent value leaves the choice to Automatic.
fn tuning_method(value: Option<&Value>) -> Result<Option<TuningMethod>, Failure> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value.as_str() {
        Some("SIMC") => Ok(Some(TuningMethod::Simc)),
        Some("AMIGO") => Ok(Some(TuningMethod::Amigo)),
        _ => Err(Failure::new(
            &texts::tuning_method_unsupported(),
            Cause::Settings,
            NextAction::ChooseMethod,
        )),
    }
}

/// The declared response: `fast`, `balanced` or `robust`; an absent value declares none.
fn tuning_objective(value: Option<&Value>) -> Result<Option<TuningObjective>, Failure> {
    let Some(value) = value else {
        return Ok(None);
    };
    match value.as_str() {
        Some("fast") => Ok(Some(TuningObjective::Fast)),
        Some("balanced") => Ok(Some(TuningObjective::Balanced)),
        Some("robust") => Ok(Some(TuningObjective::Robust)),
        _ => Err(invalid_requirement(&texts::tuning_objective_unsupported())),
    }
}

/// The test criteria Automatic generated: a record that decodes and passes its own rules (`TypeScript`
/// `decodeAutomaticTestCriteria`).
fn automatic_criteria(value: Option<&Value>) -> Result<Option<AutomaticCriteria>, Failure> {
    let Some(value) = value else {
        return Ok(None);
    };
    decode_value::<AutomaticCriteria>(value.clone())
        .ok()
        .filter(|criteria| validate_criteria(criteria).is_ok())
        .map(Some)
        .ok_or_else(|| invalid_requirement(&texts::automatic_criteria_invalid()))
}

/// The failure of a model that cannot tune the loop: its sentence, its cause, and the action that fixes it (A11.2). A model
/// that is not one input and one output has the channel-binding sentence (`channel_bindings_unmapped`, from
/// `loopChannels.ts`); the other sentences are those of [`compatibility_text`].
fn compatibility_failure(refusal: &CompatibilityRefusal) -> Failure {
    let (cause, next_action) = match refusal {
        CompatibilityRefusal::Evidence
        | CompatibilityRefusal::InputUnit { .. }
        | CompatibilityRefusal::OutputMeasurementRange { .. }
        | CompatibilityRefusal::OutputUnit { .. }
        | CompatibilityRefusal::SampleTime => (Cause::ModelOrData, NextAction::Identify),
        CompatibilityRefusal::UnmappedChannels => (Cause::Implementation, NextAction::OpenChannels),
        CompatibilityRefusal::InputActuatorRange { .. } => {
            (Cause::Settings, NextAction::OpenLimits)
        }
        CompatibilityRefusal::SetpointUnit { .. } | CompatibilityRefusal::NoControlledOutput => {
            (Cause::Implementation, NextAction::OpenLoopSetup)
        }
    };
    let sentence = match refusal {
        CompatibilityRefusal::UnmappedChannels => texts::channel_bindings_unmapped(),
        other => compatibility_text(other),
    };
    Failure::new(&sentence, cause, next_action)
}

/// The loop's plant input and measured output as the plan reads them: the loop's signals, the model's channels, the operating
/// point they start from, and the actuator's limits and rates (`TypeScript` `connectedModelChannels` without channel bindings).
struct PlantConnection<'a> {
    /// The manipulated variable: the plant input the controller writes and the run drives.
    manipulated: &'a LoopSignal,
    /// The actuator's measured position, when the loop declares one; the applied value that the run checks.
    actuator: Option<&'a LoopSignal>,
    /// The process variable: the measured output.
    process: &'a LoopSignal,
    /// The setpoint that the measured output follows.
    setpoint: &'a LoopSignal,
    /// The model's input.
    model_input: &'a ModelChannel,
    /// The model's output.
    model_output: &'a ModelChannel,
    /// The operating point's input: the value the run starts from.
    input_origin: f64,
    /// The operating point's output: the value the setpoint step starts from.
    output_origin: f64,
    /// The smallest value the actuator accepts: the loop's output minimum.
    actuator_minimum: f64,
    /// The largest value the actuator accepts: the loop's output maximum.
    actuator_maximum: f64,
    /// The largest rise of the output per second, when the loop declares one.
    rate_up: Option<f64>,
    /// The largest fall of the output per second, when the loop declares one.
    rate_down: Option<f64>,
}

/// Connects the loop to the model for a loop without channel bindings, which the gate of [`engineer_verification_plan`]
/// guarantees. The compatibility check has already refused a model that is not one input and one output
/// (`channel_bindings_unmapped`) and a loop without its output limits (B3.9), so the refusals below only guard the types.
fn plant_connection<'a>(
    loop_definition: &'a LoopDefinition,
    model: &'a GeneralModel,
    setpoint: &'a LoopSignal,
) -> Result<PlantConnection<'a>, Failure> {
    let identity = model.identity();
    let (Some(model_input), Some(model_output)) =
        (identity.inputs.first(), identity.outputs.first())
    else {
        return Err(compatibility_failure(
            &CompatibilityRefusal::UnmappedChannels,
        ));
    };
    let limits = &loop_definition.limits;
    let (Some(actuator_minimum), Some(actuator_maximum)) =
        (limits.output_minimum, limits.output_maximum)
    else {
        return Err(compatibility_failure(
            &CompatibilityRefusal::InputActuatorRange {
                id: model_input.id.clone(),
            },
        ));
    };
    let point = identity.operating_point.as_ref();
    Ok(PlantConnection {
        manipulated: &loop_definition.signals.manipulated_variable,
        actuator: loop_definition.signals.actuator.as_ref(),
        process: &loop_definition.signals.process_variable,
        setpoint,
        model_input,
        model_output,
        input_origin: point
            .and_then(|point| point.inputs.first().copied())
            .unwrap_or(0.0),
        output_origin: point
            .and_then(|point| point.outputs.first().copied())
            .unwrap_or(0.0),
        actuator_minimum,
        actuator_maximum,
        rate_up: limits.rate_up_per_s,
        rate_down: limits.rate_down_per_s,
    })
}

/// The task period of the loop: a whole number of milliseconds from 1 to 1000 (`TypeScript` `engineerVerificationPlan`).
///
/// # Errors
/// Returns a [`Failure`] when the period is not such a number.
fn task_period_ms(loop_definition: &LoopDefinition) -> Result<f64, Failure> {
    let period = loop_definition.controller.task_period_ms;
    if period.fract() == 0.0 && (1.0..=MAXIMUM_TASK_PERIOD_MS).contains(&period) {
        Ok(period)
    } else {
        Err(Failure::new(
            &texts::task_period_unsupported(),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ))
    }
}

/// The run's duration in ms: the validation duration rounded up to whole task periods. It may not exceed the longest
/// experiment, and it must run past the response deadline (`TypeScript` `engineerVerificationPlan`; A7).
///
/// # Errors
/// Returns a [`Failure`] when the run is longer than the limit or does not run past the deadline.
fn run_duration_ms(task: &EngineerRequirements, period_ms: f64) -> Result<f64, Failure> {
    let duration = (task.duration_seconds * 1_000.0 / period_ms).ceil() * period_ms;
    let deadline = response_deadline_seconds(task.settling_time_seconds, period_ms);
    if whole_ms(duration) > ISOLATED_SIMULATION_DURATION_LIMIT_MS || deadline >= duration / 1_000.0
    {
        return Err(Failure::new(
            &texts::duration_does_not_pass_deadline(),
            Cause::Settings,
            NextAction::OpenRequirements,
        ));
    }
    Ok(duration)
}

/// The symbols the run observes, each once, in the order `TypeScript` lists them: the plant input and its applied value, the
/// measured output and its setpoint, the loop's further signals, then the parameters (`observations`). The controller's
/// states are not listed here: their initial values come from the caller, and their symbols are the commissioning stage's.
fn observed_symbols(
    loop_definition: &LoopDefinition,
    channels: &PlantConnection<'_>,
    parameters: &[Parameter],
) -> Vec<String> {
    let additional = loop_definition.signals.additional.as_deref().unwrap_or(&[]);
    let candidates = [
        Some(channels.manipulated.symbol.as_str()),
        channels.actuator.map(|signal| signal.symbol.as_str()),
        Some(channels.process.symbol.as_str()),
        Some(channels.setpoint.symbol.as_str()),
    ]
    .into_iter()
    .flatten()
    .chain(additional.iter().map(|signal| signal.symbol.as_str()))
    .chain(parameters.iter().map(|parameter| parameter.symbol.as_str()));
    let mut seen = BTreeSet::new();
    candidates
        .filter(|symbol| seen.insert(*symbol))
        .map(str::to_owned)
        .collect()
}

/// Each parameter's value, checked against its bounds, and a `REAL` parameter's value checked for its single-precision
/// representation (`TypeScript` `engineerVerificationPlan`, the parameter loop).
///
/// # Errors
/// Returns a [`Failure`] naming the first parameter that is missing, outside its bounds or not representable.
fn checked_parameters<'a>(
    parameters: &'a [Parameter],
    values: &BTreeMap<String, f64>,
) -> Result<Vec<(&'a Parameter, f64)>, Failure> {
    parameters
        .iter()
        .map(|parameter| {
            let value = values.get(&parameter.symbol).copied().unwrap_or(f64::NAN);
            let in_bounds =
                value.is_finite() && value >= parameter.minimum && value <= parameter.maximum;
            let representable = parameter.iec_type == RealType::Lreal
                || single_precision(value).to_bits() == value.to_bits();
            if in_bounds && representable {
                Ok((parameter, value))
            } else {
                Err(Failure::new(
                    &texts::parameter_not_representable(&parameter.symbol),
                    Cause::Settings,
                    NextAction::Tune,
                ))
            }
        })
        .collect()
}

/// The checks of the controlled output (`TypeScript` `engineerVerificationPlan`, the output's loop): the response deadline and
/// the target error after it (C0, C1), an overshoot limit when the task declares one and the run applies a step (B4.3,
/// N8.3), and the output's bounds. The step's target becomes the setpoint's initial value.
///
/// # Errors
/// Returns a [`Failure`] when the deadline or the target lies outside the run or the declared scope.
fn output_requirements(
    channels: &PlantConnection<'_>,
    task: &EngineerRequirements,
    period_ms: f64,
    window: Window,
    initial_values: &mut BTreeMap<String, f64>,
) -> Result<Vec<RunRequirement>, Failure> {
    let process = channels.process;
    let setpoint = channels.setpoint;
    let model_output = channels.model_output;
    let target = channels.output_origin + task.reference_change;
    let deadline = response_deadline_seconds(task.settling_time_seconds, period_ms);
    let outside = deadline >= window.end
        || outside_range(target, process.range.minimum, process.range.maximum)
        || outside_range(target, setpoint.range.minimum, setpoint.range.maximum)
        || outside_range(
            target,
            model_output.operating_range.minimum,
            model_output.operating_range.maximum,
        );
    if outside {
        return Err(Failure::new(
            &texts::target_outside_scope(&process.symbol),
            Cause::TaskInfeasible,
            NextAction::OpenRequirements,
        ));
    }
    initial_values.insert(setpoint.symbol.clone(), target);
    let mut requirements = vec![
        requirement(
            "response-deadline",
            &process.symbol,
            window,
            RunCheck::Settling {
                target,
                band: task.maximum_error,
                hold_seconds: window.end - deadline,
                deadline_seconds: deadline,
            },
        ),
        requirement(
            "target-error",
            &process.symbol,
            Window {
                start: deadline,
                end: window.end,
            },
            RunCheck::Error {
                reference: ErrorReference::Channel {
                    channel: setpoint.symbol.clone(),
                },
                statistic: ErrorStatistic::Maximum,
                maximum: task.maximum_error,
            },
        ),
    ];
    // A declared overshoot limit bounds the peak of this step, in output units; a run that applies no step declares none.
    if let Some(percent) = task
        .maximum_overshoot_percent
        .filter(|_| task.reference_change != 0.0)
    {
        requirements.push(requirement(
            "overshoot",
            &process.symbol,
            window,
            RunCheck::Overshoot {
                initial: channels.output_origin,
                target,
                maximum: percent / 100.0 * task.reference_change.abs(),
            },
        ));
    }
    requirements.push(requirement(
        "process-bounds",
        &process.symbol,
        window,
        RunCheck::Bounds {
            minimum: process
                .range
                .minimum
                .max(model_output.operating_range.minimum),
            maximum: process
                .range
                .maximum
                .min(model_output.operating_range.maximum),
        },
    ));
    Ok(requirements)
}

/// The checks of the plant input (`TypeScript` `engineerVerificationPlan`, the input's loop): its bounds, the bounds of the
/// requested value when an actuator reports the applied one, and the rates that the actuator allows.
fn input_requirements(channels: &PlantConnection<'_>, window: Window) -> Vec<RunRequirement> {
    let manipulated = channels.manipulated;
    let applied = channels.actuator.unwrap_or(manipulated);
    let mut requirements = vec![requirement(
        "output-bounds",
        &applied.symbol,
        window,
        RunCheck::Bounds {
            minimum: channels
                .actuator_minimum
                .max(channels.model_input.operating_range.minimum),
            maximum: channels
                .actuator_maximum
                .min(channels.model_input.operating_range.maximum),
        },
    )];
    if channels.actuator.is_some() {
        requirements.push(requirement(
            "requested-output-bounds",
            &manipulated.symbol,
            window,
            RunCheck::Bounds {
                minimum: manipulated.range.minimum,
                maximum: manipulated.range.maximum,
            },
        ));
    }
    for (id, direction, maximum) in [
        ("output-rate-up", Movement::Up, channels.rate_up),
        ("output-rate-down", Movement::Down, channels.rate_down),
    ] {
        if let Some(maximum) = maximum {
            requirements.push(requirement(
                id,
                &applied.symbol,
                window,
                RunCheck::Rate {
                    maximum,
                    direction: Some(direction),
                },
            ));
        }
    }
    requirements
}

/// The fixed parameters: each keeps its checked value through the run (C1: the fixed parameter values are checked through
/// every target response).
fn parameter_requirements(parameters: &[(&Parameter, f64)], window: Window) -> Vec<RunRequirement> {
    parameters
        .iter()
        .map(|(parameter, value)| {
            requirement(
                &format!("{FIXED_PARAMETER_PREFIX}{}", parameter.symbol),
                &parameter.symbol,
                window,
                RunCheck::Bounds {
                    minimum: *value,
                    maximum: *value,
                },
            )
        })
        .collect()
}

/// The identity prefix of the requirement that a fixed parameter holds its value through the run; the parameter's symbol
/// follows it.
pub const FIXED_PARAMETER_PREFIX: &str = "fixed-parameter:";

/// One requirement of the run: its identity, its channel, its window and its check.
fn requirement(id: &str, channel: &str, window: Window, check: RunCheck) -> RunRequirement {
    RunRequirement {
        id: id.to_owned(),
        channel: channel.to_owned(),
        window,
        check,
    }
}

/// Whether a value lies below `minimum` or above `maximum`; a `NaN` lies in neither, as in `TypeScript`'s comparisons.
fn outside_range(value: f64, minimum: f64, maximum: f64) -> bool {
    value < minimum || value > maximum
}

/// One check per condition: a requirement whose channel, window and check equal an earlier one's is dropped. The condition
/// is everything but the identity (`TypeScript` `engineerVerificationPlan`, the condition comparison).
fn unique_conditions(requirements: Vec<RunRequirement>) -> Vec<RunRequirement> {
    let mut unique: Vec<RunRequirement> = Vec::with_capacity(requirements.len());
    for candidate in requirements {
        if !unique.iter().any(|kept| same_condition(kept, &candidate)) {
            unique.push(candidate);
        }
    }
    unique
}

/// Whether two requirements check the same channel, in the same window, with the same check.
fn same_condition(left: &RunRequirement, right: &RunRequirement) -> bool {
    left.channel == right.channel && left.window == right.window && left.check == right.check
}

/// The scans per recorded sample (A4.2): the larger of the period that keeps the observed run within the sample and cell
/// limits, and the process's record period, which follows its settling time (`TypeScript` `engineerVerificationPlan`). The
/// ON/OFF output's own rule of that function is not part of this port.
fn record_every(step_ms: u64, duration_ms: u64, observed: usize, settling_seconds: f64) -> u64 {
    let channels = u64::try_from(observed).unwrap_or(u64::MAX);
    let settling_ms = whole_ms(settling_seconds * 1_000.0);
    isolated_record_every(step_ms, duration_ms, channels).max(record_period_executions(
        duration_ms,
        step_ms,
        ISOLATED_SIMULATION_SAMPLE_LIMIT,
        Some(settling_ms),
    ))
}

/// A whole, nonnegative number of milliseconds as an integer. The callers pass whole values that they have checked against
/// their limits, so the conversion is exact; a value beyond `u64` saturates, and the limits then refuse it.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Exact for the checked whole values; saturating otherwise.
fn whole_ms(value: f64) -> u64 {
    value as u64
}

#[cfg(test)]
#[path = "engineer_verification_plan_tests.rs"]
mod tests;
