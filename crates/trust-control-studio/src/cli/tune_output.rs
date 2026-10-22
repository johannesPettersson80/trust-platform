//! The readable text of the `tune` verb (specification 35, §0.1b, B4). Each function reads the fields of the result and holds no rule:
//! whether the candidate meets its requirements, and why a margin or a prediction is missing, come from the result itself. The texts of
//! `output` serve the other verbs; these are kept apart so that each file stays small.

use serde_json::Value;
use trust_control::tuning::structure::is_exactly_zero;

use super::output::{member, number_field, quantity, text_field, MISSING};

/// The last line of a tune result (B4.1): Tune keeps its candidate in the study and writes nothing to the controller.
pub const NOT_APPLIED_TEXT: &str =
    "The candidate is not applied: nothing was written to the controller.";

/// The text of a tune result (§0.1b, B4): the candidate's structure and gains with their units, the method, the reason, the
/// structures Automatic rejected with theirs, the outcome against the requirements, the margins, the predicted settling time
/// and overshoot, and last that nothing was written to the controller (B4.1). It reads the fields of the result and decides
/// nothing; a field the result lacks reads as "—".
#[must_use]
pub fn tune_text(result: &Value) -> String {
    let mut lines = vec![
        candidate_line(result),
        format!(
            "Method: {}",
            text_field(result, "methodDescription")
                .or_else(|| text_field(result, "methodTitle"))
                .unwrap_or(MISSING)
        ),
        format!(
            "Reason: {}",
            text_field(result, "reason").unwrap_or(MISSING)
        ),
    ];
    lines.extend(rejected_lines(result));
    lines.extend([
        outcome_line(result),
        margins_line(result),
        prediction_line(result),
        NOT_APPLIED_TEXT.to_string(),
    ]);
    lines.join("\n")
}

