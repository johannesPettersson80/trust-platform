//! The sentences of Tune (specification 35, B4.7, C0 to C2 and C5; numerical annex N5.7 to N5.11, N6.3 and N6.4). A rule
//! decides; these functions say what it decided, in the words the engineer reads. The sign sentence of N6.3 is
//! `identification::texts::gain_sign_contradiction`, so both refusals read alike.

use crate::experiment::number_text::{js_number, js_to_fixed, js_to_precision};
use crate::tuning::objective::TuningMethod;
use crate::tuning::structure::is_exactly_zero;

/// The one short progress message while Tune designs the candidate (Tune page specification, "While tuning runs").
pub const DESIGNING: &str = "Designing the candidate…";

/// `validateModel`: a form that needs a stable model has an unstable one.
pub const STABLE_MODEL_REQUIRED: &str = "A stable model is required for frequency results.";
/// `validateModel`: a zero, infinite or NaN gain.
pub const GAIN_NOT_FINITE_OR_ZERO: &str = "Model gain must be finite and non-zero.";
/// `validateModel`: a time constant that is not finite and positive.
pub const TIME_CONSTANT_NOT_POSITIVE: &str = "Model time constants must be finite and positive.";
/// `validateTuningModel`: a dead time that is not finite and not negative.
pub const DEAD_TIME_NEGATIVE: &str = "Model dead time must be finite and non-negative.";
/// N5.5: AMIGO has no integrating form.
pub const AMIGO_INTEGRATING: &str = "AMIGO is unavailable for integrating models.";
/// N5.3: SIMC for an integrating model needs a resolved positive effective delay.
pub const SIMC_INTEGRATING_DELAY: &str =
    "SIMC integrating PI requires resolved positive effective delay.";
/// N5.5: AMIGO needs a positive dead time.
pub const AMIGO_NEEDS_DELAY: &str = "AMIGO requires resolved positive dead time.";
/// N5.5: AMIGO's domain is `0 < L/T ≤ 1`.
pub const AMIGO_DELAY_DOMAIN: &str =
    "AMIGO delay ratio is outside the qualified delay domain L/T <= 1.";
/// N5.2 and N5.7: a second-order model with complex poles is refused for SIMC, with the advice to choose another method.
pub const HALF_RULE_COMPLEX_POLES: &str =
    "The second-order model has complex poles, so SIMC's half rule does not apply. Choose Automatic or another tuning rule.";
/// N5.2: SIMC's half rule needs a stable second-order model.
pub const HALF_RULE_NOT_STABLE: &str =
    "SIMC's half rule requires a stable second-order model with a nonzero gain.";
/// N5.7: a formula result that is not finite and positive.
pub const PI_NOT_FINITE: &str = "The model does not yield finite positive PI settings.";
/// N5.10: the constrained derivative design needs a stable first-order model with a finite time constant.
pub const DERIVATIVE_NEEDS_STABLE_FIRST_ORDER: &str =
    "The constrained derivative design requires a stable first-order model with a finite time constant.";
/// N5.10: a constrained derivative gain that is not finite and nonzero.
pub const DERIVATIVE_NOT_FINITE: &str =
    "Typed derivative design did not produce a finite nonzero derivative gain.";
/// C1: a typed proportional gain that is not finite.
pub const TYPED_P_NEEDS_FINITE_GAIN: &str = "Typed proportional design requires a finite gain.";
/// N5.8 and C1: a simple proportional design has no transport-delay adapter.
pub const SIMPLE_P_NO_DELAY: &str = "Simple P design has no transport-delay adapter.";
/// C1: a first-order model whose gain or time constant does not allow a simple proportional design.
pub const SIMPLE_P_MODEL: &str = "Simple P design requires finite a1>0, a0>0, and nonzero b.";
/// C1: the requirements of the simple proportional design are not usable.
pub const SIMPLE_P_REQUIREMENTS: &str =
    "Simple P design requires finite nonzero reference, positive error/settling limits, and ordered gain bounds.";
