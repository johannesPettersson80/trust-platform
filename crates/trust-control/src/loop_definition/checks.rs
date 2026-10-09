//! The rules of the loop contract: the field rules checked when a loop file is read, and the
//! relationships between its parts checked when it is opened (specification 35, §17.1, A5).

use std::collections::BTreeSet;

use crate::limits::{MAXIMUM_EXPERIMENT_DURATION_MS, MINIMUM_EXPERIMENT_DURATION_MS};

use super::{
    Activation, AuxiliaryRole, ControllerLaw, Direction, Execution, Experiment, ExperimentKind,
    InitialSource, InitialValue, InitializedState, LoopDefinition, LoopRejection, LoopSignal,
    ParameterBinding, ParameterPurpose, PidInitialization, PostScan, Range, StateType, Topology,
};

impl LoopDefinition {
    /// The setpoint, measurement, output and actuator, with their names in the contract.
    pub(super) fn primary_signals(&self) -> Vec<(&'static str, &LoopSignal)> {
        let signals = &self.signals;
        let mut primary = Vec::with_capacity(4);
        if let Some(setpoint) = &signals.setpoint {
            primary.push(("setpoint", setpoint));
        }
        primary.push(("processVariable", &signals.process_variable));
        primary.push(("manipulatedVariable", &signals.manipulated_variable));
        if let Some(actuator) = &signals.actuator {
            primary.push(("actuator", actuator));
        }
        primary
    }

    /// The field rules that need no other part of the loop.
    pub(super) fn check_fields(&self) -> Result<(), String> {
        for (name, value) in [
            ("id", &self.id),
            ("name", &self.name),
            ("projectId", &self.project_id),
            ("runtimeId", &self.runtime_id),
        ] {
            nonempty(name, value)?;
        }
        revision("revision", &self.revision)?;
        self.check_controller()?;
        self.check_signals()?;
        let limits = &self.limits;
        if !(limits.output_minimum.is_finite()
            && limits.output_maximum.is_finite()
            && limits.output_minimum < limits.output_maximum
            && optional_nonnegative(limits.rate_up_per_s)
            && optional_nonnegative(limits.rate_down_per_s))
        {
            return Err("the output limits are not a range with nonnegative rates".into());
        }
        let timing = &self.observed_sample_timing;
        if !(timing.nominal_period_ms.is_finite()
            && timing.nominal_period_ms > 0.0
            && optional_nonnegative(timing.maximum_jitter_ms))
        {
            return Err(
                "the sample timing needs a positive period and a nonnegative jitter".into(),
            );
        }
        if let Some(binding) = &self.parameter_binding {
            check_parameter_binding(binding, &self.id)?;
        }
        if let Some(law) = &self.controller.law {
            let binding = self
                .parameter_binding
                .as_ref()
                .ok_or("a controller law needs a parameter binding")?;
            check_law(law, binding)?;
        }
        if let Some(experiment) = &self.experiment {
            check_experiment(experiment)?;
        }
        self.check_parameter_purpose()?;
        self.check_hold_parameter()
    }

