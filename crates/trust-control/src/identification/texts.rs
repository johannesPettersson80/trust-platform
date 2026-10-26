//! The sentences of Identify (specification 35, A11, B3.9 and B5.5; numerical annex N2 and N6): the texts the engineer
//! reads for each decision of the identification rules. A rule decides; these functions say what it decided, in plain
//! words. Numbers print as JavaScript prints them (`experiment::number_text`), so every sentence reads as the `TypeScript`
//! sentence did.

use trust_control_math::identify::gain::{gain_sign_contradicts, ControllerAction};
use trust_control_math::identify::relay::{RelayQuantity, RelayRecordError, RelayRejection};
use trust_control_math::identify::score::{IndependentResponseScore, ScoreStatus};
use trust_control_math::sampling::SampleGap;

use crate::experiment::number_text::{js_number, js_round, js_to_fixed, js_to_precision};
use crate::identification::acquisition_settings::{AcquisitionError, ChannelDirection};
use crate::identification::method_applicability::MethodRefusal;
use crate::identification::model_compatibility::CompatibilityRefusal;
use crate::identification::policy::PolicyError;
use crate::recording::even_sampling::{EvenSamplingError, MINIMUM_EVEN_SCANS};

/// The smallest response a fit accepts, as a multiple of the measured noise (N2.6): a response below twenty times the
/// baseline noise is refused (B3.9).
pub const MINIMUM_RESPONSE_TO_NOISE_RATIO: f64 = 20.0;

/// The smallest fraction of its fitted final change a held step must reach to be accepted (A10.3, N2.3). A response
/// that reaches less is refused with the hold its time constant suggests.
pub const MINIMUM_REACHED_FRACTION: f64 = 0.63;

/// The word that names the excitation in the noise refusal: a step, or an excitation of another kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExcitationWord {
    /// A held step.
    Step,
    /// Any other excitation.
    Excitation,
}

impl ExcitationWord {
    /// The word as the sentence prints it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Step => "step",
            Self::Excitation => "excitation",
        }
    }
}

/// The refusal when a gain's sign contradicts the controller action (B3.9, N6.3), or `None` when they agree or the gain
/// is zero or not finite. The sentence states the direction in plain words, never the stored action names.
#[must_use]
pub fn gain_sign_contradiction(gain: f64, unit: &str, action: ControllerAction) -> Option<String> {
    if !gain_sign_contradicts(gain, action) {
        return None;
    }
    let response = if gain > 0.0 { "rises" } else { "falls" };
    let setup = match action {
        ControllerAction::Reverse => {
            "increase the output while the measurement is above the setpoint"
        }
        ControllerAction::Direct => {
            "increase the output while the measurement is below the setpoint"
        }
    };
    Some(format!(
        "The identified gain is {}{} (the measurement {response} when the output rises), but the loop is set up to {setup}. Check the measured output, the plant input and the controller action in the loop setup.",
        engineering_text(gain),
        unit_suffix(unit),
    ))
}

/// The refusal when a fitted gain is zero (N5.7, N6.3): the measured output does not respond to the plant input, so the
/// gain gives no sign to check and no controller can be designed from it. The sentence gives the gain with its unit and
/// names the next action.
#[must_use]
pub fn zero_gain_refusal(unit: &str) -> String {
    format!(
        "The identified gain is 0{}: the measured output does not respond to the plant input. Check the measured output, the plant input and the loop setup.",
        unit_suffix(unit),
    )
}

/// The refusal for a response too small against the measured noise (N2.6, B3.9), or `None` when it is large enough,
/// or when the noise is unknown (zero, or not a number).
#[must_use]
pub fn noise_refusal(
    response_size: f64,
    noise_rms: f64,
    unit: &str,
    excitation: ExcitationWord,
) -> Option<String> {
    if noise_rms.is_nan() || noise_rms <= 0.0 {
        return None;
    }
    if response_size.is_nan() || response_size >= MINIMUM_RESPONSE_TO_NOISE_RATIO * noise_rms {
        return None;
    }
    let suffix = unit_suffix(unit);
    Some(format!(
        "The response ({}{suffix}) is smaller than {} times the measured noise ({}{suffix} RMS). Permit a larger {} in Limits…, or choose another method.",
        engineering_text(response_size),
        js_number(MINIMUM_RESPONSE_TO_NOISE_RATIO),
        engineering_text(noise_rms),
        excitation.word(),
    ))
}