/// C1: no gain within the bounds meets the steady-error and settling requirements.
pub const SIMPLE_P_BOUNDS: &str =
    "Proportional gain bounds cannot satisfy the declared steady-error and settling requirements.";
/// C1: no gain within the bounds meets the prediction requirements.
pub const SIMPLE_P_PREDICTION: &str =
    "Finite proportional gain bounds cannot satisfy the declared prediction requirements.";
/// C1: the gain is outside the bounds it was designed within.
pub const SIMPLE_P_OUTSIDE_BOUNDS: &str =
    "Calculated proportional gain is outside the declared bounds.";
/// N5.8: no gain within the bounds meets the integrating plant's requirements.
pub const INTEGRATING_P_BOUNDS: &str =
    "Proportional gain bounds cannot satisfy the integrating plant error and settling requirements.";
/// N5.8: an integrating P design needs a finite, nonzero rate gain.
pub const INTEGRATING_P_RATE: &str = "Integrating P design requires a finite nonzero rate gain.";
/// N5.8: the integrating P design is not numerically identifiable.
pub const INTEGRATING_P_UNIDENTIFIABLE: &str =
    "Integrating P design is numerically unidentifiable within the declared bounds.";
/// C1: an engineer-entered gain that is not finite.
pub const MANUAL_P_NOT_FINITE: &str = "Manual proportional gain must be finite.";
/// C1: an engineer-entered gain outside its declared bounds.
pub const MANUAL_P_OUTSIDE_BOUNDS: &str =
    "Manual proportional gain is outside the declared bounds.";
/// N5.9: a manual first-order prediction with numerical uncertainty.
pub const MANUAL_P_UNCERTAIN: &str = "Manual proportional prediction has numerical uncertainty.";
/// N5.8: a manual integrating prediction with numerical uncertainty.
pub const MANUAL_INTEGRATING_UNCERTAIN: &str =
    "Manual integrating prediction has numerical uncertainty.";
/// N5.9: a manual first-order gain whose pole is nonnegative has no asymptotic bound.
pub const MANUAL_P_NO_BOUND: &str =
    "No asymptotic steady-error or transient bound exists because the selected closed-loop pole is nonnegative.";
/// N5.8: a manual integrating gain whose pole is nonnegative has no asymptotic bound.
pub const MANUAL_INTEGRATING_NO_BOUND: &str =
    "The selected pole is nonnegative; no asymptotic error bound is established.";
/// C1: a derivative term that the first-order pole matching does not justify.
pub const PID_NOT_JUSTIFIED: &str =
    "A derivative term is not justified by this first-order pole-matching rule; choose PI or the declared native tuning policy.";
/// N5.12(c): a placement whose bandwidth is above the sample-rate limit.
pub const PLACEMENT_BANDWIDTH: &str =
    "The requested bandwidth needs a faster sample period or a longer response time.";
/// N5.12(c): a structure with no pole-placement form for this model.
pub const PLACEMENT_NEEDS_MODEL: &str =
    "Pole matching needs a delay-free first/second-order model without zeros; use a compatible declared native tuning policy for other structures.";
/// N5.12(c): a placement whose settings are not finite.
pub const PLACEMENT_NONFINITE: &str = "Pole-matched settings are nonfinite.";
/// N5.12(c): a placement whose characteristic polynomial is not strictly stable.
pub const PLACEMENT_NOT_STABLE: &str =
    "The coefficient-matched controller does not have a strictly stable continuous characteristic polynomial.";
/// N5.12(c): a second-order plant whose own decay is below the placement's, which proportional action cannot change. The
/// decay the plant keeps and the decay the settling time needs are printed to three significant digits, as the
/// TypeScript sentence of `pidTaskDesign.ts` prints them.
#[must_use]
pub fn proportional_decay_too_slow(decay: f64, needed: f64, settling_seconds: f64) -> String {
    format!(
        "P leaves the error's decay rate at {} 1/s whatever Kp, and settling by {} s needs at least {} 1/s. Compare PD or PID.",
        engineering(js_to_precision(decay, 3), None),
        engineering(settling_seconds, None),
        engineering(js_to_precision(needed, 3), None),
    )
}
/// N5.12(c) and N6.1: a placement whose gain has the sign opposite to the loop's action.
pub const PLACEMENT_NEGATIVE_GAIN: &str =
    "A placed gain has the sign opposite to the loop's action (N6.1), so this structure is not placed.";
