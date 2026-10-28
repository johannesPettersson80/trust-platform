//! The declared response and the tuning method (specification 35, B4.2 and B4.3; numerical annex N5.2 to N5.5 and N5.4).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use trust_control_math::tuning::ResponseChoice;

/// The response the engineer declares (N5.4). It chooses `λ` and changes the ranking, never the limits or the
/// acceptance requirements. The words are the stored ones of the requirement record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TuningObjective {
    /// The Faster value of `λ`.
    Fast,
    /// Twice the Faster value of `λ`.
    Balanced,
    /// Four times the Faster value of `λ`.
    Robust,
}

/// The response that Automatic uses. The Tune page declares no response choice for Automatic, so it is always filled as
/// More robust, whether or not the criteria were generated (specification `control-studio-tune-page.md`, "Control
/// requirements"; specification 35 C1).
pub const AUTOMATIC_OBJECTIVE: TuningObjective = TuningObjective::Robust;

impl TuningObjective {
    /// The response choice of the numerical annex (N5.4): fast is the Faster value, balanced twice it and robust four
    /// times it.
    #[must_use]
    pub fn response_choice(self) -> ResponseChoice {
        match self {
            Self::Fast => ResponseChoice::Faster,
            Self::Balanced => ResponseChoice::Balanced,
            Self::Robust => ResponseChoice::MoreRobust,
        }
    }
}

/// A named formula method (N5.2 to N5.5). Automatic is not a method here: it chooses among them (N5.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum TuningMethod {
    /// SIMC, Skogestad (2003): N5.2 and N5.3.
    #[serde(rename = "SIMC")]
    Simc,
    /// AMIGO, Åström and Hägglund (2004): N5.5.
    #[serde(rename = "AMIGO")]
    Amigo,
}

impl TuningMethod {
    /// The method's word as the engineer and the stored requirement record write it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Simc => "SIMC",
            Self::Amigo => "AMIGO",
        }
    }

    /// The method's title as the description shows it.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Simc => "SIMC PI (Skogestad 2003)",
            Self::Amigo => "AMIGO PI (Åström and Hägglund 2004)",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_declared_response_selects_its_choice_of_the_annex() {
        assert_eq!(
            TuningObjective::Fast.response_choice(),
            ResponseChoice::Faster
        );
        assert_eq!(
            TuningObjective::Balanced.response_choice(),
            ResponseChoice::Balanced
        );
        assert_eq!(
            TuningObjective::Robust.response_choice(),
            ResponseChoice::MoreRobust
        );
    }

    #[test]
    fn automatic_fills_the_response_as_more_robust() {
        assert_eq!(AUTOMATIC_OBJECTIVE, TuningObjective::Robust);
    }

    #[test]
    fn the_method_words_are_the_stored_ones() {
        assert_eq!(TuningMethod::Simc.word(), "SIMC");
        assert_eq!(TuningMethod::Amigo.word(), "AMIGO");
        assert_eq!(TuningMethod::Simc.title(), "SIMC PI (Skogestad 2003)");
    }
}
