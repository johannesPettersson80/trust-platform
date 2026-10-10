//! The tuning components of a program's law (specification 35, C5; `projectTuningComponents.ts`): the explicit grids of values
//! that Tune may try for a low-pass filter time, a reference weighting, or a back-calculation tracking time, the declarations
//! that are checked before a search, and the candidate values that the search realizes within the bounds and the pins.
//!
//! A notch is a declared kind: its section is the discrete second-order filter of C5, designed by `trust_control_math::notch`
//! for each centre frequency and quality factor in its grid, and kept only when its denominator is strictly stable.

#![allow(clippy::module_name_repetitions)] // `ProjectTuningComponent` names the module's subject.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::notch::{self, NotchCoefficients};

use crate::identification::refusal::Refusal;
use crate::tuning::project_model::ModelParameter;
use crate::tuning::settings_pins::single_precision;
use crate::tuning::structure::is_exactly_zero;
use crate::tuning::texts;

/// The most components a program law may declare (`validProjectComponents`).
pub const MAXIMUM_COMPONENTS: usize = 8;
/// The most values a component's grid may hold (`validProjectComponents`).
pub const MAXIMUM_GRID_VALUES: usize = 16;
/// The only coefficient convention a notch may declare: the direct form with minus feedback (C5).
const NOTCH_CONVENTION: &str = "direct-form-minus-feedback";
/// The number of coefficient symbols of a notch, in the order `b0, b1, b2, a1, a2` (C5).
const NOTCH_SYMBOLS: usize = 5;

/// A component of a program's law whose coefficients Tune searches (C5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ProjectTuningComponent {
    /// A first- or second-order low-pass filter on the measurement, with the time constants Tune may try.
    #[serde(rename_all = "camelCase")]
    LowPass {
        /// The time constant's symbol.
        time_constant_symbol: String,
        /// The time constants to try, in seconds.
        time_constants_seconds: Vec<f64>,
        /// The filter's order, one or two.
        order: u8,
    },
    /// A reference weighting of the proportional and derivative terms.
    #[serde(rename_all = "camelCase")]
    ReferenceWeighting {
        /// The proportional weight's symbol.
        proportional_symbol: String,
        /// The derivative weight's symbol.
        derivative_symbol: String,
        /// The proportional weights to try, each between zero and one.
        proportional_weights: Vec<f64>,
        /// The derivative weights to try, each between zero and one.
        derivative_weights: Vec<f64>,
    },
    /// The back-calculation tracking time, whose gain is `1/Tt`.
    #[serde(rename_all = "camelCase")]
    BackCalculation {
        /// The tracking gain's symbol, in reciprocal seconds.
        gain_symbol: String,
        /// The tracking times to try, in seconds.
        tracking_times_seconds: Vec<f64>,
    },
    /// A notch filter on the measurement: the discrete second-order section of C5, whose five coefficients are bound
    /// parameters, with the centre frequencies and quality factors that Tune may try.
    #[serde(rename_all = "camelCase")]
    Notch {
        /// The coefficient convention; only the direct form with minus feedback is declared.
        convention: String,
        /// The five coefficient symbols, in the order `b0`, `b1`, `b2`, `a1` and `a2`.
        symbols: Vec<String>,
        /// The centre frequencies to try, in hertz, each positive.
        frequencies_hz: Vec<f64>,
        /// The quality factors to try, each at least one half.
        quality_factors: Vec<f64>,
    },
}

/// One realized choice of a component: the value of each of its symbols, and the complexity that orders the choices.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentChoice {
    /// The value of each symbol of the component.
    pub values: BTreeMap<String, f64>,
    /// The complexity of the choice: zero for the plain loop, one or two for the filtered forms.
    pub complexity: u8,
}

impl ProjectTuningComponent {
    /// The symbols the component writes, in the order the choices list them.
    #[must_use]
    pub fn symbols(&self) -> Vec<&str> {
        match self {
            Self::LowPass {
                time_constant_symbol,
                ..
            } => vec![time_constant_symbol.as_str()],
            Self::ReferenceWeighting {
                proportional_symbol,
                derivative_symbol,
                ..
            } => vec![proportional_symbol.as_str(), derivative_symbol.as_str()],
            Self::BackCalculation { gain_symbol, .. } => vec![gain_symbol.as_str()],
            Self::Notch { symbols, .. } => symbols.iter().map(String::as_str).collect(),
        }
    }

