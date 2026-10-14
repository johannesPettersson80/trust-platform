//! The readable text of the reviewed verbs `apply`, `restore` and `keep` (specification 35, §0.1b). Each function reads the fields of its
//! answer and holds no rule: whether a result succeeded, whether Restore or Keep is offered, and what a refusal means all come from the
//! answer itself. The texts of `output` serve the other verbs; these are kept apart so that each file stays small.

use serde_json::Value;

/// What a text prints for a field that its answer lacks.
const MISSING: &str = "—";

/// The last line of a review (§0.1b): a review writes nothing, neither to the controller nor to the project.
const REVIEW_WRITES_NOTHING: &str = "Nothing was written.";

/// The text of a review of Apply or Restore (§0.1b, B6.1, B6.4): the change's description, the mode the loop was found in when it is
/// known, each parameter's running and next value, and the identity that the confirmation gives back with `--review`.
#[must_use]
pub fn application_review_text(review: &Value) -> String {
    let mut lines = vec![text_field(review, "description")
        .unwrap_or(MISSING)
        .to_string()];
    if let Some(mode) = text_field(review, "foundMode") {
        lines.push(format!("The loop was found in {mode} mode."));
    }
    lines.extend(items(review, "parameters").map(|parameter| {
        let unit =
            text_field(parameter, "unit").map_or_else(String::new, |unit| format!(" {unit}"));
        format!(
            "  {}: running {}{unit}, next {}{unit}.",
            text_field(parameter, "symbol").unwrap_or(MISSING),
            quantity(parameter, "running"),
            quantity(parameter, "next"),
        )
    }));
    if flag(review, "physical") {
        lines.push("This is a physical controller.".to_string());
    }
    lines.push(format!(
        "Review identity: {}.",
        text_field(review, "identity").unwrap_or(MISSING)
    ));
    lines.push(REVIEW_WRITES_NOTHING.to_string());
    lines.push(
        "To confirm this exact change, run the same command with --review <identity>.".to_string(),
    );
    lines.join("\n")
}

/// The text of a finished Apply (§0.1b, B6.3): its status and reason, whether the previous settings were restored after a failure
/// (B6.7), and which of Restore and Keep in project is offered next (B6.4, B6.5).
#[must_use]
pub fn apply_text(outcome: &Value) -> String {
    let result = outcome.get("result").unwrap_or(&Value::Null);
    let mut lines = vec![format!(
        "Apply {}: {}",
        text_field(result, "status").unwrap_or(MISSING),
        text_field(result, "reason").unwrap_or(MISSING),
    )];
    if flag(outcome, "restored") {
        lines.push("The previous settings were restored after the failure.".to_string());
    }
    if flag(outcome, "restoreAvailable") {
        lines.push("Restore previous settings is available: run restore with --review to return the previous settings.".to_string());
    }
    if flag(outcome, "keepAvailable") {
        lines.push("Keep in project is available: run keep with --review to make these values the project's.".to_string());
    }
    lines.join("\n")
}

/// The text of a finished Restore (§0.1b, B6.4): "Restored" only when the runtime confirmed the previous settings, else "Not restored",
/// with the reason and the mode the loop was left in.
#[must_use]
pub fn restore_text(outcome: &Value) -> String {
    let reason = text_field(outcome, "reason").unwrap_or(MISSING);
    let mut lines = vec![if flag(outcome, "restored") {
        format!("Restored: {reason}")
    } else {
        format!("Not restored: {reason}")
    }];
    if let Some(mode) = text_field(outcome, "mode") {
        lines.push(format!("The loop is in {mode} mode."));
    }
    lines.join("\n")
}

/// The text of a review of Keep in project (§0.1b, B6.5): the record, the values it would keep, the fields without a setting, the files
/// it would change and the identity that the confirmation gives back.
#[must_use]
pub fn keep_review_text(review: &Value) -> String {
    let record = text_field(review, "record").unwrap_or(MISSING);
    let values = review.get("values").unwrap_or(&Value::Null);
    let mut lines = vec![format!(
        "Keep in project of {record}: the running values become the record's initial values in the project."
    )];
    for field in strings(review, "kept") {
        lines.push(format!("  {field}: {}", quantity(values, field)));
    }
    let unknown: Vec<&str> = strings(review, "unknown").collect();
    if !unknown.is_empty() {
        lines.push(format!("No setting for {} is kept.", unknown.join(", ")));
    }
    let files: Vec<&str> = items(review, "files")
        .filter_map(|file| text_field(file, "path"))
        .collect();
    if !files.is_empty() {
        lines.push(format!("Files to change: {}.", files.join(", ")));
    }
    lines.push(format!(
        "Review identity: {}.",
        text_field(review, "identity").unwrap_or(MISSING)
    ));
    lines.push(REVIEW_WRITES_NOTHING.to_string());
    lines.push(
        "To confirm this exact edit, run the same command with --review <identity>.".to_string(),
    );
    lines.join("\n")
}