/// C1: a PI whose second-order damping cannot be supplied.
pub const PI_NEEDS_DAMPING: &str =
    "PI cannot supply the missing second-order damping; compare PD or PID.";
/// C1: a PI whose fixed damping cannot meet the requested response time.
pub const PI_FIXED_DAMPING: &str =
    "PI's fixed damping cannot meet the requested response time; compare PD/PID.";
/// C1: a first-order plant whose structure has no declared derivative policy.
pub const PID_NOT_CONFIGURED: &str =
    "Choose P/PI or a declared derivative tuning policy for this first-order plant.";
/// B4.7 and C0: a model with explicit zero dynamics has no scalar design.
pub const ZERO_DYNAMICS: &str = "The selected model has explicit zero dynamics (possibly inverse response); this scalar adapter has no qualified zero-cancellation or inverse-response design. Use a declared state-feedback or project-owned adapter.";
/// B4.7 and C0: higher-order dynamics outside the coefficient-matching domain.
pub const HIGHER_ORDER_DYNAMICS: &str = "The selected model has oscillatory or higher-order dynamics outside the shared coefficient-matching domain. Use a declared state-feedback or project-owned adapter, or revise the model structure.";
/// C0: a multivariable loop in the scalar comparison.
pub const MULTIVARIABLE_LOOP: &str = "This is a multivariable loop. Use the declared state-feedback/LQR/LQG adapter or a compatible project-owned decoupled/predictive adapter.";
/// C0: an ON/OFF actuator in the continuous comparison.
pub const ON_OFF_OWNED: &str = "The ON/OFF adapter owns this actuator: use its hysteresis and dwell design instead of continuous P/PI/PD/PID tuning.";
/// B4.4: an automatic comparison with no proportional gain binding.
pub const NO_PROPORTIONAL_BINDING: &str =
    "The declared automatic controller comparison has no compatible proportional gain binding.";
/// C11: a trajectory feedforward the model does not support.
pub const TRAJECTORY_REFUSED: &str = "A trajectory feedforward needs a model n/(a·s² + b·s), an integrator with or without a lag; this model has no trajectory feedforward rule.";
/// C2: a program seed the scalar rules do not design.
pub const UNSUPPORTED_SEED: &str = "Declare a supported typed controller seed.";
/// C2: a proportional seed parameter that the loop does not bind.
pub const SEED_NOT_BOUND: &str = "The proportional seed parameter is not bound.";
/// C5: a declaration of tuning components that does not hold together (`validProjectComponents`).
pub const COMPONENTS_INVALID: &str =
    "Declare valid tuning components: each symbol bound once, with its unit and its listed values.";
/// C5: no component value combination is representable and stable.
pub const NO_REPRESENTABLE_COMPONENT: &str =
    "No representable component candidate satisfies its pinned settings, bounds and filter stability restrictions.";
/// C5: a notch frequency that is not positive or not below the sampled Nyquist limit.
pub const NOTCH_NYQUIST: &str = "Notch frequency must lie below the actual sampled Nyquist limit.";
/// C0: a model-derived setting that names no bound parameter.
pub const MODEL_SETTING_NOT_BOUND: &str =
    "A model-derived setting must name an actual bound ST parameter.";
/// C0: a model timing field whose unit is not seconds.
pub const MODEL_TIMING_UNIT: &str = "Model timing fields require seconds.";
/// C0: a model with no stable first-order time constant in seconds.
pub const MODEL_NO_TIME_CONSTANT: &str =
    "The selected model does not supply a stable first-order time constant in seconds.";
/// B4.2 and C0: a manual or imported record that is not a Control.PID settings object.
pub const SETTINGS_NOT_OBJECT: &str = "Control.PID settings must be a JSON object.";
/// B4.2: a Control.PID record that does not hold exactly the required fields.
pub const SETTINGS_FIELD_SET: &str =
    "Control.PID settings must contain every required field exactly once.";