    /// The unit each symbol of the component must declare (`componentSymbols` and the unit check of the TypeScript).
    fn unit(&self) -> &'static str {
        match self {
            Self::LowPass { .. } => "s",
            Self::ReferenceWeighting { .. } | Self::Notch { .. } => "1",
            Self::BackCalculation { .. } => "1/s",
        }
    }
}

/// Checks the declared components before a search (`validProjectComponents`): at most eight, each with its own symbols
/// that the loop binds with the component's unit, non-empty grids of at most sixteen finite values in their domains, and an
/// order of one or two for a low-pass filter.
///
/// # Errors
/// Returns a [`Refusal`] with the declaration sentence when a component breaks one of these rules.
pub fn validate_components(
    components: &[ProjectTuningComponent],
    parameters: &[ModelParameter],
) -> Result<(), Refusal> {
    let invalid = || Refusal::new(texts::COMPONENTS_INVALID);
    if components.len() > MAXIMUM_COMPONENTS {
        return Err(invalid());
    }
    let mut seen: Vec<&str> = Vec::new();
    for component in components {
        for symbol in component.symbols() {
            let bound = parameters
                .iter()
                .find(|parameter| parameter.symbol == symbol)
                .ok_or_else(invalid)?;
            if seen.contains(&symbol) || bound.unit != component.unit() {
                return Err(invalid());
            }
            seen.push(symbol);
        }
        let grids_ok = match component {
            ProjectTuningComponent::LowPass {
                time_constants_seconds,
                order,
                ..
            } => {
                matches!(*order, 1 | 2) && grid_valid(time_constants_seconds, |value| value >= 0.0)
            }
            ProjectTuningComponent::ReferenceWeighting {
                proportional_weights,
                derivative_weights,
                ..
            } => {
                grid_valid(proportional_weights, |value| (0.0..=1.0).contains(&value))
                    && grid_valid(derivative_weights, |value| (0.0..=1.0).contains(&value))
            }
            ProjectTuningComponent::BackCalculation {
                tracking_times_seconds,
                ..
            } => grid_valid(tracking_times_seconds, |value| value > 0.0),
            ProjectTuningComponent::Notch {
                convention,
                symbols,
                frequencies_hz,
                quality_factors,
            } => {
                convention.as_str() == NOTCH_CONVENTION
                    && symbols.len() == NOTCH_SYMBOLS
                    && grid_valid(frequencies_hz, |value| value > 0.0)
                    && grid_valid(quality_factors, |value| value >= 0.5)
            }
        };
        if !grids_ok {
            return Err(invalid());
        }
    }
    Ok(())
}

/// A grid is non-empty, holds at most sixteen values, and every value is finite and in its domain.
fn grid_valid(values: &[f64], in_domain: impl Fn(f64) -> bool) -> bool {
    !values.is_empty()
        && values.len() <= MAXIMUM_GRID_VALUES
        && values
            .iter()
            .all(|value| value.is_finite() && in_domain(*value))
}

/// The realized choices of a component (`componentChoices` for the low-pass, the reference weighting and the back-calculation):
/// each candidate is the grid's combination, realized as the bound parameter represents it (single precision for `REAL`),
/// kept within its bounds, equal to any pinned value, and unique. The task period in seconds sets the back-calculation's
/// `1/max(Δt, Tt)`.
///
/// # Errors
/// Returns [`texts::NO_REPRESENTABLE_COMPONENT`] when no choice is representable within the bounds and the pins.
pub fn component_choices(
    component: &ProjectTuningComponent,
    parameters: &[ModelParameter],
    pins: &BTreeMap<String, f64>,
    task_period_seconds: f64,
) -> Result<Vec<ComponentChoice>, Refusal> {
    let candidates = raw_choices(component, task_period_seconds)?;
    let mut realized: Vec<ComponentChoice> = Vec::new();
    for candidate in candidates {
        let Some(values) = realize(&candidate.values, parameters, pins) else {
            continue;
        };
        let Some(complexity) = complexity_within_domain(component, &values) else {
            continue;
        };
        if realized.iter().any(|previous| previous.values == values) {
            continue;
        }
        realized.push(ComponentChoice { values, complexity });
    }
    if realized.is_empty() {
        return Err(Refusal::new(texts::NO_REPRESENTABLE_COMPONENT));
    }
    Ok(realized)
}

