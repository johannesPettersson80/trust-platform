//! The progress sentences of Restore previous settings (specification 35, B6.4, A8.5, A11.1): what a Restore shows at each of its
//! steps while it runs, with the values involved. Its refusals and its result are trust-control's (`commissioning::checkpoint_restore`
//! and its `texts`, `commissioning::texts`, `commissioning::application_texts`); none of those is repeated here.

use std::collections::BTreeMap;

use trust_control::tuning::texts::{engineering, unit_suffix};

/// The cancel of the operations the commissioning checkpoint owns, by their identities, before anything is written (E6.10, E7.6, E11.4).
#[must_use]
pub fn cancelling(parameter_operation: &str, transfer: Option<&str>) -> String {
    match transfer {
        Some(transfer) => format!(
            "Ending the operations of the commissioning checkpoint by their identities: parameter operation {parameter_operation} and transfer {transfer}."
        ),
        None => format!(
            "Ending the parameter operation {parameter_operation} of the commissioning checkpoint by its identity."
        ),
    }
}

/// The transfer that returns the loop to Manual at its current output, with the references Apply found (B6.4, A6.2): the mode signal
/// and each reference with its value.
#[must_use]
pub fn to_manual(mode_symbol: &str, references: &BTreeMap<String, f64>) -> String {
    format!(
        "Returning the loop to Manual ({mode_symbol}) at its current output, with the references Apply found: {}.",
        values_text(references)
    )
}

/// The rollback of the parameter record to the revision and the values Apply found (E7.5).
#[must_use]
pub fn rolling_back(revision: &str, values: &BTreeMap<String, f64>) -> String {
    format!(
        "Rolling the parameter record back to revision {revision}: {}.",
        values_text(values)
    )
}

/// The stop of the recording the checkpoint still holds, whose samples are kept before its buffer is released (E4.7, E11.6).
#[must_use]
pub fn stopping_recording(recording_id: &str) -> String {
    format!(
        "Stopping recording {recording_id}, which the commissioning checkpoint holds, and keeping its samples."
    )
}

/// The transfer that returns the loop to Auto, the mode Apply found it in, with the largest first output change it allows (A6.2,
/// A6.4): `tolerance`, plus `entry_error_gain` times the error the loop enters with when the entry has that step (annex E6.9).
#[must_use]
pub fn to_auto(
    mode_symbol: &str,
    tolerance: f64,
    entry_error_gain: Option<f64>,
    output_unit: &str,
) -> String {
    let step = entry_error_gain.map_or_else(String::new, |gain| {
        format!(
            ", plus {} times the error it enters Auto with",
            engineering(gain, None)
        )
    });
    format!(
        "Returning the loop to Auto ({mode_symbol}), the mode Apply found it in, with a first output change of at most {}{}{step}.",
        engineering(tolerance, None),
        unit_suffix(output_unit)
    )
}

/// The reference step that returns the references Apply found, after the loop is back in Auto (B6.4; C13 step 8).
#[must_use]
pub fn to_references(references: &BTreeMap<String, f64>) -> String {
    format!(
        "Returning the references Apply found: {}.",
        values_text(references)
    )
}

/// Values as a progress sentence names them: each symbol with its value, comma-separated.
fn values_text(values: &BTreeMap<String, f64>) -> String {
    values
        .iter()
        .map(|(symbol, value)| format!("{symbol} {}", engineering(*value, None)))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::to_auto;

    /// Annex E6.9: a return to Auto whose allowance has the integral step names that step beside the tolerance Studio sends,
    /// since the runtime adds it for the error the loop enters with; a return without it names the tolerance alone. Catches a
    /// text that states only the tolerance sent while the runtime allows more.
    #[test]
    fn the_return_to_auto_names_the_step_on_the_error_it_enters_with() {
        assert_eq!(
            to_auto("P1.Manual", 0.25, Some(0.01), "%"),
            "Returning the loop to Auto (P1.Manual), the mode Apply found it in, with a first output change of at most \
             0.25 %, plus 0.01 times the error it enters Auto with."
        );
        assert_eq!(
            to_auto("P1.Manual", 0.25, None, "%"),
            "Returning the loop to Auto (P1.Manual), the mode Apply found it in, with a first output change of at most \
             0.25 %."
        );
    }
}