/// C1: an anti-windup method that is neither of the two the record allows.
pub const SETTINGS_ANTI_WINDUP: &str =
    "Control.PID anti-windup must be BackCalculation or Combined.";
/// C1: a stored record whose output minimum exceeds its maximum.
pub const SETTINGS_OUTPUT_ORDER: &str =
    "Control.PID output minimum must not exceed output maximum.";
/// C1: a stored record whose rate minimum exceeds its maximum.
pub const SETTINGS_RATE_ORDER: &str = "Control.PID rate minimum must not exceed rate maximum.";
/// C1: a tracking time that is not positive.
pub const SETTINGS_TRACKING_TIME: &str = "Control.PID tracking time must be positive.";
/// C1: a candidate whose output minimum is not below its maximum.
pub const CANDIDATE_OUTPUT_ORDER: &str = "Control.PID output minimum must be below output maximum.";
/// C1: a candidate whose rate minimum exceeds its maximum.
pub const CANDIDATE_RATE_ORDER: &str = "Control.PID rate minimum must not exceed rate maximum.";
/// C1: a candidate with a setting that is not finite.
pub const CANDIDATE_NOT_FINITE: &str = "Control.PID settings must be finite.";
/// B4.3: requirements that are not all finite numbers.
pub const TASK_NOT_FINITE: &str = "Enter finite task requirements.";
/// B4.3 and A6.4: an Auto-entry tolerance that is not a finite nonnegative value.
pub const TRANSFER_TOLERANCE: &str =
    "Auto-entry tolerance must be blank for automatic or a finite nonnegative value in output units.";
/// B4.3: a tuning method that is neither SIMC nor AMIGO.
pub const TUNING_METHOD_CHOICE: &str = "Choose a supported tuning method.";
/// B4.3: a tuning objective that is not fast, balanced or robust.
pub const TUNING_OBJECTIVE_CHOICE: &str = "Choose a supported tuning objective.";
/// B4.3: a disturbance, noise or uncertainty bound that is not finite or not in range.
pub const BOUNDS_IN_RANGE: &str =
    "Enter finite disturbance/noise bounds and gain uncertainty between zero and one.";
/// A7: a validation window longer than the runtime supports.
pub const DURATION_TOO_LONG: &str =
    "The model needs a longer validation window than this runtime supports. Review the model and test duration.";
/// B4.3: automatic criteria that do not decode.
pub const CRITERIA_INVALID: &str = "Invalid automatic test criteria.";
/// B4.3: the test design needs the model's operating point.
pub const NEEDS_OPERATING_POINT: &str =
    "The model needs its operating point before automatic tuning setup.";
/// C1: the gain selection is neither Automatic nor Manual.
pub const GAIN_MODE_CHOICE: &str = "Choose Automatic or Manual gain selection.";
/// C1: a manual proportional gain that the engineer did not enter as a finite number.
pub const MANUAL_GAIN_ENTER: &str = "Enter a finite manual proportional gain.";
/// B4.2: pinned settings that belong to another loop or family.
pub const PIN_DIFFERENT_LOOP: &str =
    "Pinned settings belong to a different loop or controller; explicitly clear them before changing the design.";
/// C0 and A11.1: a pinned setting outside its declared bounds — a gain by its magnitude (N6.4), another setting by its
/// signed value — or not representable in its `REAL` parameter, with the setting, its value and its bounds.
#[must_use]
pub fn pin_outside_bounds(
    symbol: &str,
    value: f64,
    bounds: (f64, f64),
    gain: bool,
    representable: bool,
) -> String {
    let value_text = engineering(value, None);
    if !representable {
        return format!(
            "Pinned setting {symbol} = {value_text} cannot be stored exactly in its REAL parameter. Change or clear the pin."
        );
    }
    let (lowest, highest) = (engineering(bounds.0, None), engineering(bounds.1, None));
    if gain {
        format!(
            "Pinned setting {symbol} = {value_text} is outside its bounds: its magnitude must lie between {lowest} and {highest}. Change or clear the pin."
        )
    } else {
        format!(
            "Pinned setting {symbol} = {value_text} is outside its bounds {lowest} to {highest}. Change or clear the pin."
        )
    }
}
/// C0: pinned integral or derivative settings that conflict with the family.
pub const PIN_FAMILY_CONFLICT: &str =
    "Pinned integral/derivative settings conflict with the accepted controller family.";