/// The grid's combinations before realization, with their complexity. A notch's grid starts from the plain loop, the
/// section that passes the signal unchanged (`b0 = 1`, the others zero), and then holds one section per frequency and
/// quality factor (C5).
///
/// # Errors
/// Returns the notch sentence when a centre frequency is not positive or not below the sampled Nyquist limit.
fn raw_choices(
    component: &ProjectTuningComponent,
    task_period_seconds: f64,
) -> Result<Vec<ComponentChoice>, Refusal> {
    let symbols = component.symbols();
    let choice = |values: &[f64], complexity: u8| ComponentChoice {
        values: symbols
            .iter()
            .zip(values)
            .map(|(symbol, value)| ((*symbol).to_string(), *value))
            .collect(),
        complexity,
    };
    let choices: Vec<ComponentChoice> = match component {
        ProjectTuningComponent::LowPass {
            time_constants_seconds,
            order,
            ..
        } => {
            let mut times = vec![0.0];
            times.extend(time_constants_seconds.iter().copied());
            times
                .into_iter()
                .map(|time| choice(&[time], if is_exactly_zero(time) { 0 } else { *order }))
                .collect()
        }
        ProjectTuningComponent::ReferenceWeighting {
            proportional_weights,
            derivative_weights,
            ..
        } => proportional_weights
            .iter()
            .flat_map(|beta| derivative_weights.iter().map(move |gamma| (*beta, *gamma)))
            .map(|(beta, gamma)| {
                let plain = (beta - 1.0).abs() <= 0.0 && is_exactly_zero(gamma);
                choice(&[beta, gamma], u8::from(!plain))
            })
            .collect(),
        ProjectTuningComponent::BackCalculation {
            tracking_times_seconds,
            ..
        } => tracking_times_seconds
            .iter()
            .map(|time| choice(&[1.0 / time.max(task_period_seconds)], 1))
            .collect(),
        ProjectTuningComponent::Notch {
            frequencies_hz,
            quality_factors,
            ..
        } => {
            let mut sections = vec![choice(&[1.0, 0.0, 0.0, 0.0, 0.0], 0)];
            for frequency in frequencies_hz {
                for quality in quality_factors {
                    let section = notch::design(*frequency, *quality, task_period_seconds)
                        .map_err(|_| Refusal::new(texts::NOTCH_NYQUIST))?;
                    sections.push(choice(&notch_coefficients(&section), 2));
                }
            }
            sections
        }
    };
    Ok(choices)
}

/// The five coefficients of a notch in the order of its symbols, `b0, b1, b2, a1, a2` (C5).
fn notch_coefficients(section: &NotchCoefficients) -> [f64; NOTCH_SYMBOLS] {
    let [first, second, third] = section.feed_forward;
    let [fourth, fifth] = section.feedback;
    [first, second, third, fourth, fifth]
}

/// Realizes a candidate's values for the bound parameters: a pinned value stands, a `REAL` value is rounded to single
/// precision, and each value must be finite, within its bounds, and unchanged by the rounding if it is pinned.
fn realize(
    values: &BTreeMap<String, f64>,
    parameters: &[ModelParameter],
    pins: &BTreeMap<String, f64>,
) -> Option<BTreeMap<String, f64>> {
    let mut realized = BTreeMap::new();
    for (symbol, candidate) in values {
        let parameter = parameters
            .iter()
            .find(|parameter| &parameter.symbol == symbol)?;
        let pinned = pins.get(symbol).copied();
        let value = pinned.unwrap_or(*candidate);
        let stored = if parameter.real {
            single_precision(value)
        } else {
            value
        };
        let within = stored.is_finite()
            && stored >= parameter.bounds.minimum
            && stored <= parameter.bounds.maximum;
        let kept = pinned.is_none_or(|pin| (pin - stored).abs() <= 0.0);
        if !within || !kept {
            return None;
        }
        realized.insert(symbol.clone(), stored);
    }
    Some(realized)
}

/// The complexity of a realized choice, or `None` when it is outside its component's domain (`componentChoices`): a time
/// constant of zero or more, weights between zero and one, and a tracking gain of zero or more.
fn complexity_within_domain(
    component: &ProjectTuningComponent,
    values: &BTreeMap<String, f64>,
) -> Option<u8> {
    match component {
        ProjectTuningComponent::LowPass {
            time_constant_symbol,
            order,
            ..
        } => {
            let time = *values.get(time_constant_symbol)?;
            (time >= 0.0).then_some(if is_exactly_zero(time) { 0 } else { *order })
        }
        ProjectTuningComponent::ReferenceWeighting {
            proportional_symbol,
            derivative_symbol,
            ..
        } => {
            let beta = *values.get(proportional_symbol)?;
            let gamma = *values.get(derivative_symbol)?;
            let inside = (0.0..=1.0).contains(&beta) && (0.0..=1.0).contains(&gamma);
            let plain = (beta - 1.0).abs() <= 0.0 && is_exactly_zero(gamma);
            inside.then_some(u8::from(!plain))
        }
        ProjectTuningComponent::BackCalculation { gain_symbol, .. } => {
            let gain = *values.get(gain_symbol)?;
            (gain >= 0.0).then_some(u8::from(!is_exactly_zero(gain)))
        }
        ProjectTuningComponent::Notch { symbols, .. } => {
            let realized = realized_notch(symbols, values)?;
            let section = NotchCoefficients {
                feed_forward: [realized[0], realized[1], realized[2]],
                feedback: [realized[3], realized[4]],
            };
            // The plain loop is `b0 = 1` with every other coefficient zero; any other section is complexity two.
            let plain = is_exactly_zero(realized[0] - 1.0)
                && realized[1..].iter().all(|value| is_exactly_zero(*value));
            section.is_stable().then_some(if plain { 0 } else { 2 })
        }
    }
}

