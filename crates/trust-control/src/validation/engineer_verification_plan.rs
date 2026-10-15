//! The verification plan of a loop (specification 35, B5.3, C0, C1 and C8, A6.4 and B4.3; numerical annex N8.2 and N8.3;
//! `TypeScript` `engineerVerificationPlan.ts`): the task's requirements as the run checks them, the run's sample grid, its initial
//! values and the symbols it observes. The plan covers every model input and output that the loop's channels bind (`bound_channels`):
//! each controlled output's response and bounds, each actuated input's bounds and rates, and every input's value at its operating point.
//! A measured disturbance is not a controller output, so nothing bounds it (C8). The requirement record, the model compatibility guard
//! and the criteria checks are the existing Rust rules that the plan calls.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::commissioning::initial_state::{operating_input, operating_output};
use crate::commissioning::project_law::additional_mode_values;
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
use crate::loop_definition::{ChannelSignal, LoopDefinition, Parameter, RealType};
use crate::model::general::js_text;
use crate::model::simulation::ActuatorKind;
use crate::model::GeneralModel;
use crate::tuning::automatic_criteria::{validate_criteria, AutomaticCriteria};
use crate::tuning::objective::{TuningMethod, TuningObjective};
use crate::tuning::requirements::{response_deadline_seconds, TuningRequirements};
use crate::tuning::settings_pins::single_precision;
use crate::validation::gain_schedule_case;
use crate::validation::isolated_run_sampling::{isolated_record_every, scan_count};
use crate::validation::model_route::{bound_channels, BoundChannels, BoundOutput};
use crate::validation::on_off_case;
use crate::validation::run_validation::{
    ErrorReference, ErrorStatistic, Movement, RunCheck, RunRequirement, Window,
};
use crate::validation::texts;

pub use crate::tuning::requirements::MAXIMUM_OVERSHOOT_PERCENT;

/// The longest task period the runtime adapter runs, in whole milliseconds (`TypeScript` `engineerVerificationPlan`).
const MAXIMUM_TASK_PERIOD_MS: f64 = 1_000.0;

/// The task requirements of a verification (`TypeScript` `EngineerRequirements`). It is the tuning requirement record, which
/// holds the same values and checks.
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
    /// The values the run starts from, by symbol: the mode, the plant inputs at their operating points and the setpoint target.
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

/// What the checks of the outputs share: the model they are read on, the task they hold the loop to, the task period and the run's
/// window.
struct Checks<'a> {
    /// The model whose operating point and ranges the checks read.
    model: &'a GeneralModel,
    /// The task's requirements.
    task: &'a EngineerRequirements,
    /// The task period, in ms.
    period_ms: f64,
    /// The run's window.
    window: Window,
}

