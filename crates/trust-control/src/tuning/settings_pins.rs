//! Pinned settings and the entry of a Control.PID record's settings (specification 35, B4.2, C0 and C1; numerical annex
//! N6.4). A pinned value is kept through every design: it is checked against the bound parameter, then kept by the designs
//! that follow. The gains are checked by magnitude (N6.4); the weights and the filter time by their signed bounds.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::identification::refusal::Refusal;
use crate::loop_definition::{ControllerLaw, LoopDefinition, ParameterBinding, RealType};
use crate::tuning::cascade_candidate::{cascade_mapped_parameters, served_cascade};
use crate::tuning::project_candidate::{served_seed, served_trajectory};
use crate::tuning::project_seed::ParallelPidSeed;
use crate::tuning::proportional::GainBounds;
use crate::tuning::structure::{is_exactly_zero, Structure};
use crate::tuning::texts;

/// The most pinned values a record may hold (`decodeControllerSettingsPins`).
pub const MAXIMUM_PINNED_VALUES: usize = 64;

/// The record field a bound parameter holds (C1, the parallel record of `Control.PID`).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum GainKey {
    /// The proportional gain `Kp`.
    ProportionalGain,
    /// The integral gain `Ki`.
    IntegralGain,
    /// The derivative gain `Kd`.
    DerivativeGain,
    /// The back-calculation tracking gain `1/Tt` (C0).
    BackCalculationGain,
    /// The proportional setpoint weight `β` (C0).
    ProportionalSetpointWeight,
    /// The derivative setpoint weight `γ`.
    DerivativeSetpointWeight,
    /// The measurement filter time `Tf` in seconds (C0).
    MeasurementFilterSeconds,
}

impl GainKey {
    /// Whether the key is a gain, whose bounds apply to its magnitude (N6.4). The other keys are checked by their signed
    /// bounds.
    #[must_use]
    pub fn is_gain(self) -> bool {
        matches!(
            self,
            Self::ProportionalGain | Self::IntegralGain | Self::DerivativeGain
        )
    }
}

/// A bound parameter of the loop's record, with the record field it holds (`ParameterBinding`).
#[derive(Debug, Clone, PartialEq)]
pub struct MappedParameter {
    /// The parameter's symbol in the program.
    pub symbol: String,
    /// The record field the parameter holds.
    pub key: GainKey,
    /// The parameter's declared bounds.
    pub bounds: GainBounds,
    /// Whether the parameter is `REAL` (a value is rounded to single precision before it is written, C0).
    pub real: bool,
}

/// The pins a user set on the settings of a controller (`ControllerSettingsPins`): the loop they belong to, the family
/// they were set for, and each pinned symbol's value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControllerSettingsPins {
    /// The revision of the loop the pins belong to.
    pub loop_revision: String,
    /// The family the pins were set for.
    pub family: Structure,
    /// The pinned value of each symbol.
    pub values: BTreeMap<String, f64>,
}

/// The pins a request of `controlStudio/tune` or `controlStudio/preview` names (B4.2): the loop revision they belong to and each
/// pinned symbol's value. The family is not named by the page: the study gives it (`study::tune::resolve_pins`), so a request holds
/// no family of its own.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PinnedSettings {
    /// The revision of the loop the pins belong to.
    pub loop_revision: String,
    /// The pinned value of each symbol.
    pub values: BTreeMap<String, f64>,
}

impl PinnedSettings {
    /// The pins with the family they are set for (B4.2): the family the request or the study gives.
    #[must_use]
    pub fn with_family(&self, family: Structure) -> ControllerSettingsPins {
        ControllerSettingsPins {
            loop_revision: self.loop_revision.clone(),
            family,
            values: self.values.clone(),
        }
    }
}

impl From<&ControllerSettingsPins> for PinnedSettings {
    /// The request's form of pins the study holds: their loop revision and values, without their family.
    fn from(pins: &ControllerSettingsPins) -> Self {
        Self {
            loop_revision: pins.loop_revision.clone(),
            values: pins.values.clone(),
        }
    }
}