/// The realized values of a notch's five symbols, in their order, or `None` when a symbol has no value.
fn realized_notch(
    symbols: &[String],
    values: &BTreeMap<String, f64>,
) -> Option<[f64; NOTCH_SYMBOLS]> {
    let mut coefficients = [0.0; NOTCH_SYMBOLS];
    for (slot, symbol) in coefficients.iter_mut().zip(symbols) {
        *slot = *values.get(symbol)?;
    }
    Some(coefficients)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // The grids are compared with the values they were built from.

    use super::*;
    use crate::tuning::proportional::GainBounds;

    fn parameter(symbol: &str, unit: &str) -> ModelParameter {
        ModelParameter {
            symbol: symbol.to_string(),
            unit: unit.to_string(),
            real: false,
            bounds: GainBounds {
                minimum: 0.0,
                maximum: 10.0,
            },
        }
    }

    fn low_pass() -> ProjectTuningComponent {
        ProjectTuningComponent::LowPass {
            time_constant_symbol: "Main.Tf".to_string(),
            time_constants_seconds: vec![0.5, 1.0],
            order: 1,
        }
    }

    #[test]
    fn a_low_pass_grid_starts_from_the_unfiltered_loop() {
        let choices = component_choices(
            &low_pass(),
            &[parameter("Main.Tf", "s")],
            &BTreeMap::new(),
            0.1,
        )
        .expect("choices");
        let times: Vec<f64> = choices
            .iter()
            .map(|choice| choice.values["Main.Tf"])
            .collect();
        assert_eq!(times, vec![0.0, 0.5, 1.0]);
        assert_eq!(choices[0].complexity, 0);
        assert_eq!(choices[1].complexity, 1);
    }

    #[test]
    fn a_pinned_time_is_the_only_choice_it_leaves() {
        let pins = BTreeMap::from([("Main.Tf".to_string(), 0.5)]);
        let choices = component_choices(&low_pass(), &[parameter("Main.Tf", "s")], &pins, 0.1)
            .expect("choices");
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].values["Main.Tf"], 0.5);
    }

    #[test]
    fn a_back_calculation_gain_is_one_over_the_longer_of_the_period_and_the_tracking_time() {
        let component = ProjectTuningComponent::BackCalculation {
            gain_symbol: "Main.Kt".to_string(),
            tracking_times_seconds: vec![0.05, 2.0],
        };
        let choices = component_choices(
            &component,
            &[parameter("Main.Kt", "1/s")],
            &BTreeMap::new(),
            0.1,
        )
        .expect("choices");
        assert_eq!(choices[0].values["Main.Kt"], 10.0);
        assert_eq!(choices[1].values["Main.Kt"], 0.5);
    }

    #[test]
    fn a_component_whose_unit_is_wrong_is_refused_with_the_declaration_sentence() {
        let refusal =
            validate_components(&[low_pass()], &[parameter("Main.Tf", "1")]).expect_err("unit");
        assert_eq!(refusal.0, "Declare valid tuning components: each symbol bound once, with its unit and its listed values.");
    }

    #[test]
    fn the_reference_weighting_pairs_every_proportional_and_derivative_weight() {
        let component = ProjectTuningComponent::ReferenceWeighting {
            proportional_symbol: "Main.Beta".to_string(),
            derivative_symbol: "Main.Gamma".to_string(),
            proportional_weights: vec![0.0, 1.0],
            derivative_weights: vec![0.0, 0.5],
        };
        let parameters = [parameter("Main.Beta", "1"), parameter("Main.Gamma", "1")];
        let choices =
            component_choices(&component, &parameters, &BTreeMap::new(), 0.1).expect("choices");
        assert_eq!(choices.len(), 4);
        assert_eq!(
            choices[1].complexity, 1,
            "beta 0 with gamma 0.5 is a filtered form"
        );
    }

    /// A notch coefficient: unitless, with bounds wide enough for every section.
    fn coefficient(symbol: &str) -> ModelParameter {
        ModelParameter {
            symbol: symbol.to_string(),
            unit: "1".to_string(),
            real: false,
            bounds: GainBounds {
                minimum: -10.0,
                maximum: 10.0,
            },
        }
    }

    const NOTCH_SYMBOL_NAMES: [&str; 5] = ["Main.B0", "Main.B1", "Main.B2", "Main.A1", "Main.A2"];

    fn notch_component(
        frequencies_hz: Vec<f64>,
        quality_factors: Vec<f64>,
    ) -> ProjectTuningComponent {
        ProjectTuningComponent::Notch {
            convention: NOTCH_CONVENTION.to_string(),
            symbols: NOTCH_SYMBOL_NAMES
                .iter()
                .map(|symbol| String::from(*symbol))
                .collect(),
            frequencies_hz,
            quality_factors,
        }
    }

    fn notch_parameters() -> Vec<ModelParameter> {
        NOTCH_SYMBOL_NAMES
            .iter()
            .map(|symbol| coefficient(symbol))
            .collect()
    }

    #[test]
    fn a_notch_grid_starts_from_the_plain_loop_and_realizes_its_stable_sections() {
        // 50 Hz at h = 1 ms with Q = 10 is the mains notch of trust_control_math::notch (b0 = 0.98478, a2 = 0.96957).
        let choices = component_choices(
            &notch_component(vec![50.0], vec![10.0]),
            &notch_parameters(),
            &BTreeMap::new(),
            0.001,
        )
        .expect("choices");
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0].complexity, 0);
        assert_eq!(choices[0].values["Main.B0"], 1.0);
        assert_eq!(choices[0].values["Main.A2"], 0.0);
        assert_eq!(choices[1].complexity, 2);
        assert!((choices[1].values["Main.B0"] - 0.984_784_246_600_387_5).abs() <= 1e-12);
        assert!((choices[1].values["Main.A2"] - 0.969_568_493_200_775_2).abs() <= 1e-12);
    }

    #[test]
    fn the_notch_has_zero_gain_at_its_centre_frequency() {
        // The numerator b0 + b1·e^(−jω) + b2·e^(−2jω) vanishes at ω = 2π·f·h, the centre of the designed section (C5).
        let choices = component_choices(
            &notch_component(vec![50.0], vec![10.0]),
            &notch_parameters(),
            &BTreeMap::new(),
            0.001,
        )
        .expect("choices");
        let section = &choices[1].values;
        let omega = 2.0 * std::f64::consts::PI * 50.0 * 0.001;
        let real = section["Main.B0"]
            + section["Main.B1"] * omega.cos()
            + section["Main.B2"] * (2.0 * omega).cos();
        let imaginary =
            -(section["Main.B1"] * omega.sin() + section["Main.B2"] * (2.0 * omega).sin());
        assert!(
            real.hypot(imaginary) <= 1e-12,
            "the gain at the centre is {}",
            real.hypot(imaginary)
        );
    }

    #[test]
    fn a_notch_centre_at_or_above_the_sampled_nyquist_limit_is_refused() {
        // At h = 1 ms the sampled Nyquist limit is 500 Hz, so 600 Hz is above it.
        let refusal = component_choices(
            &notch_component(vec![600.0], vec![10.0]),
            &notch_parameters(),
            &BTreeMap::new(),
            0.001,
        )
        .expect_err("above the Nyquist limit");
        assert_eq!(refusal.0, texts::NOTCH_NYQUIST);
    }

    #[test]
    fn a_notch_must_declare_the_direct_form_with_five_unit_symbols_and_its_grids() {
        let parameters = notch_parameters();
        validate_components(&[notch_component(vec![50.0], vec![10.0])], &parameters)
            .expect("a valid notch");
        let mut other_convention = notch_component(vec![50.0], vec![10.0]);
        if let ProjectTuningComponent::Notch { convention, .. } = &mut other_convention {
            *convention = "direct-form".to_string();
        }
        let refusal =
            validate_components(&[other_convention], &parameters).expect_err("convention");
        assert_eq!(refusal.0, texts::COMPONENTS_INVALID);
        let refusal = validate_components(&[notch_component(vec![50.0], vec![0.4])], &parameters)
            .expect_err("a quality factor below one half");
        assert_eq!(refusal.0, texts::COMPONENTS_INVALID);
    }
}