/// C0: a pinned value for a parameter the selected adapter does not hold.
pub const PIN_OTHER_PARAMETER: &str =
    "Pinned settings contain a parameter outside the selected P adapter.";
/// C1: a manual record that lacks a required setting.
pub const REQUIRED_SETTING_MISSING: &str = "Enter every required controller setting before tuning.";
/// B4.2: pinned settings that were set for another loop revision (`tune.ts`).
pub const PINS_LOOP_REVISION: &str = "Pinned settings do not match the current loop revision.";
/// B4.2: a family that the pinned settings do not hold, asked for by the request (`tune.ts`).
pub const PINS_FAMILY: &str = "Clear pinned settings before changing controller family.";
/// B4.2 and C0: a back-calculation tracking gain that is not finite and positive.
pub const TRACKING_GAIN_POSITIVE: &str =
    "The back-calculation tracking gain must be finite and positive.";
/// B4.2: the manual method without the settings it takes.
pub const MANUAL_NEEDS_SETTINGS: &str = "Enter the controller settings for manual tuning.";
/// B4.2: settings entered with a method that takes none.
pub const SETTINGS_NEED_MANUAL: &str = "Settings are entered only with the manual method.";
/// B4.2: manual settings without the controller configuration that they take (the page's Automatic family).
pub const MANUAL_NEEDS_CONTROLLER: &str =
    "Choose a controller configuration before entering manual settings.";
/// B4.2: pinned settings that name no family, when the request names no structure and the study holds neither a candidate nor
/// pins of a family.
pub const PINS_NEED_FAMILY: &str = "Choose a controller configuration before keeping settings.";
/// B4.2 and A8.1: the tuning method the page chose, which the engine does not run.
pub const METHOD_NOT_RUN: &str = "The engine does not run the tuning method the page chose.";
/// B4.5: a manual preview whose settings are missing, or are not numbers by their symbols.
pub const PREVIEW_NEEDS_SETTINGS: &str =
    "The preview needs the controller settings, each by its symbol and as a number.";
/// B4.5: the reason of entered settings, predicted on the model and not yet validated.
pub const MANUAL_READY: &str =
    "Your settings, predicted on the selected model. Validate before applying.";
/// B4.4: the method in words for entered settings, which no formula gives.
pub const MANUAL_DESCRIPTION: &str =
    "The controller settings the engineer entered, predicted on the selected model.";
/// N5.R: the relay rule of a study with no qualified Feedback relay record (`relayCycleEvidence.ts`).
pub const RELAY_NOT_IDENTIFIED: &str =
    "Identify the plant with Feedback relay and repeatable cycles before using the relay rule.";
/// N2.9: a relay record with fewer than three complete cycles.
pub const RELAY_TOO_FEW_CYCLES: &str =
    "At least three complete feedback relay cycles are required.";
/// N2.9: relay output levels with no finite, non-zero separation.
pub const RELAY_OUTPUT_SEPARATION: &str =
    "Relay output levels must have a finite non-zero separation.";
/// N2.9: a relay cycle without finite peak, trough and positive period evidence.
pub const RELAY_CYCLE_EVIDENCE: &str =
    "Every relay cycle needs finite peak, trough and positive period evidence.";
/// N2.9: relay cycles whose mean amplitude or period is not identifiable.
pub const RELAY_NOT_IDENTIFIABLE: &str = "Relay cycle amplitude or period is not identifiable.";
/// N2.9: relay output levels that are not finite and ordered (`relayTuning.ts`).
pub const RELAY_LEVELS: &str = "Relay output levels must be finite and ordered.";
/// N2.9: relay samples whose times, measurements or outputs are not finite, or whose times do not increase.
pub const RELAY_SAMPLES: &str =
    "Relay samples need finite strictly increasing measured times, process values and outputs.";
