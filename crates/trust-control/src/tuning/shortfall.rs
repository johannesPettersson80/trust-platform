//! The shortfall of a candidate's predicted response (specification 35, B4.4 and B4.5; numerical annex N5.12 and N8.1). The
//! rules of Automatic take a candidate's response through [`ResponsePredictor`] and say what the candidate misses, in plain
//! words. The response itself is computed elsewhere: the prediction of layer 2b implements the trait for the selected model.
//! Nothing here simulates the loop.

use trust_control_math::identify::gain::ControllerAction;

use crate::experiment::number_text::js_to_precision;
use crate::tuning::proportional::GainBounds;
use crate::tuning::requirements::{response_deadline_seconds, TuningRequirements};
use crate::tuning::structure::{is_exactly_zero, Structure};
use crate::tuning::texts;
use crate::tuning::typed_pid::TypedDesign;

/// The tolerance of the deadline comparison in seconds (`responseDeadlineSeconds` of the TypeScript: `− 1e-9`).
const DEADLINE_TOLERANCE_SECONDS: f64 = 1e-9;
/// The sentence that the steady-error and the settling misses end with (B4.4).
const SLOW_SENTENCE: &str = "Allow a longer settling time or a larger error.";

/// One point of a predicted response: the process value at a time from the start of the trial, in seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimePoint {
    /// The time from the start of the trial, in seconds.
    pub time_seconds: f64,
    /// The predicted process value of the candidate at that time, in the process variable's unit.
    pub process_value: f64,
}

/// The trial that a prediction covers (N5.12, B5.3): the setpoint change, or the constant input disturbance with the
/// setpoint unchanged from the operating point. The predictor reads the size of each from the requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trial {
    /// The setpoint change `ΔR`, with no load.
    SetpointChange,
    /// The constant input disturbance `d`, with the setpoint unchanged.
    InputDisturbance,
}

/// The predicted response of a candidate on the selected model (B4.5). Layer 2b implements this for the selected model, and
/// the rules of Automatic take a response through it. `None` when no prediction is available; the candidate then goes to
/// Validate unpredicted (N5.12: "Without a model prediction, the declared choice is used and Validate decides").
pub trait ResponsePredictor {
    /// The predicted process value of the candidate for the trial, from the start of the trial to the end of the test, in
    /// time order.
    fn predict(
        &self,
        design: &TypedDesign,
        trial: Trial,
        requirements: &TuningRequirements,
    ) -> Option<Vec<TimePoint>>;
}

/// The bounds of one gain with the unit its parameter declares.
#[derive(Debug, Clone, PartialEq)]
pub struct GainLimit {
    /// The bounds of the parameter (N6.4: applied to the magnitude).
    pub bounds: GainBounds,
    /// The parameter's unit, as the sentence prints it.
    pub unit: String,
}

/// The bounds of the gains a loop declares (`parameterBinding`): each is present only when the loop binds that term.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GainLimits {
    /// The proportional gain's bounds.
    pub proportional: Option<GainLimit>,
    /// The integral gain's bounds.
    pub integral: Option<GainLimit>,
    /// The derivative gain's bounds.
    pub derivative: Option<GainLimit>,
}

/// The output limits of the loop (`limits.outputMinimum` and `limits.outputMaximum`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputLimits {
    /// The smallest output.
    pub minimum: f64,
    /// The largest output.
    pub maximum: f64,
}

/// What Automatic needs to know about the loop and the model to design and to say what a candidate misses (B4.4): the
/// controller action, the task period, the operating point, the limits, the units and the predictor.
pub struct DesignContext<'a> {
    /// The controller action (N6.1).
    pub action: ControllerAction,
    /// The task period in milliseconds (`taskPeriodMs`).
    pub task_period_ms: f64,
    /// The operating point's input `u0`.
    pub operating_input: f64,
    /// The operating point's output.
    pub operating_output: f64,
    /// The output limits of the loop.
    pub output_limits: OutputLimits,
    /// The bounds of the loop's gains.
    pub gain_limits: GainLimits,
    /// The process variable's unit, as the sentences print it.
    pub process_unit: &'a str,
    /// The manipulated variable's unit, as the sentences print it.
    pub manipulated_unit: &'a str,
    /// The prediction of the candidates on the selected model.
    pub predictor: &'a dyn ResponsePredictor,
}

