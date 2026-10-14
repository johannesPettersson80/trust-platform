//! The pole placement of Automatic (specification 35, B4.4 and C1; numerical annex N5.12(c)): when no formula response meets
//! the requirements, the closed-loop poles are placed with damping `ζ = 0.8` and an envelope decay that starts at
//! `max(4, ln(|Δr|/b))`, and each following placement is 25 % faster, at most eight of them, all below the sample-rate limit.
//!
//! This module holds the sequence and its rules. The coefficient matching that turns a placement into gains is a numerical
//! formula of `trust_control_math::placement`: [`ModelPlacement`] reads an identified model, calls that formula, and supplies
//! the gains through [`PolePlacement`]. The sequence itself only sees the trait.

#![allow(clippy::module_name_repetitions)] // `PolePlacement` and `PlacementRefusal` name the module's subject.

use trust_control_math::identify::LowOrderModelKind;
use trust_control_math::model::TransferFunction;
use trust_control_math::placement::{
    place_poles, Family, PlacementPlant, PlacementRefusal as MathRefusal, PlacementRequest,
};

use crate::identification::plant::IdentifiedModel;
use crate::identification::refusal::Refusal;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::shortfall::DesignContext;
use crate::tuning::structure::{ParallelGains, Structure};
use crate::tuning::texts;

/// The damping of the placed poles (N5.12(c): `ζ = 0.8`).
pub const DAMPING: f64 = 0.8;
/// The most placements tried (N5.12(c)).
pub const MAXIMUM_PLACEMENTS: usize = 8;
/// Each placement is this many times faster than the previous one (N5.12(c): `δk+1 = 1.25·δk`).
pub const ENVELOPE_STEP: f64 = 1.25;
/// The envelope of the 2 % settling band, the slowest first placement (N5.12(c)).
pub const TWO_PERCENT_ENVELOPE: f64 = 4.0;
/// The sample-rate limit: each placement keeps `ω·h` below this (N5.12(c): `ωh < 0.2`).
pub const SAMPLE_RATE_LIMIT: f64 = 0.2;

/// Why a placement gives no gains (N5.12(c), C1).
#[derive(Debug, Clone, PartialEq)]
pub enum PlacementRefusal {
    /// The structure has no pole-placement form for this model: the sequence stops (C1).
    NoPlacementForm,
    /// The placement is refused for this envelope: the sequence tries the next one.
    Refused(Refusal),
}

/// The coefficient matching of N5.12(c): the gains of a family whose closed-loop poles have damping [`DAMPING`] and the
/// decay rate `envelope / Tset`. [`ModelPlacement`] supplies it for an identified model; the sequence only uses it.
pub trait PolePlacement {
    /// The gains of `family` that place the poles for the envelope, or the reason there are none.
    ///
    /// # Errors
    /// Returns [`PlacementRefusal::NoPlacementForm`] when the family has no placement for the model, and
    /// [`PlacementRefusal::Refused`] when this envelope gives no usable gains.
    fn gains(
        &self,
        family: Structure,
        envelope: f64,
        settling_time_seconds: f64,
    ) -> Result<ParallelGains, PlacementRefusal>;
}

/// The envelope of the first placement: `max(4, ln(|Δr| / b))` (N5.12(c)). It reaches the band `b` from the change `Δr` by
/// the settling time, and is never slower than the 2 % envelope.
#[must_use]
pub fn first_envelope(reference_change: f64, settling_band: f64) -> f64 {
    TWO_PERCENT_ENVELOPE.max((reference_change.abs() / settling_band).ln())
}

/// The envelopes the placements try, in order: the first, and each 25 % faster, at most [`MAXIMUM_PLACEMENTS`] of them.
pub fn envelopes(first: f64) -> impl Iterator<Item = f64> {
    (0..MAXIMUM_PLACEMENTS)
        .map(move |index| first * ENVELOPE_STEP.powi(i32::try_from(index).unwrap_or(i32::MAX)))
}

/// Whether a placement stays below the sample-rate limit (N5.12(c)): `ω·h < 0.2`, where `ω = envelope / (ζ·Tset)` and `h` is
/// the task period in seconds. A placement at or above the limit is refused, and the sequence stops there.
#[must_use]
pub fn within_sample_rate(envelope: f64, settling_time_seconds: f64, task_period_ms: f64) -> bool {
    let omega = envelope / (DAMPING * settling_time_seconds);
    omega * task_period_ms / 1000.0 < SAMPLE_RATE_LIMIT
}

