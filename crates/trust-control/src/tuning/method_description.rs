//! The description of a formula method for an identified model (`describeTuningMethod` of `tuning.ts`; specification 35,
//! B4.4 and B4.7): the method's title, whether it applies to the model with the declared response, the reason it does not
//! when it does not, the inputs it reads, and the numbers of its unapplied result.
//!
//! The assumption and equation copy of the TypeScript description is not part of this module yet; the copy needs its own
//! pass with the engineer-facing wording. The gain margins that the description also shows belong to the prediction of
//! layer 2b.

#![allow(clippy::module_name_repetitions)] // `MethodDescription` names the module's subject.

use trust_control_math::identify::gain::ControllerAction;
use trust_control_math::identify::LowOrderModelKind;

use crate::identification::plant::IdentifiedModel;
use crate::tuning::formula::{stored, tune_identified};
use crate::tuning::objective::{TuningMethod, TuningObjective};

/// Whether a method applies to the model with the declared response (B4.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptionStatus {
    /// The method applies, and its result is shown.
    Feasible,
    /// The method does not apply: its reason is shown and it has no result.
    Unavailable,
}

/// The units of a method's gains in the model's units (`units` of the description): the plant gain, and the three gains of
/// the parallel record.
#[derive(Debug, Clone, PartialEq)]
pub struct GainUnits {
    /// The plant's gain: output per input, or output per input per second for an integrating model.
    pub plant_gain: String,
    /// The proportional gain: input per output.
    pub proportional_gain: String,
    /// The integral gain: input per output per second.
    pub integral_gain: String,
    /// The derivative gain: input seconds per output.
    pub derivative_gain: String,
}

/// The numbers of a feasible method's unapplied result (`result` of the description).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DescribedResult {
    /// The signed proportional gain.
    pub proportional_gain: f64,
    /// The integral gain `Kp / Ti`, stored to eight significant digits.
    pub integral_gain: f64,
    /// The integral time in seconds.
    pub integral_time_seconds: f64,
}

/// The description of a formula method for a model (`TuningMethodDescription`).
#[derive(Debug, Clone, PartialEq)]
pub struct MethodDescription {
    /// The method.
    pub method: TuningMethod,
    /// The method's title.
    pub title: &'static str,
    /// Whether the method applies.
    pub status: DescriptionStatus,
    /// The reason the method does not apply, when it does not.
    pub reason: Option<String>,
    /// The name the gain has in the model: `Kprime` for an integrating model, `K` otherwise.
    pub gain_name: &'static str,
    /// The model's gain.
    pub gain: f64,
    /// The model's time constant in seconds; absent for an integrating model or when the model has none.
    pub time_constant_seconds: Option<f64>,
    /// The model's dead time in seconds, zero when none is resolved.
    pub delay_seconds: f64,
    /// The declared response.
    pub objective: TuningObjective,
    /// The closed-loop time constant `λ` of SIMC, when the method has one.
    pub closed_loop_time_seconds: Option<f64>,
    /// The units of the gains.
    pub units: GainUnits,
    /// The result, when the method applies.
    pub result: Option<DescribedResult>,
}