    pub(super) fn check_controller(&self) -> Result<(), String> {
        let controller = &self.controller;
        nonempty("controller.instance", &controller.instance)?;
        nonempty("controller.type", &controller.type_name)?;
        revision("controller.revision", &controller.revision)?;
        if !(controller.task_period_ms.is_finite() && controller.task_period_ms > 0.0) {
            return Err("the controller's task period must be positive".into());
        }
        for (name, value) in [
            ("modeSignal", &controller.mode_signal),
            ("trackingSignal", &controller.tracking_signal),
            ("manualOutputSignal", &controller.manual_output_signal),
            ("manualOutputAddress", &controller.manual_output_address),
        ] {
            if let Some(value) = value {
                nonempty(name, value)?;
            }
        }
        if controller.manual_output_type.is_some() && controller.manual_output_signal.is_none() {
            return Err("a manual output type needs a manual output signal".into());
        }
        if controller.manual_output_follows_output.is_some()
            && (controller.manual_output_follows_output != Some(true)
                || controller.manual_output_signal.is_none())
        {
            return Err("manualOutputFollowsOutput is true, with a manual output signal".into());
        }
        if let Some(PidInitialization::ParallelPid {
            integral_state,
            derivative_state,
            proportional_setpoint_weight,
            derivative_setpoint_weight,
            ..
        }) = &controller.pid_initialization
        {
            for state in [integral_state, derivative_state] {
                nonempty("pidInitialization state", &state.symbol)?;
                nonempty("pidInitialization unit", &state.unit)?;
                if !(state.minimum.is_finite()
                    && state.maximum.is_finite()
                    && state.minimum < state.maximum)
                {
                    return Err(format!("{} needs a range", state.symbol));
                }
            }
            if !proportional_setpoint_weight.is_finite() || !derivative_setpoint_weight.is_finite()
            {
                return Err("the PID initialization's weights must be finite".into());
            }
        }
        Ok(())
    }

    pub(super) fn check_signals(&self) -> Result<(), String> {
        let open_loop = self.topology == Some(Topology::OpenLoop);
        if open_loop == self.signals.setpoint.is_some() {
            return Err(if open_loop {
                "an open loop has no setpoint"
            } else {
                "a closed loop needs a setpoint"
            }
            .into());
        }
        for (name, signal) in self.primary_signals() {
            check_signal(
                name,
                &signal.symbol,
                signal.address.as_deref(),
                &signal.iec_type,
                signal.range,
            )?;
        }
        for signal in self.signals.additional.iter().flatten() {
            nonempty("additional signal id", &signal.id)?;
            nonempty("additional signal channel", &signal.channel)?;
            check_signal(
                &signal.id,
                &signal.symbol,
                signal.address.as_deref(),
                &signal.iec_type,
                signal.range,
            )?;
        }
        Ok(())
    }

    pub(super) fn check_parameter_purpose(&self) -> Result<(), String> {
        let Some(purpose) = self.parameter_purpose else {
            return Ok(());
        };
        let binding = self
            .parameter_binding
            .as_ref()
            .ok_or("a parameter purpose needs a parameter binding")?;
        if purpose == ParameterPurpose::Experiment {
            let activation = self
                .experiment
                .as_ref()
                .filter(|experiment| experiment.execution == Execution::ProjectScripted)
                .and_then(|experiment| experiment.activation.as_ref())
                .ok_or("experiment parameters need a project experiment with an activation")?;
            if binding.mode.symbol != activation.symbol || binding.mode.manual_value {
                return Err("experiment parameters are bound to the activation, whose Manual value is FALSE".into());
            }
        }
        Ok(())
    }

    /// A hold parameter is a declared experiment parameter in seconds whose longest permitted value
    /// the declared maximum duration can record.
    pub(super) fn check_hold_parameter(&self) -> Result<(), String> {
        let Some(experiment) = &self.experiment else {
            return Ok(());
        };
        let Some(symbol) = &experiment.hold_parameter else {
            return Ok(());
        };
        let binding = self.parameter_binding.as_ref().filter(|_| {
            self.parameter_purpose == Some(ParameterPurpose::Experiment)
                && experiment.execution == Execution::ProjectScripted
        });
        let hold = binding
            .and_then(|binding| {
                binding
                    .parameters
                    .iter()
                    .find(|parameter| &parameter.symbol == symbol)
            })
            .filter(|parameter| parameter.unit == "s" && parameter.minimum >= 0.0)
            .ok_or("the hold parameter is a declared experiment parameter in seconds")?;
        let period = self.observed_sample_timing.nominal_period_ms;
        let baseline_ms =
            (experiment.baseline_seconds.unwrap_or(0.0) * 1_000.0 / period).ceil() * period;
        let hold_ms = (hold.maximum * 1_000.0 / period - 1e-9).ceil() * period;
        if (experiment.duration_seconds * 1_000.0).round() < baseline_ms + hold_ms + period {
            return Err(
                "the experiment's duration cannot record the longest permitted hold".into(),
            );
        }
        Ok(())
    }

