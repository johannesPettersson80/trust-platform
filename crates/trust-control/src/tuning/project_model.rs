//! Model-derived settings of a program's law (specification 35, C0; `projectModelParameters.ts`): a bound parameter that the
//! program takes from the model's timing, gain or time constant, with the unit and the bounds checked before anything is
//! written. The sampled-matrix sources of the TypeScript (the state-space coefficients of a sampled model) are not part of
//! this module.

#![allow(clippy::module_name_repetitions)] // `ModelSource` names the module's subject.

use std::collections::BTreeMap;

use crate::identification::refusal::Refusal;
use crate::tuning::plant::TuningPlant;
use crate::tuning::proportional::GainBounds;
use crate::tuning::structure::is_exactly_zero;
use crate::tuning::texts;

/// Where a model-derived setting takes its value from (C0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelSource {
    /// The task period in seconds.
    SampleTime,
    /// The plant's dead time in seconds.
    PlantDelay,
    /// The plant's static gain `K`, in the output unit per the input unit.
    PlantGain,
    /// The inverse of the plant's static gain, in the input unit per the output unit.
    InversePlantGain,
    /// The plant's time constant `T` in seconds, for a first-order plant.
    PlantTimeConstant,
}

/// A bound parameter that a model-derived setting writes: its symbol, its unit, whether it is `REAL`, and its bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelParameter {
    /// The parameter's symbol.
    pub symbol: String,
    /// The parameter's declared unit.
    pub unit: String,
    /// Whether the parameter is `REAL` (rounded to single precision, C0).
    pub real: bool,
    /// The parameter's declared bounds.
    pub bounds: GainBounds,
}

/// The units of the loop's output and input, which a plant gain's unit is built from.
#[derive(Debug, Clone, PartialEq)]
pub struct LoopUnits {
    /// The process variable's unit (the plant's output).
    pub output: String,
    /// The manipulated variable's unit (the plant's input).
    pub input: String,
}

/// The values of the model-derived settings (`projectModelParameters`): each mapped symbol must be a bound parameter, the
/// timing fields must be in seconds, the gain fields must carry the unit of their gain, and every value must be representable
/// within its bounds. A plant without the first-order form has no time constant, and an integrating plant has no static gain.
///
/// # Errors
/// Returns a [`Refusal`] with the sentence of the first setting that does not hold.
pub fn model_values(
    mapping: &BTreeMap<String, ModelSource>,
    parameters: &[ModelParameter],
    task_period_ms: f64,
    plant: &TuningPlant,
    units: &LoopUnits,
) -> Result<BTreeMap<String, f64>, Refusal> {
    let mut result = BTreeMap::new();
    for (symbol, source) in mapping {
        let parameter = parameters
            .iter()
            .find(|candidate| &candidate.symbol == symbol)
            .ok_or_else(|| Refusal::new(texts::MODEL_SETTING_NOT_BOUND))?;
        let value = model_value(*source, parameter, task_period_ms, plant, units)?;
        let stored = if parameter.real {
            crate::tuning::settings_pins::single_precision(value)
        } else {
            value
        };
        if !stored.is_finite()
            || stored < parameter.bounds.minimum
            || stored > parameter.bounds.maximum
        {
            return Err(Refusal::new(texts::model_value_out_of_bounds(symbol)));
        }
        result.insert(symbol.clone(), stored);
    }
    Ok(result)
}

/// One model-derived value before its representation is checked.
fn model_value(
    source: ModelSource,
    parameter: &ModelParameter,
    task_period_ms: f64,
    plant: &TuningPlant,
    units: &LoopUnits,
) -> Result<f64, Refusal> {
    match source {
        ModelSource::SampleTime => {
            require_seconds(parameter)?;
            Ok(task_period_ms / 1000.0)
        }
        ModelSource::PlantDelay => {
            require_seconds(parameter)?;
            Ok(plant.delay_seconds())
        }
        ModelSource::PlantGain => {
            require_unit(parameter, &format!("{}/{}", units.output, units.input))?;
            static_gain(plant, &parameter.symbol)
        }
        ModelSource::InversePlantGain => {
            require_unit(parameter, &format!("{}/{}", units.input, units.output))?;
            let gain = static_gain(plant, &parameter.symbol)?;
            Ok(1.0 / gain)
        }
        ModelSource::PlantTimeConstant => match plant {
            TuningPlant::FirstOrder(model)
                if parameter.unit == "s" && model.time_constant_seconds > 0.0 =>
            {
                Ok(model.time_constant_seconds)
            }
            _ => Err(Refusal::new(texts::MODEL_NO_TIME_CONSTANT)),
        },
    }
}