/// Decodes a pin record (`decodeControllerSettingsPins`): a non-empty loop revision, a family, at most 64 finite values
/// under non-empty symbols. Anything else is no pin.
#[must_use]
pub fn decode_pins(value: &serde_json::Value) -> Option<ControllerSettingsPins> {
    let pins: ControllerSettingsPins = crate::json::decode_value(value.clone()).ok()?;
    let valid = !pins.loop_revision.is_empty()
        && pins.values.len() <= MAXIMUM_PINNED_VALUES
        && pins
            .values
            .iter()
            .all(|(symbol, value)| !symbol.is_empty() && value.is_finite());
    valid.then_some(pins)
}

/// The gains with the pins applied (`pinnedControllerGains`): each pinned symbol must be a bound parameter of the loop, its
/// value within the bounds (magnitudes for the gains) and representable in its type, and the family must not conflict with
/// a pinned integral or derivative term. Without pins the gains are kept.
///
/// # Errors
/// Returns a [`Refusal`] when the pins belong to another loop or family, a pin is not a bound parameter or is outside its
/// bounds, or a pinned term conflicts with the family.
pub fn apply_pins(
    loop_revision: &str,
    family: Structure,
    gains: &BTreeMap<GainKey, f64>,
    pins: Option<&ControllerSettingsPins>,
    parameters: &[MappedParameter],
) -> Result<BTreeMap<GainKey, f64>, Refusal> {
    let Some(pins) = pins else {
        return Ok(gains.clone());
    };
    if pins.loop_revision != loop_revision || pins.family != family {
        return Err(Refusal::new(texts::PIN_DIFFERENT_LOOP));
    }
    let mut result = gains.clone();
    for (symbol, value) in &pins.values {
        let Some(parameter) = parameters
            .iter()
            .find(|candidate| &candidate.symbol == symbol)
        else {
            return Err(Refusal::new(format!(
                "Pinned setting {symbol} does not match the bound ST parameter and representable bounds."
            )));
        };
        if !within_bounds(*value, parameter) {
            return Err(Refusal::new(outside_bounds_sentence(
                symbol, *value, parameter,
            )));
        }
        result.insert(parameter.key, *value);
    }
    let integral = result.get(&GainKey::IntegralGain).copied().unwrap_or(0.0);
    let derivative = result.get(&GainKey::DerivativeGain).copied().unwrap_or(0.0);
    let proportional_only =
        matches!(family, Structure::P | Structure::PD) && !is_exactly_zero(integral);
    let no_derivative =
        matches!(family, Structure::P | Structure::PI) && !is_exactly_zero(derivative);
    if proportional_only || no_derivative {
        return Err(Refusal::new(texts::PIN_FAMILY_CONFLICT));
    }
    Ok(result)
}

/// The bound parameters of a loop's PID law, each with the record field it holds (C0, C1): the proportional, integral and
/// derivative gains, the setpoint weights, the measurement filter time and the back-calculation tracking gain. A symbol that
/// the parameter binding does not hold is not mapped, and a loop without a PID law maps none.
#[must_use]
pub fn mapped_parameters(loop_definition: &LoopDefinition) -> Vec<MappedParameter> {
    let Some(binding) = &loop_definition.parameter_binding else {
        return Vec::new();
    };
    if let Some(seed) = served_seed(loop_definition) {
        return seed_parameters(seed, binding);
    }
    if let Some(seed) = served_cascade(loop_definition) {
        return cascade_mapped_parameters(seed, binding);
    }
    let Some(ControllerLaw::Pid(pid)) = &loop_definition.controller.law else {
        return Vec::new();
    };
    let tracking_gain = pid
        .anti_windup
        .as_ref()
        .map(|anti_windup| anti_windup.tracking_gain_symbol.as_str());
    [
        (
            Some(pid.proportional_gain_symbol.as_str()),
            GainKey::ProportionalGain,
        ),
        (
            Some(pid.integral_gain_symbol.as_str()),
            GainKey::IntegralGain,
        ),
        (
            Some(pid.derivative_gain_symbol.as_str()),
            GainKey::DerivativeGain,
        ),
        (
            pid.proportional_setpoint_weight_symbol.as_deref(),
            GainKey::ProportionalSetpointWeight,
        ),
        (
            pid.derivative_setpoint_weight_symbol.as_deref(),
            GainKey::DerivativeSetpointWeight,
        ),
        (
            pid.measurement_filter_time_symbol.as_deref(),
            GainKey::MeasurementFilterSeconds,
        ),
        (tracking_gain, GainKey::BackCalculationGain),
    ]
    .into_iter()
    .filter_map(|(symbol, key)| mapped_parameter(binding, symbol?, key))
    .collect()
}