/// Whether a placement's gains are all non-negative in the magnitude frame (N5.12(c), N6.1): each gain multiplied by the
/// action's sign is not negative, so a direct loop's gains are at least zero and a reverse loop's are at most zero.
#[must_use]
pub fn in_magnitude_frame(gains: ParallelGains, action_sign: f64) -> bool {
    [
        gains.proportional_gain,
        gains.integral_gain,
        gains.derivative_gain,
    ]
    .iter()
    .all(|gain| gain * action_sign >= 0.0)
}

/// The pole placement of one identified model (N5.12(c), `poleMatchedPid` of `pidTaskDesign.ts`): the model's transfer
/// function `b/(a2·s² + a1·s + a0)` with its dead time, read once, the steady-error stiffness that P and PD must meet, and
/// the task period. A model without a placement form (a dead time, zeros, a static gain or an order above two) gives every
/// placement [`PlacementRefusal::NoPlacementForm`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelPlacement {
    plant: Option<PlacementPlant>,
    minimum_stiffness: f64,
    sample_time_seconds: f64,
}

impl ModelPlacement {
    /// The placement of an identified model for the requirements, the operating input and the task period of the loop.
    #[must_use]
    pub fn for_identified(
        model: &IdentifiedModel,
        requirements: &TuningRequirements,
        context: &DesignContext,
    ) -> Self {
        let delay = model.dead_time_seconds.unwrap_or(0.0);
        let transfer = identified_transfer(model);
        let plant = transfer
            .as_ref()
            .and_then(|transfer| PlacementPlant::new(transfer, delay).ok());
        let minimum_stiffness = transfer.map_or(0.0, |transfer| {
            steady_error_stiffness(&transfer, requirements, context.operating_input)
        });
        Self {
            plant,
            minimum_stiffness,
            sample_time_seconds: context.task_period_ms / 1000.0,
        }
    }
}

impl PolePlacement for ModelPlacement {
    fn gains(
        &self,
        family: Structure,
        envelope: f64,
        settling_time_seconds: f64,
    ) -> Result<ParallelGains, PlacementRefusal> {
        let plant = self.plant.ok_or(PlacementRefusal::NoPlacementForm)?;
        let request = PlacementRequest {
            decay_rate: envelope / settling_time_seconds,
            damping: DAMPING,
            // Only P and PD take the steady-error stiffness; PI and PID keep their matched poles (N5.12(c)).
            minimum_stiffness: match family {
                Structure::P | Structure::PD => self.minimum_stiffness,
                Structure::PI | Structure::PID => 0.0,
            },
            sample_time_seconds: self.sample_time_seconds,
            maximum_omega_sample_product: SAMPLE_RATE_LIMIT,
        };
        let needed_decay = request.decay_rate;
        place_poles(plant, family_of(family), request)
            .map(|gains| ParallelGains {
                proportional_gain: gains.proportional_gain,
                integral_gain: gains.integral_gain,
                derivative_gain: gains.derivative_gain,
            })
            .map_err(|refusal| {
                refusal_of(refusal, family, plant, needed_decay, settling_time_seconds)
            })
    }
}

/// The transfer function of an identified form, with its gain `K` (or the rate gain `K′`) in the numerator: a first-order
/// `K/(τ·s + 1)`, a second-order `K/(τ1·τ2·s² + (τ1 + τ2)·s + 1)`, an integrating `K′/s`, or an integrating form with a lag
/// `K′/(τ·s² + s)`. A static gain has no placement form.
fn identified_transfer(model: &IdentifiedModel) -> Option<TransferFunction> {
    let denominator = match model.kind {
        LowOrderModelKind::FirstOrder | LowOrderModelKind::FirstOrderDeadTime => {
            vec![model.time_constant_seconds?, 1.0]
        }
        LowOrderModelKind::SecondOrder | LowOrderModelKind::SecondOrderDeadTime => {
            let first = model.time_constant_seconds?;
            let second = model.second_time_constant_seconds?;
            vec![first * second, first + second, 1.0]
        }
        LowOrderModelKind::Integrating => match model.time_constant_seconds {
            Some(lag) => vec![lag, 1.0, 0.0],
            None => vec![1.0, 0.0],
        },
        LowOrderModelKind::StaticGain => return None,
    };
    TransferFunction::new(vec![model.gain], denominator).ok()
}

