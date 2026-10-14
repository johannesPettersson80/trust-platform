//! Supported controller-state alignment, evaluated at the owning task boundary.
use super::*;
#[path = "initialization_formulas.rs"]
mod formulas;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceChannel {
    reference_symbol: String,
    process_variable: String,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputChannel {
    manual_command_symbol: String,
    output_symbol: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Tracking {
    kind: String,
    reference_symbol: String,
    manual_command_symbol: String,
    pub(super) process_variable: String,
    gain_symbol: Option<String>,
    align_reference: Option<bool>,
    proportional_gain_symbol: Option<String>,
    integral_gain_symbol: Option<String>,
    derivative_gain_symbol: Option<String>,
    integral_state_symbol: Option<String>,
    derivative_state_symbol: Option<String>,
    proportional_setpoint_weight: Option<f64>,
    derivative_setpoint_weight: Option<f64>,
    cycle_seconds: Option<f64>,
    align_output: Option<bool>,
    initialization_values: Option<BTreeMap<String, f64>>,
    initialization_formulas: Option<BTreeMap<String, formulas::Formula>>,
    bounded_state_symbols: Option<Vec<String>>,
    #[serde(default)]
    additional_references: Vec<ReferenceChannel>,
    #[serde(default)]
    additional_commands: Vec<OutputChannel>,
    #[serde(default)]
    additional_measurements: Vec<String>,
}

pub(super) fn validate(binding: &Binding, policy: &Tracking, manual: bool) -> Result<(), String> {
    if policy.kind != "declared-state-v1"
        && (policy.initialization_values.is_some()
            || policy.initialization_formulas.is_some()
            || policy.bounded_state_symbols.is_some()
            || !policy.additional_references.is_empty()
            || !policy.additional_commands.is_empty()
            || !binding.additional_outputs.is_empty()
            || !policy.additional_measurements.is_empty())
    {
        return Err("declared-state fields cannot be mixed with another tracking law".into());
    }
    match policy.kind.as_str() {
        "declared-state-v1" => validate_declared_states(binding, policy, manual),
        "proportional" => validate_proportional(binding, policy, manual),
        "parallel-pid-v1" => validate_parallel_pid(binding, policy, manual),
        _ => Err("unsupported controller-state tracking kind".into()),
    }
}

pub(super) fn accepts_live_state(policy: &Tracking, symbol: &str) -> bool {
    policy.kind == "parallel-pid-v1"
        && (policy.integral_state_symbol.as_deref() == Some(symbol)
            || policy.derivative_state_symbol.as_deref() == Some(symbol))
        || policy.kind == "declared-state-v1"
            && policy
                .initialization_values
                .as_ref()
                .is_some_and(|values| values.contains_key(symbol))
}

pub(super) fn bounded_post_scan_state(policy: &Tracking, symbol: &str) -> bool {
    policy.kind == "declared-state-v1"
        && policy
            .bounded_state_symbols
            .as_ref()
            .is_some_and(|symbols| symbols.iter().any(|value| value == symbol))
}

pub(super) fn accepts_live_measurement(policy: &Tracking, symbol: &str) -> bool {
    policy.kind == "declared-state-v1"
        && (policy
            .additional_references
            .iter()
            .any(|channel| channel.process_variable == symbol)
            || policy
                .additional_measurements
                .iter()
                .any(|measurement| measurement == symbol))
}

pub(super) fn fresh_applied_feedback(
    member: &NumberMember,
    storage: &VariableStorage,
    io_bindings: &[IoBinding],
) -> Result<f64, String> {
    let reference = parameters::resolve_path(storage, &member.symbol)?.reference;
    let mapped = io_bindings.iter().any(|binding| {
        binding.address.area == IoArea::Input
            && match &binding.target {
                IoTarget::Name(name) => name.eq_ignore_ascii_case(&member.symbol),
                IoTarget::Reference(value) => value == &reference,
            }
    });
    if !mapped {
        return Err(
            "applied actuator feedback must be acquired through a declared input binding".into(),
        );
    }
    let value = number(storage, member)?;
    if value < member.minimum || value > member.maximum {
        return Err(format!(
            "applied actuator feedback {} = {value} is outside its reviewed range [{}, {}]",
            member.symbol, member.minimum, member.maximum
        ));
    }
    Ok(value)
}

pub(super) fn held_output(
    output: &NumberMember,
    storage: &VariableStorage,
    io_bindings: &[IoBinding],
    snapshot: Option<&crate::io::IoSnapshot>,
) -> Result<f64, String> {
    let reference = parameters::resolve_path(storage, &output.symbol)?.reference;
    let mapped = io_bindings.iter().find(|entry| match &entry.target {
        IoTarget::Name(name) => name.eq_ignore_ascii_case(&output.symbol),
        IoTarget::Reference(value) => value == &reference,
    });
    // An output the program hands on in its own code, such as a PID's output that drives a heater
    // block owning the I/O, holds what the last scan left at the task boundary: the value that
    // scan drove the plant with (specification 35, §17.1). A mapped one holds what the last I/O
    // commit sent.
    let Some(mapped) = mapped else {
        return reviewed_image(output, number(storage, output)?);
    };
    let value = snapshot
        .and_then(|snapshot| super::super::recording::snapshot_value(snapshot, &mapped.address))
        .ok_or("committed output snapshot unavailable")?;
    let number = match (output.iec_type.as_str(), value) {
        ("REAL", Value::Real(value)) => f64::from(*value),
        ("LREAL", Value::LReal(value)) => *value,
        ("BOOL", Value::Bool(value)) => f64::from(u8::from(*value)),
        _ => return Err("committed output IEC type mismatch".into()),
    };
    reviewed_image(output, number)
}

/// A committed output's value: a finite number inside its reviewed range (A5.3, E6.6).
fn reviewed_image(output: &NumberMember, number: f64) -> Result<f64, String> {
    if !number.is_finite() {
        return Err(format!(
            "committed output {} has bad quality: not a finite number",
            output.symbol
        ));
    }
    if number < output.minimum || number > output.maximum {
        return Err(format!(
            "committed output {} = {number} is outside its reviewed range [{}, {}]",
            output.symbol, output.minimum, output.maximum
        ));
    }
    Ok(number)
}

pub(super) fn calculate(
    binding: &Binding,
    policy: &Tracking,
    requested: &BTreeMap<String, f64>,
    storage: &VariableStorage,
    outputs: &BTreeMap<String, f64>,
    manual: bool,
) -> Result<BTreeMap<String, f64>, String> {
    let output = *outputs
        .get(&binding.output.symbol)
        .ok_or("committed output is missing")?;
    match policy.kind.as_str() {
        "declared-state-v1" => {
            let mut values = requested.clone();
            // An Auto entry sets each reference to its measurement, so that its first output is bumpless (E6.4, A6.4).
            let align_references = policy.align_output == Some(true);
            if align_references {
                let pv = guard_number(binding, storage, &policy.process_variable)?;
                values.insert(
                    policy.reference_symbol.clone(),
                    rounded(state(binding, &policy.reference_symbol)?, pv)?,
                );
            }
            values.insert(
                policy.manual_command_symbol.clone(),
                rounded(state(binding, &policy.manual_command_symbol)?, output)?,
            );
            for channel in &policy.additional_references {
                if align_references {
                    let pv = guard_number(binding, storage, &channel.process_variable)?;
                    values.insert(
                        channel.reference_symbol.clone(),
                        rounded(state(binding, &channel.reference_symbol)?, pv)?,
                    );
                }
            }
            for channel in &policy.additional_commands {
                let held = *outputs
                    .get(&channel.output_symbol)
                    .ok_or("committed additional output is missing")?;
                values.insert(
                    channel.manual_command_symbol.clone(),
                    rounded(state(binding, &channel.manual_command_symbol)?, held)?,
                );
            }
            for (symbol, initial) in policy
                .initialization_values
                .as_ref()
                .ok_or("initialization values are missing")?
            {
                let member = state(binding, symbol)?;
                let value = if policy.align_output == Some(true) {
                    if let Some(formula) = policy
                        .initialization_formulas
                        .as_ref()
                        .and_then(|formulas| formulas.get(symbol))
                    {
                        formulas::calculate(binding, formula, storage, outputs)?
                    } else {
                        *initial
                    }
                } else {
                    number(storage, member)?
                };
                values.insert(symbol.clone(), rounded(member, value)?);
            }
            Ok(values)
        }
        "proportional" => calculate_proportional(binding, policy, requested, storage, output),
        "parallel-pid-v1" => {
            calculate_parallel_pid(binding, policy, requested, storage, output, manual)
        }
        _ => Err("unsupported controller-state tracking kind".into()),
    }
}

fn validate_proportional(binding: &Binding, policy: &Tracking, manual: bool) -> Result<(), String> {
    let gain = policy
        .gain_symbol
        .as_ref()
        .ok_or("tracking P gain is missing")?;
    let align = policy
        .align_reference
        .ok_or("tracking P reference policy is missing")?;
    if policy.reference_symbol == policy.manual_command_symbol
        || policy.process_variable == *gain
        || align && manual
        || !states(
            binding,
            [&policy.reference_symbol, &policy.manual_command_symbol],
        )
        || !guards(binding, [&policy.process_variable, gain])
        || policy.proportional_gain_symbol.is_some()
        || policy.integral_gain_symbol.is_some()
        || policy.derivative_gain_symbol.is_some()
        || policy.integral_state_symbol.is_some()
        || policy.derivative_state_symbol.is_some()
        || policy.proportional_setpoint_weight.is_some()
        || policy.derivative_setpoint_weight.is_some()
        || policy.cycle_seconds.is_some()
        || policy.align_output.is_some()
    {
        return Err("tracking must name distinct declared P state and guard roles".into());
    }
    Ok(())
}

fn validate_parallel_pid(binding: &Binding, policy: &Tracking, manual: bool) -> Result<(), String> {
    let proportional_gain = policy
        .proportional_gain_symbol
        .as_ref()
        .ok_or("tracking PID proportional gain is missing")?;
    let integral_gain = policy
        .integral_gain_symbol
        .as_ref()
        .ok_or("tracking PID integral gain is missing")?;
    let derivative_gain = policy
        .derivative_gain_symbol
        .as_ref()
        .ok_or("tracking PID derivative gain is missing")?;
    let integral_state = policy
        .integral_state_symbol
        .as_ref()
        .ok_or("tracking PID integral state is missing")?;
    let derivative_state = policy
        .derivative_state_symbol
        .as_ref()
        .ok_or("tracking PID derivative state is missing")?;
    let proportional_weight = policy
        .proportional_setpoint_weight
        .ok_or("tracking PID proportional weight is missing")?;
    let derivative_weight = policy
        .derivative_setpoint_weight
        .ok_or("tracking PID derivative weight is missing")?;
    let cycle_seconds = policy
        .cycle_seconds
        .ok_or("tracking PID cycle time is missing")?;
    let _align_output = policy
        .align_output
        .ok_or("tracking PID output-alignment policy is missing")?;
    let state_symbols = [
        &policy.reference_symbol,
        &policy.manual_command_symbol,
        integral_state,
        derivative_state,
    ];
    let guard_symbols = [
        &policy.process_variable,
        proportional_gain,
        integral_gain,
        derivative_gain,
    ];
    if manual && _align_output
        || has_duplicate(&state_symbols)
        || has_duplicate(&guard_symbols)
        || state_symbols
            .iter()
            .any(|state| guard_symbols.contains(state))
        || !states(binding, state_symbols)
        || !guards(binding, guard_symbols)
        || !(0.0..=1.0).contains(&proportional_weight)
        || !(0.0..=1.0).contains(&derivative_weight)
        || !cycle_seconds.is_finite()
        || !(0.0..=60.0).contains(&cycle_seconds)
        || policy.gain_symbol.is_some()
        || policy.align_reference.is_some()
    {
        return Err(
            "tracking must name distinct declared parallel PID states, gains and timing".into(),
        );
    }
    Ok(())
}

fn states<const N: usize>(binding: &Binding, symbols: [&String; N]) -> bool {
    symbols.iter().all(|symbol| {
        binding
            .states
            .iter()
            .any(|member| &member.symbol == *symbol)
    })
}

fn guards<const N: usize>(binding: &Binding, symbols: [&String; N]) -> bool {
    symbols.iter().all(|symbol| {
        binding
            .guards
            .iter()
            .any(|member| &member.symbol == *symbol)
    })
}

fn has_duplicate<const N: usize>(symbols: &[&String; N]) -> bool {
    symbols
        .iter()
        .enumerate()
        .any(|(index, symbol)| symbols[..index].contains(symbol))
}

fn calculate_proportional(
    binding: &Binding,
    policy: &Tracking,
    requested: &BTreeMap<String, f64>,
    storage: &VariableStorage,
    output: f64,
) -> Result<BTreeMap<String, f64>, String> {
    let mut values = requested.clone();
    let member = |symbol: &str| state(binding, symbol);
    let gain_symbol = policy
        .gain_symbol
        .as_deref()
        .ok_or("tracking P gain is missing")?;
    let align_reference = policy
        .align_reference
        .ok_or("tracking P reference policy is missing")?;
    values.insert(
        policy.manual_command_symbol.clone(),
        rounded(member(&policy.manual_command_symbol)?, output)?,
    );
    if align_reference {
        let gain = binding
            .guards
            .iter()
            .find(|member| member.symbol == gain_symbol)
            .ok_or("tracking gain unavailable")?;
        let pv = binding
            .guards
            .iter()
            .find(|member| member.symbol == policy.process_variable)
            .ok_or("tracking PV unavailable")?;
        let gain = number(storage, gain)?;
        if gain == 0.0 {
            return Err("a zero P gain cannot align the reference".into());
        }
        let reference = number(storage, pv)? + output / gain;
        values.insert(
            policy.reference_symbol.clone(),
            rounded(member(&policy.reference_symbol)?, reference)?,
        );
    }
    Ok(values)
}

fn calculate_parallel_pid(
    binding: &Binding,
    policy: &Tracking,
    requested: &BTreeMap<String, f64>,
    storage: &VariableStorage,
    output: f64,
    manual: bool,
) -> Result<BTreeMap<String, f64>, String> {
    let mut values = requested.clone();
    let proportional_gain_symbol = policy
        .proportional_gain_symbol
        .as_deref()
        .ok_or("tracking PID proportional gain is missing")?;
    let integral_gain_symbol = policy
        .integral_gain_symbol
        .as_deref()
        .ok_or("tracking PID integral gain is missing")?;
    let derivative_gain_symbol = policy
        .derivative_gain_symbol
        .as_deref()
        .ok_or("tracking PID derivative gain is missing")?;
    let integral_state_symbol = policy
        .integral_state_symbol
        .as_deref()
        .ok_or("tracking PID integral state is missing")?;
    let derivative_state_symbol = policy
        .derivative_state_symbol
        .as_deref()
        .ok_or("tracking PID derivative state is missing")?;
    let proportional_weight = policy
        .proportional_setpoint_weight
        .ok_or("tracking PID proportional weight is missing")?;
    let derivative_weight = policy
        .derivative_setpoint_weight
        .ok_or("tracking PID derivative weight is missing")?;
    let cycle_seconds = policy
        .cycle_seconds
        .ok_or("tracking PID cycle time is missing")?;
    let align_output = policy
        .align_output
        .ok_or("tracking PID output-alignment policy is missing")?;
    if !align_output {
        if manual {
            values.insert(
                policy.manual_command_symbol.clone(),
                rounded(state(binding, &policy.manual_command_symbol)?, output)?,
            );
        }
        return Ok(values);
    }
    let reference = *requested
        .get(&policy.reference_symbol)
        .ok_or("tracking PID reference value is missing")?;
    let proportional_gain = guard_number(binding, storage, proportional_gain_symbol)?;
    let integral_gain = guard_number(binding, storage, integral_gain_symbol)?;
    let _derivative_gain = guard_number(binding, storage, derivative_gain_symbol)?;
    let process_variable = guard_number(binding, storage, &policy.process_variable)?;
    let proportional = proportional_gain * (proportional_weight * reference - process_variable);
    let integral =
        output - proportional - integral_gain * (reference - process_variable) * cycle_seconds;
    let derivative_state = derivative_weight * reference - process_variable;
    values.insert(
        policy.manual_command_symbol.clone(),
        rounded(state(binding, &policy.manual_command_symbol)?, output)?,
    );
    values.insert(
        integral_state_symbol.into(),
        rounded(state(binding, integral_state_symbol)?, integral)?,
    );
    values.insert(
        derivative_state_symbol.into(),
        rounded(state(binding, derivative_state_symbol)?, derivative_state)?,
    );
    Ok(values)
}

/** The declared parallel-PID adapter owns the one-scan state equation used for confirmation. */
pub(super) fn expected_state_after_scan(
    binding: &Binding,
    policy: &Tracking,
    applied: &BTreeMap<String, f64>,
    storage: &VariableStorage,
    member: &NumberMember,
    manual: bool,
) -> Result<f64, String> {
    let initial = *applied
        .get(&member.symbol)
        .ok_or("applied tracking state unavailable")?;
    if manual || policy.kind != "parallel-pid-v1" {
        return Ok(initial);
    }
    let integral_state = policy
        .integral_state_symbol
        .as_deref()
        .ok_or("tracking PID integral state is missing")?;
    let derivative_state = policy
        .derivative_state_symbol
        .as_deref()
        .ok_or("tracking PID derivative state is missing")?;
    let reference = number(storage, state(binding, &policy.reference_symbol)?)?;
    let process_variable = guard_number(binding, storage, &policy.process_variable)?;
    if member.symbol == integral_state {
        let integral_gain = guard_number(
            binding,
            storage,
            policy
                .integral_gain_symbol
                .as_deref()
                .ok_or("tracking PID integral gain is missing")?,
        )?;
        let cycle_seconds = policy
            .cycle_seconds
            .ok_or("tracking PID cycle time is missing")?;
        return rounded(
            member,
            initial + integral_gain * (reference - process_variable) * cycle_seconds,
        );
    }
    if member.symbol == derivative_state {
        let weight = policy
            .derivative_setpoint_weight
            .ok_or("tracking PID derivative weight is missing")?;
        return rounded(member, weight * reference - process_variable);
    }
    Ok(initial)
}

fn state<'a>(binding: &'a Binding, symbol: &str) -> Result<&'a NumberMember, String> {
    binding
        .states
        .iter()
        .find(|member| member.symbol == symbol)
        .ok_or("tracking state unavailable".into())
}