/// The result of checking a candidate against the requirements on its prediction.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The predicted response meets every requirement it covers.
    Meets,
    /// No prediction is available for the candidate.
    Unpredicted,
    /// The predicted response misses, in the sentence the engineer reads.
    Misses(String),
}

/// Whether the requirements declare a constant input disturbance that is a load: a finite, nonzero value.
#[must_use]
pub fn has_load(requirements: &TuningRequirements) -> bool {
    requirements
        .constant_input_disturbance
        .is_some_and(|load| load.is_finite() && !is_exactly_zero(load))
}

/// The predicted outcome of a candidate for the setpoint change and, when the requirements declare an input disturbance,
/// for the load trial (`predictedShortfall`, N5.12(a) and (b)). A gain outside its declared bounds is a miss first.
#[must_use]
pub fn predicted_outcome(
    design: &TypedDesign,
    requirements: &TuningRequirements,
    context: &DesignContext,
) -> Outcome {
    if let Some(range) = gain_range_shortfall(design, context) {
        let sentence = if has_integral_action(design) {
            format!("{range}. {SLOW_SENTENCE}")
        } else {
            format!(
                "{range}, to keep its steady-state error within {} {}. Choose a structure with integral action or allow a larger error.",
                significant_text(requirements.maximum_error - requirements.settling_band, 2),
                context.process_unit
            )
        };
        return Outcome::Misses(sentence);
    }
    let Some(points) = context
        .predictor
        .predict(design, Trial::SetpointChange, requirements)
    else {
        return Outcome::Unpredicted;
    };
    let deadline =
        response_deadline_seconds(requirements.settling_time_seconds, context.task_period_ms);
    let remaining = after_deadline(&points, deadline);
    if remaining.is_empty() {
        return Outcome::Unpredicted;
    }
    let setpoint_miss = shortfall(
        &remaining,
        &Miss {
            target: context.operating_output + requirements.reference_change,
            deadline,
            design,
            requirements,
            unit: context.process_unit,
            under: None,
        },
    );
    if let Some(sentence) = setpoint_miss {
        return Outcome::Misses(sentence);
    }
    match requirements.constant_input_disturbance {
        Some(load) if has_load(requirements) => load_outcome(design, requirements, context, load),
        _ => Outcome::Meets,
    }
}

/// The predicted outcome of the load trial: the setpoint unchanged, the declared input disturbance acting from the start,
/// and the output from the deadline to the end within the allowed error and the settling band (N5.12(b)).
fn load_outcome(
    design: &TypedDesign,
    requirements: &TuningRequirements,
    context: &DesignContext,
    load: f64,
) -> Outcome {
    let Some(points) = context
        .predictor
        .predict(design, Trial::InputDisturbance, requirements)
    else {
        return Outcome::Unpredicted;
    };
    let deadline =
        response_deadline_seconds(requirements.settling_time_seconds, context.task_period_ms);
    let remaining = after_deadline(&points, deadline);
    if remaining.is_empty() {
        return Outcome::Unpredicted;
    }
    let under = format!(
        "under the {} {} input disturbance",
        texts::engineering(load, None),
        context.manipulated_unit
    );
    let miss = Miss {
        target: context.operating_output,
        deadline,
        design,
        requirements,
        unit: context.process_unit,
        under: Some(&under),
    };
    match shortfall(&remaining, &miss) {
        Some(sentence) => Outcome::Misses(sentence),
        None => Outcome::Meets,
    }
}

