//! The sign rule of a plant gain (N6.1–N6.3): a loop whose controller acts `direct` has a positive gain, one whose
//! controller acts `reverse` has a negative gain. The identified gain must have the sign its action requires.

#![allow(clippy::module_name_repetitions)] // `GainSign` names the module's subject.

/// The direction in which a controller's output moves (N6.1, B1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerAction {
    /// The output increases while the measurement is below the setpoint, as for heating: `s_a = +1`.
    Direct,
    /// The output increases while the measurement is above the setpoint, as for cooling: `s_a = −1`.
    Reverse,
}

/// The sign of a gain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GainSign {
    /// A positive gain: the measurement rises when the output rises.
    Positive,
    /// A negative gain: the measurement falls when the output rises.
    Negative,
}

/// The sign a plant gain must have for a controller action (N6.2): positive for `direct`, negative for `reverse`.
#[must_use]
pub fn required_gain_sign(action: ControllerAction) -> GainSign {
    match action {
        ControllerAction::Direct => GainSign::Positive,
        ControllerAction::Reverse => GainSign::Negative,
    }
}

/// Whether an identified gain contradicts its controller action (N6.3): a finite, non-zero gain whose sign is not the
/// sign the action requires. A zero or non-finite gain is not a contradiction; those are refused elsewhere (N5.7).
#[must_use]
pub fn gain_sign_contradicts(gain: f64, action: ControllerAction) -> bool {
    if !gain.is_finite() || gain == 0.0 {
        return false;
    }
    let actual = if gain > 0.0 {
        GainSign::Positive
    } else {
        GainSign::Negative
    };
    actual != required_gain_sign(action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gain_contradicts_its_action_only_when_its_sign_is_the_other_one() {
        assert!(!gain_sign_contradicts(2.0, ControllerAction::Direct));
        assert!(gain_sign_contradicts(-2.0, ControllerAction::Direct));
        assert!(!gain_sign_contradicts(-2.0, ControllerAction::Reverse));
        assert!(gain_sign_contradicts(2.0, ControllerAction::Reverse));
    }

    #[test]
    fn a_zero_or_non_finite_gain_is_not_a_sign_contradiction() {
        assert!(!gain_sign_contradicts(0.0, ControllerAction::Reverse));
        assert!(!gain_sign_contradicts(f64::NAN, ControllerAction::Direct));
        assert!(!gain_sign_contradicts(
            f64::NEG_INFINITY,
            ControllerAction::Direct
        ));
    }

    #[test]
    fn the_required_sign_follows_the_action() {
        assert_eq!(
            required_gain_sign(ControllerAction::Direct),
            GainSign::Positive
        );
        assert_eq!(
            required_gain_sign(ControllerAction::Reverse),
            GainSign::Negative
        );
    }
}
