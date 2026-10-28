//! The design of a switching controller's settings (specification 35, C3, Tune; annex N10): the hysteresis from the measurement's noise and
//! the settling band of the process requirement (D-8), and the two minimum times from the switched equipment's own limits and the task
//! period. A minimum time is never shorter than the equipment's limit, and Tune refuses while a limit is undeclared, since it never
//! guesses one (D-9). The output levels are the block's (D-11). The rules are the TypeScript `designOnOffController`'s; every value is
//! checked against its parameter's binding (C0).

use std::collections::BTreeMap;

use crate::failure::{Cause, Failure, NextAction};
use crate::loop_definition::{
    LoopDefinition, OnOffController, Parameter, ParameterBinding, RealType,
};
use crate::model::simulation::ActuatorCapabilities;
use crate::tuning::on_off_candidate::OnOffCandidate;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::settings_pins::single_precision;

/// The refusal of a switching controller whose channels do not declare the switched equipment's minimum times (D-9, C3).
const EQUIPMENT_UNDECLARED: &str = "Declare the switched equipment's minimum ON and OFF times in Setup, zero when it has no such limit. Tune never guesses them.";
/// The refusal of a hysteresis whose unit is not the measurement's unit.
const HYSTERESIS_UNIT: &str = "Switching hysteresis must use the measured process unit.";
/// The refusal of output levels outside the output's limits.
const LEVELS_OUTSIDE_LIMITS: &str = "Switching levels exceed the available actuator limits.";
/// The refusal of a band that leaves no switching design inside the permitted error band (D-8).
const BAND_LEAVES_NO_DESIGN: &str =
    "The declared noise/process band leaves no switching design inside the permitted error band.";
/// The defect of a design reached without a switching block or a parameter binding, which the checks before the start refuse.
const NO_SWITCHING_LAW: &str = "Tune reached the switching design without a declared switching controller and its parameter binding.";

/// The settings of a switching controller for the requirements of the study (C3, D-8, D-9, D-11): the hysteresis, and the two minimum
/// times, each checked against its bound. The minimum times are the larger of the task period, the equipment's limit and the
/// parameter's own minimum, rounded up to a whole number of task periods.
///
/// # Errors
/// The block has no parameter binding or no switching block; the hysteresis is not in the measurement's unit; the levels exceed the
/// output's limits; the switched equipment's minimum times are undeclared; the band leaves no design inside the error limit; or a
/// setting does not fit its bound.
pub(crate) fn design(
    loop_definition: &LoopDefinition,
    requirements: &TuningRequirements,
    pins: Option<&BTreeMap<String, f64>>,
) -> Result<OnOffCandidate, Failure> {
    let (Some(block), Some(binding)) = (
        loop_definition.controller.on_off.as_ref(),
        loop_definition.parameter_binding.as_ref(),
    ) else {
        return Err(Failure::new(
            NO_SWITCHING_LAW,
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        ));
    };
    check_levels(loop_definition, block.low_output, block.high_output)?;
    check_hysteresis_unit(loop_definition, binding, &block.hysteresis_symbol)?;
    let (on_minimum, off_minimum) = equipment_minimums(loop_definition)?;
    let half_width = designed_half_width(block.noise_band, requirements)?;
    let period = loop_definition.controller.task_period_ms / 1000.0;
    let mut values = BTreeMap::new();
    values.insert(
        block.hysteresis_symbol.clone(),
        setting(
            binding,
            &block.hysteresis_symbol,
            block.band_convention.declared_for(half_width),
        )?,
    );
    values.insert(
        block.minimum_on_time_symbol.clone(),
        dwell(binding, &block.minimum_on_time_symbol, on_minimum, period)?,
    );
    values.insert(
        block.minimum_off_time_symbol.clone(),
        dwell(binding, &block.minimum_off_time_symbol, off_minimum, period)?,
    );
    // The engineer's pins replace the designed settings after the design (B4.2), each inside its binding.
    if let Some(pins) = pins {
        for (symbol, value) in pins {
            let pinned = pinned_setting(
                block,
                binding,
                symbol,
                *value,
                period,
                (on_minimum, off_minimum),
            )?;
            values.insert(symbol.clone(), pinned);
        }
    }
    Ok(OnOffCandidate {
        applied: false,
        values,
    })
}

/// A pinned setting (B4.2, C3): one of the switching controller's three settings, inside its binding. A pin of any other symbol is
/// refused, and so is a value the binding does not hold. Each refusal names the symbol as its field.
fn pinned_setting(
    block: &OnOffController,
    binding: &ParameterBinding,
    symbol: &str,
    value: f64,
    period: f64,
    equipment: (f64, f64),
) -> Result<f64, Failure> {
    let declared = [
        block.hysteresis_symbol.as_str(),
        block.minimum_on_time_symbol.as_str(),
        block.minimum_off_time_symbol.as_str(),
    ];
    if !declared.contains(&symbol) {
        return Err(Failure::new(
            &format!("Pinned switching setting {symbol} is not declared by the ON/OFF adapter."),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        )
        .with_field(symbol));
    }
    if !value.is_finite() {
        return Err(pin_outside(symbol));
    }
    // A minimum time is never below the switched equipment's declared limit of the same kind (C3), and a pinned one is rounded up to
    // whole task periods as a designed one is. The hysteresis is no time: it is only checked against its binding.
    let limit = if symbol == block.minimum_on_time_symbol.as_str() {
        Some(("ON", equipment.0))
    } else if symbol == block.minimum_off_time_symbol.as_str() {
        Some(("OFF", equipment.1))
    } else {
        None
    };
    let Some((word, equipment_minimum)) = limit else {
        return setting(binding, symbol, value).map_err(|_| pin_outside(symbol));
    };
    if value < equipment_minimum {
        return Err(Failure::new(
            &format!(
                "Pinned minimum {word} time {symbol} is {value} s, below the switched equipment's minimum {word} time of {equipment_minimum} s. A dwell is never shorter than the equipment's limit (C3)."
            ),
            Cause::Settings,
            NextAction::Tune,
        )
        .with_field(symbol));
    }
    let minimum = parameter_of(binding, symbol).map_or(0.0, |parameter| parameter.minimum);
    let rounded = whole_periods(period.max(value).max(minimum), period);
    setting(binding, symbol, rounded).map_err(|_| pin_outside(symbol))
}