/// The values of a response from the deadline to the end of the test (the `timeResponse` filter of the TypeScript).
fn after_deadline(points: &[TimePoint], deadline: f64) -> Vec<f64> {
    points
        .iter()
        .filter(|point| point.time_seconds >= deadline - DEADLINE_TOLERANCE_SECONDS)
        .map(|point| point.process_value)
        .collect()
}

/// What a response misses: its target, the deadline, the candidate and its requirements, the unit of the process variable,
/// and the load's phrase when the miss is of the load trial.
struct Miss<'a> {
    target: f64,
    deadline: f64,
    design: &'a TypedDesign,
    requirements: &'a TuningRequirements,
    unit: &'a str,
    under: Option<&'a str>,
}

/// The sentence of a predicted response that misses (`predictedShortfall` and `loadShortfall`): the steady error of a
/// structure without integral action, the error from the target after the deadline, or the movement after it. `None` when
/// the response stays within the allowed error and the settling band.
fn shortfall(remaining: &[f64], miss: &Miss) -> Option<String> {
    let last = remaining.last().copied()?;
    let error = remaining
        .iter()
        .map(|value| (value - miss.target).abs())
        .fold(0.0_f64, f64::max);
    let movement = remaining
        .iter()
        .map(|value| (value - last).abs())
        .fold(0.0_f64, f64::max);
    let allowed = miss.requirements.maximum_error;
    let band = miss.requirements.settling_band;
    let after = format!("after {} s", texts::engineering(miss.deadline, None));
    let under = miss
        .under
        .map(|phrase| format!(" {phrase}"))
        .unwrap_or_default();
    let value = |number: f64| format!("{} {}", significant_text(number, 2), miss.unit);
    if !has_integral_action(miss.design) && error > allowed && movement <= band {
        let steady = (last - miss.target).abs();
        return Some(format!(
            "settles{under} with a steady-state error of {}, where {} is allowed. Choose a structure with integral action or allow a larger error.",
            value(steady),
            value(allowed)
        ));
    }
    if error > allowed {
        return Some(format!(
            "is{under} up to {} from the target {after}, where {} is allowed. {SLOW_SENTENCE}",
            value(error),
            value(allowed)
        ));
    }
    if movement > band {
        return Some(format!(
            "still moves{under} {} {after}, more than the ±{} settling band. {SLOW_SENTENCE}",
            value(movement),
            value(band)
        ));
    }
    None
}

/// Whether a structure has integral action, so that a steady error is removed (`PI` and `PID`).
fn has_integral_action(design: &TypedDesign) -> bool {
    matches!(design.structure, Structure::PI | Structure::PID)
}

/// The sentence of a gain whose magnitude leaves its declared range, or `None` when every gain the loop binds is within its
/// range (`gainRangeShortfall`). The bounds apply to magnitudes (N6.4).
fn gain_range_shortfall(design: &TypedDesign, context: &DesignContext) -> Option<String> {
    let gains = [
        (
            "Kp",
            context.gain_limits.proportional.as_ref(),
            design.gains.proportional_gain,
        ),
        (
            "Ki",
            context.gain_limits.integral.as_ref(),
            design.gains.integral_gain,
        ),
        (
            "Kd",
            context.gain_limits.derivative.as_ref(),
            design.gains.derivative_gain,
        ),
    ];
    for (label, limit, value) in gains {
        let Some(limit) = limit else { continue };
        if !value.is_finite() {
            continue;
        }
        let (lowest, highest) = limit.bounds.magnitude_range();
        let magnitude = value.abs();
        if magnitude > highest {
            return Some(texts::gain_above_maximum(
                label,
                magnitude,
                highest,
                &limit.unit,
            ));
        }
        if magnitude < lowest {
            return Some(texts::gain_below_minimum(
                label,
                magnitude,
                lowest,
                &limit.unit,
            ));
        }
    }
    None
}