/// The bound parameters of a program's own law (C2): the seed's gain variables, each with the record field of its gain, so that
/// the pins, the manual settings and the gain limits key on the variables the program holds.
fn seed_parameters(seed: &ParallelPidSeed, binding: &ParameterBinding) -> Vec<MappedParameter> {
    [
        (
            Some(seed.proportional_symbol.as_str()),
            GainKey::ProportionalGain,
        ),
        (seed.integral_symbol.as_deref(), GainKey::IntegralGain),
        (seed.derivative_symbol.as_deref(), GainKey::DerivativeGain),
    ]
    .into_iter()
    .filter_map(|(symbol, key)| mapped_parameter(binding, symbol?, key))
    .collect()
}

/// The bound parameter with `symbol`, as the record field `key` holds it: its bounds and whether it is `REAL` (C0). A symbol the
/// binding does not hold maps nothing.
fn mapped_parameter(
    binding: &ParameterBinding,
    symbol: &str,
    key: GainKey,
) -> Option<MappedParameter> {
    let parameter = binding
        .parameters
        .iter()
        .find(|parameter| parameter.symbol == symbol)?;
    Some(MappedParameter {
        symbol: symbol.to_owned(),
        key,
        bounds: GainBounds {
            minimum: parameter.minimum,
            maximum: parameter.maximum,
        },
        real: matches!(parameter.iec_type, RealType::Real),
    })
}

/// Whether a value is within a parameter's bounds and representable in its type (C0): a gain by its magnitude (N6.4), the
/// other settings by their signed bounds, and a `REAL` value only if single precision keeps it.
fn within_bounds(value: f64, parameter: &MappedParameter) -> bool {
    let bounds = parameter.bounds;
    let in_range = if parameter.key.is_gain() {
        let (lowest, highest) = bounds.magnitude_range();
        value.abs() >= lowest && value.abs() <= highest
    } else {
        value >= bounds.minimum && value <= bounds.maximum
    };
    // Exact identity, not a tolerance: single precision must hold the value unchanged (C0).
    in_range && (!parameter.real || single_precision(value).to_bits() == value.to_bits())
}

/// The sentence of a pinned value that [`within_bounds`] refuses (A11.1): the setting, its value and its bounds as the check
/// reads them — a gain by its magnitude (N6.4) — or, for a value within them, that its `REAL` parameter cannot hold it exactly.
fn outside_bounds_sentence(symbol: &str, value: f64, parameter: &MappedParameter) -> String {
    let gain = parameter.key.is_gain();
    let bounds = if gain {
        parameter.bounds.magnitude_range()
    } else {
        (parameter.bounds.minimum, parameter.bounds.maximum)
    };
    let checked = if gain { value.abs() } else { value };
    let in_range = checked >= bounds.0 && checked <= bounds.1;
    texts::pin_outside_bounds(symbol, value, bounds, gain, !in_range)
}