/// The refusal for a held step that has not reached the required fraction of its fitted final change (A10.3): the
/// fraction in percent, the time held so far, and the hold the fitted time constant suggests, `L + 3τ` (N2.3).
#[must_use]
pub fn partial_response_refusal(
    fraction: f64,
    hold_so_far_seconds: f64,
    delay_seconds: f64,
    time_constant_seconds: f64,
) -> String {
    format!(
        "The response reached {} % of its fitted final change in {} min; hold the step for about {} min (Limits… → maximum duration).",
        js_number(js_round(fraction * 100.0)),
        minutes_text(hold_so_far_seconds),
        minutes_text(delay_seconds + 3.0 * time_constant_seconds),
    )
}

/// The reason an independent model check gives (B5.5): what passed, or what to change for the part that failed.
#[must_use]
pub fn independent_response_reason(score: &IndependentResponseScore) -> &'static str {
    if score.status == ScoreStatus::Pass {
        return "Frozen predictions agree with independent plant measurements over the whole response and its final 10 %.";
    }
    if !score.whole_within_limit {
        return "The fitted model does not predict the independent plant response within its error limit over the whole response. Identify again with a different method, a larger step, or a longer duration so the fit covers the response it must predict.";
    }
    "The fitted model predicts the start of the independent response but not its final 10 %, where the response settles. Identify again with a longer duration so the recording covers the settled response, or with a different method."
}

/// The phrase that names a gap in a recording: its length and its start, to the millisecond, as the `TypeScript` prints
/// them (`gapPhrase`).
#[must_use]
pub fn gap_phrase(gap: SampleGap) -> String {
    format!(
        "a gap of {} s at {} s",
        js_number(js_to_fixed(gap.length_seconds, 3)),
        js_number(js_to_fixed(gap.start_seconds, 3)),
    )
}

/// The sentence for a relay record that cannot be read as cycles (`extractRelayCycles`).
#[must_use]
pub fn relay_record_text(error: RelayRecordError) -> &'static str {
    match error {
        RelayRecordError::Levels => "Relay output levels must be finite and ordered.",
        RelayRecordError::Samples => {
            "Relay samples need finite strictly increasing measured times, process values and outputs."
        }
        RelayRecordError::TooFewTransitions => {
            "Relay records need at least two measured high-output transitions."
        }
        RelayRecordError::IntermediateOutput => {
            "A relay cycle contains clipped or intermediate output values."
        }
    }
}

/// The sentence for relay cycles that do not qualify a candidate (`qualifyRelayCycles`).
#[must_use]
pub fn relay_rejection_text(reason: RelayRejection) -> String {
    match reason {
        RelayRejection::TooFewCycles => {
            "At least three complete feedback relay cycles are required.".to_string()
        }
        RelayRejection::OutputSeparation => {
            "Relay output levels must have a finite non-zero separation.".to_string()
        }
        RelayRejection::CycleEvidence => {
            "Every relay cycle needs finite peak, trough and positive period evidence.".to_string()
        }
        RelayRejection::NotIdentifiable => {
            "Relay cycle amplitude or period is not identifiable.".to_string()
        }
        RelayRejection::Spread(quantity) => {
            let name = match quantity {
                RelayQuantity::Amplitude => "amplitude",
                RelayQuantity::Period => "period",
            };
            format!(
                "Relay {name} spread exceeds 10% of its mean. Record more settled cycles or use another permitted identification method."
            )
        }
    }
}

/// The sentence for an acquisition setting that cannot be planned (`acquisitionSettings.ts`).
#[must_use]
pub fn acquisition_text(error: &AcquisitionError) -> String {
    match error {
        AcquisitionError::LimitsRequired => {
            "Set the maximum experiment duration in Limits before identification.".to_string()
        }
        AcquisitionError::SetpointReference => {
            "Choose a declared setpoint signal in the measured output's units.".to_string()
        }
        AcquisitionError::UnknownChannelRange(direction) => {
            let name = match direction {
                ChannelDirection::Input => "input",
                ChannelDirection::Output => "output",
            };
            format!("Unknown {name} channel range.")
        }
        AcquisitionError::RangeOutsideLimits {
            signal,
            minimum,
            maximum,
            ..
        } => format!(
            "Experiment range for {signal} must stay within its configured limits ({} to {}).",
            js_number(*minimum),
            js_number(*maximum),
        ),
        AcquisitionError::RangeNotValid { signal, .. } => {
            format!("The range of {signal} must be finite, with its minimum below its maximum.")
        }
    }
}