/// N2.9: a relay record with fewer than two measured high-output transitions.
pub const RELAY_TRANSITIONS: &str =
    "Relay records need at least two measured high-output transitions.";
/// N2.9: a relay cycle that holds an output which is neither relay level.
pub const RELAY_INTERMEDIATE_OUTPUT: &str =
    "A relay cycle contains clipped or intermediate output values.";

/// N5.R: the relay rule's reason, with `Ku` and `Pu` as the engineer reads them (`relayCycleEvidence.ts`).
#[must_use]
pub fn relay_rule_reason(ultimate_gain: f64, period_seconds: f64) -> String {
    format!(
        "Relay rule from Ku = {} and Pu = {} s: Kp = 0.45·Ku, Ti = Pu/1.2. Validate before applying.",
        engineering(ultimate_gain, None),
        engineering(period_seconds, None)
    )
}

/// N5.R: the relay rule's method in words: its qualified cycles, with `Ku` and `Pu`.
#[must_use]
pub fn relay_rule_description(ultimate_gain: f64, period_seconds: f64, cycles: usize) -> String {
    format!(
        "Relay rule (N5.R) from {cycles} qualified relay cycles: Ku = {} and Pu = {} s, Kp = 0.45·Ku, Ti = Pu/1.2.",
        engineering(ultimate_gain, None),
        engineering(period_seconds, None)
    )
}

/// N2.9: a relay spread above 10 % of its mean, naming the quantity that varies (`relayTuning.ts`).
#[must_use]
pub fn relay_spread(quantity: &str) -> String {
    format!(
        "Relay {quantity} spread exceeds 10% of its mean. Record more settled cycles or use another permitted identification method."
    )
}

/// B4.5: a setting that the loop does not bind (`manualPreview.ts`).
#[must_use]
pub fn not_a_setting(symbol: &str) -> String {
    format!("{symbol} is not a setting of this controller.")
}

/// B4.2 and C2: a pinned or entered setting that the family does not take (`family_setting_symbols`).
#[must_use]
pub fn not_a_family_setting(symbol: &str, family: &str) -> String {
    format!("{symbol} is not a setting of family {family}.")
}

/// B4.5 and C0: a setting outside the range its parameter declares, or not finite (`manualPreview.ts`). The numbers are
/// printed as the engineering format prints them, as the other sentences of Tune do.
#[must_use]
pub fn outside_bounds(symbol: &str, minimum: f64, maximum: f64, unit: &str, value: f64) -> String {
    format!(
        "{symbol} must be between {} and {} {unit}; {} is outside that range.",
        engineering(minimum, None),
        engineering(maximum, None),
        engineering(value, None)
    )
}

/// The sentence of a manual setting whose entry no finite number reads (B4.5): its range, and that the entry is not a number.
#[must_use]
pub fn setting_not_a_number(symbol: &str, minimum: f64, maximum: f64, unit: &str) -> String {
    format!(
        "{symbol} must be between {} and {} {unit}; the entry is not a number.",
        engineering(minimum, None),
        engineering(maximum, None)
    )
}

/// A number in a sentence, rounded to `digits` significant digits as JavaScript's `toPrecision` rounds it.
#[must_use]
pub fn significant(value: f64, digits: usize) -> String {
    js_number(js_to_precision(value, digits))
}

/// A number as `formatEngineeringNumber` prints it (`chartContract.ts`): the given fraction digits, or those the
/// magnitude gets automatically, with an exponent form below their reach. Non-finite values print as the chart shows
/// them.
#[must_use]
pub fn engineering(value: f64, fraction_digits: Option<usize>) -> String {
    if value.is_infinite() {
        return if value > 0.0 {
            "∞".to_string()
        } else {
            "−∞".to_string()
        };
    }
    if !value.is_finite() {
        return "—".to_string();
    }
    if is_exactly_zero(value) {
        return "0".to_string();
    }
    let magnitude = value.abs();
    let digits = fraction_digits.map_or_else(
        || automatic_fraction_digits(magnitude),
        |given| given.min(6),
    );
    let reach = 10f64.powi(-i32::try_from(digits).unwrap_or(6));
    if (fraction_digits.is_none() && magnitude < 0.001) || magnitude < reach {
        return format!("{value:.2e}");
    }
    trim_fraction(&format!("{:.*}", digits, js_to_fixed(value, digits)))
}