/// The text of a finished Keep in project (§0.1b, B6.5): the sentence the engine gives for the values it kept, the fields without a
/// setting, or the project's values that already held them (`keep_texts::kept_reason`).
#[must_use]
pub fn keep_text(outcome: &Value) -> String {
    text_field(outcome, "reason").unwrap_or(MISSING).to_string()
}

/// A number field as text, or a dash when the answer lacks it.
fn quantity(object: &Value, field: &str) -> String {
    object
        .get(field)
        .and_then(Value::as_f64)
        .map_or_else(|| MISSING.to_string(), |value| value.to_string())
}

fn flag(object: &Value, field: &str) -> bool {
    object.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn text_field<'a>(object: &'a Value, field: &str) -> Option<&'a str> {
    object.get(field).and_then(Value::as_str)
}

/// The items of the array in field `field` of a JSON object; none when the object lacks the array.
fn items<'a>(object: &'a Value, field: &str) -> impl Iterator<Item = &'a Value> {
    object
        .get(field)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

/// The strings of the array in field `field` of a JSON object.
fn strings<'a>(object: &'a Value, field: &str) -> impl Iterator<Item = &'a str> {
    items(object, field).filter_map(Value::as_str)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_review_names_its_identity_and_says_that_nothing_is_written() {
        let text = application_review_text(&json!({
            "description": "Apply the validated candidate to Main.TIC101.",
            "foundMode": "auto",
            "parameters": [{ "symbol": "Main.Kp", "unit": "%/degC", "running": 1.5, "next": 2.25 }],
            "physical": false,
            "identity": "sha256:the-review",
        }));
        assert!(text.contains("found in auto mode"), "{text}");
        assert!(
            text.contains("Main.Kp: running 1.5 %/degC, next 2.25 %/degC."),
            "{text}"
        );
        assert!(
            text.contains("Review identity: sha256:the-review."),
            "{text}"
        );
        assert!(text.contains("Nothing was written."), "{text}");
        assert!(text.contains("--review <identity>"), "{text}");
    }

    #[test]
    fn an_applied_result_says_what_restore_and_keep_offer() {
        let offered = apply_text(&json!({
            "result": { "status": "passed", "reason": "The response met every requirement." },
            "restored": false,
            "restoreAvailable": true,
            "keepAvailable": true,
        }));
        assert!(
            offered.starts_with("Apply passed: The response met every requirement."),
            "{offered}"
        );
        assert!(
            offered.contains("Restore previous settings is available"),
            "{offered}"
        );
        assert!(
            offered.contains("Keep in project is available"),
            "{offered}"
        );
        let failed = apply_text(&json!({
            "result": { "status": "failed", "reason": "A requirement failed." },
            "restored": true,
            "restoreAvailable": false,
            "keepAvailable": false,
        }));
        assert!(
            failed.contains("previous settings were restored after the failure"),
            "{failed}"
        );
        assert!(!failed.contains("Keep in project is available"), "{failed}");
    }

    #[test]
    fn a_restore_says_restored_only_when_the_runtime_confirmed_it() {
        let confirmed = restore_text(&json!({
            "restored": true,
            "reason": "The previous settings were confirmed back.",
            "mode": "auto",
        }));
        assert!(
            confirmed.starts_with("Restored: The previous settings were confirmed back."),
            "{confirmed}"
        );
        assert!(
            confirmed.contains("The loop is in auto mode."),
            "{confirmed}"
        );
        let unconfirmed = restore_text(&json!({
            "restored": false,
            "reason": "The runtime has not confirmed the settings.",
        }));
        assert!(unconfirmed.starts_with("Not restored:"), "{unconfirmed}");
        assert!(!unconfirmed.contains("Restored:"), "{unconfirmed}");
    }

    #[test]
    fn a_keep_names_its_record_its_fields_and_the_fields_without_a_setting() {
        let review = keep_review_text(&json!({
            "record": "Main.TIC101_Parameters",
            "values": { "Kp": 2.5, "Ki": 0.1 },
            "kept": ["Kp", "Ki"],
            "unknown": ["Kd"],
            "files": [{ "path": "/project/Main.st", "before": "a", "after": "b" }],
            "identity": "sha256:keep",
        }));
        assert!(review.contains("Kp: 2.5"), "{review}");
        assert!(review.contains("No setting for Kd is kept."), "{review}");
        assert!(
            review.contains("Files to change: /project/Main.st."),
            "{review}"
        );
        assert!(review.contains("Review identity: sha256:keep."), "{review}");
        let kept = keep_text(&json!({
            "record": "Main.TIC101_Parameters",
            "kept": ["Kp", "Ki"],
            "unknown": ["Kd"],
            "reason": "Kept in the project: Kp, Ki of Main.TIC101_Parameters. A restart or a new download keeps them. The equipment has no setting for Kd; copy it into the PLC code by hand.",
        }));
        assert_eq!(
            kept,
            "Kept in the project: Kp, Ki of Main.TIC101_Parameters. A restart or a new download keeps them. The equipment has no setting for Kd; copy it into the PLC code by hand."
        );
    }
}