/// The smallest proportional stiffness `a0 + b·Kp` that the steady-error allowance requires, for P and PD (N5.9, N5.12(c)):
/// `N/allowance`, with `N = max(|a0·ΔR + b·u0|, |b·(u0 − d)|)` the numerator of the steady-error bound, `ΔR` the setpoint
/// change, `u0` the operating input, `d` the constant input disturbance, and `allowance` the error limit less the settling
/// band. A negative ratio adds no requirement; a nonfinite one refuses the placement.
fn steady_error_stiffness(
    transfer: &TransferFunction,
    requirements: &TuningRequirements,
    operating_input: f64,
) -> f64 {
    let (Some(&gain), Some(&constant)) = (transfer.numerator.first(), transfer.denominator.last())
    else {
        return 0.0;
    };
    let disturbance = requirements.constant_input_disturbance.unwrap_or(0.0);
    let numerator = (constant * requirements.reference_change + gain * operating_input)
        .abs()
        .max((gain * (operating_input - disturbance)).abs());
    let allowance = requirements.maximum_error - requirements.settling_band;
    let ratio = numerator / allowance;
    if ratio < 0.0 {
        0.0
    } else {
        ratio
    }
}

/// The family of the placement library for a structure.
fn family_of(structure: Structure) -> Family {
    match structure {
        Structure::P => Family::P,
        Structure::PI => Family::PI,
        Structure::PD => Family::PD,
        Structure::PID => Family::PID,
    }
}

/// The sentence of a second-order plant whose own decay is below the placement's (`pidTaskDesign.ts`): the decay the plant
/// keeps under proportional action, `a1/(2·a2)` clamped at zero, against the decay the settling time needs. The library
/// refuses this only for a second-order plant, so the first-order arm keeps no decay to report.
fn decay_sentence(plant: PlacementPlant, needed_decay: f64, settling_seconds: f64) -> String {
    let kept = match plant {
        PlacementPlant::SecondOrder {
            leading, linear, ..
        } => (linear / (2.0 * leading)).max(0.0),
        PlacementPlant::FirstOrder { .. } => 0.0,
    };
    texts::proportional_decay_too_slow(kept, needed_decay, settling_seconds)
}