/// A value rounded to single precision, as a `REAL` parameter stores it (C0).
#[allow(clippy::cast_possible_truncation)] // Single precision is the rounding the record requires (C0), not an accident.
#[must_use]
pub fn single_precision(value: f64) -> f64 {
    f64::from(value as f32)
}

/// The settings of a running controller read from its bound parameter values (`currentControllerGains`): a value is taken
/// when it is finite, and the result exists only when a proportional gain is bound.
#[must_use]
pub fn current_gains(
    values: &BTreeMap<String, f64>,
    parameters: &[MappedParameter],
) -> Option<BTreeMap<GainKey, f64>> {
    let mut gains = BTreeMap::new();
    for parameter in parameters {
        if let Some(value) = values
            .get(&parameter.symbol)
            .filter(|value| value.is_finite())
        {
            gains.insert(parameter.key, *value);
        }
    }
    gains
        .contains_key(&GainKey::ProportionalGain)
        .then_some(gains)
}

/// The values a manual entry writes (`manualControllerCandidate.ts`): every setting the family needs must be present, a
/// pinned value that the engineer changed is refused, and the pins are merged into the entry. The returned symbols are the
/// entered values, checked by [`apply_pins`] for their bounds.
///
/// # Errors
/// Returns a [`Refusal`] when a required setting is missing, or a setting is entered that a pin holds with another value.
pub fn merge_manual_entry(
    family: Structure,
    entered: &BTreeMap<String, f64>,
    pins: &BTreeMap<String, f64>,
    parameters: &[MappedParameter],
) -> Result<BTreeMap<String, f64>, Refusal> {
    let mut values = entered.clone();
    for (symbol, pinned) in pins {
        if let Some(current) = values.get(symbol)
            && (current - pinned).abs() > 0.0
        {
            return Err(Refusal::new(format!(
                "Unpin {symbol} before changing its setting."
            )));
        }
        values.insert(symbol.clone(), *pinned);
    }
    for parameter in required_keys(family, parameters) {
        if !values
            .get(&parameter.symbol)
            .is_some_and(|value| value.is_finite())
        {
            return Err(Refusal::new(texts::REQUIRED_SETTING_MISSING));
        }
    }
    Ok(values)
}

/// The bound parameters a family needs in a manual entry (`required` of `manualControllerCandidate.ts`): the proportional
/// gain always, the integral gain for PI and PID, the derivative gain for PD and PID, the tracking gain and the setpoint
/// weight for PI and PID, the derivative weight for PD and PID, and the filter time whenever it is bound.
fn required_keys(family: Structure, parameters: &[MappedParameter]) -> Vec<&MappedParameter> {
    parameters
        .iter()
        .filter(|parameter| match parameter.key {
            GainKey::ProportionalGain | GainKey::MeasurementFilterSeconds => true,
            GainKey::IntegralGain => family.has_integral(),
            GainKey::DerivativeGain | GainKey::DerivativeSetpointWeight => family.has_derivative(),
            GainKey::BackCalculationGain | GainKey::ProportionalSetpointWeight => {
                family.has_integral()
            }
        })
        .collect()
}

