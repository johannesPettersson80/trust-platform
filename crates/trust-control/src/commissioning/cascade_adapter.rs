//! The cascade's commissioning (specification 35, C0 and C6): the parameter values of its two PIs, each written to its own record; the
//! measured outputs of its two measurements, which its initial states read; and the inner measurement that its transfer reads beside
//! the process variable. The values form one record set, which Apply and Validate write in one transaction with both modes (C0, C6).

use std::collections::BTreeMap;

use super::adapters::{bound_values, unrepresented_terms, ControllerGains, Term};
use super::texts;
use super::transfer::{TransferMember, TransferValueType};
use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{channel_signal, LoopDefinition, ParameterBinding};
use crate::model::GeneralModel;
use crate::tuning::project_seed::{CascadeGains, CascadePidSeed};
use crate::tuning::structure::{is_exactly_zero, ParallelGains};

/// The refusal of a cascade whose inner integral gain the candidate does not hold (C6).
const INNER_INTEGRAL_REQUIRED: &str = "A cascade's inner controller needs an integral gain.";
/// The refusal of a cascade whose outer PI the candidate does not hold (C6).
const OUTER_PI_REQUIRED: &str = "A cascade candidate holds no outer controller's PI.";
/// The refusal of a cascade gain with a derivative term, since both controllers are PI (C6).
const DERIVATIVE_NOT_HELD: &str =
    "A cascade's PIs have no derivative term, so a derivative gain must be zero.";

/// The values that a cascade's two PIs write (C6): the inner controller's proportional and integral gains, the outer controller's, and a
/// zero for each derivative gain. The observed values of the other record fields are kept, as for every PID record (C0).
///
/// # Errors
/// The inner proportional or integral gain, or the outer PI is missing; a derivative gain is nonzero; the loop has no parameter binding;
/// or a value is outside its binding.
pub(super) fn cascade_values(
    binding: Option<&ParameterBinding>,
    seed: &CascadePidSeed,
    gains: &ControllerGains,
    observed: &BTreeMap<String, f64>,
) -> Result<BTreeMap<String, f64>, Failure> {
    let Some(binding) = binding else {
        return Err(Failure::new(
            &texts::parameter_pid_record_required(),
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ));
    };
    let Some(proportional) = gains.proportional_gain else {
        return Err(Failure::new(
            &texts::project_proportional_gain_required(),
            Cause::Settings,
            NextAction::Tune,
        ));
    };
    let Some(integral) = gains.integral_gain else {
        return Err(Failure::new(
            INNER_INTEGRAL_REQUIRED,
            Cause::Settings,
            NextAction::Tune,
        ));
    };
    if gains
        .derivative_gain
        .is_some_and(|value| !is_exactly_zero(value))
    {
        return Err(Failure::new(
            DERIVATIVE_NOT_HELD,
            Cause::Settings,
            NextAction::Tune,
        ));
    }
    let Some(outer) = gains.outer_pid else {
        return Err(Failure::new(
            OUTER_PI_REQUIRED,
            Cause::Settings,
            NextAction::Tune,
        ));
    };
    unrepresented_terms(gains)?;
    let inner = ParallelGains {
        proportional_gain: proportional,
        integral_gain: integral,
        derivative_gain: 0.0,
    };
    let written = cascade_terms(seed, inner, outer);
    bound_values(binding, &written, observed)
}

/// The symbol of the measured output at a model index, for a cascade (C6): the loop's signal that is bound to that output. `None` for a
/// loop that is not a cascade, whose measurement the scalar mapping gives.
#[must_use]
pub(super) fn cascade_measurement_symbol(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    index: usize,
) -> Option<String> {
    if !loop_definition.is_cascade_channels() {
        return None;
    }
    let channels = loop_definition.channels.as_ref()?;
    let output = model.identity().outputs.get(index)?;
    let bound = channels
        .outputs
        .iter()
        .find(|bound| bound.model_channel_id == output.id)?;
    channel_signal(loop_definition, &bound.signal_id).map(|signal| signal.symbol.to_owned())
}

/// The inner measurement as a member of a cascade's transfer (C6): the transfer reads it beside the outer process variable, because the
/// inner controller starts at its present value. Empty for a loop that is not a cascade.
///
/// # Errors
/// The inner measurement's signal is not declared, or it is not `REAL` or `LREAL`.
pub(super) fn cascade_guards(
    loop_definition: &LoopDefinition,
) -> Result<Vec<TransferMember>, Failure> {
    if !loop_definition.is_cascade_channels() {
        return Ok(Vec::new());
    }
    let Some(channels) = loop_definition.channels.as_ref() else {
        return Ok(Vec::new());
    };
    let Some(inner) = channels
        .outputs
        .iter()
        .find(|output| output.setpoint_signal_id.is_none())
    else {
        return Ok(Vec::new());
    };
    let signal = loop_definition
        .signals
        .additional
        .as_ref()
        .and_then(|additional| {
            additional
                .iter()
                .find(|signal| signal.id == inner.signal_id)
        })
        .ok_or_else(|| {
            Failure::new(
                &texts::init_primary_coordinates(),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })?;
    let iec_type = match signal.iec_type.as_str() {
        "REAL" => TransferValueType::Real,
        "LREAL" => TransferValueType::Lreal,
        _ => {
            return Err(Failure::new(
                &texts::init_primary_coordinates(),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            ))
        }
    };
    Ok(vec![TransferMember {
        symbol: signal.symbol.clone(),
        iec_type,
        unit: signal.unit.clone(),
        minimum: signal.range.minimum,
        maximum: signal.range.maximum,
    }])
}

/// The six terms of the two PIs, the inner controller's first (C6): each controller's proportional and integral gain, and its derivative
/// as zero.
fn cascade_terms(
    seed: &CascadePidSeed,
    inner: ParallelGains,
    outer: ParallelGains,
) -> Vec<Term<'_>> {
    let mut written = Vec::from(controller_terms(&seed.inner, inner));
    written.extend(controller_terms(&seed.outer, outer));
    written
}

/// One controller's three terms: its proportional, integral and derivative gains, each a bound gain of its record (C6).
fn controller_terms(names: &CascadeGains, designed: ParallelGains) -> [Term<'_>; 3] {
    [
        Term {
            symbol: names.proportional_symbol.as_str(),
            gain: true,
            value: designed.proportional_gain,
        },
        Term {
            symbol: names.integral_symbol.as_str(),
            gain: true,
            value: designed.integral_gain,
        },
        Term {
            symbol: names.derivative_symbol.as_str(),
            gain: true,
            value: designed.derivative_gain,
        },
    ]
}

#[cfg(test)]
#[path = "cascade_adapter_tests.rs"]
mod tests;