/// The sequence's two kinds of refusal (`automaticPidResponse.ts`): a plant, a sample period or a request with no
/// placement form stops the sequence; every other refusal lets the next placement try (N5.12(c)).
fn refusal_of(
    refusal: MathRefusal,
    structure: Structure,
    plant: PlacementPlant,
    needed_decay: f64,
    settling_seconds: f64,
) -> PlacementRefusal {
    let continues = |sentence: &str| PlacementRefusal::Refused(Refusal::new(sentence));
    match refusal {
        MathRefusal::Delay
        | MathRefusal::Zeros
        | MathRefusal::Order
        | MathRefusal::ZeroGain
        | MathRefusal::LeadingCoefficient
        | MathRefusal::SampleRate => PlacementRefusal::NoPlacementForm,
        MathRefusal::InvalidInput | MathRefusal::NonfiniteGain => {
            continues(texts::PLACEMENT_NONFINITE)
        }
        MathRefusal::DerivativeOnFirstOrder => continues(if structure == Structure::PID {
            texts::PID_NOT_JUSTIFIED
        } else {
            texts::PID_NOT_CONFIGURED
        }),
        MathRefusal::ProportionalDecayTooSlow => {
            continues(&decay_sentence(plant, needed_decay, settling_seconds))
        }
        MathRefusal::IntegralWithoutDamping => continues(texts::PI_NEEDS_DAMPING),
        MathRefusal::IntegralTooSlow => continues(texts::PI_FIXED_DAMPING),
        MathRefusal::NotStable => continues(texts::PLACEMENT_NOT_STABLE),
        MathRefusal::NegativeGain => continues(texts::PLACEMENT_NEGATIVE_GAIN),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // The envelope sequence is checked against exact products.

    use super::*;
    use crate::tuning::design_to_requirements::pole_matched;
    use crate::tuning::shortfall::{GainLimits, OutputLimits, ResponsePredictor, TimePoint, Trial};
    use crate::tuning::typed_pid::TypedDesign;
    use trust_control_math::identify::gain::ControllerAction;

    #[test]
    fn the_first_envelope_is_the_two_percent_envelope_for_a_small_change() {
        // ln(1/0.2) = 1.6 < 4: the 2 % envelope is the slowest first placement.
        assert_eq!(first_envelope(1.0, 0.2), 4.0);
        // ln(100/0.5) = ln(200) = 5.3 > 4: the change needs a faster first placement.
        assert!((first_envelope(100.0, 0.5) - 200.0_f64.ln()).abs() < 1e-12);
    }

    #[test]
    fn each_placement_is_a_quarter_faster_and_there_are_eight_of_them() {
        let all: Vec<f64> = envelopes(4.0).collect();
        assert_eq!(all.len(), 8);
        assert_eq!(all[0], 4.0);
        assert_eq!(all[1], 5.0);
        assert_eq!(all[2], 6.25);
    }

    #[test]
    fn the_sample_rate_limit_is_strict() {
        // ω = 4/(0.8·10) = 0.5 rad/s; at 100 ms, ω·h = 0.05 < 0.2; at 500 ms, ω·h = 0.25 ≥ 0.2.
        assert!(within_sample_rate(4.0, 10.0, 100.0));
        assert!(!within_sample_rate(4.0, 10.0, 500.0));
    }

    #[test]
    fn a_direct_loop_needs_non_negative_gains_and_a_reverse_loop_non_positive_ones() {
        let direct = ParallelGains {
            proportional_gain: 1.0,
            integral_gain: 0.1,
            derivative_gain: 0.0,
        };
        assert!(in_magnitude_frame(direct, 1.0));
        assert!(!in_magnitude_frame(direct, -1.0));
        let reverse = ParallelGains {
            proportional_gain: -1.0,
            integral_gain: -0.1,
            derivative_gain: 0.0,
        };
        assert!(in_magnitude_frame(reverse, -1.0));
        assert!(!in_magnitude_frame(reverse, 1.0));
    }

    /// A predictor that predicts nothing: the placement tests read the gains only.
    struct NoPrediction;

    impl ResponsePredictor for NoPrediction {
        fn predict(
            &self,
            _design: &TypedDesign,
            _trial: Trial,
            _requirements: &TuningRequirements,
        ) -> Option<Vec<TimePoint>> {
            None
        }
    }

    fn identified(
        kind: LowOrderModelKind,
        gain: f64,
        first: f64,
        second: Option<f64>,
    ) -> IdentifiedModel {
        IdentifiedModel {
            kind,
            gain,
            time_constant_seconds: Some(first),
            second_time_constant_seconds: second,
            dead_time_seconds: None,
            normalized_error: Some(0.0),
            normalized_estimation_error: None,
            normalized_validation_error: None,
            estimation_residuals: None,
            validation_residuals: None,
            uncertainty: None,
            stable: true,
            on_search_boundary: false,
        }
    }

    /// The requirements of the placement tests: a 10-unit change, a 5-unit error limit, a 1-unit band and a 1 s deadline.
    fn requirements(maximum_error: f64) -> TuningRequirements {
        TuningRequirements {
            reference_change: 10.0,
            maximum_error,
            settling_band: 1.0,
            settling_time_seconds: 1.0,
            duration_seconds: 5.0,
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

    /// The loop of the placement tests: a direct action, a 10 ms task and the operating input `u0 = 0`.
    fn with_context<T>(run: impl FnOnce(&DesignContext) -> T) -> T {
        let predictor = NoPrediction;
        let context = DesignContext {
            action: ControllerAction::Direct,
            task_period_ms: 10.0,
            operating_input: 0.0,
            operating_output: 0.0,
            output_limits: OutputLimits {
                minimum: -100.0,
                maximum: 100.0,
            },
            gain_limits: GainLimits::default(),
            process_unit: "level",
            manipulated_unit: "valve",
            predictor: &predictor,
        };
        run(&context)
    }

    #[test]
    fn the_automatic_placement_of_a_first_order_model_gives_the_layer_one_gains() {
        // G = 2/(s + 1) with a 4 s deadline: the first envelope max(4, ln(10/1)) = 4 gives σ = 4/4 = 1, and the PI matching of trust_control_math
        // gives Kp = 0.5 and Ki = 0.78125 (the reference values of the placement library).
        let model = identified(LowOrderModelKind::FirstOrder, 2.0, 1.0, None);
        let design = with_context(|context| {
            let placement = ModelPlacement::for_identified(&model, &requirements(5.0), context);
            let deadline = TuningRequirements {
                settling_time_seconds: 4.0,
                ..requirements(5.0)
            };
            pole_matched(&placement, Structure::PI, &deadline).expect("a PI placement")
        });
        assert_eq!(design.structure, Structure::PI);
        assert!((design.gains.proportional_gain - 0.5).abs() <= 1e-12);
        assert!((design.gains.integral_gain - 0.781_25).abs() <= 1e-12);
        assert_eq!(design.gains.derivative_gain, 0.0);
    }

    #[test]
    fn a_proportional_placement_meets_the_steady_error_allowance_and_no_more() {
        // G = 2/(s + 1), σ = 3. The steady-error numerator is |ΔR| = 10 with u0 = 0, so N/allowance is 10/4 = 2.5 when
        // the error limit is 5 (the placement stays at a·σ = 3, Kp = 1), and 10/1 = 10 when it is 2 (Kp = (10 − 1)/2 = 4.5).
        let model = identified(LowOrderModelKind::FirstOrder, 2.0, 1.0, None);
        let loose = with_context(|context| {
            ModelPlacement::for_identified(&model, &requirements(5.0), context)
                .gains(Structure::P, 3.0, 1.0)
                .expect("a P placement")
        });
        assert!((loose.proportional_gain - 1.0).abs() <= 1e-12);
        let tight = with_context(|context| {
            ModelPlacement::for_identified(&model, &requirements(2.0), context)
                .gains(Structure::P, 3.0, 1.0)
                .expect("a P placement")
        });
        assert!((tight.proportional_gain - 4.5).abs() <= 1e-12);
    }

    #[test]
    fn a_second_order_pid_placement_uses_the_identified_pair_and_its_third_pole() {
        // K = 0.5, τ1 = 1 and τ2 = 0.5 is G(s) = 1/(s² + 3s + 2). With σ = 1 and ζ = 0.8 the placement gives the reference
        // gains of the placement library: Kp = 7.0625, Ki = 5.859375 and Kd = 2.75.
        let model = identified(LowOrderModelKind::SecondOrder, 0.5, 1.0, Some(0.5));
        let gains = with_context(|context| {
            ModelPlacement::for_identified(&model, &requirements(5.0), context)
                .gains(Structure::PID, 1.0, 1.0)
                .expect("a PID placement")
        });
        assert!((gains.proportional_gain - 7.0625).abs() <= 1e-12);
        assert!((gains.integral_gain - 5.859_375).abs() <= 1e-12);
        assert!((gains.derivative_gain - 2.75).abs() <= 1e-12);
    }

    #[test]
    fn a_model_without_a_placement_form_stops_the_sequence() {
        // A dead time has no pole placement (N5.12(c)); a static gain has no transfer function with poles.
        let mut delayed = identified(LowOrderModelKind::FirstOrderDeadTime, 2.0, 1.0, None);
        delayed.dead_time_seconds = Some(0.5);
        let static_gain = identified(LowOrderModelKind::StaticGain, 2.0, 1.0, None);
        for model in [delayed, static_gain] {
            let refusal = with_context(|context| {
                let placement = ModelPlacement::for_identified(&model, &requirements(5.0), context);
                placement.gains(Structure::PI, 4.0, 1.0)
            });
            assert_eq!(refusal, Err(PlacementRefusal::NoPlacementForm));
        }
    }

    #[test]
    fn a_derivative_term_on_a_first_order_model_is_refused_with_its_sentence() {
        let model = identified(LowOrderModelKind::FirstOrder, 2.0, 1.0, None);
        let refusal = with_context(|context| {
            let placement = ModelPlacement::for_identified(&model, &requirements(5.0), context);
            placement.gains(Structure::PD, 4.0, 1.0)
        });
        let Err(PlacementRefusal::Refused(sentence)) = refusal else {
            panic!("a PD placement on a first-order model is refused");
        };
        assert_eq!(sentence.0, texts::PID_NOT_CONFIGURED);
    }
}