/// The static gain of a first-order or second-order plant, whose `K` is its gain at zero frequency. An integrating plant has
/// none, so its value is not representable.
fn static_gain(plant: &TuningPlant, symbol: &str) -> Result<f64, Refusal> {
    match plant {
        TuningPlant::FirstOrder(model) if !is_exactly_zero(model.gain) => Ok(model.gain),
        TuningPlant::SecondOrder(model) if !is_exactly_zero(model.gain) => Ok(model.gain),
        _ => Err(Refusal::new(texts::model_value_out_of_bounds(symbol))),
    }
}

/// A timing field must be in seconds.
fn require_seconds(parameter: &ModelParameter) -> Result<(), Refusal> {
    if parameter.unit == "s" {
        Ok(())
    } else {
        Err(Refusal::new(texts::MODEL_TIMING_UNIT))
    }
}

/// A gain field must carry the unit of its gain.
fn require_unit(parameter: &ModelParameter, expected: &str) -> Result<(), Refusal> {
    if parameter.unit == expected {
        Ok(())
    } else {
        Err(Refusal::new(texts::model_gain_unit(expected)))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // The derived settings are exact quotients and copies of the model's numbers.

    use super::*;
    use trust_control_math::tuning::{FirstOrderPlusDelay, IntegratingModel};

    fn parameter(symbol: &str, unit: &str) -> ModelParameter {
        ModelParameter {
            symbol: symbol.to_string(),
            unit: unit.to_string(),
            real: false,
            bounds: GainBounds {
                minimum: -1000.0,
                maximum: 1000.0,
            },
        }
    }

    fn units() -> LoopUnits {
        LoopUnits {
            output: "°C".to_string(),
            input: "%".to_string(),
        }
    }

    fn first_order() -> TuningPlant {
        TuningPlant::FirstOrder(FirstOrderPlusDelay {
            gain: 0.5,
            time_constant_seconds: 40.0,
            delay_seconds: 2.0,
        })
    }

    #[test]
    fn timing_and_gain_settings_come_from_the_model() {
        let mapping = BTreeMap::from([
            ("Main.Ts".to_string(), ModelSource::SampleTime),
            ("Main.Td".to_string(), ModelSource::PlantDelay),
            ("Main.K".to_string(), ModelSource::PlantGain),
            ("Main.Ki".to_string(), ModelSource::InversePlantGain),
            ("Main.T".to_string(), ModelSource::PlantTimeConstant),
        ]);
        let parameters = vec![
            parameter("Main.Ts", "s"),
            parameter("Main.Td", "s"),
            parameter("Main.K", "°C/%"),
            parameter("Main.Ki", "%/°C"),
            parameter("Main.T", "s"),
        ];
        let values =
            model_values(&mapping, &parameters, 100.0, &first_order(), &units()).expect("derived");
        assert_eq!(values["Main.Ts"], 0.1);
        assert_eq!(values["Main.Td"], 2.0);
        assert_eq!(values["Main.K"], 0.5);
        assert_eq!(values["Main.Ki"], 2.0);
        assert_eq!(values["Main.T"], 40.0);
    }

    #[test]
    fn a_gain_field_with_the_wrong_unit_is_refused() {
        let mapping = BTreeMap::from([("Main.K".to_string(), ModelSource::PlantGain)]);
        let parameters = vec![parameter("Main.K", "%/°C")];
        assert_eq!(
            model_values(&mapping, &parameters, 100.0, &first_order(), &units())
                .expect_err("unit")
                .0,
            "Model gain fields require °C/% units."
        );
    }

    #[test]
    fn an_integrating_plant_has_no_static_gain_or_time_constant() {
        let tank = TuningPlant::Integrating(IntegratingModel {
            rate_gain: 0.05,
            delay_seconds: 1.0,
        });
        let gain = BTreeMap::from([("Main.K".to_string(), ModelSource::PlantGain)]);
        assert!(model_values(
            &gain,
            &[parameter("Main.K", "°C/%")],
            100.0,
            &tank,
            &units()
        )
        .is_err());
        let time = BTreeMap::from([("Main.T".to_string(), ModelSource::PlantTimeConstant)]);
        assert_eq!(
            model_values(&time, &[parameter("Main.T", "s")], 100.0, &tank, &units())
                .expect_err("no T")
                .0,
            "The selected model does not supply a stable first-order time constant in seconds."
        );
    }

    #[test]
    fn a_value_outside_its_bounds_is_refused() {
        let mapping = BTreeMap::from([("Main.Td".to_string(), ModelSource::PlantDelay)]);
        let narrow = ModelParameter {
            symbol: "Main.Td".to_string(),
            unit: "s".to_string(),
            real: true,
            bounds: GainBounds {
                minimum: 0.0,
                maximum: 1.0,
            },
        };
        assert_eq!(
            model_values(&mapping, &[narrow], 100.0, &first_order(), &units())
                .expect_err("out of bounds")
                .0,
            "Model-derived parameter Main.Td exceeds its representable bound."
        );
    }
}