/// One line for each structure Automatic attempted and rejected, with its reason (B4.4; Part C, "Candidates and results":
/// the result keeps the rejected candidates), as `Rejected P: <reason>`.
fn rejected_lines(result: &Value) -> Vec<String> {
    member(result, "attempts")
        .as_array()
        .map(|attempts| {
            attempts
                .iter()
                .filter(|attempt| text_field(attempt, "status") == Some("rejected"))
                .map(|attempt| {
                    format!(
                        "Rejected {}: {}",
                        text_field(attempt, "structure").unwrap_or(MISSING),
                        text_field(attempt, "reason").unwrap_or(MISSING)
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The candidate's structure and its three gains with their units, as
/// `Structure: PI. Kp = 2.5 %/°C, Ki = 0.31 %/(°C·s), Kd = 0`.
fn candidate_line(result: &Value) -> String {
    // A program's own law answers its structure and gains in `projectCandidate` (C2); a Control.PID answers its record.
    let project = member(result, "projectCandidate");
    let (candidate, gains) = if project.is_null() {
        let candidate = member(result, "candidate");
        (candidate, member(candidate, "parameters"))
    } else {
        (project, member(project, "gains"))
    };
    let units = member(result, "gainUnits");
    format!(
        "Structure: {}. {}, {}, {}",
        text_field(candidate, "structure").unwrap_or(MISSING),
        gain_text(
            "Kp",
            number_field(gains, "proportionalGain"),
            text_field(units, "proportional")
        ),
        gain_text(
            "Ki",
            number_field(gains, "integralGain"),
            text_field(units, "integral")
        ),
        gain_text(
            "Kd",
            number_field(gains, "derivativeGain"),
            text_field(units, "derivative")
        ),
    )
}

/// One gain as `Kp = 2.5 %/°C`. A gain of exactly zero is a term the structure does not use, so it has no unit to read.
fn gain_text(symbol: &str, gain: Option<f64>, unit: Option<&str>) -> String {
    let Some(gain) = gain else {
        return format!("{symbol} = {MISSING}");
    };
    let unit = if is_exactly_zero(gain) {
        ""
    } else {
        unit.unwrap_or_default()
    };
    format!("{symbol} = {}", quantity(Some(gain), unit))
}

/// The candidate's outcome against the mandatory requirements (B4.4, B4.5): it meets them, it misses them by the shortfall
/// the result states, or there is no prediction for the reason the result states.
fn outcome_line(result: &Value) -> String {
    let outcome = member(result, "outcome");
    let words = match text_field(outcome, "status") {
        Some("meets") => "Meets".to_string(),
        Some("misses") => format!(
            "Misses: {}",
            text_field(outcome, "shortfall").unwrap_or(MISSING)
        ),
        Some("unpredicted") => format!(
            "No prediction: {}",
            text_field(outcome, "reason").unwrap_or(MISSING)
        ),
        _ => MISSING.to_string(),
    };
    format!("Outcome: {words}")
}

/// The candidate's margins on one line (B4.6, N7.7): the gain, phase and delay margins and the peak sensitivity, followed by
/// the sentence that says why its crossover margins are not available when the margins carry one. Without margins, the line
/// gives the reason the result states.
fn margins_line(result: &Value) -> String {
    let margins = member(result, "margins");
    if !margins.is_object() {
        return match text_field(result, "marginsUnavailable") {
            Some(reason) => format!("Margins: not available. {reason}"),
            None => format!("Margins: {MISSING}"),
        };
    }
    let line = format!(
        "Margins: gain margin {}, phase margin {}, delay margin {}, peak sensitivity {}",
        margin_text(member(margins, "gainMarginDb"), "dB"),
        margin_text(member(margins, "phaseMarginDegrees"), "degrees"),
        margin_text(member(result, "delayMarginSeconds"), "s"),
        quantity(number_field(margins, "peakSensitivity"), ""),
    );
    match text_field(margins, "unavailable") {
        Some(reason) => format!("{line}. {reason}"),
        None => line,
    }
}

/// One margin (N7.7, N7.8): a finite margin is its number with its unit, an unbounded margin is infinity (B9.5: "∞", not
/// missing data), and a margin that the evaluated range does not show is "not observed in range", never infinite.
fn margin_text(margin: &Value, unit: &str) -> String {
    if let Some(finite) = number_field(margin, "finite") {
        return quantity(Some(finite), unit);
    }
    match margin.as_str() {
        Some("unbounded") => "∞".to_string(),
        Some("not-observed-in-range") => "not observed in range".to_string(),
        _ => MISSING.to_string(),
    }
}

/// The predicted settling time and overshoot (B4.5), or the reason the result states for having no prediction.
fn prediction_line(result: &Value) -> String {
    if let Some(reason) = text_field(result, "predictionUnavailable") {
        return format!("Prediction: not available. {reason}");
    }
    format!(
        "Predicted settling time: {}, overshoot: {}",
        quantity(number_field(result, "settlingTimeSeconds"), "s"),
        quantity(number_field(result, "overshootPercent"), "%"),
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A tune result as the engine answers it: a PI candidate that meets its requirements, with finite margins.
    fn tuned() -> Value {
        json!({
            "candidate": {
                "applied": false,
                "source": "SIMC",
                "structure": "PI",
                "parameters": {
                    "proportionalGain": 2.5,
                    "integralGain": 0.31,
                    "derivativeGain": 0,
                },
            },
            "method": "SIMC",
            "methodTitle": "SIMC PI (Skogestad 2003)",
            "methodDescription": "SIMC PI with the more robust response, λ = 49 s.",
            "reason": "PI is the simplest structure that meets every requirement.",
            "gainUnits": {
                "proportional": "%/°C",
                "integral": "%/(°C·s)",
                "derivative": "(%·s)/°C",
            },
            "outcome": { "status": "meets" },
            "settlingTimeSeconds": 120.37,
            "overshootPercent": 0.4,
            "margins": {
                "gainMarginDb": { "finite": 9.4 },
                "phaseMarginDegrees": { "finite": 62.13 },
                "peakSensitivity": 1.31,
                "peakSensitivityRadPerSecond": 0.05,
            },
            "delayMarginSeconds": { "finite": 21.7 },
        })
    }

    #[test]
    fn a_tune_result_reads_as_its_candidate_method_reason_outcome_margins_and_prediction() {
        assert_eq!(
            tune_text(&tuned()),
            [
                "Structure: PI. Kp = 2.5 %/°C, Ki = 0.31 %/(°C·s), Kd = 0",
                "Method: SIMC PI with the more robust response, λ = 49 s.",
                "Reason: PI is the simplest structure that meets every requirement.",
                "Outcome: Meets",
                "Margins: gain margin 9.4 dB, phase margin 62.13 degrees, delay margin 21.7 s, peak sensitivity 1.31",
                "Predicted settling time: 120.4 s, overshoot: 0.4 %",
                NOT_APPLIED_TEXT,
            ]
            .join("\n")
        );
    }

    #[test]
    fn automatic_lists_each_rejected_structure_with_its_reason() {
        let mut result = tuned();
        result["attempts"] = json!([
            { "structure": "P", "status": "rejected", "reason": "It cannot meet the 0.1 °C error." },
            { "structure": "PI", "status": "feasible", "reason": "A compatible candidate on the model." },
        ]);
        assert_eq!(
            rejected_lines(&result),
            ["Rejected P: It cannot meet the 0.1 °C error."]
        );
        assert_eq!(rejected_lines(&tuned()), [] as [String; 0]);
    }

    #[test]
    fn a_gain_of_zero_is_a_term_the_structure_does_not_use_and_has_no_unit() {
        let mut result = tuned();
        result["candidate"]["structure"] = json!("P");
        result["candidate"]["parameters"]["integralGain"] = json!(0);
        assert_eq!(
            candidate_line(&result),
            "Structure: P. Kp = 2.5 %/°C, Ki = 0, Kd = 0"
        );
    }

    #[test]
    fn an_outcome_is_met_missed_by_its_shortfall_or_without_a_prediction_for_its_reason() {
        let outcome_of = |outcome: Value| {
            let mut result = tuned();
            result["outcome"] = outcome;
            outcome_line(&result)
        };
        assert_eq!(outcome_of(json!({ "status": "meets" })), "Outcome: Meets");
        assert_eq!(
            outcome_of(json!({
                "status": "misses",
                "shortfall": "Settling time 340 s is above the 300 s limit.",
            })),
            "Outcome: Misses: Settling time 340 s is above the 300 s limit."
        );
        assert_eq!(
            outcome_of(json!({
                "status": "unpredicted",
                "reason": "The law has no prediction.",
            })),
            "Outcome: No prediction: The law has no prediction."
        );
    }

    #[test]
    fn a_margin_is_its_number_infinity_or_not_observed_in_range() {
        let margins_of = |margins: Value, delay: Value| {
            let mut result = tuned();
            result["margins"] = margins;
            result["delayMarginSeconds"] = delay;
            margins_line(&result)
        };
        assert_eq!(
            margins_of(json!({
                "gainMarginDb": "unbounded",
                "phaseMarginDegrees": { "finite": 71.5 },
                "peakSensitivity": 1.2,
                "peakSensitivityRadPerSecond": 0.1,
            }), json!({ "finite": 1.5 })),
            "Margins: gain margin ∞, phase margin 71.5 degrees, delay margin 1.5 s, peak sensitivity 1.2"
        );
        assert_eq!(
            margins_of(json!({
                "gainMarginDb": "not-observed-in-range",
                "phaseMarginDegrees": "not-observed-in-range",
                "peakSensitivity": 1.2,
                "peakSensitivityRadPerSecond": 0.1,
                "unavailable": "The grid is too coarse for the pure delay.",
            }), json!("not-observed-in-range")),
            "Margins: gain margin not observed in range, phase margin not observed in range, delay margin not observed in range, peak sensitivity 1.2. The grid is too coarse for the pure delay."
        );
    }

    #[test]
    fn a_result_without_margins_or_prediction_gives_the_reasons_it_states() {
        let mut result = tuned();
        result["outcome"] = json!({
            "status": "unpredicted",
            "reason": "The law has no prediction.",
        });
        result["margins"] = Value::Null;
        result["marginsUnavailable"] = json!("A switching law shows no margins.");
        result["settlingTimeSeconds"] = Value::Null;
        result["overshootPercent"] = Value::Null;
        result["predictionUnavailable"] = json!("The law has no prediction.");
        assert_eq!(
            tune_text(&result),
            [
                "Structure: PI. Kp = 2.5 %/°C, Ki = 0.31 %/(°C·s), Kd = 0",
                "Method: SIMC PI with the more robust response, λ = 49 s.",
                "Reason: PI is the simplest structure that meets every requirement.",
                "Outcome: No prediction: The law has no prediction.",
                "Margins: not available. A switching law shows no margins.",
                "Prediction: not available. The law has no prediction.",
                NOT_APPLIED_TEXT,
            ]
            .join("\n")
        );
    }

    #[test]
    fn the_method_line_falls_back_to_the_title_and_a_missing_field_reads_as_a_dash() {
        let mut result = tuned();
        result["methodDescription"] = Value::Null;
        result["methodTitle"] = json!("Automatic");
        assert_eq!(tune_text(&result).lines().nth(1), Some("Method: Automatic"));
        let dashes = [
            "Structure: —. Kp = —, Ki = —, Kd = —",
            "Method: —",
            "Reason: —",
            "Outcome: —",
            "Margins: —",
            "Predicted settling time: —, overshoot: —",
            NOT_APPLIED_TEXT,
        ]
        .join("\n");
        assert_eq!(tune_text(&json!({})), dashes);
        assert_eq!(tune_text(&Value::Null), dashes);
    }
}
