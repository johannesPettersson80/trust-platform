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