    /// A signal declared without a unit is dimensionless: recordings and requirements need it stated.
    pub(super) fn state_dimensionless_units(&mut self) {
        let signals = &mut self.signals;
        let units = signals
            .setpoint
            .iter_mut()
            .chain(signals.actuator.iter_mut())
            .chain([
                &mut signals.process_variable,
                &mut signals.manipulated_variable,
            ])
            .map(|signal| &mut signal.unit)
            .chain(
                signals
                    .additional
                    .iter_mut()
                    .flatten()
                    .map(|signal| &mut signal.unit),
            );
        for unit in units {
            if unit.trim().is_empty() {
                "1".clone_into(unit);
            }
        }
    }

    pub(super) fn validate_additional(
        &self,
        execution: Option<Execution>,
    ) -> Result<(), LoopRejection> {
        let mut ids: BTreeSet<&str> = [
            "setpoint",
            "processVariable",
            "manipulatedVariable",
            "actuator",
        ]
        .into_iter()
        .collect();
        let mut roles = BTreeSet::new();
        let mut units: std::collections::BTreeMap<&str, (Option<&str>, Option<&str>)> =
            std::collections::BTreeMap::new();
        for signal in self.signals.additional.iter().flatten() {
            if !ids.insert(&signal.id) {
                return Err(LoopRejection::DuplicateSignalId(signal.id.clone()));
            }
            if !roles.insert((signal.channel.as_str(), signal.role)) {
                return Err(LoopRejection::DuplicateSignalRole(signal.id.clone()));
            }
            if !auxiliary_direction_accepted(execution, signal.role, signal.direction) {
                return Err(LoopRejection::InvalidSignalDirection("additional".into()));
            }
            let boolean_evidence = signal.iec_type.eq_ignore_ascii_case("BOOL")
                && matches!(signal.role, AuxiliaryRole::Quality | AuxiliaryRole::State);
            if !is_numeric(&signal.iec_type) && !boolean_evidence {
                return Err(LoopRejection::IncompatibleSignalType("additional".into()));
            }
            let entry = units.entry(signal.channel.as_str()).or_default();
            match signal.role {
                AuxiliaryRole::Setpoint => entry.0 = Some(signal.unit.as_str()),
                AuxiliaryRole::ProcessVariable => entry.1 = Some(signal.unit.as_str()),
                _ => {}
            }
        }
        for (channel, (setpoint, measurement)) in units {
            if let (Some(setpoint), Some(measurement)) = (setpoint, measurement) {
                if setpoint != measurement {
                    return Err(LoopRejection::IncompatibleAuxiliaryUnits(channel.into()));
                }
            }
        }
        Ok(())
    }