/// The sentence that says why an identification method is unavailable for a loop (`identificationMethodApplicability.ts`).
#[must_use]
pub fn method_refusal_text(refusal: MethodRefusal) -> &'static str {
    match refusal {
        MethodRefusal::RelayDirection => {
            "Relay identification needs an explicitly declared process direction."
        }
        MethodRefusal::RelayManualOwnership => {
            "Relay identification needs Manual command and mode ownership."
        }
        MethodRefusal::FrequencyOnOff => {
            "Frequency excitation needs intermediate output levels; choose a binary step or pulse."
        }
        MethodRefusal::SetpointReference => {
            "Setpoint-step identification needs the configured reference signal."
        }
        MethodRefusal::OpenLoopBindings => {
            "Open-loop identification needs the declared Manual command and mode bindings."
        }
        MethodRefusal::ClosedLoopModeBinding => {
            "Closed-loop setpoint identification needs the declared controller mode binding."
        }
    }
}

/// The sentence for a choice that the identification policy or the loop's state does not allow (`identificationPolicy.ts`).
#[must_use]
pub fn policy_error_text(error: &PolicyError) -> String {
    match error {
        PolicyError::SampleNotAvailable => {
            "Fresh synchronized SP, PV, and MV are required.".to_string()
        }
        PolicyError::ModeUnavailable => "Automatic identification cannot choose a method: the loop's Auto/Manual mode is unavailable. Reconnect the runtime, or choose a method.".to_string(),
        PolicyError::NoSafeMethodInManual => "Automatic identification has no safe method: the loop is in Manual, and open-loop tests are not permitted by the identification policy or the missing Manual command. Put the loop in Auto with settings that hold it and identify again, or choose a method.".to_string(),
        PolicyError::NoSafeMethodWithoutSetpoint => "Automatic identification has no safe method: the loop has no declared setpoint binding, and open-loop tests are not permitted by the identification policy or the missing Manual command. Declare the setpoint binding, or choose a method.".to_string(),
        PolicyError::PinnedLoadStep => "Load-step identification requires an explicit disturbance model channel; choose a supported acquisition method instead of substituting an MV step.".to_string(),
        PolicyError::PinnedClosedLoopOnly => "The pinned identification method conflicts with the declared closed-loop-only acquisition policy.".to_string(),
        PolicyError::PinnedHoldRestriction => "The pinned sustained step conflicts with the declared hold restriction.".to_string(),
        PolicyError::DuplicateExcitation => "The project owns this experiment's scenario, so Control Studio writes no duplicate excitation. Choose Automatic to observe the project's scenario.".to_string(),
        PolicyError::PlantInputLimits(failure) => failure.message.clone(),
        PolicyError::NoHeadroom {
            symbol,
            value,
            unit,
        } => format!(
            "Automatic identification has no headroom to step {symbol} from {} {unit}. Open Limits… to widen its permitted range, or choose another method.",
            js_number(*value),
        ),
    }
}

/// The sentence for a model that cannot be used to tune this loop (`engineerModelCompatibility.ts`).
#[must_use]
pub fn compatibility_text(refusal: &CompatibilityRefusal) -> String {
    match refusal {
        CompatibilityRefusal::Evidence => {
            "The model needs measurement evidence or independent validation before tuning."
                .to_string()
        }
        CompatibilityRefusal::UnmappedChannels => {
            "Map every model input and output in loop channel bindings.".to_string()
        }
        CompatibilityRefusal::InputActuatorRange { id } => {
            format!("Selected model input {id} has no usable actuator range.")
        }
        CompatibilityRefusal::InputUnit { id, symbol } => {
            format!("Selected model input {id} unit does not match {symbol}.")
        }
        CompatibilityRefusal::OutputMeasurementRange { id } => {
            format!("Selected model output {id} has no usable measurement range.")
        }
        CompatibilityRefusal::OutputUnit { id, symbol } => {
            format!("Selected model output {id} unit does not match {symbol}.")
        }
        CompatibilityRefusal::SetpointUnit { symbol } => {
            format!("Setpoint unit does not match {symbol}.")
        }
        CompatibilityRefusal::NoControlledOutput => {
            "Declare at least one controlled output and its setpoint binding.".to_string()
        }
        CompatibilityRefusal::SampleTime => {
            "Selected discrete model sample time must match the controller task period.".to_string()
        }
    }
}

