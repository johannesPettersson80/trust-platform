//! The sentences of the structures' design rules, as Control Studio's TypeScript writes them (A11.1). Each sentence that
//! names a value names it; the sentences of a path take the subject of the path (`controller output`, `cascade inner`, ...).

/// The cascade's model must be a state-space model from the plant input to both measurements (C6).
pub const CASCADE_MODEL: &str =
    "A cascade is designed on one model from the plant input to both measurements.";

/// The cascade's model must have the plant input and the two measurements of its channels (C6).
pub const CASCADE_CHANNELS: &str =
    "The model's channels do not match the cascade's plant input and measurements.";

/// A feedforward is designed on one model from the controller output and the measured disturbance to the measurement (C8).
pub const FEEDFORWARD_CHANNELS: &str = "A feedforward is designed on one model from the controller output and the measured disturbance to the measurement.";

/// A feedforward needs the disturbance's identification step, which must be positive (C8).
pub const FEEDFORWARD_STEP: &str = "Declare the measured disturbance's identification step.";

/// A Smith predictor is designed on a model from the controller output to the measurement (C9).
pub const SMITH_MODEL: &str =
    "A Smith predictor is designed on a model from the controller output to the measurement.";

/// A PID group's pairs need two or more members, each with its own input and output channel (C7).
pub const PAIR_EACH_MEMBER: &str = "Pair every group member's input and output channel once.";

/// The members' steady-state gains are singular, so their outputs cannot be set independently (N4.5).
pub const GROUP_SINGULAR_OUTPUTS: &str =
    "The members' steady-state gains are singular: their outputs cannot be set independently.";

/// The sentence that follows a conflicting relative gain when no pairing has positive relative gains (N4.5).
pub const NO_POSITIVE_PAIRING: &str = " No pairing of these channels has positive relative gains.";

/// A decoupler needs one row and one column per group member (C7).
pub const DECOUPLER_MEMBERS: &str = "A decoupler needs one row and one column per group member.";

/// A decoupler is designed on a state-space group model (C7).
pub const DECOUPLER_STATE_MODEL: &str = "A decoupler is designed on a state-space group model.";

/// The model's channels must hold each group member's input and output (C7).
pub const DECOUPLER_CHANNELS: &str = "The model's channels do not match the group's members.";

/// The state equations of the group's model have no solution, so its steady-state gain is not defined.
pub const DECOUPLER_STATE_SINGULAR: &str = "The decoupler matrix is singular.";

/// The members' steady-state gains are singular, so no decoupler separates them (C7).
pub const DECOUPLER_GAINS_SINGULAR: &str =
    "The members' steady-state gains are singular: no decoupler can separate them.";

/// A gain schedule needs two to four operating points (C10).
pub const SCHEDULE_POINTS: &str = "A gain schedule needs two to four operating points.";

/// Each operating point needs its own name and its own two gain variables (C10).
pub const SCHEDULE_NAMES: &str =
    "Give each operating point its own name and its own two gain variables.";

/// The path has no steady-state change to reduce (N5.15).
#[must_use]
pub fn no_steady_gain(subject: &str) -> String {
    format!("The {subject} path has no steady-state gain.")
}

/// The path has no step response to reduce (N5.15).
#[must_use]
pub fn no_step_response(subject: &str) -> String {
    format!("The {subject} path has no step response to reduce.")
}

/// A structure designs on a state-space model, not on a transfer function (C6, C7, C8).
#[must_use]
pub fn state_space_only(subject: &str) -> String {
    format!("The {subject} is designed on a state-space model.")
}

/// The model cannot be sampled at the task period by the exact zero-order hold (N4.7(a)).
#[must_use]
pub fn cannot_sample(subject: &str) -> String {
    format!(
        "The {subject} model cannot be sampled at the task period (its discretization is singular or not finite)."
    )
}

/// The path does not reach its steady state within the model's sample period (TypeScript `openLoopStepResponse`).
#[must_use]
pub fn too_slow(subject: &str) -> String {
    format!("A {subject} path is too slow for the model's sample period to reach steady state.")
}

/// The path does not settle: one of its poles is not stable enough to set a settling horizon (TypeScript `settlingHorizon`).
#[must_use]
pub fn does_not_settle(subject: &str) -> String {
    format!("A {subject} path does not settle: the design needs a stable model.")
}

/// The poles of the model cannot be computed, so its step responses cannot be calculated.
#[must_use]
pub fn poles_unavailable(subject: &str) -> String {
    format!("Cannot calculate the {subject}'s step responses: its poles cannot be computed.")
}

/// The response settings of the model are not usable, so its step response cannot be calculated.
#[must_use]
pub fn responses_unavailable(subject: &str) -> String {
    format!(
        "Cannot calculate the {subject}'s step responses: its response settings are not usable."
    )
}

/// The settling time must be longer than the model's delay, which the Smith predictor's loop has to wait for (C9).
#[must_use]
pub fn smith_delay(delay_seconds: f64) -> String {
    format!(
        "The settling time must be longer than the model's delay of {} s: no controller acts sooner.",
        significant(delay_seconds, 3)
    )
}

/// A pairing's diagonal relative gain is not positive, so that loop would work against the others (N4.5); `better` is the
/// sentence that names the pairing to use, or the one that names none.
#[must_use]
pub fn relative_gain_conflict(input: &str, output: &str, value: f64, better: &str) -> String {
    format!(
        "The relative gain of {input} → {output} is {}, so that loop would work against the others.{better}",
        significant(value, 2)
    )
}

/// The sentence that names the pairing to use: `pairs` lists its pairs, sorted by input (N4.5).
#[must_use]
pub fn pairing_instead(pairs: &str) -> String {
    format!(" Pair {pairs} instead.")
}

/// JavaScript's `String(Number(value.toPrecision(digits)))`: the value rounded to significant digits, written without trailing
/// zeros, as the TypeScript sentences write a number.
#[must_use]
pub fn significant(value: f64, digits: usize) -> String {
    let rounded: f64 = format!("{:.*e}", digits.saturating_sub(1), value)
        .parse()
        .unwrap_or(value);
    rounded.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_is_written_as_the_sentences_write_it() {
        assert_eq!(significant(-0.1905, 2), "-0.19");
        assert_eq!(significant(1.0, 3), "1");
        assert_eq!(significant(0.9994, 3), "0.999");
        assert_eq!(significant(0.9994, 2), "1");
        assert_eq!(significant(1.2345, 3), "1.23");
    }

    #[test]
    fn the_relative_gain_sentence_names_the_loop_and_the_pairing() {
        let sentence =
            relative_gain_conflict("u1", "y1", -0.1905, &pairing_instead("u1 → y2 and u2 → y1"));
        assert_eq!(
            sentence,
            "The relative gain of u1 → y1 is -0.19, so that loop would work against the others. Pair u1 → y2 and u2 → y1 instead."
        );
    }
}
