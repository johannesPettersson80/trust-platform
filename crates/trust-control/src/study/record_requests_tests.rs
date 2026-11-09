//! Tests of the requests a study keeps for its live recordings (specification 35, B3.6): Fit refits any live recording the study
//! keeps, each from the request it was taken with.

use super::*;
use crate::recording::EvidenceSource;
use crate::study::identify::tests::{id_of, provisional, record_request, recording_file};

/// The request of the runtime recording `runtime_recording_id`, with a scheduling value.
fn kept(runtime_recording_id: &str, scheduling_value: Option<f64>) -> KeptRequest {
    KeptRequest {
        record_request: record_request(runtime_recording_id),
        scheduling_value,
    }
}

/// B3.6: the study keeps the request of each live recording, so each is fitted again from its own: the Identify result's
/// recording `a` from the request its section keeps, the earlier recording `b` from the one the study kept for it. A recording the
/// study keeps no request for is refused with the sentence of B3.6. Wrong behaviour caught: only the Identify result's recording
/// fitted again (`b` refused, as before), and `b` fitted with `a`'s request.
#[test]
fn every_kept_live_recording_is_fitted_again_from_its_own_request() {
    let file_a = recording_file(EvidenceSource::SimulatedPlant, 20.0);
    let file_b = recording_file(EvidenceSource::SimulatedPlant, 30.0);
    let (a, b) = (id_of(&file_a), id_of(&file_b));
    let mut study = Study::new("loop:tank", "rev-1");
    keep_record_request(&mut study, &b, kept(&file_b.run.recording_id, Some(41.5))).unwrap();
    let section =
        provisional(&file_a).with_record_request(record_request(&file_a.run.recording_id));
    study.set_section(&section).unwrap();
    keep_record_request(&mut study, &a, kept(&file_a.run.recording_id, None)).unwrap();

    assert_eq!(
        refit_request(&study, &a),
        Ok(kept(&file_a.run.recording_id, None))
    );
    assert_eq!(
        refit_request(&study, &b),
        Ok(kept(&file_b.run.recording_id, Some(41.5)))
    );
    let other = id_of(&recording_file(EvidenceSource::SimulatedPlant, 40.0));
    let refusal = refit_request(&study, &other).unwrap_err();
    assert_eq!(refusal, refit_refusal(&other));
    assert_eq!(
        refusal.message,
        format!("Recording {other} cannot be fitted again: the study keeps no experiment request for it.")
    );
    assert_eq!(
        study.recordings,
        [b.clone(), a.clone()],
        "the study lists both recordings"
    );
}

/// B3.6: a study written before it kept requests still refits its Identify result from the request the section keeps, with the
/// section's scheduling value, and refuses its other recordings. Wrong behaviour caught: an older study whose Identify result
/// can no longer be fitted again.
#[test]
fn a_study_written_before_the_requests_refits_its_identify_result_from_its_section() {
    let file_a = recording_file(EvidenceSource::SimulatedPlant, 20.0);
    let a = id_of(&file_a);
    let mut study = Study::new("loop:tank", "rev-1");
    let section = provisional(&file_a)
        .with_record_request(record_request(&file_a.run.recording_id))
        .with_scheduling_value(Some(12.0));
    study.set_section(&section).unwrap();
    assert_eq!(study.section::<RecordRequests>(), Ok(None));
    assert_eq!(
        refit_request(&study, &a),
        Ok(kept(&file_a.run.recording_id, Some(12.0)))
    );
    let b = id_of(&recording_file(EvidenceSource::SimulatedPlant, 30.0));
    assert_eq!(refit_request(&study, &b), Err(refit_refusal(&b)));
}

/// A kept request fits only the file of the runtime recording it was taken with (B10.3). A request taken with runtime recording
/// `run-elsewhere` is refused for a file that holds another, and the refusal names both. Wrong behaviour caught: a request applied
/// to another recording's samples.
#[test]
fn a_kept_request_fits_only_the_file_of_its_own_runtime_recording() {
    let file = recording_file(EvidenceSource::SimulatedPlant, 20.0);
    let id = id_of(&file);
    assert_eq!(
        kept(&file.run.recording_id, None).check_recording(&id, &file),
        Ok(())
    );
    let refusal = kept("run-elsewhere", None)
        .check_recording(&id, &file)
        .unwrap_err();
    assert_eq!(
        refusal.message,
        format!(
            "The study's record requests cannot be used: the request of recording {id} was taken with runtime recording \
             run-elsewhere, but its file holds runtime recording {}.",
            file.run.recording_id
        )
    );
}

/// The section is kept under `recordRequests`, round-trips through the study's document, and refuses a key that is no
/// recording's identifier. Wrong behaviour caught: a section the study cannot read back, and a request kept for no recording.
#[test]
fn the_requests_round_trip_under_their_key_and_name_only_recordings() {
    let file_a = recording_file(EvidenceSource::SimulatedPlant, 20.0);
    let a = id_of(&file_a);
    let mut study = Study::new("loop:tank", "rev-1");
    keep_record_request(&mut study, &a, kept(&file_a.run.recording_id, None)).unwrap();
    assert!(study.sections.contains_key("recordRequests"));
    let decoded = Study::decode(&study.encode().unwrap(), "loop:tank").unwrap();
    assert_eq!(
        decoded.section::<RecordRequests>(),
        study.section::<RecordRequests>()
    );
    assert_eq!(
        refit_request(&decoded, &a),
        Ok(kept(&file_a.run.recording_id, None))
    );

    let mut bad = RecordRequests::default();
    bad.0
        .insert("rec-1".into(), kept(&file_a.run.recording_id, None));
    assert!(bad.check().is_err(), "rec-1 is no recording's identifier");
}