/// A pin that its binding does not hold: outside its bounds, or not representable as its type. The symbol is its field.
fn pin_outside(symbol: &str) -> Failure {
    Failure::new(
        &format!("Pinned switching setting {symbol} does not match its bound parameter and representable bounds."),
        Cause::Settings,
        NextAction::Tune,
    )
    .with_field(symbol)
}

/// A time as a whole number of task periods, rounded up: the rounding of a designed or a pinned minimum time (C3).
fn whole_periods(seconds: f64, period: f64) -> f64 {
    (seconds / period).ceil() * period
}

/// The output levels lie inside the output's limits, where the loop declares them (`limits.outputMinimum` and `outputMaximum`).
fn check_levels(loop_definition: &LoopDefinition, low: f64, high: f64) -> Result<(), Failure> {
    let limits = &loop_definition.limits;
    let below = limits.output_minimum.is_some_and(|minimum| low < minimum);
    let above = limits.output_maximum.is_some_and(|maximum| high > maximum);
    if below || above {
        return Err(Failure::new(
            LEVELS_OUTSIDE_LIMITS,
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    Ok(())
}

/// The hysteresis is in the measurement's unit, so that its band is a distance of the process variable.
fn check_hysteresis_unit(
    loop_definition: &LoopDefinition,
    binding: &ParameterBinding,
    symbol: &str,
) -> Result<(), Failure> {
    if parameter_of(binding, symbol)?.unit == loop_definition.signals.process_variable.unit {
        Ok(())
    } else {
        Err(Failure::new(
            HYSTERESIS_UNIT,
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ))
    }
}

/// The switched equipment's minimum ON and OFF times, from the actuator of the loop's one input (C3). Each must be declared, and zero is
/// a declaration; an undeclared time refuses the design rather than being taken as zero (D-9).
fn equipment_minimums(loop_definition: &LoopDefinition) -> Result<(f64, f64), Failure> {
    let actuator = loop_definition
        .channels
        .as_ref()
        .and_then(|channels| channels.inputs.first())
        .map(|input| input.actuator);
    match actuator {
        Some(ActuatorCapabilities {
            minimum_on_seconds: Some(on),
            minimum_off_seconds: Some(off),
            ..
        }) => Ok((on, off)),
        _ => Err(Failure::new(
            EQUIPMENT_UNDECLARED,
            Cause::Settings,
            NextAction::OpenLoopSetup,
        )),
    }
}

/// The designed half-width of the band: the larger of the measurement's noise half-band and the settling band of the requirement (D-8).
/// It must lie below the largest error the requirement allows, or no switching design fits inside that error.
fn designed_half_width(
    noise_band: Option<f64>,
    requirements: &TuningRequirements,
) -> Result<f64, Failure> {
    let band = noise_band.unwrap_or(0.0).max(requirements.settling_band);
    if band >= requirements.maximum_error {
        return Err(Failure::new(
            BAND_LEAVES_NO_DESIGN,
            Cause::Settings,
            NextAction::OpenRequirements,
        ));
    }
    Ok(band)
}

/// A minimum time: the larger of the task period, the equipment's limit and the parameter's minimum, rounded up to a whole number of
/// task periods. The rounded value must still be at least the equipment's limit.
fn dwell(
    binding: &ParameterBinding,
    symbol: &str,
    equipment: f64,
    period: f64,
) -> Result<f64, Failure> {
    let minimum = parameter_of(binding, symbol)?.minimum;
    let seconds = whole_periods(period.max(equipment).max(minimum), period);
    let value = setting(binding, symbol, seconds)?;
    if value < equipment {
        return Err(Failure::new(
            &format!("Rounded dwell for {symbol} is below the equipment minimum."),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ));
    }
    Ok(value)
}

/// A setting as its bound parameter holds it: a `REAL` is rounded to single precision, and the value must be finite and inside the
/// parameter's declared bounds (`parameter` of `designOnOffController`).
fn setting(binding: &ParameterBinding, symbol: &str, proposed: f64) -> Result<f64, Failure> {
    let parameter = parameter_of(binding, symbol)?;
    let value = match parameter.iec_type {
        RealType::Real => single_precision(proposed),
        RealType::Lreal => proposed,
    };
    if value.is_finite() && value >= parameter.minimum && value <= parameter.maximum {
        Ok(value)
    } else {
        Err(Failure::new(
            &format!("Switching setting {symbol} is outside its representable bounds."),
            Cause::Settings,
            NextAction::OpenLoopSetup,
        ))
    }
}

/// The bound parameter a symbol names.
fn parameter_of<'a>(binding: &'a ParameterBinding, symbol: &str) -> Result<&'a Parameter, Failure> {
    binding
        .parameters
        .iter()
        .find(|parameter| parameter.symbol == symbol)
        .ok_or_else(|| {
            Failure::new(
                &format!("{symbol} is not a bound parameter of the switching controller."),
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            )
        })
}

#[cfg(test)]
#[path = "on_off_tests.rs"]
pub(crate) mod tests;