    pub(super) fn validate_addresses(&self) -> Result<(), LoopRejection> {
        let mut bound: Vec<(&str, Option<&str>)> = self
            .primary_signals()
            .into_iter()
            .map(|(name, signal)| (name, signal.address.as_deref()))
            .collect();
        bound.extend(
            self.signals
                .additional
                .iter()
                .flatten()
                .map(|signal| (signal.id.as_str(), signal.address.as_deref())),
        );
        for (index, (name, address)) in bound.iter().enumerate() {
            for (other_name, other_address) in &bound[..index] {
                if let (Some(address), Some(other_address)) = (address, other_address) {
                    if addresses_overlap(address, other_address) {
                        return Err(LoopRejection::AmbiguousSignalAddress(
                            (*other_name).into(),
                            (*name).into(),
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn validate_pid_initialization(
        &self,
        initialization: &PidInitialization,
    ) -> Result<(), String> {
        let Some(ControllerLaw::Pid(_)) = &self.controller.law else {
            return Err("a PID initialization needs a PID law".into());
        };
        let reference = self
            .signals
            .setpoint
            .as_ref()
            .ok_or("a PID initialization needs a setpoint")?;
        let binding = self
            .parameter_binding
            .as_ref()
            .ok_or("a PID initialization needs a parameter binding")?;
        match initialization {
            PidInitialization::ControlPid {
                maximum_snapshot_age_ms,
                states,
                application_states,
            } => {
                check_snapshot_age(*maximum_snapshot_age_ms, self.controller.task_period_ms)?;
                check_library_pid_states(
                    &self.controller.instance,
                    states,
                    &reference.unit,
                    &self.signals.manipulated_variable.unit,
                )?;
                let root = format!(
                    "{}.",
                    self.controller
                        .instance
                        .split('.')
                        .next()
                        .unwrap_or_default()
                        .to_uppercase()
                );
                let mut symbols = BTreeSet::new();
                for state in states.iter().chain(application_states.iter().flatten()) {
                    if !symbols.insert(state.symbol.to_uppercase()) {
                        return Err(format!("{} is declared twice", state.symbol));
                    }
                }
                if application_states
                    .iter()
                    .flatten()
                    .any(|state| !state.symbol.to_uppercase().starts_with(&root))
                {
                    return Err(
                        "application states belong to the controller's owning program".into(),
                    );
                }
                Ok(())
            }
            PidInitialization::ParallelPid {
                integral_state,
                derivative_state,
                proportional_setpoint_weight,
                derivative_setpoint_weight,
                maximum_snapshot_age_ms,
            } => {
                let parameters: BTreeSet<&str> = binding
                    .parameters
                    .iter()
                    .map(|parameter| parameter.symbol.as_str())
                    .collect();
                let signals = [
                    &reference.symbol,
                    &self.signals.process_variable.symbol,
                    &self.signals.manipulated_variable.symbol,
                ];
                for state in [integral_state, derivative_state] {
                    if parameters.contains(state.symbol.as_str())
                        || signals.contains(&&state.symbol)
                    {
                        return Err(format!(
                            "{} is a parameter or a signal, not a state",
                            state.symbol
                        ));
                    }
                }
                if integral_state.symbol == derivative_state.symbol {
                    return Err("the integral and derivative states are distinct".into());
                }
                if integral_state.unit != self.signals.manipulated_variable.unit
                    || derivative_state.unit != reference.unit
                {
                    return Err("the integral state has the output's unit and the derivative state the setpoint's".into());
                }
                if !(0.0..=1.0).contains(proportional_setpoint_weight)
                    || !(0.0..=1.0).contains(derivative_setpoint_weight)
                {
                    return Err("the setpoint weights lie in [0, 1]".into());
                }
                check_snapshot_age(*maximum_snapshot_age_ms, self.controller.task_period_ms)
            }
        }
    }
}

/// The seven retained states of the shipped `Control.PID`, each bound to its declared coordinate.
fn check_library_pid_states(
    instance: &str,
    states: &[InitializedState],
    output_unit: &str,
    input_unit: &str,
) -> Result<(), String> {
    let output = |index| InitialValue::From(InitialSource::ModelOutput { index });
    let input = |index| InitialValue::From(InitialSource::ModelInput { index });
    let expected = [
        ("Setpoint", output_unit.to_string(), output(0)),
        (
            "SetpointRate",
            format!("{output_unit}/s"),
            InitialValue::Constant(0.0),
        ),
        ("ProcessValue", output_unit.to_string(), output(0)),
        (
            "ProcessValueRate",
            format!("{output_unit}/s"),
            InitialValue::Constant(0.0),
        ),
        (
            "FeedForward",
            input_unit.to_string(),
            InitialValue::Constant(0.0),
        ),
        ("NominalOutput", input_unit.to_string(), input(0)),
        ("SaturatedOutput", input_unit.to_string(), input(0)),
    ];
    if states.len() != expected.len() {
        return Err(format!("declare all seven retained states for {instance}"));
    }
    for (member, unit, initial) in expected {
        let symbol = format!("{instance}.StateValue.{member}");
        let found = states.iter().find(|state| state.symbol == symbol);
        let fits = found.is_some_and(|state| {
            state.iec_type == StateType::Lreal
                && state.unit == unit
                && state.post_scan == PostScan::Bounded
                && state.minimum < state.maximum
                && state.initial == initial
        });
        if !fits {
            return Err(format!("Control.PID {instance}.{member} must bind its retained state and its declared operating-point coordinate"));
        }
    }
    Ok(())
}

fn check_snapshot_age(age_ms: u64, task_period_ms: f64) -> Result<(), String> {
    // A snapshot age of up to 10 s is exact as an f64.
    #[allow(clippy::cast_precision_loss)]
    let age = age_ms as f64;
    if age < task_period_ms || age_ms > 10_000 {
        return Err("the snapshot age is at least one task period and at most 10 s".into());
    }
    Ok(())
}

fn check_parameter_binding(binding: &ParameterBinding, loop_id: &str) -> Result<(), String> {
    if binding.loop_id != loop_id {
        return Err("the parameter binding belongs to another loop".into());
    }
    label("task_name", &binding.task_name)?;
    label("mode.symbol", &binding.mode.symbol)?;
    if binding.parameters.is_empty() || binding.parameters.len() > 64 {
        return Err("a parameter binding has 1 to 64 parameters".into());
    }
    let mut symbols: BTreeSet<&str> = [binding.mode.symbol.as_str()].into_iter().collect();
    for parameter in &binding.parameters {
        label("parameter symbol", &parameter.symbol)?;
        label("parameter unit", &parameter.unit)?;
        label("parameter convention", &parameter.convention)?;
        if !symbols.insert(&parameter.symbol) {
            return Err(format!("{} is bound twice", parameter.symbol));
        }
        if !(parameter.minimum.is_finite()
            && parameter.maximum.is_finite()
            && parameter.minimum <= parameter.maximum)
        {
            return Err(format!("{} needs a range", parameter.symbol));
        }
    }
    Ok(())
}

fn check_law(law: &ControllerLaw, binding: &ParameterBinding) -> Result<(), String> {
    let parameter = |symbol: &str| {
        binding
            .parameters
            .iter()
            .find(|parameter| parameter.symbol == symbol)
    };
    match law {
        ControllerLaw::Proportional { gain_symbol } => {
            parameter(gain_symbol)
                .ok_or_else(|| format!("{gain_symbol} is not a bound parameter"))?;
            Ok(())
        }
        ControllerLaw::Pid(pid) => {
            label("parameterRecordSymbol", &pid.parameter_record_symbol)?;
            if let Some(symbol) = &pid.parameter_revision_symbol {
                label("parameterRevisionSymbol", symbol)?;
            }
            let optional = [
                pid.anti_windup
                    .as_ref()
                    .map(|anti_windup| &anti_windup.tracking_gain_symbol),
                pid.proportional_setpoint_weight_symbol.as_ref(),
                pid.derivative_setpoint_weight_symbol.as_ref(),
                pid.measurement_filter_time_symbol.as_ref(),
            ];
            let symbols: Vec<&String> = [
                &pid.proportional_gain_symbol,
                &pid.integral_gain_symbol,
                &pid.derivative_gain_symbol,
            ]
            .into_iter()
            .chain(optional.into_iter().flatten())
            .collect();
            let distinct: BTreeSet<&String> = symbols.iter().copied().collect();
            if distinct.len() != symbols.len() {
                return Err("the PID's record fields are distinct".into());
            }
            for symbol in &symbols {
                parameter(symbol).ok_or_else(|| format!("{symbol} is not a bound parameter"))?;
            }
            if let Some(anti_windup) = &pid.anti_windup {
                if parameter(&anti_windup.tracking_gain_symbol)
                    .is_none_or(|parameter| parameter.unit != "1/s")
                {
                    return Err("the tracking gain is in 1/s".into());
                }
            }
            for symbol in [
                &pid.proportional_setpoint_weight_symbol,
                &pid.derivative_setpoint_weight_symbol,
            ]
            .into_iter()
            .flatten()
            {
                if parameter(symbol).is_none_or(|parameter| {
                    parameter.unit != "1" || parameter.minimum < 0.0 || parameter.maximum > 1.0
                }) {
                    return Err(format!("{symbol} is a weight in [0, 1]"));
                }
            }
            if let Some(symbol) = &pid.measurement_filter_time_symbol {
                if parameter(symbol)
                    .is_none_or(|parameter| parameter.unit != "s" || parameter.minimum < 0.0)
                {
                    return Err(format!("{symbol} is a nonnegative time in s"));
                }
            }
            Ok(())
        }
    }
}

fn check_experiment(experiment: &Experiment) -> Result<(), String> {
    // The duration limits are whole seconds, exact as f64.
    #[allow(clippy::cast_precision_loss)]
    let (minimum, maximum) = (
        MINIMUM_EXPERIMENT_DURATION_MS as f64 / 1_000.0,
        MAXIMUM_EXPERIMENT_DURATION_MS as f64 / 1_000.0,
    );
    if !(experiment.amplitude.is_finite()
        && experiment.duration_seconds.is_finite()
        && (minimum..=maximum).contains(&experiment.duration_seconds)
        && (1..=0xffff_ffff).contains(&experiment.seed))
    {
        return Err("the experiment needs a finite amplitude, a duration of 1 s to 24 h and a seed of 1 to 2^32 - 1".into());
    }
    if let Some(baseline) = experiment.baseline_seconds {
        if experiment.execution != Execution::ProjectScripted
            || experiment.activation.is_none()
            || !(baseline.is_finite() && baseline >= 0.0 && baseline < experiment.duration_seconds)
        {
            return Err(
                "a baseline belongs to an activated project experiment and is shorter than it"
                    .into(),
            );
        }
    }
    for activation in [&experiment.activation, &experiment.abort]
        .into_iter()
        .flatten()
    {
        check_activation(activation)?;
    }
    match experiment.execution {
        Execution::ProjectScripted => {
            if experiment.kind != ExperimentKind::Observation || experiment.amplitude != 0.0 {
                return Err("a project experiment is an observation with no amplitude".into());
            }
            if let Some(abort) = &experiment.abort {
                let activation = experiment
                    .activation
                    .as_ref()
                    .ok_or("an abort needs an activation")?;
                if abort.asserted_value.is_none()
                    || abort.address == activation.address
                    || abort.symbol == activation.symbol
                {
                    return Err("an abort is a distinct variable with its asserted value".into());
                }
            }
            Ok(())
        }
        Execution::ControlStudio => {
            if experiment.activation.is_some() || experiment.abort.is_some() {
                return Err("a Control Studio experiment has no activation or abort".into());
            }
            if (experiment.kind == ExperimentKind::Observation) != (experiment.amplitude == 0.0) {
                return Err(
                    "an observation has no amplitude and every other experiment one".into(),
                );
            }
            Ok(())
        }
    }
}

fn check_activation(activation: &Activation) -> Result<(), String> {
    nonempty("activation symbol", &activation.symbol)?;
    if parse_address(&activation.address).is_none()
        || activation.iec_type != "BOOL"
        || activation.direction != Direction::Write
    {
        return Err(format!(
            "{} is a written BOOL with an address",
            activation.symbol
        ));
    }
    Ok(())
}

fn check_signal(
    name: &str,
    symbol: &str,
    address: Option<&str>,
    iec_type: &str,
    range: Range,
) -> Result<(), String> {
    nonempty(name, symbol)?;
    nonempty(name, iec_type)?;
    if let Some(address) = address {
        if parse_address(address).is_none() {
            return Err(format!(
                "{name}'s address {address} is not a direct address"
            ));
        }
    }
    if !(range.minimum.is_finite() && range.maximum.is_finite() && range.minimum < range.maximum) {
        return Err(format!("{name} needs a range"));
    }
    Ok(())
}

/// The direction a primary signal has: a project experiment owns every signal, so Control Studio
/// only reads them; otherwise Control Studio writes the output and reads the rest.
pub(super) fn expected_primary_direction(execution: Option<Execution>, name: &str) -> Direction {
    if execution == Some(Execution::ProjectScripted) || name != "manipulatedVariable" {
        Direction::Read
    } else {
        Direction::Write
    }
}

/// Whether a further signal's direction fits its role: a project experiment only reads; otherwise
/// a disturbance may be read or written, an output is written and the rest are read.
fn auxiliary_direction_accepted(
    execution: Option<Execution>,
    role: AuxiliaryRole,
    direction: Direction,
) -> bool {
    if execution == Some(Execution::ProjectScripted) {
        return direction == Direction::Read;
    }
    match role {
        AuxiliaryRole::Disturbance => true,
        AuxiliaryRole::ManipulatedVariable => direction == Direction::Write,
        _ => direction == Direction::Read,
    }
}

const NUMERIC_IEC_TYPES: [&str; 10] = [
    "SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT", "REAL", "LREAL",
];

pub(super) fn is_numeric(iec_type: &str) -> bool {
    NUMERIC_IEC_TYPES
        .iter()
        .any(|numeric| numeric.eq_ignore_ascii_case(iec_type))
}

/// A direct address's area and bit span: `%IX0.3`, `%QW4`, `%ML16` and so on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AddressSpan {
    area: char,
    start_bit: u64,
    end_bit: u64,
}

pub(super) fn parse_address(address: &str) -> Option<AddressSpan> {
    let upper = address.to_ascii_uppercase();
    let rest = upper.strip_prefix('%')?;
    let mut chars = rest.chars();
    let area = chars
        .next()
        .filter(|area| matches!(area, 'I' | 'Q' | 'M'))?;
    let size = chars.next()?;
    let width = match size {
        'X' => 1,
        'B' => 8,
        'W' => 16,
        'D' => 32,
        'L' => 64,
        _ => return None,
    };
    let numbers = chars.as_str();
    let (byte, bit) = match numbers.split_once('.') {
        Some((byte, bit)) => (byte, Some(bit)),
        None => (numbers, None),
    };
    if byte.is_empty() || !byte.bytes().all(|digit| digit.is_ascii_digit()) {
        return None;
    }
    let byte: u64 = byte.parse().ok()?;
    // A bit number counts only for a bit address; on a wider address it is accepted and ignored.
    let bit: u64 = match bit {
        Some(bit) if !bit.is_empty() && bit.bytes().all(|digit| digit.is_ascii_digit()) => {
            if size == 'X' {
                bit.parse().ok()?
            } else {
                0
            }
        }
        Some(_) => return None,
        None => 0,
    };
    if bit > 7 {
        return None;
    }
    let start_bit = byte.checked_mul(8)?.checked_add(bit)?;
    let end_bit = start_bit.checked_add(width)?;
    Some(AddressSpan {
        area,
        start_bit,
        end_bit,
    })
}

pub(super) fn addresses_overlap(left: &str, right: &str) -> bool {
    match (parse_address(left), parse_address(right)) {
        (Some(left), Some(right)) if left.area == right.area => {
            left.start_bit < right.end_bit && right.start_bit < left.end_bit
        }
        _ => left == right,
    }
}

fn nonempty(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{name} is empty"));
    }
    Ok(())
}

fn revision(name: &str, value: &str) -> Result<(), String> {
    match value.strip_prefix("sha256:") {
        Some(rest) if !rest.is_empty() => Ok(()),
        _ => Err(format!("{name} is a sha256: revision")),
    }
}

/// A label of a parameter binding: non-empty, at most 1024 characters, no control characters.
fn label(name: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value.chars().count() > 1024
        || value.chars().any(char::is_control)
    {
        return Err(format!("{name} is not a label"));
    }
    Ok(())
}

fn optional_nonnegative(value: Option<f64>) -> bool {
    value.is_none_or(|value| value.is_finite() && value >= 0.0)
}
