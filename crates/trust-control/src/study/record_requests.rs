//! The requests a study's live recordings were taken with (specification 35, B3.6): each recording a live identification kept,
//! with what fitting it again needs and its file does not hold, so that Fit refits any live recording the study keeps, not only
//! the recording of its Identify result.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::failure::{Cause, Failure, NextAction};
use crate::recording::{Completeness, RecordingFile};
use crate::study::identify::{refit_refusal, IdentifySection, StoredRecordRequest};
use crate::study::{is_recording_id, Study, StudySection};

/// The request one live recording was taken with, and the value of the scheduling variable that its run read when it started
/// (C10 Identify, D-21).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeptRequest {
    /// The program revision and the connected request: the reviewed plan and the steady baseline window.
    pub record_request: StoredRecordRequest,
    /// The scheduling variable's value at the start of the run, in its unit; absent when the loop has no schedule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduling_value: Option<f64>,
}

impl KeptRequest {
    /// Checks the recording file `file` against the request: it holds the runtime recording the request was taken with, and it is
    /// complete evidence (B10.3), so that a model can come from it.
    ///
    /// # Errors
    /// The file holds another runtime recording, or it is not complete evidence.
    pub fn check_recording(&self, recording_id: &str, file: &RecordingFile) -> Result<(), Failure> {
        let requested = &self.record_request.request.acquisition.recording_id;
        if *requested != file.run.recording_id {
            return Err(refuse(&format!(
                "the request of recording {recording_id} was taken with runtime recording {requested}, but its file holds runtime recording {}",
                file.run.recording_id
            )));
        }
        if file.evidence.completeness != Completeness::Complete {
            return Err(refuse(&format!(
                "recording {recording_id} is not complete evidence, so no model comes from it (B10.3)"
            )));
        }
        Ok(())
    }
}

/// The study's section `recordRequests`: the request each live recording was taken with, by the recording's identifier.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RecordRequests(pub BTreeMap<String, KeptRequest>);

impl StudySection for RecordRequests {
    const KEY: &'static str = "recordRequests";

    fn check(&self) -> Result<(), Failure> {
        match self.0.keys().find(|id| !is_recording_id(id)) {
            Some(id) => Err(refuse(&format!(
                "{id} is not the identifier of a recording"
            ))),
            None => Ok(()),
        }
    }

    fn recording_ids(&self) -> Vec<&str> {
        self.0.keys().map(String::as_str).collect()
    }
}

/// Keeps in `study` the request that the live recording `recording_id` was taken with (B3.6), replacing an earlier one.
///
/// # Errors
/// The study's requests cannot be read, or `recording_id` is not a recording's identifier.
pub fn keep_record_request(
    study: &mut Study,
    recording_id: &str,
    kept: KeptRequest,
) -> Result<(), Failure> {
    let mut requests = study.section::<RecordRequests>()?.unwrap_or_default();
    requests.0.insert(recording_id.to_string(), kept);
    study.set_section(&requests)
}

/// The request for fitting the recording `recording_id` of `study` again (B3.6): the request its Identify result keeps, when it is
/// that result's recording, or the one the study kept for it.
///
/// # Errors
/// The study keeps no request for the recording ([`refit_refusal`]), or its sections cannot be read.
pub fn refit_request(study: &Study, recording_id: &str) -> Result<KeptRequest, Failure> {
    if let Some(section) = study.section::<IdentifySection>()?
        && let Ok(request) = section.refit_request(recording_id)
    {
        return Ok(KeptRequest {
            record_request: request.clone(),
            scheduling_value: section.scheduling_value,
        });
    }
    study
        .section::<RecordRequests>()?
        .and_then(|requests| requests.0.get(recording_id).cloned())
        .ok_or_else(|| refit_refusal(recording_id))
}

fn refuse(reason: &str) -> Failure {
    Failure::new(
        &format!("The study's record requests cannot be used: {reason}."),
        Cause::ModelOrData,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "record_requests_tests.rs"]
mod tests;