/// The sentence for a recording that cannot be placed on an even grid (`evenlySampled`).
#[must_use]
pub fn even_sampling_text(error: EvenSamplingError) -> String {
    match error {
        EvenSamplingError::TooFew => {
            format!("Model validation requires at least {MINIMUM_EVEN_SCANS} complete scans.")
        }
        EvenSamplingError::Irregular => {
            "The independent recording has missing or irregular scans.".to_string()
        }
        EvenSamplingError::Gap(gap) => format!(
            "The independent recording has missing or irregular scans: {}.",
            gap_phrase(gap),
        ),
        EvenSamplingError::Unplaceable => {
            "The recording cannot be placed on an even grid.".to_string()
        }
    }
}

/// The minutes of a duration, to two significant digits, as the `TypeScript` prints them (`minutesText`).
#[must_use]
pub fn minutes_text(seconds: f64) -> String {
    js_number(js_to_precision(seconds / 60.0, 2))
}

/// A number of the engineer's sentences to three significant digits, with a true minus sign for a negative value
/// (`engineeringText`). The first hyphen is replaced, as the `TypeScript`'s `replace("-", "−")` replaces it.
fn engineering_text(value: f64) -> String {
    js_number(js_to_precision(value, 3)).replacen('-', "−", 1)
}

/// The unit after a number in a sentence: a space and the unit, or nothing for a dimensionless value.
fn unit_suffix(unit: &str) -> String {
    if unit.is_empty() {
        String::new()
    } else {
        format!(" {unit}")
    }
}

/// The refusal for a record whose measured output does not change, so that its errors have no span to be normalised by
/// (N3.2). It is the sentence the qualification of the data gives for the same output.
pub const OUTPUT_TOO_SMALL: &str = "Output response is too small to identify a model.";

/// The refusal for a record whose plant input never changes, so that its fit has no nonzero step to fit (N2.6). It names
/// the missing step, as N2.6 requires.
pub const MISSING_NONZERO_STEP: &str =
    "The recording is missing a nonzero step: the plant input never changes, so there is no input change to fit. Check that the experiment moves the plant input, then identify again.";

/// The refusal of a runtime connection that takes no controller transfers, so that Identify cannot move the loop to the mode
/// its method needs (B3.4 step 1; annex E6).
pub const MODE_TRANSFER_UNAVAILABLE: &str =
    "This runtime connection takes no controller transfers, so Identify cannot move the loop to the mode its method needs.";

/// The refusal of a return to the mode Identify found the loop in, after the loop's bindings changed (`restore` of
/// `connectedIdentificationMode.ts`; A6.2, A8.2): the sentence of the `TypeScript`, with the loop revision the mode was changed
/// at and the loop's revision now (A11.1).
#[must_use]
pub fn mode_loop_changed(changed_at: &str, now: &str) -> String {
    format!(
        "Restore the original loop bindings before recovering identification mode: Identify changed the loop's mode at loop revision {changed_at}, and the loop is now at revision {now}."
    )
}

/// The refusal of a return to the mode Identify found the loop in, on another program than the one the mode was changed on
/// (`restore` of `connectedIdentificationMode.ts`; A6.7, E11.5): the sentence of the `TypeScript`, with both program revisions
/// (A11.1).
#[must_use]
pub fn mode_program_changed(changed_on: &str, now: &str) -> String {
    format!(
        "The program changed from revision {changed_on} to {now}; the saved identification mode cannot be restored to this program."
    )
}

/// The end of an interrupted identification whose simulator restarted or reloaded its program since it was journaled (A2.2, A8.6,
/// E11.5; B3.9): the new program revision ended every operation bound to the one it ran on, so its recording was not kept and the
/// loop's mode was not returned. It names both revisions (A11.1).
#[must_use]
pub fn ended_with_restarted_simulator(journaled: &str, now: &str) -> String {
    format!(
        "The interrupted identification ended when the simulator restarted or reloaded its program. It ran on program revision {journaled}, and the simulator now runs {now}. Its recording was not kept and the loop's mode was not returned. Check the loop's mode, then identify the plant again."
    )
}

/// The refusal of a Cancel of a journaled Apply (§0.1b `cancel`, B6.4): Cancel ends identification operations, and an Apply is
/// ended by Restore previous settings, which returns the settings and the mode the loop had before it.
#[must_use]
pub fn apply_cancel_refused(operation_id: &str) -> String {
    format!(
        "Operation {operation_id} is an Apply of controller settings, which Cancel does not end. Restore previous settings ends it: it returns the settings and the mode the loop had before Apply."
    )
}

#[cfg(test)]
#[path = "texts_tests.rs"]
mod tests;