/// Describes a formula method for an identified model without applying it (`describeTuningMethod`).
#[must_use]
pub fn describe_method(
    model: &IdentifiedModel,
    method: TuningMethod,
    objective: TuningObjective,
    action: ControllerAction,
    output_unit: &str,
    input_unit: &str,
) -> MethodDescription {
    let integrating = model.kind == LowOrderModelKind::Integrating;
    let plant_unit = format!(
        "{output_unit}/{input_unit}{}",
        if integrating { "/s" } else { "" }
    );
    let units = GainUnits {
        plant_gain: if integrating {
            format!("{output_unit}/({input_unit}·s)")
        } else {
            format!("{output_unit}/{input_unit}")
        },
        proportional_gain: format!("{input_unit}/{output_unit}"),
        integral_gain: format!("{input_unit}/({output_unit}·s)"),
        derivative_gain: format!("({input_unit}·s)/{output_unit}"),
    };
    let (status, reason, result, closed_loop) =
        match tune_identified(model, method, objective, action, &plant_unit) {
            Ok(tuning) => {
                let gains = tuning.gains();
                (
                    DescriptionStatus::Feasible,
                    None,
                    Some(DescribedResult {
                        proportional_gain: tuning.proportional_gain,
                        integral_gain: stored(gains.integral_gain),
                        integral_time_seconds: tuning.integral_time_seconds,
                    }),
                    tuning.closed_loop_time_seconds,
                )
            }
            Err(refusal) => (DescriptionStatus::Unavailable, Some(refusal.0), None, None),
        };
    MethodDescription {
        method,
        title: method.title(),
        status,
        reason,
        gain_name: if integrating { "Kprime" } else { "K" },
        gain: model.gain,
        time_constant_seconds: if integrating {
            None
        } else {
            model.time_constant_seconds
        },
        delay_seconds: model.dead_time_seconds.unwrap_or(0.0),
        objective,
        closed_loop_time_seconds: closed_loop,
        units,
        result,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // The described units and the described gains are compared exactly.

    use super::*;

    fn model(
        kind: LowOrderModelKind,
        gain: f64,
        tau: Option<f64>,
        delay: Option<f64>,
    ) -> IdentifiedModel {
        IdentifiedModel {
            kind,
            gain,
            time_constant_seconds: tau,
            second_time_constant_seconds: None,
            dead_time_seconds: delay,
            normalized_error: 0.0,
            normalized_estimation_error: None,
            normalized_validation_error: None,
            estimation_residuals: None,
            validation_residuals: None,
            uncertainty: None,
            stable: true,
            on_search_boundary: false,
        }
    }

    #[test]
    fn a_feasible_simc_description_shows_its_units_inputs_and_result() {
        let fopdt = model(
            LowOrderModelKind::FirstOrderDeadTime,
            1.0,
            Some(1.0),
            Some(1.0),
        );
        let described = describe_method(
            &fopdt,
            TuningMethod::Simc,
            TuningObjective::Balanced,
            ControllerAction::Direct,
            "°C",
            "%",
        );
        assert_eq!(described.status, DescriptionStatus::Feasible);
        assert_eq!(described.title, "SIMC PI (Skogestad 2003)");
        assert_eq!(described.units.plant_gain, "°C/%");
        assert_eq!(described.units.proportional_gain, "%/°C");
        assert_eq!(described.closed_loop_time_seconds, Some(2.0));
        let result = described.result.expect("feasible has a result");
        assert!((result.proportional_gain - 1.0 / 3.0).abs() < 1e-7);
        assert_eq!(result.integral_time_seconds, 1.0);
        assert_eq!(described.gain_name, "K");
    }

    #[test]
    fn an_unavailable_amigo_description_gives_the_reason_and_no_result() {
        let no_delay = model(
            LowOrderModelKind::FirstOrderDeadTime,
            1.0,
            Some(1.0),
            Some(0.0),
        );
        let described = describe_method(
            &no_delay,
            TuningMethod::Amigo,
            TuningObjective::Balanced,
            ControllerAction::Direct,
            "%",
            "°C",
        );
        assert_eq!(described.status, DescriptionStatus::Unavailable);
        assert_eq!(
            described.reason.as_deref(),
            Some("AMIGO requires resolved positive dead time.")
        );
        assert!(described.result.is_none());
    }

    #[test]
    fn an_integrating_description_names_the_rate_gain_and_its_units() {
        let tank = model(LowOrderModelKind::Integrating, 0.05, None, Some(1.0));
        let described = describe_method(
            &tank,
            TuningMethod::Simc,
            TuningObjective::Fast,
            ControllerAction::Direct,
            "level-%",
            "valve-%",
        );
        assert_eq!(described.gain_name, "Kprime");
        assert_eq!(described.units.plant_gain, "level-%/(valve-%·s)");
        assert_eq!(described.time_constant_seconds, None);
        assert_eq!(
            described.result.expect("feasible").integral_time_seconds,
            8.0
        );
    }
}

#[cfg(test)]
#[path = "method_description_identify_tune_tests.rs"]
mod identify_tune_tests;