fn guard_number(binding: &Binding, storage: &VariableStorage, symbol: &str) -> Result<f64, String> {
    let member = binding
        .guards
        .iter()
        .find(|member| member.symbol == symbol)
        .ok_or("tracking guard unavailable")?;
    number(storage, member)
}

fn rounded(member: &NumberMember, value: f64) -> Result<f64, String> {
    let value = if member.iec_type == "REAL" {
        f64::from(value as f32)
    } else {
        value
    };
    write_value(member, value)?;
    Ok(value)
}

fn validate_declared_states(
    binding: &Binding,
    policy: &Tracking,
    manual: bool,
) -> Result<(), String> {
    let values = policy
        .initialization_values
        .as_ref()
        .ok_or("initialization values are missing")?;
    let bounded = policy
        .bounded_state_symbols
        .as_ref()
        .ok_or("post-scan state policy is missing")?;
    if manual && policy.align_output == Some(true)
        || policy.align_output.is_none()
        || policy.reference_symbol == policy.manual_command_symbol
        || !states(
            binding,
            [&policy.reference_symbol, &policy.manual_command_symbol],
        )
        || !guards(binding, [&policy.process_variable])
        || values.len() + 2 + policy.additional_references.len() + policy.additional_commands.len()
            != binding.states.len()
        || values.contains_key(&policy.reference_symbol)
        || values.contains_key(&policy.manual_command_symbol)
        // A bounded state is an initialized one, or the manual output the program sets from the
        // output in Auto, so Manual takes over where the loop is (specification 35, §17.1).
        || bounded.len() > values.len() + 1
        || bounded.iter().collect::<HashSet<_>>().len() != bounded.len()
        || bounded
            .iter()
            .any(|symbol| !values.contains_key(symbol) && symbol != &policy.manual_command_symbol)
        || policy.gain_symbol.is_some()
        || policy.proportional_gain_symbol.is_some()
        || policy.integral_gain_symbol.is_some()
        || policy.derivative_gain_symbol.is_some()
    {
        return Err(
            "initialization must name the complete, distinct declared controller states".into(),
        );
    }
    if policy.additional_measurements.len() > 16
        || policy
            .additional_measurements
            .iter()
            .any(|symbol| !guards(binding, [symbol]))
    {
        return Err("additional measurements must name declared process guards".into());
    }
    let mut roles = HashSet::from([
        policy.reference_symbol.as_str(),
        policy.manual_command_symbol.as_str(),
    ]);
    let mut outputs = HashSet::new();
    if policy.additional_commands.len() != binding.additional_outputs.len()
        || policy.additional_references.len() > 15
    {
        return Err("declared tracking must cover every additional actuator exactly once".into());
    }
    for channel in &policy.additional_references {
        if !roles.insert(&channel.reference_symbol)
            || values.contains_key(&channel.reference_symbol)
            || !states(binding, [&channel.reference_symbol])
            || !guards(binding, [&channel.process_variable])
        {
            return Err("invalid additional reference tracking role".into());
        }
    }
    for channel in &policy.additional_commands {
        if !roles.insert(&channel.manual_command_symbol)
            || values.contains_key(&channel.manual_command_symbol)
            || !states(binding, [&channel.manual_command_symbol])
            || !outputs.insert(&channel.output_symbol)
            || !binding
                .additional_outputs
                .iter()
                .any(|output| output.symbol == channel.output_symbol)
        {
            return Err("invalid additional actuator tracking role".into());
        }
    }
    if let Some(formulas) = &policy.initialization_formulas {
        for (symbol, formula) in formulas {
            if !values.contains_key(symbol) {
                return Err("initialization formula must name a declared controller state".into());
            }
            formulas::validate(binding, symbol, formula)?;
        }
    }
    for (symbol, value) in values {
        write_value(state(binding, symbol)?, *value)?;
    }
    Ok(())
}