/// The parameter symbols that a family's manual settings and its pins take (B4.2, C0, C2): the bound parameters that the manual
/// entry of `family` requires, in the order of the mapping. A program's own law gives the variables of its seed that the family
/// uses; a Control.PID gives the record fields that the family uses. The page asks for these, and Tune refuses any other.
#[must_use]
pub fn family_setting_symbols(loop_definition: &LoopDefinition, family: Structure) -> Vec<String> {
    // A proportional law (C13) has one setting, its gain, for every family the page shows (`manualTuneSettings.ts`).
    if let Some(ControllerLaw::Proportional { gain_symbol }) = &loop_definition.controller.law {
        let bound = loop_definition
            .parameter_binding
            .as_ref()
            .is_some_and(|binding| {
                binding
                    .parameters
                    .iter()
                    .any(|parameter| parameter.symbol == *gain_symbol)
            });
        return if bound {
            vec![gain_symbol.clone()]
        } else {
            Vec::new()
        };
    }
    let mut symbols: Vec<String> = required_keys(family, &mapped_parameters(loop_definition))
        .into_iter()
        .map(|parameter| parameter.symbol.clone())
        .collect();
    // A trajectory-following law's manual entry also takes its rate and acceleration feedforward gains (C11), when the binding holds them.
    if let Some(trajectory) = served_trajectory(loop_definition) {
        let bound = loop_definition
            .parameter_binding
            .as_ref()
            .map(|binding| &binding.parameters);
        symbols.extend(
            [&trajectory.rate_symbol, &trajectory.acceleration_symbol]
                .into_iter()
                .filter(|symbol| {
                    bound.is_some_and(|parameters| {
                        parameters
                            .iter()
                            .any(|parameter| parameter.symbol == **symbol)
                    })
                })
                .cloned(),
        );
    }
    symbols
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // The pinned values are compared exactly with the values that were pinned.

    use super::*;

    fn parameter(symbol: &str, key: GainKey, minimum: f64, maximum: f64) -> MappedParameter {
        MappedParameter {
            symbol: symbol.to_string(),
            key,
            bounds: GainBounds { minimum, maximum },
            real: false,
        }
    }

    #[test]
    fn a_manual_pi_entry_keeps_its_values_and_refuses_the_bad_ones() {
        // The manual PI of `control-studio-manual-controller-candidate.test.ts`: the entered values are kept exactly, a missing
        // integral gain or one outside its bounds is refused, and a derivative gain on a PI is a family conflict.
        let entered = BTreeMap::from([
            ("Main.Kp".to_string(), 1.234_567_890_123_45),
            ("Main.Ki".to_string(), 0.789_012_345_678_9),
        ]);
        let values = merge_manual_entry(Structure::PI, &entered, &BTreeMap::new(), &parameters())
            .expect("complete");
        let pins = ControllerSettingsPins {
            loop_revision: "r1".to_string(),
            family: Structure::PI,
            values,
        };
        let kept = apply_pins(
            "r1",
            Structure::PI,
            &BTreeMap::new(),
            Some(&pins),
            &parameters(),
        )
        .expect("in bounds");
        assert_eq!(kept[&GainKey::ProportionalGain], 1.234_567_890_123_45);
        assert_eq!(kept[&GainKey::IntegralGain], 0.789_012_345_678_9);
        let out_of_bounds = ControllerSettingsPins {
            loop_revision: "r1".to_string(),
            family: Structure::PI,
            values: BTreeMap::from([("Main.Kp".to_string(), 1.0), ("Main.Ki".to_string(), 11.0)]),
        };
        assert!(apply_pins(
            "r1",
            Structure::PI,
            &BTreeMap::new(),
            Some(&out_of_bounds),
            &parameters()
        )
        .is_err());
        let with_derivative = ControllerSettingsPins {
            loop_revision: "r1".to_string(),
            family: Structure::PI,
            values: BTreeMap::from([
                ("Main.Kp".to_string(), 1.0),
                ("Main.Ki".to_string(), 0.5),
                ("Main.Kd".to_string(), 2.0),
            ]),
        };
        assert_eq!(
            apply_pins(
                "r1",
                Structure::PI,
                &BTreeMap::new(),
                Some(&with_derivative),
                &parameters()
            )
            .expect_err("PI has no derivative")
            .0,
            "Pinned integral/derivative settings conflict with the accepted controller family."
        );
        let missing = BTreeMap::from([("Main.Kp".to_string(), 1.0)]);
        assert_eq!(
            merge_manual_entry(Structure::PI, &missing, &BTreeMap::new(), &parameters())
                .expect_err("no Ki")
                .0,
            "Enter every required controller setting before tuning."
        );
    }

    fn parameters() -> Vec<MappedParameter> {
        vec![
            parameter("Main.Kp", GainKey::ProportionalGain, 0.0, 100.0),
            parameter("Main.Ki", GainKey::IntegralGain, 0.0, 10.0),
            parameter("Main.Kd", GainKey::DerivativeGain, 0.0, 10.0),
        ]
    }

    fn gains(proportional: f64, integral: f64) -> BTreeMap<GainKey, f64> {
        BTreeMap::from([
            (GainKey::ProportionalGain, proportional),
            (GainKey::IntegralGain, integral),
        ])
    }

    #[test]
    fn a_pin_is_kept_through_the_design() {
        let pins = ControllerSettingsPins {
            loop_revision: "r1".to_string(),
            family: Structure::PI,
            values: BTreeMap::from([("Main.Kp".to_string(), 2.5)]),
        };
        let applied = apply_pins(
            "r1",
            Structure::PI,
            &gains(1.0, 0.2),
            Some(&pins),
            &parameters(),
        )
        .expect("pinned");
        assert_eq!(applied[&GainKey::ProportionalGain], 2.5);
        assert_eq!(applied[&GainKey::IntegralGain], 0.2);
    }

    #[test]
    fn a_pin_for_another_loop_or_family_is_refused() {
        let pins = ControllerSettingsPins {
            loop_revision: "old".to_string(),
            family: Structure::PI,
            values: BTreeMap::new(),
        };
        assert_eq!(
            apply_pins("r1", Structure::PI, &gains(1.0, 0.2), Some(&pins), &parameters()).expect_err("other loop").0,
            "Pinned settings belong to a different loop or controller; explicitly clear them before changing the design."
        );
    }

    #[test]
    fn a_gain_pin_is_checked_by_magnitude() {
        let reverse_pin = ControllerSettingsPins {
            loop_revision: "r1".to_string(),
            family: Structure::P,
            values: BTreeMap::from([("Main.Kp".to_string(), -2.0)]),
        };
        assert!(apply_pins(
            "r1",
            Structure::P,
            &gains(1.0, 0.0),
            Some(&reverse_pin),
            &parameters()
        )
        .is_ok());
    }

    #[test]
    fn a_pinned_integral_term_conflicts_with_a_proportional_family() {
        let pins = ControllerSettingsPins {
            loop_revision: "r1".to_string(),
            family: Structure::P,
            values: BTreeMap::from([("Main.Ki".to_string(), 1.0)]),
        };
        assert_eq!(
            apply_pins(
                "r1",
                Structure::P,
                &gains(1.0, 0.0),
                Some(&pins),
                &parameters()
            )
            .expect_err("conflict")
            .0,
            "Pinned integral/derivative settings conflict with the accepted controller family."
        );
    }

    #[test]
    fn a_pin_of_a_symbol_the_loop_does_not_bind_is_refused() {
        let pins = ControllerSettingsPins {
            loop_revision: "r1".to_string(),
            family: Structure::P,
            values: BTreeMap::from([("Main.Other".to_string(), 1.0)]),
        };
        assert!(apply_pins(
            "r1",
            Structure::P,
            &gains(1.0, 0.0),
            Some(&pins),
            &parameters()
        )
        .is_err());
    }

    #[test]
    fn a_real_value_must_survive_single_precision() {
        assert_eq!(single_precision(0.5), 0.5);
        assert_ne!(single_precision(0.1), 0.1);
    }

    #[test]
    fn a_manual_entry_needs_its_family_terms_and_refuses_a_changed_pin() {
        let entered = BTreeMap::from([("Main.Kp".to_string(), 1.0)]);
        assert_eq!(
            merge_manual_entry(Structure::PI, &entered, &BTreeMap::new(), &parameters())
                .expect_err("missing Ki")
                .0,
            "Enter every required controller setting before tuning."
        );
        let complete = BTreeMap::from([("Main.Kp".to_string(), 1.0), ("Main.Ki".to_string(), 0.5)]);
        let pinned = BTreeMap::from([("Main.Kp".to_string(), 2.0)]);
        assert_eq!(
            merge_manual_entry(Structure::PI, &complete, &pinned, &parameters())
                .expect_err("changed pin")
                .0,
            "Unpin Main.Kp before changing its setting."
        );
    }

    #[test]
    fn a_pid_law_maps_its_bound_parameters_to_their_record_fields() {
        use crate::identification::identify_result::tests::plant_loop;
        use crate::loop_definition::{
            AntiWindup, AntiWindupMethod, ModeBinding, Parameter, ParameterBinding, PidLaw,
        };

        let bound = |symbol: &str, iec_type: RealType| Parameter {
            symbol: symbol.to_owned(),
            iec_type,
            unit: "1".to_owned(),
            convention: "gain".to_owned(),
            minimum: 0.0,
            maximum: 10.0,
        };
        let mut loop_definition = plant_loop();
        loop_definition.controller.law = Some(ControllerLaw::Pid(Box::new(PidLaw {
            parameter_record_symbol: "Main.Parameters".to_owned(),
            parameter_revision_symbol: None,
            proportional_gain_symbol: "Main.Kp".to_owned(),
            integral_gain_symbol: "Main.Ki".to_owned(),
            derivative_gain_symbol: "Main.Kd".to_owned(),
            proportional_setpoint_weight_symbol: Some("Main.Beta".to_owned()),
            derivative_setpoint_weight_symbol: None,
            measurement_filter_time_symbol: Some("Main.Tf".to_owned()),
            anti_windup: Some(AntiWindup {
                method: AntiWindupMethod::BackCalculation,
                tracking_gain_symbol: "Main.Kt".to_owned(),
            }),
        })));
        // The tracking gain `Main.Kt` is not bound, so it is not mapped; `Main.Tf` is REAL, the others LREAL.
        loop_definition.parameter_binding = Some(ParameterBinding {
            loop_id: "tank".to_owned(),
            task_name: "MainTask".to_owned(),
            parameters: vec![
                bound("Main.Kp", RealType::Lreal),
                bound("Main.Ki", RealType::Lreal),
                bound("Main.Kd", RealType::Lreal),
                bound("Main.Beta", RealType::Lreal),
                bound("Main.Tf", RealType::Real),
            ],
            mode: ModeBinding {
                symbol: "Main.Mode".to_owned(),
                manual_value: false,
            },
        });
        let mapped: Vec<(String, GainKey, bool)> = mapped_parameters(&loop_definition)
            .into_iter()
            .map(|parameter| (parameter.symbol, parameter.key, parameter.real))
            .collect();
        assert_eq!(
            mapped,
            vec![
                ("Main.Kp".to_string(), GainKey::ProportionalGain, false),
                ("Main.Ki".to_string(), GainKey::IntegralGain, false),
                ("Main.Kd".to_string(), GainKey::DerivativeGain, false),
                (
                    "Main.Beta".to_string(),
                    GainKey::ProportionalSetpointWeight,
                    false
                ),
                (
                    "Main.Tf".to_string(),
                    GainKey::MeasurementFilterSeconds,
                    true
                ),
            ]
        );
        loop_definition.controller.law = None;
        assert_eq!(
            mapped_parameters(&loop_definition),
            [] as [MappedParameter; 0]
        );
    }

    #[test]
    fn decoded_pins_need_a_revision_and_finite_values() {
        let good =
            serde_json::json!({"loopRevision": "r1", "family": "PI", "values": {"Main.Kp": 1.0}});
        assert!(decode_pins(&good).is_some());
        let no_revision = serde_json::json!({"loopRevision": "", "family": "PI", "values": {}});
        assert!(decode_pins(&no_revision).is_none());
    }
}

#[cfg(test)]
#[path = "settings_pins_ported_tests.rs"]
mod ported_tests;