/// A number rounded to `digits` significant digits and printed as the sentences print it (`formatEngineeringNumber` of
/// `Number(value.toPrecision(digits))`).
fn significant_text(value: f64, digits: usize) -> String {
    texts::engineering(js_to_precision(value, digits), None)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // The compared outcomes are exact sentences and exact states.

    use super::*;
    use crate::tuning::structure::ParallelGains;

    /// A predictor that answers with fixed responses for the two trials.
    struct Scripted {
        setpoint: Option<Vec<TimePoint>>,
        load: Option<Vec<TimePoint>>,
    }

    impl ResponsePredictor for Scripted {
        fn predict(
            &self,
            _design: &TypedDesign,
            trial: Trial,
            _requirements: &TuningRequirements,
        ) -> Option<Vec<TimePoint>> {
            match trial {
                Trial::SetpointChange => self.setpoint.clone(),
                Trial::InputDisturbance => self.load.clone(),
            }
        }
    }

    fn requirements() -> TuningRequirements {
        TuningRequirements {
            channel_requirements: None,
            reference_change: 2.0,
            maximum_error: 0.4,
            settling_band: 0.1,
            settling_time_seconds: 10.0,
            duration_seconds: 30.0,
            constant_input_disturbance: None,
            measurement_noise_band: None,
            gain_uncertainty_fraction: None,
            transfer_tolerance: None,
            maximum_overshoot_percent: None,
            tuning_method: None,
            tuning_objective: None,
            automatic_criteria: None,
        }
    }

    fn design(structure: Structure, proportional: f64, integral: f64) -> TypedDesign {
        TypedDesign {
            structure,
            gains: ParallelGains {
                proportional_gain: proportional,
                integral_gain: integral,
                derivative_gain: 0.0,
            },
        }
    }

    fn trace(values: &[(f64, f64)]) -> Vec<TimePoint> {
        values
            .iter()
            .map(|&(time_seconds, process_value)| TimePoint {
                time_seconds,
                process_value,
            })
            .collect()
    }

    fn context(predictor: &dyn ResponsePredictor) -> DesignContext<'_> {
        DesignContext {
            action: ControllerAction::Direct,
            task_period_ms: 100.0,
            operating_input: 0.0,
            operating_output: 0.0,
            output_limits: OutputLimits {
                minimum: -10.0,
                maximum: 10.0,
            },
            gain_limits: GainLimits::default(),
            process_unit: "°C",
            manipulated_unit: "%",
            predictor,
        }
    }

    #[test]
    fn a_response_that_settles_in_the_band_meets_the_requirements() {
        let predictor = Scripted {
            setpoint: Some(trace(&[
                (9.9, 1.0),
                (10.0, 2.0),
                (11.0, 2.01),
                (12.0, 1.99),
            ])),
            load: None,
        };
        let outcome = predicted_outcome(
            &design(Structure::PI, 1.0, 0.1),
            &requirements(),
            &context(&predictor),
        );
        assert_eq!(outcome, Outcome::Meets);
    }

    #[test]
    fn a_response_that_keeps_moving_says_how_far_after_the_deadline() {
        let predictor = Scripted {
            setpoint: Some(trace(&[(10.0, 2.0), (11.0, 2.0), (12.0, 2.3)])),
            load: None,
        };
        let outcome = predicted_outcome(
            &design(Structure::PI, 1.0, 0.1),
            &requirements(),
            &context(&predictor),
        );
        let Outcome::Misses(sentence) = outcome else {
            panic!("the response moves after the deadline");
        };
        assert!(
            sentence
                .starts_with("still moves 0.3 °C after 10 s, more than the ±0.1 °C settling band."),
            "{sentence}"
        );
    }

    #[test]
    fn an_error_from_the_target_after_the_deadline_is_named() {
        let predictor = Scripted {
            setpoint: Some(trace(&[(10.0, 1.0), (11.0, 1.0), (12.0, 1.0)])),
            load: None,
        };
        let outcome = predicted_outcome(
            &design(Structure::PI, 1.0, 0.1),
            &requirements(),
            &context(&predictor),
        );
        let Outcome::Misses(sentence) = outcome else {
            panic!("the response is a unit below the target");
        };
        assert!(
            sentence
                .starts_with("is up to 1 °C from the target after 10 s, where 0.4 °C is allowed."),
            "{sentence}"
        );
    }

    #[test]
    fn a_proportional_offset_asks_for_integral_action() {
        let predictor = Scripted {
            setpoint: Some(trace(&[(10.0, 1.5), (11.0, 1.5), (12.0, 1.5)])),
            load: None,
        };
        let outcome = predicted_outcome(
            &design(Structure::P, 1.0, 0.0),
            &requirements(),
            &context(&predictor),
        );
        let Outcome::Misses(sentence) = outcome else {
            panic!("a steady offset of 0.5 is above 0.4");
        };
        assert!(
            sentence
                .contains("settles with a steady-state error of 0.5 °C, where 0.4 °C is allowed."),
            "{sentence}"
        );
        assert!(
            sentence.ends_with("Choose a structure with integral action or allow a larger error."),
            "{sentence}"
        );
    }

    #[test]
    fn no_prediction_is_unpredicted_and_no_response_after_the_deadline_is_too() {
        let none = Scripted {
            setpoint: None,
            load: None,
        };
        assert_eq!(
            predicted_outcome(
                &design(Structure::PI, 1.0, 0.1),
                &requirements(),
                &context(&none)
            ),
            Outcome::Unpredicted
        );
        let early = Scripted {
            setpoint: Some(trace(&[(0.0, 2.0), (1.0, 2.0)])),
            load: None,
        };
        assert_eq!(
            predicted_outcome(
                &design(Structure::PI, 1.0, 0.1),
                &requirements(),
                &context(&early)
            ),
            Outcome::Unpredicted
        );
    }

    #[test]
    fn a_gain_above_its_declared_magnitude_is_a_miss_first() {
        let predictor = Scripted {
            setpoint: Some(trace(&[(10.0, 2.0)])),
            load: None,
        };
        let mut limited = context(&predictor);
        limited.gain_limits.proportional = Some(GainLimit {
            bounds: GainBounds {
                minimum: 0.0,
                maximum: 0.5,
            },
            unit: "%/°C".to_string(),
        });
        let outcome = predicted_outcome(&design(Structure::P, 1.0, 0.0), &requirements(), &limited);
        assert_eq!(
            outcome,
            Outcome::Misses(
                "needs Kp of magnitude 1 %/°C, above its declared maximum of 0.5 %/°C, to keep its steady-state error within 0.3 °C. Choose a structure with integral action or allow a larger error.".to_string()
            )
        );
    }

    #[test]
    fn a_load_trial_is_checked_only_when_a_disturbance_is_declared() {
        let predictor = Scripted {
            setpoint: Some(trace(&[(10.0, 2.0), (11.0, 2.0)])),
            load: Some(trace(&[(10.0, 0.5), (11.0, 0.5)])),
        };
        let mut with_load = requirements();
        with_load.constant_input_disturbance = Some(1.0);
        let outcome = predicted_outcome(
            &design(Structure::PI, 1.0, 0.1),
            &with_load,
            &context(&predictor),
        );
        let Outcome::Misses(sentence) = outcome else {
            panic!("the load trial is a half unit away from the operating output");
        };
        assert!(
            sentence.starts_with("is under the 1 % input disturbance up to 0.5 °C from the target"),
            "{sentence}"
        );
        assert_eq!(
            predicted_outcome(
                &design(Structure::PI, 1.0, 0.1),
                &requirements(),
                &context(&predictor)
            ),
            Outcome::Meets
        );
    }
}
