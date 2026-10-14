//! The declared analysis of Control Studio (N7.5, N7.6): an explicit linear representation that the
//! engineer supplies, with a bounded parameter map and one swept parameter. It is not inferred from custom
//! ST and needs no live controller. Its pole paths and its stored record are here; the root solver is in
//! `trust_control_math`.

// The exact comparisons below are deliberate: a zero leading coefficient, a zero denominator, a listed
// swept value and a stored gain are compared as they are written, not within a tolerance.
#![allow(clippy::float_cmp)]

use std::cmp::Ordering;
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::classical::Transfer;
use trust_control_math::polynomial::convolve;
use trust_control_math::root_locus::root_locus;

use crate::analysis::{limits, texts};
use crate::failure::{Cause, Failure, NextAction};
use crate::model::{ModelSource, ModelValidation};

/// The kind of the plant of a declared analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PlantKind {
    /// A continuous transfer function.
    TransferFunction,
}

/// The plant: a continuous transfer function with a delay in seconds, and its units and provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclaredPlant {
    /// The kind of the plant.
    pub kind: PlantKind,
    /// The numerator coefficients, from the highest power down.
    pub numerator: Vec<f64>,
    /// The denominator coefficients, from the highest power down; its leading coefficient is not zero.
    pub denominator: Vec<f64>,
    /// The pure delay in seconds, zero or more.
    pub delay_seconds: f64,
    /// The unit of the input.
    pub input_unit: String,
    /// The unit of the output.
    pub output_unit: String,
    /// The revision of the model.
    pub revision: String,
    /// Where the model came from.
    pub source: ModelSource,
    /// Whether the model is validated.
    pub validation: ModelValidation,
}

/// A coefficient of the controller: a number, or a parameter times an optional finite scale (default one).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum DeclaredCoefficient {
    /// A fixed number.
    Constant(f64),
    /// A parameter of the controller, scaled.
    Parameter(ParameterCoefficient),
}

/// A parameter of the controller as a coefficient, with its scale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ParameterCoefficient {
    /// The name of the parameter.
    pub parameter: String,
    /// The finite scale of the parameter; one when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
}

/// A controller parameter: its value, its allowed range and its unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclaredParameter {
    /// The value in use.
    pub value: f64,
    /// The smallest allowed value.
    pub minimum: f64,
    /// The largest allowed value.
    pub maximum: f64,
    /// The unit of the parameter.
    pub unit: String,
}

/// The controller: its numerator and denominator in parameters, and the parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclaredController {
    /// The numerator coefficients, from the highest power down.
    pub numerator: Vec<DeclaredCoefficient>,
    /// The denominator coefficients, from the highest power down.
    pub denominator: Vec<DeclaredCoefficient>,
    /// The parameters by name.
    pub parameters: BTreeMap<String, DeclaredParameter>,
}

/// The swept parameter, its values and the selected value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclaredSweep {
    /// The swept parameter: a parameter of the controller.
    pub parameter: String,
    /// The gains to evaluate, in the parameter's range.
    pub values: Vec<f64>,
    /// The value that the analysis reports, one of `values`.
    pub selected_value: f64,
}

/// A declared analysis: the plant, the controller and the sweep (N7.6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclaredControlAnalysis {
    /// The plant.
    pub plant: DeclaredPlant,
    /// The controller.
    pub controller: DeclaredController,
    /// The swept parameter.
    pub root_locus: DeclaredSweep,
}

/// A pole of the closed loop: its real and imaginary parts.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Pole {
    /// The real part.
    pub real: f64,
    /// The imaginary part.
    pub imaginary: f64,
}

/// The poles of the closed loop at one gain, sorted by real part, then imaginary part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GainPoles {
    /// The gain.
    pub gain: f64,
    /// The poles at the gain.
    pub poles: Vec<Pole>,
}

/// A fixed parameter of a result: its value and its unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FixedParameter {
    /// The value.
    pub value: f64,
    /// The unit.
    pub unit: String,
}

