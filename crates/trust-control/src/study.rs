//! The study a loop keeps in its project (specification 35, A8): `control-studio/<loop id>/study.json`,
//! and one file per recording under `recordings/`.
//!
//! The study is a versioned container. Each stage keeps its own section, defined by that stage
//! (a [`StudySection`]: [`identify`], [`general_model_persistence`] and [`tune`]), and the whole study stays
//! within the storage limit of A7. Only this format is read: a study of another format is refused, and
//! the engineer starts a new one (nothing earlier was released).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::failure::{Cause, Failure, NextAction};
use crate::limits::STUDY_STORAGE_LIMIT_BYTES;
use crate::messages::{AnalysisPage, PageView};
use crate::model::{decode_general_model, GeneralModel};

pub mod analysis_records;
pub mod apply;
pub mod current_model;
pub mod general_model_persistence;
pub mod identify;
pub mod inputs;
pub mod manual_identification;
pub mod run_plan;
pub mod tune;
pub mod validate;

/// The study format this crate reads and writes.
pub const STUDY_FORMAT: u32 = 1;

/// One of the three pages of Control Studio (specification 35, §0.2). The study keeps the page last viewed (A8.1), and
/// reopening shows it (A8.6). The words are the names of the pages, as the stages are named.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub enum Page {
    /// The Identify page: where a study opens (A8.6).
    #[default]
    Identify,
    /// The Tune page.
    Tune,
    /// The Validate page.
    Validate,
}

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
    /// The page the engineer last viewed (A8.1). A study stored before it was recorded opens at Identify.
    #[serde(default)]
    pub page: Page,
    /// The analysis tool the Tune page shows in place of the tuning result, with its inputs (§0.1a, `controlStudio/page`; A8.1). Absent
    /// when no analysis is shown, and in a study stored before it was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<AnalysisPage>,
    /// The page's own view choices, kept as the page names them (§0.1a, `controlStudio/page`; A8.1): reopening shows them again.
    /// Absent when the page sent none, and in a study stored before it was recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<PageView>,
    /// The source of the model the study kept last (A8.4): `Identify` after a fit, `Supplied` after `importModel` (B3.7). Absent when
    /// neither was kept, and in a study stored before it was recorded; the current model is then the one section the study holds
    /// ([`current_model::current_model`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_source: Option<current_model::CurrentModelSource>,
}

impl Study {
    /// An empty study of the loop, which opens at the Identify page (A8.6).
    #[must_use]
    pub fn new(loop_id: &str, loop_revision: &str) -> Self {
        Self {
            format: STUDY_FORMAT,
            loop_id: loop_id.to_string(),
            loop_revision: loop_revision.to_string(),
            sections: BTreeMap::new(),
            recordings: Vec::new(),
            page: Page::Identify,
            analysis: None,
            view: None,
            model_source: None,
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
            crate::json::decode_text(text).map_err(|error| storage_failure(&error.to_string()))?;
        if study.loop_id != loop_id {
            return Err(storage_failure(&format!(
                "the study belongs to loop {}, not {loop_id}",
                study.loop_id
            )));
        }
        Ok(study)
    }

    /// The section `S` of the study, checked, or `None` when the study has none. A section that breaks one of its
    /// rules, or that refers to a recording the study does not list, is not read (B10.4: a record is accepted whole or
    /// not at all).
    ///
    /// # Errors
    /// The stored section is not an `S`, breaks a rule of `S`, or refers to a recording the study does not list.
    pub fn section<S: StudySection>(&self) -> Result<Option<S>, Failure> {
        let Some(value) = self.sections.get(S::KEY) else {
            return Ok(None);
        };
        let section: S = crate::json::decode_value(value.clone())
            .map_err(|error| section_failure(S::KEY, &error.to_string()))?;
        section.check()?;
        if let Some(missing) = section
            .recording_ids()
            .into_iter()
            .find(|id| !self.lists(id))
        {
            return Err(section_failure(
                S::KEY,
                &format!("it refers to recording {missing}, which the study does not list"),
            ));
        }
        Ok(Some(section))
    }

    /// Keeps `section` as the study's section `S`, replacing an earlier one, and lists the recordings it refers to.
    /// Nothing is changed when the section breaks a rule.
    ///
    /// # Errors
    /// The section breaks a rule of `S`.
    pub fn set_section<S: StudySection>(&mut self, section: &S) -> Result<(), Failure> {
        section.check()?;
        let value = serde_json::to_value(section)
            .map_err(|error| section_failure(S::KEY, &error.to_string()))?;
        for id in section.recording_ids() {
            if !self.lists(id) {
                self.recordings.push(id.to_string());
            }
        }
        self.sections.insert(S::KEY.to_string(), value);
        Ok(())
    }

    /// Whether the study lists the recording `id`.
    fn lists(&self, id: &str) -> bool {
        self.recordings.iter().any(|listed| listed == id)
    }
}

/// One stage's section of the study (A8.1). A stage defines its section type; the study keeps it under
/// [`StudySection::KEY`] and reads it back only when every rule of the type holds. Defaults give each section its
/// document form: canonical JSON, within the storage limit of A7.
#[allow(clippy::module_name_repetitions)] // The section is the study's own concept.
pub trait StudySection: Serialize + DeserializeOwned + Sized {
    /// The key of the section in [`Study::sections`]: the name of its stage.
    const KEY: &'static str;

