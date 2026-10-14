//! The sentences of frequency and excitation identification (specification 35, B3.9; numerical annex N2.8): what a refused
//! frequency-response estimate or test signal says, in plain words. The rules decide; these functions name the decision.

use trust_control_math::excitation::ExcitationRejection;
use trust_control_math::frequency_identification::FrequencyRejection;

/// The sentence of a refused frequency-response estimate (N2.8, N4.3).
#[must_use]
pub fn frequency_refusal(reason: FrequencyRejection) -> &'static str {
    match reason {
        FrequencyRejection::InconsistentDimensions => "The input and output records differ in length.",
        FrequencyRejection::NonfiniteInput => {
            "A sample time, a sample, a tone or the coherence limit is not a finite number, or the sample time is not positive."
        }
        FrequencyRejection::InsufficientData => {
            "The record is too short for the requested segments, or the coherence limit or the tones are invalid."
        }
        FrequencyRejection::AliasRisk => {
            "A tone is not positive or is at or above the Nyquist frequency of the sample time."
        }
        FrequencyRejection::PoorExcitation => {
            "The input has no energy at a requested tone, so the response there cannot be estimated."
        }
        FrequencyRejection::LowCoherence => {
            "A tone's coherence is below the minimum, so the response there is not repeatable enough to fit."
        }
    }
}

/// The sentence of a refused test signal (N2.8).
#[must_use]
pub fn excitation_refusal(reason: ExcitationRejection) -> &'static str {
    match reason {
        ExcitationRejection::InvalidConfiguration => {
            "The excitation declaration is invalid; check its order, length, amplitude, seed, tones and bound."
        }
        ExcitationRejection::AliasRisk => "A multisine tone is at or above the Nyquist frequency of the sample time.",
    }
}