/// The fraction digits `formatEngineeringNumber` shows for a magnitude when none are given.
fn automatic_fraction_digits(magnitude: f64) -> usize {
    if magnitude >= 1000.0 {
        0
    } else if magnitude >= 100.0 {
        1
    } else if magnitude >= 1.0 {
        2
    } else if magnitude >= 0.01 {
        4
    } else {
        6
    }
}

/// Drops the zeros after the decimal point, and the point itself when nothing follows it.
fn trim_fraction(text: &str) -> String {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        text.to_string()
    }
}

/// The unit after a number in a sentence: a space and the unit, or nothing for a dimensionless value.
#[must_use]
pub fn unit_suffix(unit: &str) -> String {
    if unit.is_empty() {
        String::new()
    } else {
        format!(" {unit}")
    }
}

/// N5.7: the refusal of a model whose normalized error is above the limit of 0.25 (`validateTuningModel`).
#[must_use]
pub fn model_error_above_limit(method: TuningMethod) -> String {
    format!(
        "{} PI tuning requires accepted model error no greater than 0.25.",
        method.word()
    )
}

/// `tuneController`: the refusal of a method that needs a stable first-order model.
#[must_use]
pub fn needs_first_order(method: TuningMethod) -> String {
    format!(
        "{} PI tuning requires a stable first-order model.",
        method.word()
    )
}

/// N6.3: the sentence that gives a proportional gain's required sign for the controller action, in plain words.
#[must_use]
pub fn proportional_sign_requirement(positive: bool) -> String {
    format!(
        "The proportional gain must be {} for this controller action.",
        if positive { "positive" } else { "negative" }
    )
}

/// C2: a parallel law without the integral gain a family needs.
#[must_use]
pub fn seed_needs_integral(family: &str) -> String {
    format!("A {family} design needs an integral gain; the program's law has no integral gain.")
}

/// C2: a parallel law without the derivative gain a family needs.
#[must_use]
pub fn seed_needs_derivative(family: &str) -> String {
    format!("A {family} design needs a derivative gain; the program's law has no derivative gain.")
}

/// N5.9: the refusal of a PD whose predicted steady error is above the declared allowance.
#[must_use]
pub fn pd_steady_error_exceeds(steady_error: f64) -> String {
    format!(
        "PD predicted steady error {} exceeds the declared design allowance.",
        js_number(steady_error)
    )
}

/// B4.4: the refusal of an initial output outside the declared output limits.
#[must_use]
pub fn initial_output_outside(output: f64, minimum: f64, maximum: f64) -> String {
    format!(
        "Automatic controller initial output {} is outside the declared output limit {}…{}.",
        js_number(output),
        js_number(minimum),
        js_number(maximum)
    )
}

/// B4.4: the reason a family cannot meet the requirements on the model.
#[must_use]
pub fn family_cannot_meet(family: &str, reason: &str) -> String {
    format!("{family} cannot meet the requirements on the model: {reason}")
}

/// B4.4: a gain whose magnitude is above its declared maximum, in the words the shortfall uses.
#[must_use]
pub fn gain_above_maximum(label: &str, magnitude: f64, maximum: f64, unit: &str) -> String {
    format!(
        "needs {label} of magnitude {}{}, above its declared maximum of {}{}",
        engineering(magnitude, Some(4)),
        unit_suffix(unit),
        engineering(maximum, Some(4)),
        unit_suffix(unit)
    )
}