/// Plans the verification run of a loop tuned on `model` (`TypeScript` `engineerVerificationPlan`). The task's requirements become the
/// run's checks (C0, C1, B4.3): for each controlled output, the response deadline and the target error after it, an overshoot limit when
/// the task declares one and the run applies a step, and the output's bounds; for each actuated input, its bounds, the bounds of the
/// requested value when an actuator reports the applied one, and the rates that the actuator allows; and the bounds of the fixed
/// parameters. The run takes the sample grid of its duration (A4.2). `values` are the controller's parameter values; `controller_initial`
/// are the controller's initial values at the model's operating point, which the caller computes (commissioning).
///
/// # Errors
/// Returns the failure of [`LoopDefinition::rust_engine_support`] first, when the loop uses a feature the Rust engine does not run (a
/// PID group, a cascade, a declared analysis, and the other parked features) or has no controller law. Then it returns a [`Failure`]
/// when the loop has no parameter or setpoint binding, the model is not compatible with the loop, its channels do not bind it
/// (`channel_bindings_unmapped`), the task period is not a whole number of milliseconds from 1 to 1000, the run is too long, does not
/// pass its response deadline or needs too many scans, too many channels are observed, a parameter is out of its bounds or not
/// representable, or a target lies outside the declared scope.
pub fn engineer_verification_plan(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    task: &EngineerRequirements,
    values: &BTreeMap<String, f64>,
    controller_initial: &BTreeMap<String, f64>,
) -> Result<VerificationPlan, Failure> {
    // Before anything else, the Rust engine's gate refuses the parked features (checklist V7) and a loop without a law.
    loop_definition.rust_engine_support()?;
    let Some(binding) = &loop_definition.parameter_binding else {
        return Err(binding_missing());
    };
    if loop_definition.signals.setpoint.is_none() {
        return Err(binding_missing());
    }
    assert_engineer_model_compatibility(loop_definition, model)
        .map_err(|refusal| compatibility_failure(&refusal))?;
    let bound = bound_channels(loop_definition, model)?;
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
    let observed = observed_symbols(loop_definition, &bound, &binding.parameters);
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
    // A cascade's outer controller starts in Auto with the inner one, as its additional mode (C6).
    initial_values.extend(additional_mode_values(loop_definition));
    // Each input starts at its operating value, a measured disturbance included, so that it holds that value unless a case writes it.
    for (index, input) in bound.inputs.iter().enumerate() {
        let origin = operating_input(model, index);
        initial_values.insert(input.command_symbol.clone(), origin);
        initial_values.insert(input.signal.symbol.to_owned(), origin);
    }
    let checks = Checks {
        model,
        task,
        period_ms,
        window,
    };
    let mut requirements =
        output_requirements(&bound, &checks, loop_definition, &mut initial_values)?;
    requirements.extend(input_requirements(&bound, window));
    requirements.extend(on_off_case::switching_requirements(
        loop_definition,
        values,
        window,
    )?);
    requirements.extend(parameter_requirements(&parameters, window));
    // A switching controller's levels and dwell times are judged at every execution (C3), so its run records every one.
    let every = if loop_definition.controller.on_off.is_some() {
        on_off_case::record_every(step_ms, duration_ms, observed.len())
    } else {
        record_every(
            step_ms,
            duration_ms,
            observed.len(),
            task.settling_time_seconds,
        )
    };
    Ok(VerificationPlan {
        step_ms,
        duration_ms,
        max_total_cycles: scans,
        record_every: every,
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

/// The refusal of a controlled output that has a setpoint but no requirements to hold it to (`TypeScript` `Declare requirements`).
pub(crate) fn output_requirements_missing(id: &str) -> Failure {
    Failure::new(
        &format!("Declare requirements for controlled output {id}."),
        Cause::Settings,
        NextAction::OpenRequirements,
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
/// whose channels do not bind it has the channel-binding sentence (`channel_bindings_unmapped`, from `loopChannels.ts`); the other
/// sentences are those of [`compatibility_text`].
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

/// The symbols the run observes, each once, in the order `TypeScript` lists them: each input's signal and its applied value, each
/// output's measurement and its setpoint, the loop's further signals, then the parameters (`observations`), then a schedule's active gains
/// (C10). The controller's states are
/// not listed here: their initial values come from the caller, and their symbols are the commissioning stage's.
fn observed_symbols(
    loop_definition: &LoopDefinition,
    bound: &BoundChannels<'_>,
    parameters: &[Parameter],
) -> Vec<String> {
    let additional = loop_definition.signals.additional.as_deref().unwrap_or(&[]);
    let inputs = bound.inputs.iter().flat_map(|input| {
        std::iter::once(input.signal.symbol).chain(input.applied.map(|applied| applied.symbol))
    });
    let outputs = bound.outputs.iter().flat_map(|output| {
        std::iter::once(output.signal.symbol)
            .chain(output.reference.map(|reference| reference.symbol))
    });
    let active_gains = gain_schedule_case::observed_active_gains(loop_definition);
    let candidates = inputs
        .chain(outputs)
        .chain(additional.iter().map(|signal| signal.symbol.as_str()))
        .chain(parameters.iter().map(|parameter| parameter.symbol.as_str()))
        // A switching controller's latch and setpoint are read at every scan, for its criteria (C3).
        .chain(
            loop_definition
                .controller
                .on_off
                .as_ref()
                .and_then(|block| block.latch_symbol()),
        )
        .chain(
            loop_definition
                .signals
                .setpoint
                .as_ref()
                .map(|setpoint| setpoint.symbol.as_str()),
        )
        .chain(active_gains.iter().map(String::as_str));
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

/// The checks of the measured outputs (`TypeScript` `engineerVerificationPlan`, the output loop): an output with a setpoint has its response
/// deadline and target error, and an overshoot limit when the task declares one and the run applies a step (B4.3, N8.3); every output has its
/// bounds. A step's target becomes the setpoint's initial value.
///
/// # Errors
/// An output with a setpoint has no requirements to hold it to; or a response deadline or a target lies outside the run or the declared scope.
fn output_requirements(
    bound: &BoundChannels<'_>,
    checks: &Checks<'_>,
    loop_definition: &LoopDefinition,
    initial_values: &mut BTreeMap<String, f64>,
) -> Result<Vec<RunRequirement>, Failure> {
    let several = bound.outputs.len() > 1;
    let mut requirements = Vec::new();
    for (index, output) in bound.outputs.iter().enumerate() {
        let prefix = if several {
            format!("{}:", output.channel.id)
        } else {
            String::new()
        };
        if let Some(reference) = output.reference {
            let required = output_task(output, loop_definition, checks.task)
                .ok_or_else(|| output_requirements_missing(&output.channel.id))?;
            requirements.extend(response_requirements(
                checks,
                output,
                reference,
                &required,
                index,
                &prefix,
                initial_values,
            )?);
        }
        requirements.push(output_bounds(checks, output, &prefix));
    }
    Ok(requirements)
}

/// The task an output is held to: the channel's own requirements when it declares them, else the task itself for the loop's process
/// variable (`TypeScript` `output.requirements ?? (output.signal.symbol === processVariable.symbol ? task : undefined)`). A channel's own
/// requirements carry no overshoot limit.
fn output_task(
    output: &BoundOutput<'_>,
    loop_definition: &LoopDefinition,
    task: &EngineerRequirements,
) -> Option<EngineerRequirements> {
    match output.requirements {
        Some(own) => Some(EngineerRequirements {
            reference_change: own.reference_change,
            maximum_error: own.maximum_error,
            settling_band: own.settling_band,
            settling_time_seconds: own.settling_time_seconds,
            maximum_overshoot_percent: None,
            ..task.clone()
        }),
        None if output.signal.symbol
            == loop_definition.signals.process_variable.symbol.as_str() =>
        {
            Some(task.clone())
        }
        None => None,
    }
}

/// The response checks of one controlled output (`TypeScript` `engineerVerificationPlan`, the output loop): the target is the output's operating
/// value plus the reference change; the deadline and the target must lie inside the run and inside every declared range; the target becomes
/// the setpoint's initial value; then the settling and error checks, and the overshoot limit when the task declares one.
///
/// # Errors
/// The response deadline is not inside the run, or the target lies outside the output's, the setpoint's or the model's range
/// (`target_outside_scope`).
fn response_requirements(
    checks: &Checks<'_>,
    output: &BoundOutput<'_>,
    reference: ChannelSignal<'_>,
    required: &EngineerRequirements,
    index: usize,
    prefix: &str,
    initial_values: &mut BTreeMap<String, f64>,
) -> Result<Vec<RunRequirement>, Failure> {
    let window = checks.window;
    let origin = operating_output(checks.model, index);
    let target = origin + required.reference_change;
    let deadline = response_deadline_seconds(required.settling_time_seconds, checks.period_ms);
    let model_range = output.channel.operating_range;
    let outside = deadline >= window.end
        || outside_range(
            target,
            output.signal.range.minimum,
            output.signal.range.maximum,
        )
        || outside_range(target, reference.range.minimum, reference.range.maximum)
        || outside_range(target, model_range.minimum, model_range.maximum);
    if outside {
        return Err(Failure::new(
            &texts::target_outside_scope(output.signal.symbol),
            Cause::TaskInfeasible,
            NextAction::OpenRequirements,
        ));
    }
    initial_values.insert(reference.symbol.to_owned(), target);
    let symbol = output.signal.symbol;
    let mut requirements = vec![
        requirement(
            &format!("{prefix}response-deadline"),
            symbol,
            window,
            RunCheck::Settling {
                target,
                band: required.maximum_error,
                hold_seconds: window.end - deadline,
                deadline_seconds: deadline,
            },
        ),
        requirement(
            &format!("{prefix}target-error"),
            symbol,
            Window {
                start: deadline,
                end: window.end,
            },
            RunCheck::Error {
                reference: ErrorReference::Channel {
                    channel: reference.symbol.to_owned(),
                },
                statistic: ErrorStatistic::Maximum,
                maximum: required.maximum_error,
            },
        ),
    ];
    // A declared overshoot limit bounds the peak of this step, in output units; a run that applies no step declares none.
    if let Some(percent) = required
        .maximum_overshoot_percent
        .filter(|_| required.reference_change != 0.0)
    {
        requirements.push(requirement(
            &format!("{prefix}overshoot"),
            symbol,
            window,
            RunCheck::Overshoot {
                initial: origin,
                target,
                maximum: percent / 100.0 * required.reference_change.abs(),
            },
        ));
    }
    Ok(requirements)
}

/// The bounds of a measured output (`TypeScript` `process-bounds`): its own range, within the range that its model output may take.
fn output_bounds(checks: &Checks<'_>, output: &BoundOutput<'_>, prefix: &str) -> RunRequirement {
    let model_range = output.channel.operating_range;
    requirement(
        &format!("{prefix}process-bounds"),
        output.signal.symbol,
        checks.window,
        RunCheck::Bounds {
            minimum: output.signal.range.minimum.max(model_range.minimum),
            maximum: output.signal.range.maximum.min(model_range.maximum),
        },
    )
}

/// The checks of the actuated inputs (`TypeScript` `engineerVerificationPlan`, the input loop): the bounds of each input, the bounds of the
/// requested value when an actuator reports the applied one, and the rates that the actuator allows. A measured disturbance has none
/// (C8): scenarios move it, and nothing bounds it as an output.
fn input_requirements(bound: &BoundChannels<'_>, window: Window) -> Vec<RunRequirement> {
    let actuated = bound
        .inputs
        .iter()
        .filter(|input| !input.disturbance)
        .count();
    let mut requirements = Vec::new();
    for input in bound.inputs.iter().filter(|input| !input.disturbance) {
        let prefix = if actuated == 1 {
            String::new()
        } else {
            format!("{}:", input.channel.id)
        };
        let applied = input.applied.unwrap_or(input.signal);
        if input.actuator.kind == ActuatorKind::OnOff {
            requirements.extend(on_off_case::actuator_requirements(
                &input.actuator,
                applied.symbol,
                &prefix,
                window,
            ));
        }
        requirements.push(requirement(
            &format!("{prefix}output-bounds"),
            applied.symbol,
            window,
            RunCheck::Bounds {
                minimum: input
                    .actuator
                    .minimum
                    .max(input.channel.operating_range.minimum),
                maximum: input
                    .actuator
                    .maximum
                    .min(input.channel.operating_range.maximum),
            },
        ));
        if input.applied.is_some() {
            requirements.push(requirement(
                &format!("{prefix}requested-output-bounds"),
                input.signal.symbol,
                window,
                RunCheck::Bounds {
                    minimum: input.signal.range.minimum,
                    maximum: input.signal.range.maximum,
                },
            ));
        }
        for (id, direction, maximum) in [
            ("output-rate-up", Movement::Up, input.actuator.rate_up_per_s),
            (
                "output-rate-down",
                Movement::Down,
                input.actuator.rate_down_per_s,
            ),
        ] {
            if let Some(maximum) = maximum {
                requirements.push(requirement(
                    &format!("{prefix}{id}"),
                    applied.symbol,
                    window,
                    RunCheck::Rate {
                        maximum,
                        direction: Some(direction),
                    },
                ));
            }
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