/// The pole paths of a declared analysis (N7.5): the poles at each gain of the sweep, and at evenly
/// spaced gains across the parameter's whole range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclaredRootLocusResult {
    /// The swept parameter.
    pub parameter: String,
    /// The unit of the swept parameter.
    pub parameter_unit: String,
    /// The selected value.
    pub selected_value: f64,
    /// The parameters that stay fixed, by name.
    pub fixed_parameters: BTreeMap<String, FixedParameter>,
    /// The model's source, revision and validation.
    pub provenance: String,
    /// The approximations of the calculation.
    pub approximation: String,
    /// The poles at each gain of the sweep.
    pub branches: Vec<GainPoles>,
    /// The poles across the declared range, for drawing; a gain where the loop is undefined is left out.
    pub path: Vec<GainPoles>,
}

/// A declared analysis with its calculated pole paths, as it is stored in a study.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedDeclaredAnalysis {
    /// The definition that was calculated.
    pub definition: DeclaredControlAnalysis,
    /// The result of the calculation.
    pub result: DeclaredRootLocusResult,
}

/// Checks a declared analysis: the plant, the parameters, the coefficients, the sweep and the denominators
/// at every gain (N7.6). The analysis changes nothing.
///
/// # Errors
/// Returns the declared-invalid sentence when any rule fails.
pub fn check_declared(analysis: &DeclaredControlAnalysis) -> Result<(), Failure> {
    if declared_is_valid(analysis) {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// Calculates the pole paths of a declared analysis (N7.5, N7.6).
///
/// # Errors
/// Returns the declared-invalid sentence for an invalid definition, the zero-denominator sentence for a
/// gain at which the controller's denominator vanishes, and the solver sentence when the roots of a
/// gain's characteristic polynomial cannot be computed.
#[allow(clippy::too_many_lines)] // One pass over the declared gains, in the order N7.6 states its checks.
pub fn calculate_declared_root_locus(
    analysis: &DeclaredControlAnalysis,
) -> Result<DeclaredRootLocusResult, Failure> {
    check_declared(analysis)?;
    let plant = &analysis.plant;
    let controller = &analysis.controller;
    let sweep = &analysis.root_locus;
    let (plant_numerator, plant_denominator) = if plant.delay_seconds > 0.0 {
        // The first-order Padé approximation of the delay, identified in the result (N7.5).
        let half = plant.delay_seconds / 2.0;
        (
            convolve(&plant.numerator, &[-half, 1.0]),
            convolve(&plant.denominator, &[half, 1.0]),
        )
    } else {
        (plant.numerator.clone(), plant.denominator.clone())
    };
    let poles_at = |gain: f64| -> Result<GainPoles, Failure> {
        let numerator = convolve(
            &plant_numerator,
            &substitute(&controller.numerator, &sweep.parameter, gain, controller),
        );
        let denominator = convolve(
            &plant_denominator,
            &substitute(&controller.denominator, &sweep.parameter, gain, controller),
        );
        if denominator.iter().all(|value| *value == 0.0) {
            return Err(solver_failure(&texts::declared_denominator_zero(
                &sweep.parameter,
                gain,
            )));
        }
        let branches = root_locus(
            &Transfer {
                numerator,
                denominator,
                delay_seconds: 0.0,
            },
            &[1.0],
        )
        .map_err(|_| solver_failure(texts::ROOT_SOLVER_FAILED))?;
        let branch = branches
            .into_iter()
            .next()
            .ok_or_else(|| solver_failure(texts::ROOT_SOLVER_FAILED))?;
        let mut poles: Vec<Pole> = branch
            .poles
            .iter()
            .map(|pole| Pole {
                real: pole.real,
                imaginary: pole.imaginary,
            })
            .collect();
        poles.sort_by(compare_poles);
        Ok(GainPoles { gain, poles })
    };
    let branches = sweep
        .values
        .iter()
        .map(|&gain| poles_at(gain))
        .collect::<Result<Vec<_>, _>>()?;
    let range = &controller.parameters[&sweep.parameter];
    let gains: Vec<f64> = if range.maximum > range.minimum {
        (0..limits::POLE_PATH_POINTS)
            .map(|index| range.minimum + (range.maximum - range.minimum) * as_f64(index) / 200.0)
            .collect()
    } else {
        vec![range.minimum]
    };
    let path = gains
        .into_iter()
        .filter_map(|gain| poles_at(gain).ok())
        .collect();
    let fixed_parameters = controller
        .parameters
        .iter()
        .filter(|(name, _)| **name != sweep.parameter)
        .map(|(name, parameter)| {
            (
                name.clone(),
                FixedParameter {
                    value: parameter.value,
                    unit: parameter.unit.clone(),
                },
            )
        })
        .collect();
    let delay_note = if plant.delay_seconds > 0.0 {
        texts::DECLARED_DELAY_PADE
    } else {
        texts::DECLARED_NO_DELAY
    };
    Ok(DeclaredRootLocusResult {
        parameter: sweep.parameter.clone(),
        parameter_unit: range.unit.clone(),
        selected_value: sweep.selected_value,
        fixed_parameters,
        provenance: format!(
            "{} model {} · {}",
            plant.source.word(),
            plant.revision,
            plant.validation.word()
        ),
        approximation: format!("{}{delay_note}", texts::DECLARED_APPROXIMATION),
        branches,
        path,
    })
}

/// Checks a stored declared analysis against its definition: the same parameter, selection, units,
/// fixed parameters and gains, and at most the stored number of poles per gain.
///
/// # Errors
/// Returns the declared-invalid sentence when the definition or the result does not match.
pub fn check_saved_declared(saved: &SavedDeclaredAnalysis) -> Result<(), Failure> {
    let definition = &saved.definition;
    let result = &saved.result;
    check_declared(definition)?;
    let sweep = &definition.root_locus;
    let fixed_names: Vec<&String> = definition
        .controller
        .parameters
        .keys()
        .filter(|name| **name != sweep.parameter)
        .collect();
    let fixed_match = result.fixed_parameters.len() == fixed_names.len()
        && fixed_names.iter().all(|name| {
            result
                .fixed_parameters
                .get(*name)
                .zip(definition.controller.parameters.get(*name))
                .is_some_and(|(actual, expected)| {
                    actual.value == expected.value && actual.unit == expected.unit
                })
        });
    let branches_match = result.branches.len() == sweep.values.len()
        && result
            .branches
            .iter()
            .zip(&sweep.values)
            .all(|(branch, gain)| {
                branch.gain == *gain
                    && !branch.poles.is_empty()
                    && branch.poles.len() <= limits::STORED_POLES_PER_GAIN
            });
    if result.parameter == sweep.parameter
        && result.selected_value == sweep.selected_value
        && result.parameter_unit == definition.controller.parameters[&sweep.parameter].unit
        && fixed_match
        && branches_match
    {
        Ok(())
    } else {
        Err(invalid())
    }
}

/// The controller coefficients with the parameter `parameter` at `value`: a fixed number, or the value for the
/// named parameter and the stored value for any other, times its scale.
pub(crate) fn substitute(
    coefficients: &[DeclaredCoefficient],
    parameter: &str,
    value: f64,
    controller: &DeclaredController,
) -> Vec<f64> {
    coefficients
        .iter()
        .map(|coefficient| match coefficient {
            DeclaredCoefficient::Constant(constant) => *constant,
            DeclaredCoefficient::Parameter(term) => {
                let current = if term.parameter == parameter {
                    value
                } else {
                    controller
                        .parameters
                        .get(&term.parameter)
                        .map_or(0.0, |parameter| parameter.value)
                };
                term.scale.unwrap_or(1.0) * current
            }
        })
        .collect()
}

/// Whether the declared analysis passes every rule of N7.6, with the denominators checked at each gain.
fn declared_is_valid(analysis: &DeclaredControlAnalysis) -> bool {
    let plant = &analysis.plant;
    let controller = &analysis.controller;
    let sweep = &analysis.root_locus;
    let polynomial_ok = |values: &[f64]| {
        !values.is_empty()
            && values.len() <= limits::DECLARED_MAXIMUM_COEFFICIENTS
            && values.iter().all(|value| value.is_finite())
    };
    if !polynomial_ok(&plant.numerator)
        || !polynomial_ok(&plant.denominator)
        || plant.denominator[0] == 0.0
        || !plant.delay_seconds.is_finite()
        || plant.delay_seconds < 0.0
        || [&plant.input_unit, &plant.output_unit, &plant.revision]
            .iter()
            .any(|text| text.trim().is_empty())
        || controller.parameters.is_empty()
        || controller.parameters.len() > limits::DECLARED_MAXIMUM_PARAMETERS
    {
        return false;
    }
    let parameters_ok = controller.parameters.iter().all(|(name, parameter)| {
        !name.trim().is_empty()
            && parameter.value.is_finite()
            && parameter.minimum.is_finite()
            && parameter.maximum.is_finite()
            && parameter.minimum <= parameter.maximum
            && parameter.value >= parameter.minimum
            && parameter.value <= parameter.maximum
            && !parameter.unit.trim().is_empty()
    });
    let coefficients_ok = |coefficients: &[DeclaredCoefficient]| {
        !coefficients.is_empty()
            && coefficients.len() <= limits::DECLARED_MAXIMUM_COEFFICIENTS
            && coefficients.iter().all(|coefficient| match coefficient {
                DeclaredCoefficient::Constant(value) => value.is_finite(),
                DeclaredCoefficient::Parameter(term) => {
                    controller.parameters.contains_key(&term.parameter)
                        && match term.scale {
                            None => true,
                            Some(scale) => scale.is_finite(),
                        }
                }
            })
    };
    if !parameters_ok
        || !coefficients_ok(&controller.numerator)
        || !coefficients_ok(&controller.denominator)
    {
        return false;
    }
    let Some(parameter) = controller.parameters.get(&sweep.parameter) else {
        return false;
    };
    if sweep.values.is_empty()
        || sweep.values.len() > limits::DECLARED_MAXIMUM_SWEEP_VALUES
        || !sweep.selected_value.is_finite()
        || !sweep.values.iter().all(|value| {
            value.is_finite() && *value >= parameter.minimum && *value <= parameter.maximum
        })
        || !sweep.values.contains(&sweep.selected_value)
    {
        return false;
    }
    sweep.values.iter().all(|&gain| {
        let denominator = substitute(&controller.denominator, &sweep.parameter, gain, controller);
        let numerator = substitute(&controller.numerator, &sweep.parameter, gain, controller);
        denominator.iter().all(|value| value.is_finite())
            && denominator.iter().any(|value| *value != 0.0)
            && numerator.iter().all(|value| value.is_finite())
    })
}

/// A position of the pole path as a floating-point number. The path has at most 201 positions.
fn as_f64(index: usize) -> f64 {
    f64::from(u32::try_from(index).unwrap_or(u32::MAX))
}

/// The order of two poles: by real part, then by imaginary part (N7.5).
fn compare_poles(left: &Pole, right: &Pole) -> Ordering {
    left.real
        .partial_cmp(&right.real)
        .unwrap_or(Ordering::Equal)
        .then(
            left.imaginary
                .partial_cmp(&right.imaginary)
                .unwrap_or(Ordering::Equal),
        )
}

/// The failure of a declared analysis that is invalid.
fn invalid() -> Failure {
    Failure::new(
        texts::DECLARED_INVALID,
        Cause::ModelOrData,
        NextAction::None,
    )
}

/// The failure of a calculation that the solver could not qualify.
fn solver_failure(text: &str) -> Failure {
    Failure::new(text, Cause::ModelOrData, NextAction::None)
}

#[cfg(test)]
#[path = "declared_tests.rs"]
mod tests;
