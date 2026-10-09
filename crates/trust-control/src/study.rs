//! The study a loop keeps in its project (specification 35, A8): `control-studio/<loop id>/study.json`,
//! and one file per recording under `recordings/`.
//!
//! The study is a versioned container. Each stage keeps its own section, defined by that stage,
//! and the whole study stays within the storage limit of A7. Only this format is read: a study of
//! another format is refused, and the engineer starts a new one (nothing earlier was released).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::failure::{Cause, Failure, NextAction};
use crate::limits::STUDY_STORAGE_LIMIT_BYTES;

/// The study format this crate reads and writes.
pub const STUDY_FORMAT: u32 = 1;

/// One loop's study.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Study {
    /// Always [`STUDY_FORMAT`].
    pub format: u32,
    /// The loop the study belongs to.
    pub loop_id: String,
    /// The loop revision the sections were made for.
    pub loop_revision: String,
    /// Each stage's section, by the stage's name (for example `identify`), as that stage defines it.
    #[serde(default)]
    pub sections: BTreeMap<String, Value>,
    /// The identifiers of the recordings under `recordings/` that the sections refer to.
    #[serde(default)]
    pub recordings: Vec<String>,
}

impl Study {
    /// An empty study of the loop.
    #[must_use]
    pub fn new(loop_id: &str, loop_revision: &str) -> Self {
        Self {
            format: STUDY_FORMAT,
            loop_id: loop_id.to_string(),
            loop_revision: loop_revision.to_string(),
            sections: BTreeMap::new(),
            recordings: Vec::new(),
        }
    }

    /// The study as `study.json` holds it: canonical JSON, refused when over the storage limit.
    ///
    /// # Errors
    /// The study is larger than the storage limit of A7.
    pub fn encode(&self) -> Result<String, Failure> {
        let value =
            serde_json::to_value(self).map_err(|error| storage_failure(&error.to_string()))?;
        let text = canonical_json(&value);
        check_size(text.len() as u64)?;
        Ok(text)
    }

    /// Reads `study.json` of the loop `loop_id`.
    ///
    /// # Errors
    /// The text is not a study of this format, belongs to another loop, or is over the limit.
    pub fn decode(text: &str, loop_id: &str) -> Result<Self, Failure> {
        check_size(text.len() as u64)?;
        if !has_current_format(text) {
            return Err(Failure::new(
                "This study was saved in a format Control Studio no longer reads. Start a new study of the loop.",
                Cause::Settings,
                NextAction::None,
            ));
        }
        let study: Self =
            serde_json::from_str(text).map_err(|error| storage_failure(&error.to_string()))?;
        if study.loop_id != loop_id {
            return Err(storage_failure(&format!(
                "the study belongs to loop {}, not {loop_id}",
                study.loop_id
            )));
        }
        Ok(study)
    }
}

/// Whether `text` is a study document of [`STUDY_FORMAT`]; a study of an earlier format moves to
/// the loop's history (A8.3).
#[must_use]
pub fn has_current_format(text: &str) -> bool {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|value| value.get("format").and_then(Value::as_u64))
        == Some(u64::from(STUDY_FORMAT))
}

/// Refuses a study, or study and recordings together, over the storage limit of A7.
///
/// # Errors
/// `bytes` is larger than the limit.
pub fn check_size(bytes: u64) -> Result<(), Failure> {
    if bytes > STUDY_STORAGE_LIMIT_BYTES {
        return Err(Failure::new(
            &format!(
                "The study needs {} MiB; a loop keeps at most {} MiB in its project. Remove recordings it no longer needs.",
                bytes.div_ceil(1024 * 1024),
                STUDY_STORAGE_LIMIT_BYTES / (1024 * 1024)
            ),
            Cause::Settings,
            NextAction::None,
        ));
    }
    Ok(())
}

/// Deterministic JSON text for evidence equality: object keys sorted at every level, arrays in
/// order, numbers as serde writes them.
#[must_use]
pub fn canonical_json(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by_key(|(left, _)| *left);
            let body: Vec<String> = entries
                .into_iter()
                .map(|(key, item)| {
                    format!("{}:{}", Value::String(key.clone()), canonical_json(item))
                })
                .collect();
            format!("{{{}}}", body.join(","))
        }
        Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(canonical_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        other => other.to_string(),
    }
}

fn storage_failure(reason: &str) -> Failure {
    Failure::new(
        &format!("The study file cannot be read: {reason}."),
        Cause::Settings,
        NextAction::None,
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_study_round_trips_through_its_canonical_text() {
        let mut study = Study::new("loop:tank", "sha256:abc");
        study.sections.insert(
            "identify".into(),
            json!({ "model": { "gain": 1.5, "kind": "first-order" } }),
        );
        study.recordings.push("recording-1".into());
        let text = study.encode().expect("encoded");
        assert_eq!(Study::decode(&text, "loop:tank").expect("decoded"), study);
        assert_eq!(
            study.encode().expect("again"),
            text,
            "the same study gives the same text"
        );
    }

    #[test]
    fn canonical_json_sorts_keys_at_every_level_and_keeps_array_order() {
        let value = json!({ "b": [3, { "z": 1, "a": 2 }], "a": "x" });
        assert_eq!(canonical_json(&value), r#"{"a":"x","b":[3,{"a":2,"z":1}]}"#);
    }

    #[test]
    fn a_study_of_another_format_or_another_loop_is_refused() {
        let other = r#"{"format":0,"loopId":"loop:tank","loopRevision":"sha256:abc"}"#;
        assert!(Study::decode(other, "loop:tank")
            .expect_err("refused")
            .message
            .contains("no longer reads"));
        let earlier = r#"{"schemaVersion":1,"campaigns":[]}"#;
        assert!(Study::decode(earlier, "loop:tank").is_err());
        let text = Study::new("loop:other", "sha256:abc")
            .encode()
            .expect("encoded");
        assert!(Study::decode(&text, "loop:tank")
            .expect_err("refused")
            .message
            .contains("loop:other"));
    }

    #[test]
    fn a_study_over_the_storage_limit_is_refused() {
        assert!(check_size(STUDY_STORAGE_LIMIT_BYTES).is_ok());
        let failure = check_size(STUDY_STORAGE_LIMIT_BYTES + 1).expect_err("refused");
        assert!(failure.message.contains("257 MiB"), "{}", failure.message);
    }
}
