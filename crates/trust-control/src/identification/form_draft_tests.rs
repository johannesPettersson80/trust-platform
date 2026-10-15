//! Tests of the form draft decoder. The first follows `control-studio-identification-form-draft.test.ts` ("round-trips
//! raw blank fields and rejects malformed form schema"); the others pin the decoder's key lists and its length limit.

use serde_json::json;

use super::*;

/// The draft of the `TypeScript` tests: FOPDT with the held step on, every field "5" except a blank "delay.minimum".
fn form() -> IdentificationFormDraft {
    IdentificationFormDraft {
        schema_version: 1,
        model: IdentificationModelSelection::Fopdt,
        held_step: true,
        values: IDENTIFICATION_FORM_KEYS
            .iter()
            .map(|key| {
                let text = if *key == "delay.minimum" { "" } else { "5" };
                ((*key).to_string(), text.to_string())
            })
            .collect(),
        excluded_start: String::new(),
        excluded_end: String::new(),
    }
}

fn encoded(draft: &IdentificationFormDraft) -> Value {
    serde_json::to_value(draft).expect("the draft encodes")
}

#[test]
fn round_trips_raw_blank_fields_and_rejects_malformed_form_schema() {
    let draft = form();
    assert_eq!(
        decode_identification_form_draft(&encoded(&draft)),
        Some(draft.clone())
    );
    let mut version = encoded(&draft);
    version["schemaVersion"] = json!(2);
    assert_eq!(decode_identification_form_draft(&version), None);
    let mut number = encoded(&draft);
    number["values"]["delay.step"] = json!(0.1);
    assert_eq!(decode_identification_form_draft(&number), None);
}

#[test]
fn the_draft_keeps_the_field_names_of_the_form() {
    let value = encoded(&form());
    assert_eq!(value["schemaVersion"], json!(1));
    assert_eq!(value["model"], json!("fopdt"));
    assert_eq!(value["heldStep"], json!(true));
    assert_eq!(value["values"]["delay.minimum"], json!(""));
    assert_eq!(value["excludedStart"], json!(""));
    assert_eq!(value["excludedEnd"], json!(""));
}

#[test]
fn comparison_fields_are_kept_and_unknown_or_missing_fields_are_refused() {
    let mut comparison = encoded(&form());
    comparison["values"]["sensorTimeConstant.step"] = json!("0.5");
    let decoded =
        decode_identification_form_draft(&comparison).expect("a comparison field is kept");
    assert_eq!(decoded.values["sensorTimeConstant.step"], "0.5");

    let mut unknown_field = encoded(&form());
    unknown_field["values"]["delay.width"] = json!("1");
    assert_eq!(decode_identification_form_draft(&unknown_field), None);

    let mut missing = encoded(&form());
    missing["values"]
        .as_object_mut()
        .expect("the values are an object")
        .remove("timeConstant.step");
    assert_eq!(decode_identification_form_draft(&missing), None);

    let mut unknown_key = encoded(&form());
    unknown_key["structureChoice"] = json!("first_order");
    assert_eq!(decode_identification_form_draft(&unknown_key), None);

    let mut model = encoded(&form());
    model["model"] = json!("state_space");
    assert_eq!(decode_identification_form_draft(&model), None);
}

#[test]
fn a_text_may_hold_256_utf16_code_units_and_no_more() {
    let mut longest = encoded(&form());
    longest["values"]["gainStep"] = json!("é".repeat(256));
    longest["excludedEnd"] = json!("1".repeat(256));
    assert!(
        decode_identification_form_draft(&longest).is_some(),
        "256 code units fit, although they are 512 bytes"
    );
    let mut value = encoded(&form());
    value["values"]["gainStep"] = json!("1".repeat(257));
    assert_eq!(decode_identification_form_draft(&value), None);
    let mut start = encoded(&form());
    start["excludedStart"] = json!("1".repeat(257));
    assert_eq!(decode_identification_form_draft(&start), None);
}

#[test]
#[allow(clippy::float_cmp)] // The page's numbers are read exactly as typed; 0 and 5 are exact.
fn the_held_step_settings_read_the_page_numbers_and_refuse_the_first_problem_in_the_page_order() {
    let settings = held_step_settings_of(&form())
        .expect("the settings are admissible")
        .expect("the held step is on");
    assert_eq!(settings.onset_samples, 5);
    assert_eq!(settings.delay.minimum, 0.0);
    assert_eq!(settings.time_constant.minimum, 5.0);
    let mut fractional = form();
    fractional
        .values
        .insert("onsetSamples".to_string(), "2.5".to_string());
    let refusal =
        held_step_settings_of(&fractional).expect_err("a fractional onset count is refused");
    assert_eq!(
        refusal.message,
        "onsetSamples: Enter a whole number from 1 to 100."
    );
    assert_eq!(refusal.field.as_deref(), Some("onsetSamples"));
    let mut off = form();
    off.held_step = false;
    assert!(matches!(held_step_settings_of(&off), Ok(None)));
}

#[test]
fn the_sensor_lag_fields_are_read_and_checked_only_for_sopdt() {
    // The sopdt comparison reads its sensor-time grid and residual requirements (`readStepResponseSettings`); the other forms carry none.
    let mut sopdt = form();
    sopdt.model = IdentificationModelSelection::Sopdt;
    let refusal = held_step_settings_of(&sopdt)
        .expect_err("a blank sensor-time minimum is refused for sopdt");
    assert_eq!(refusal.field.as_deref(), Some("sensorTimeConstant.minimum"));
    let settings = held_step_settings_of(&form())
        .expect("the settings are admissible without the comparison")
        .expect("the held step is on");
    assert_eq!(settings.sensor_time_constant, None);
    assert_eq!(settings.model_adequacy, None);
}

#[test]
#[allow(clippy::float_cmp)] // The interval's ends are the numbers typed, read exactly.
fn the_excluded_interval_reads_one_interval_and_refuses_one_that_does_not_end_after_it_starts() {
    let mut draft = form();
    draft.held_step = false;
    assert_eq!(
        excluded_intervals_of(&draft).expect("no interval is typed"),
        [] as [ExcludedInterval; 0]
    );
    draft.excluded_start = "2".to_string();
    draft.excluded_end = "5".to_string();
    assert_eq!(
        excluded_intervals_of(&draft).expect("one interval"),
        vec![ExcludedInterval {
            start_seconds: 2.0,
            end_seconds: 5.0,
        }]
    );
    draft.excluded_end = "1".to_string();
    let refusal = excluded_intervals_of(&draft).expect_err("an end before the start is refused");
    assert_eq!(
        refusal.message,
        "Enter an exclusion interval with a nonnegative start and an end strictly after it."
    );
    assert_eq!(refusal.field.as_deref(), Some("excludedEnd"));
    draft.excluded_start = "-1".to_string();
    draft.excluded_end = "5".to_string();
    let start = excluded_intervals_of(&draft).expect_err("a negative start is refused");
    assert_eq!(start.field.as_deref(), Some("excludedStart"));
}

#[test]
fn a_blank_form_starts_with_the_automatic_model_and_no_held_step() {
    let blank = blank_form();
    assert_eq!(blank.model, IdentificationModelSelection::Automatic);
    assert!(!blank.held_step);
    assert!(valid_form_draft(&blank));
    assert_eq!(
        excluded_intervals_of(&blank).expect("no interval is typed"),
        [] as [ExcludedInterval; 0]
    );
}