    /// Every rule the section's content must satisfy (B10.4). A section that breaks one is never kept and never read.
    ///
    /// # Errors
    /// The failure names the rule that is broken.
    fn check(&self) -> Result<(), Failure>;

    /// The recordings of the study that the section refers to, by their identifiers (A8.3).
    fn recording_ids(&self) -> Vec<&str>;

    /// The section as a document: checked, canonical JSON, refused when larger than the storage limit of A7.
    ///
    /// # Errors
    /// The section breaks a rule, or its document is larger than the storage limit of A7.
    fn encode(&self) -> Result<String, Failure> {
        self.check()?;
        let value = serde_json::to_value(self)
            .map_err(|error| section_failure(Self::KEY, &error.to_string()))?;
        let text = canonical_json(&value);
        check_size(text.len() as u64)?;
        Ok(text)
    }

    /// Reads a document written by [`StudySection::encode`].
    ///
    /// # Errors
    /// The text is larger than the storage limit of A7, is not this section, or breaks a rule of the section.
    fn decode(text: &str) -> Result<Self, Failure> {
        check_size(text.len() as u64)?;
        let section: Self = crate::json::decode_text(text)
            .map_err(|error| section_failure(Self::KEY, &error.to_string()))?;
        section.check()?;
        Ok(section)
    }
}

/// Whether `id` is the identifier of a recording file: `rec-` and 32 lowercase hexadecimal digits
/// ([`crate::recording::recording_id_for`]). Sections name recordings by it, and it names a file, so nothing else is
/// ever used as a path.
#[must_use]
pub fn is_recording_id(id: &str) -> bool {
    id.strip_prefix("rec-").is_some_and(|digits| {
        digits.len() == 32
            && digits
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    })
}

/// Whether `model` satisfies the model contract: the rules of [`decode_general_model`]. A number JSON cannot carry (not
/// a number, or infinite) fails it.
pub(crate) fn is_valid_model(model: &GeneralModel) -> bool {
    serde_json::to_value(model).is_ok_and(|value| decode_general_model(&value).is_some())
}

/// Whether `text` is a study document of [`STUDY_FORMAT`]; a study of an earlier format moves to
/// the loop's history (A8.3).
#[must_use]
pub fn has_current_format(text: &str) -> bool {
    crate::json::parse_text(text)
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

fn section_failure(key: &str, reason: &str) -> Failure {
    Failure::new(
        &format!("The {key} section of the study cannot be used: {reason}."),
        Cause::ModelOrData,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "study/analysis_page_tests.rs"]
mod analysis_page_tests;

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
    fn a_new_study_opens_at_identify_and_keeps_the_page_last_viewed() {
        let mut study = Study::new("loop:tank", "sha256:abc");
        assert_eq!(
            study.page,
            Page::Identify,
            "a new study opens at Identify (A8.6)"
        );
        study.page = Page::Validate;
        let text = study.encode().expect("encoded");
        assert_eq!(
            Study::decode(&text, "loop:tank").expect("decoded").page,
            Page::Validate
        );
    }

    #[test]
    fn a_study_stored_before_the_page_was_recorded_opens_at_identify() {
        let stored = r#"{"format":1,"loopId":"loop:tank","loopRevision":"sha256:abc"}"#;
        let study = Study::decode(stored, "loop:tank").expect("decoded");
        assert_eq!(study.page, Page::Identify);
        assert_eq!(
            serde_json::to_value(Page::Tune).expect("the word"),
            json!("Tune"),
            "a page is written as its name"
        );
        assert!(
            serde_json::from_value::<Page>(json!("Monitor")).is_err(),
            "Monitor is a stage, not a page"
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

    /// A section of these tests: it refers to one recording, and its size must not be zero.
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Example {
        recording: String,
        size: u32,
    }

    impl StudySection for Example {
        const KEY: &'static str = "example";

        fn check(&self) -> Result<(), Failure> {
            if self.size == 0 {
                return Err(section_failure(Self::KEY, "the size is zero"));
            }
            Ok(())
        }

        fn recording_ids(&self) -> Vec<&str> {
            vec![self.recording.as_str()]
        }
    }

    fn example(size: u32) -> Example {
        Example {
            recording: format!("rec-{}", "0".repeat(32)),
            size,
        }
    }

    #[test]
    fn a_recording_identifier_is_rec_and_32_lowercase_hexadecimal_digits() {
        assert!(is_recording_id("rec-00112233445566778899aabbccddeeff"));
        for id in [
            "",
            "rec-",
            "rec-0011",
            "rec-00112233445566778899AABBCCDDEEFF",
            "rec-00112233445566778899aabbccddeefg",
            "../rec-00112233445566778899aabbccddeeff",
            "rec-00112233445566778899aabbccddeeff.json",
            "rec-00112233445566778899aabbccddeeff0",
        ] {
            assert!(!is_recording_id(id), "{id:?} is not a recording identifier");
        }
        assert!(
            is_recording_id(&crate::recording::recording_id_for("any text")),
            "the identifiers recording files get"
        );
    }

    #[test]
    fn a_section_is_kept_with_its_recording_listed_once_and_read_back_whole() {
        let mut study = Study::new("loop:tank", "sha256:abc");
        assert_eq!(study.section::<Example>().expect("no section"), None);
        study.set_section(&example(3)).expect("kept");
        study.set_section(&example(4)).expect("replaced");
        assert_eq!(
            study.recordings,
            vec![example(1).recording],
            "a recording is listed once"
        );
        assert_eq!(study.section::<Example>().expect("read"), Some(example(4)));
        let text = study.encode().expect("encoded");
        let reopened = Study::decode(&text, "loop:tank").expect("decoded");
        assert_eq!(
            reopened.section::<Example>().expect("read"),
            Some(example(4))
        );
        assert_eq!(
            reopened.encode().expect("again"),
            text,
            "the same study gives the same text"
        );
    }

    #[test]
    fn a_section_that_breaks_a_rule_or_names_an_unlisted_recording_is_neither_kept_nor_read() {
        let mut study = Study::new("loop:tank", "sha256:abc");
        let refused = study.set_section(&example(0)).expect_err("refused");
        assert!(
            refused.message.contains("size is zero"),
            "{}",
            refused.message
        );
        assert_eq!(study.sections, BTreeMap::new(), "nothing was kept");
        assert_eq!(study.recordings, Vec::<String>::new());

        study.set_section(&example(2)).expect("kept");
        study.recordings.clear();
        let unlisted = study.section::<Example>().expect_err("refused");
        assert!(
            unlisted.message.contains("does not list"),
            "{}",
            unlisted.message
        );

        let mut stored = Study::new("loop:tank", "sha256:abc");
        stored.recordings.push(example(1).recording);
        stored.sections.insert(
            "example".into(),
            json!({ "recording": example(1).recording, "size": 0 }),
        );
        let broken = stored.section::<Example>().expect_err("refused");
        assert!(
            broken.message.contains("size is zero"),
            "{}",
            broken.message
        );
        stored
            .sections
            .insert("example".into(), json!({ "other": 1 }));
        assert!(
            stored.section::<Example>().is_err(),
            "a document of another shape is not this section"
        );
    }

    #[test]
    fn a_section_document_is_canonical_checked_and_refused_over_the_storage_limit() {
        let section = example(5);
        let text = section.encode().expect("encoded");
        assert_eq!(
            text,
            format!(r#"{{"recording":"{}","size":5}}"#, section.recording)
        );
        assert_eq!(Example::decode(&text).expect("decoded"), section);
        let zero = text.replace(r#""size":5"#, r#""size":0"#);
        assert!(Example::decode(&zero).is_err(), "a rule of the section");
        let extra = text.replace(r#""size":5"#, r#""size":5,"extra":true"#);
        assert!(Example::decode(&extra).is_err(), "an unknown field");

        // Zero bytes are valid text; the allocation is not touched until it is read.
        let limit = usize::try_from(STUDY_STORAGE_LIMIT_BYTES).expect("addressable");
        let oversize = String::from_utf8(vec![0_u8; limit + 1]).expect("NUL is UTF-8");
        let failure = Example::decode(&oversize).expect_err("over the limit");
        assert!(failure.message.contains("257 MiB"), "{}", failure.message);
    }
}