/// B4.4: a gain whose magnitude is below its declared minimum, in the words the shortfall uses.
#[must_use]
pub fn gain_below_minimum(label: &str, magnitude: f64, minimum: f64, unit: &str) -> String {
    format!(
        "needs {label} of magnitude {}{}, below its declared minimum of {}{}",
        engineering(magnitude, Some(4)),
        unit_suffix(unit),
        engineering(minimum, Some(4)),
        unit_suffix(unit)
    )
}

/// B4.3: an overshoot limit that is blank for no limit, or a finite value above zero and at most a thousand percent.
#[must_use]
pub fn overshoot_out_of_range(label: &str) -> String {
    format!(
        "{label} must be blank for no limit, or a finite value above 0 and at most {} % of the setpoint change.",
        crate::tuning::requirements::MAXIMUM_OVERSHOOT_PERCENT
    )
}

/// C1: the message of a Control.PID record whose runtime value the runtime did not report.
#[must_use]
pub fn runtime_parameter_missing(name: &str) -> String {
    format!("Runtime controller parameter {name} is unavailable.")
}

/// C0: a model-derived value that does not fit its declared bound.
#[must_use]
pub fn model_value_out_of_bounds(symbol: &str) -> String {
    format!("Model-derived parameter {symbol} exceeds its representable bound.")
}

/// C0: a model gain field whose unit does not match the expected one.
#[must_use]
pub fn model_gain_unit(expected: &str) -> String {
    format!("Model gain fields require {expected} units.")
}

/// B4.3: a supplied criterion that is not a finite number.
#[must_use]
pub fn review_entered_value(key: &str) -> String {
    format!("Review the entered {key} value.")
}

/// B4.3: an operating point outside the operating limits of its input.
#[must_use]
pub fn review_operating_limits(symbol: &str) -> String {
    format!("Review the operating limits for {symbol}.")
}

/// B4.3: a recorded range or noise that leaves no automatic test change.
#[must_use]
pub fn no_automatic_change(symbol: &str) -> String {
    format!(
        "The recorded range or signal noise prevents an automatic test change for {symbol}. Review its limits or identification recording."
    )
}

/// B4.3: a test change outside the range the model was identified in, with its remedy.
#[must_use]
pub fn change_leaves_range(
    change: f64,
    symbol: &str,
    unit: &str,
    minimum: f64,
    maximum: f64,
    room: f64,
) -> String {
    format!(
        "The test change of {} {unit} for {symbol} leaves the range the model was identified in ({}–{} {unit}). Identify with a larger step, or choose a change of at most {} {unit}.",
        engineering(change, None),
        engineering(minimum, None),
        engineering(maximum, None),
        engineering(room, None)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engineering_numbers_follow_the_chart_formatter() {
        assert_eq!(engineering(0.0, None), "0");
        assert_eq!(engineering(1234.5, None), "1235");
        assert_eq!(engineering(12.5, None), "12.5");
        assert_eq!(engineering(2.0, None), "2");
        assert_eq!(engineering(0.0005, None), "5.00e-4");
        assert_eq!(engineering(f64::NEG_INFINITY, None), "−∞");
        assert_eq!(engineering(f64::NAN, None), "—");
        assert_eq!(engineering(1.23456, Some(2)), "1.23");
    }

    #[test]
    fn method_sentences_name_the_stored_word() {
        assert_eq!(
            model_error_above_limit(TuningMethod::Simc),
            "SIMC PI tuning requires accepted model error no greater than 0.25."
        );
        assert_eq!(
            needs_first_order(TuningMethod::Amigo),
            "AMIGO PI tuning requires a stable first-order model."
        );
    }

    #[test]
    fn the_sign_sentence_names_the_direction_in_words() {
        assert_eq!(
            proportional_sign_requirement(true),
            "The proportional gain must be positive for this controller action."
        );
        assert_eq!(
            proportional_sign_requirement(false),
            "The proportional gain must be negative for this controller action."
        );
    }

    #[test]
    fn named_sentences_carry_their_name() {
        assert_eq!(
            runtime_parameter_missing("IntegralGain"),
            "Runtime controller parameter IntegralGain is unavailable."
        );
        assert_eq!(
            review_entered_value("maximumError"),
            "Review the entered maximumError value."
        );
    }
}
