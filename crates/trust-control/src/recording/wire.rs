//! The compact samples of an isolated run (specification 35, A7) and their expansion into the per-channel form
//! (`recordedRunWire.ts`).
//!
//! A sample's `values` holds one number or null per channel, in the run's `channels` order. Its `quality`
//! holds one code per channel, an index into [`QUALITY_NAMES`], and is absent when every channel is good. A
//! missing value has no measurement, so it cannot be good. The runtime writes the same vocabulary in its worker
//! frames (`control/search/worker_checkpoint.rs`).

use serde_json::{json, Map, Value};

use crate::failure::{Cause, Failure, NextAction};

/// The quality vocabulary of the wire format, in code order (the runtime's `QUALITY_NAMES`).
const QUALITY_NAMES: [&str; 4] = ["good", "bad", "forced", "missing"];

/// Expands a run's compact samples into the per-channel form: each sample's `values` becomes an object keyed by
/// channel identifier, with `{value, quality}` for each channel. Every other field passes through unchanged. A
/// run whose `channels` or `samples` are not arrays is returned unchanged, so the run's decoder reports it.
///
/// # Errors
/// A channel has no string identifier, or a sample does not match the channels, the quality codes or the rule for
/// missing values.
pub fn expand_recorded_run_wire(value: &Value) -> Result<Value, Failure> {
    let Some(run) = value.as_object() else {
        return Ok(value.clone());
    };
    let (Some(channels), Some(samples)) = (
        run.get("channels").and_then(Value::as_array),
        run.get("samples").and_then(Value::as_array),
    ) else {
        return Ok(value.clone());
    };
    let ids = channels
        .iter()
        .map(channel_id)
        .collect::<Result<Vec<&str>, Failure>>()?;
    let expanded = samples
        .iter()
        .map(|sample| expand_sample(sample, &ids))
        .collect::<Result<Vec<Value>, Failure>>()?;
    let mut out = run.clone();
    out.insert("samples".to_string(), Value::Array(expanded));
    Ok(Value::Object(out))
}

fn channel_id(channel: &Value) -> Result<&str, Failure> {
    channel
        .as_object()
        .and_then(|object| object.get("id"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            Failure::new(
                "Invalid recorded-run channels.",
                Cause::Runtime,
                NextAction::None,
            )
        })
}

fn expand_sample(sample: &Value, ids: &[&str]) -> Result<Value, Failure> {
    let object = sample.as_object().ok_or_else(invalid_sample)?;
    let values = object
        .get("values")
        .and_then(Value::as_array)
        .ok_or_else(invalid_sample)?;
    let codes = match object.get("quality") {
        None => None,
        Some(Value::Array(codes)) if codes.len() == ids.len() => Some(codes),
        Some(_) => return Err(invalid_sample()),
    };
    if values.len() != ids.len() {
        return Err(invalid_sample());
    }
    let mut expanded = Map::new();
    for (index, id) in ids.iter().enumerate() {
        let value = values.get(index).ok_or_else(invalid_sample)?;
        let code = match codes {
            None => 0,
            Some(codes) => {
                let code = codes.get(index).ok_or_else(invalid_sample)?;
                quality_code(code).ok_or_else(invalid_sample)?
            }
        };
        if !(value.is_null() || value.is_number()) || (value.is_null() && code == 0) {
            return Err(invalid_sample());
        }
        expanded.insert(
            (*id).to_string(),
            json!({ "value": value, "quality": QUALITY_NAMES[code] }),
        );
    }
    let mut rest = object.clone();
    rest.remove("values");
    rest.remove("quality");
    rest.insert("values".to_string(), Value::Object(expanded));
    Ok(Value::Object(rest))
}

/// The index of a quality code: a whole number from 0 to 3 (the four names of [`QUALITY_NAMES`]).
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Checked to be a whole number in 0..4.
fn quality_code(value: &Value) -> Option<usize> {
    let code = value.as_f64()?;
    if code.fract().abs() > 0.0 || !(0.0..4.0).contains(&code) {
        return None;
    }
    Some(code as usize)
}

fn invalid_sample() -> Failure {
    Failure::new(
        "Invalid recorded-run sample encoding.",
        Cause::Runtime,
        NextAction::None,
    )
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::expand_recorded_run_wire;

    fn channels() -> Value {
        json!([{ "id": "PV", "unit": "degC" }, { "id": "Mode", "unit": "1" }])
    }

    #[test]
    fn compact_samples_expand_into_the_per_channel_shape_exactly() {
        let compact = json!({
            "id": "run-1",
            "channels": channels(),
            "samples": [
                { "sequence": 0, "timeSeconds": 0, "values": [1.5, null], "quality": [0, 2] },
                { "sequence": 1, "timeSeconds": 1, "values": [-2.5, 3] }
            ]
        });
        let expected = json!({
            "id": "run-1",
            "channels": channels(),
            "samples": [
                {
                    "sequence": 0,
                    "timeSeconds": 0,
                    "values": {
                        "PV": { "value": 1.5, "quality": "good" },
                        "Mode": { "value": null, "quality": "forced" }
                    }
                },
                {
                    "sequence": 1,
                    "timeSeconds": 1,
                    "values": {
                        "PV": { "value": -2.5, "quality": "good" },
                        "Mode": { "value": 3, "quality": "good" }
                    }
                }
            ]
        });
        assert_eq!(
            expand_recorded_run_wire(&compact).expect("the compact run expands"),
            expected
        );
    }

    #[test]
    fn a_run_without_array_channels_or_samples_passes_through_unchanged() {
        let run = json!({ "channels": "not a list", "samples": [] });
        assert_eq!(expand_recorded_run_wire(&run).expect("passes through"), run);
    }

    #[test]
    fn compact_samples_that_disagree_with_their_channels_are_refused() {
        let refused = [
            json!([{ "sequence": 0, "timeSeconds": 0, "values": [1] }]),
            json!([{ "sequence": 0, "timeSeconds": 0, "values": [1, 2], "quality": [0] }]),
            json!([{ "sequence": 0, "timeSeconds": 0, "values": [1, 2], "quality": [0, 4] }]),
            json!([{ "sequence": 0, "timeSeconds": 0, "values": [null, 2] }]),
            json!([{ "sequence": 0, "timeSeconds": 0, "values": [1, "2"] }]),
            json!([{
                "sequence": 0,
                "timeSeconds": 0,
                "values": { "PV": { "value": 1, "quality": "good" }, "Mode": { "value": 2, "quality": "good" } }
            }]),
            json!(["not a sample"]),
        ];
        for samples in refused {
            let run = json!({ "channels": channels(), "samples": samples });
            let failure = expand_recorded_run_wire(&run).expect_err("the sample is refused");
            assert_eq!(failure.message, "Invalid recorded-run sample encoding.");
        }
    }

    #[test]
    fn a_channel_without_a_string_identifier_is_refused() {
        let run = json!({ "channels": [{ "unit": "degC" }], "samples": [] });
        let failure = expand_recorded_run_wire(&run).expect_err("the channel is refused");
        assert_eq!(failure.message, "Invalid recorded-run channels.");
    }
}
