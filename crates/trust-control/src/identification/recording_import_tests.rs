//! Tests of the rules that read a saved recording for identification (`identificationRecordingImport.ts`).

use serde_json::json;

use super::{
    check_import, check_saved_acquisition, check_saved_provenance, display_engineering_unit,
    import_failure_reply, route_import_message, ImportCheck, ImportRoute, ImportScope,
    SavedProvenance, SavedRecordingData, REFERENCE_SENTENCE,
};
use crate::identification::refusal::Refusal;
use crate::identification::saved_result::{INVALID_SENTENCE, NOT_JSON_SENTENCE};
use crate::identification::state::RecordingProvenance;
use crate::recording::{Acquisition, Completeness, ScanRange};

fn scope() -> ImportScope {
    ImportScope {
        project_id: "project".to_owned(),
        loop_id: "loop:tank".to_owned(),
        editor_session_id: "session".to_owned(),
        loop_revision: "sha256:loop".to_owned(),
    }
}

fn acquisition(first: u64, last: u64, count: u64) -> Acquisition {
    Acquisition {
        method: "runtime_scan_buffer".to_owned(),
        completeness: Completeness::Complete,
        observed_sample_count: count,
        scan_range: Some(ScanRange { first, last }),
        missing_scan_ranges: Vec::new(),
        dropped_observed_samples: 0,
        actual_cycle_timing_available: true,
        freshness_available: false,
        reason: "Complete recording.".to_owned(),
    }
}

const SENTENCE: &str =
    "Identify requires a complete recording of 2–100000 runtime samples at a steady record period.";

#[test]
fn display_units_read_as_a_student_reads_them() {
    assert_eq!(display_engineering_unit("degC"), "°C");
    assert_eq!(display_engineering_unit("%/degC"), "%/°C");
    assert_eq!(display_engineering_unit("pp/s"), "percentage points/s");
    assert_eq!(display_engineering_unit("%/(degC*s)"), "%/(°C·s)");
    assert_eq!(
        display_engineering_unit("  degC  "),
        "°C",
        "the text is trimmed first"
    );
    assert_eq!(
        display_engineering_unit("xdegCx"),
        "xdegCx",
        "only whole words change"
    );
    assert_eq!(display_engineering_unit("   "), "");
}

#[test]
fn a_complete_steady_scan_buffer_is_an_import() {
    let data = SavedRecordingData {
        acquisition: Some(acquisition(1, 5, 5)),
        scans: vec![1, 2, 3, 4, 5],
    };
    assert!(check_saved_acquisition(&data).is_ok());
}

#[test]
fn an_incomplete_or_irregular_recording_is_refused_with_the_sentence() {
    let mut missing = acquisition(1, 5, 5);
    missing.missing_scan_ranges = vec![ScanRange { first: 3, last: 3 }];
    let refused = check_saved_acquisition(&SavedRecordingData {
        acquisition: Some(missing),
        scans: vec![1, 2, 4, 5],
    })
    .expect_err("a missing scan is refused");
    assert_eq!(refused.0, SENTENCE);
    let irregular = check_saved_acquisition(&SavedRecordingData {
        acquisition: Some(acquisition(1, 4, 4)),
        scans: vec![1, 2, 4, 5],
    })
    .expect_err("the scans must step by one period");
    assert_eq!(irregular.0, SENTENCE);
    let none = check_saved_acquisition(&SavedRecordingData {
        acquisition: None,
        scans: vec![1, 2],
    })
    .expect_err("an acquisition is required");
    assert_eq!(none.0, SENTENCE);
    let single = check_saved_acquisition(&SavedRecordingData {
        acquisition: Some(acquisition(1, 1, 1)),
        scans: vec![1],
    })
    .expect_err("two samples at least");
    assert_eq!(single.0, SENTENCE);
}

#[test]
fn provenance_refuses_an_unfinished_experiment_and_other_units() {
    let loop_definition = crate::recording::fixtures::loop_definition();
    let provenance = |status: Option<&str>, input: &str, output: &str| SavedProvenance {
        identity: RecordingProvenance {
            project_id: "project".to_owned(),
            result_id: "result".to_owned(),
            result_revision: 3,
        },
        input_unit: input.to_owned(),
        output_unit: output.to_owned(),
        experiment_status: status.map(str::to_owned),
    };
    let fixture_input = loop_definition.signals.manipulated_variable.unit.clone();
    let fixture_output = loop_definition.signals.process_variable.unit.clone();
    assert!(check_saved_provenance(
        &provenance(Some("completed"), &fixture_input, &fixture_output),
        &loop_definition
    )
    .is_ok());
    let aborted = check_saved_provenance(
        &provenance(Some("aborted"), &fixture_input, &fixture_output),
        &loop_definition,
    );
    assert_eq!(
        aborted.err().map(|refusal| refusal.0),
        Some("An aborted or failed experiment cannot supply an Identify recording.".to_owned())
    );
    let other = check_saved_provenance(&provenance(None, &fixture_input, "degF"), &loop_definition);
    assert_eq!(
        other.err().map(|refusal| refusal.0),
        Some("Recording input and output units must match this loop; no unit conversion was applied.".to_owned())
    );
}

#[test]
fn only_the_scoped_file_import_is_taken() {
    let import = json!({
        "schemaVersion": 1, "kind": "control-studio.identify.import", "projectId": "project", "loopId": "loop:tank",
        "editorSessionId": "session", "expectedLoopRevision": "sha256:loop", "messageId": "m1",
        "payload": { "json": "{}" }
    });
    assert_eq!(
        route_import_message(&import, &scope()),
        ImportRoute::Import {
            message_id: "m1".to_owned(),
            json: "{}".to_owned()
        }
    );
    let mut other_loop = import.clone();
    other_loop["loopId"] = json!("loop:other");
    assert_eq!(
        route_import_message(&other_loop, &scope()),
        ImportRoute::Ignored
    );
    let mut extra = import.clone();
    extra["payload"]["extra"] = json!(true);
    assert_eq!(
        route_import_message(&extra, &scope()),
        ImportRoute::Ignored,
        "the payload holds only the file text"
    );
    let unrelated = json!({ "kind": "control-studio.something-else" });
    assert_eq!(
        route_import_message(&unrelated, &scope()),
        ImportRoute::NotAnImport
    );
}

#[test]
fn a_refused_import_replies_with_the_failed_status_and_its_scope() {
    let reply = import_failure_reply(
        &scope(),
        "m1",
        "Stop the active recording before importing another recording.",
    );
    assert_eq!(reply["kind"], "control-studio.identification-recording");
    assert_eq!(reply["messageId"], "m1");
    assert_eq!(reply["payload"]["status"], "failed");
    assert_eq!(
        reply["payload"]["reason"],
        "Stop the active recording before importing another recording."
    );
    assert_eq!(reply["expectedLoopRevision"], "sha256:loop");
}

/// The acquisition of a complete runtime scan buffer of `count` samples from scan 1 (`recordingAcquisition`).
fn acquisition_json(count: u64) -> serde_json::Value {
    json!({
        "method": "runtime_scan_buffer",
        "completeness": "complete",
        "observedSampleCount": count,
        "scanRange": { "first": 1, "last": count },
        "missingScanRanges": [],
        "droppedObservedSamples": 0,
        "actualCycleTimingAvailable": true,
        "freshnessAvailable": false,
        "reason": "Complete recording."
    })
}

/// One raw sample at scan `scan` (`WorkflowResultSample`) of a closed loop in `%`, one second after the one before it.
fn sample_json(scan: u32) -> serde_json::Value {
    let seconds = f64::from(scan - 1);
    json!({
        "elapsedSeconds": seconds,
        "observedAtMs": 1000.0 * seconds + 1000.0,
        "actualCycleSeconds": 1.0,
        "scan": scan,
        "source": "Simulation",
        "loopRevision": "sha256:loop-1",
        "sp": 50.0,
        "pv": 50.0,
        "requestedMv": 40.0,
        "appliedMv": 40.0,
        "units": { "sp": "%", "pv": "%", "mv": "%" },
        "quality": "good",
        "freshnessMs": null,
        "controllerMode": "Auto",
        "outputLimits": { "minimum": 0.0, "maximum": 100.0 },
        "safetyState": "Normal",
        "events": []
    })
}

/// A complete, steady schema 4 result of the fixture loop: five runtime samples, scans 1 to 5 (B10.2, B10.4).
fn saved_result() -> serde_json::Value {
    let samples: Vec<serde_json::Value> = (1_u32..=5).map(sample_json).collect();
    json!({
        "schemaVersion": 4,
        "resultId": "result-1",
        "resultRevision": 3,
        "createdAt": "2026-10-10T12:00:00.000Z",
        "projectId": "project:fixture",
        "source": "Simulation",
        "loop": { "id": "loop:tank", "revision": "sha256:loop-1" },
        "controller": { "instance": "TankController", "revision": "sha256:controller-1" },
        "software": {
            "extensionVersion": "0.25.0",
            "numericalAdapters": { "trust-control-math": "1.0.0" }
        },
        "units": { "sp": "%", "pv": "%", "mv": "%" },
        "recording": {
            "rawSamples": samples,
            "selectedIntervals": [{ "startSeconds": 0.0, "endSeconds": 4.0 }],
            "acquisition": acquisition_json(5)
        },
        "evidence": { "observation": "Observed", "model": "Not evaluated" },
        "conclusion": "Recorded for identification."
    })
}

/// The provenance of a recording copied from another result of the fixture loop (`WorkflowRecordingProvenance`, schema 1).
fn provenance_json() -> serde_json::Value {
    json!({
        "schemaVersion": 1,
        "resultId": "result-0",
        "resultRevision": 1,
        "createdAt": "2026-10-09T08:30:00Z",
        "projectId": "project:fixture",
        "source": "Simulation",
        "loop": { "id": "loop:tank", "revision": "sha256:loop-1" },
        "controller": { "instance": "TankController", "revision": "sha256:controller-1" },
        "software": {
            "extensionVersion": "0.24.0",
            "numericalAdapters": { "trust-control-math": "1.0.0" }
        },
        "units": { "sp": "%", "pv": "%", "mv": "%" }
    })
}

/// The result as the import reads it, checked against the fixture loop.
fn check(value: &serde_json::Value) -> Result<ImportCheck, Refusal> {
    check_import(
        &value.to_string(),
        &crate::recording::fixtures::loop_definition(),
    )
}

/// The sentence a refused result gives.
fn refused(value: &serde_json::Value) -> String {
    check(value).expect_err("the result is refused").0
}

#[test]
fn a_complete_steady_result_of_the_loop_is_checked() {
    let answer = check(&saved_result()).expect("a complete steady result is checked");
    assert_eq!(answer.status, "checked");
    assert_eq!(answer.result_id, "result-1");
    assert_eq!(answer.result_revision, 3);
    assert_eq!(answer.project_id, "project:fixture");
    assert_eq!(answer.source, "Simulation");
    assert_eq!(answer.samples, 5);
}

#[test]
fn text_that_is_not_json_is_refused_with_its_sentence() {
    let refusal =
        check_import("{", &crate::recording::fixtures::loop_definition()).expect_err("not JSON");
    assert_eq!(refusal.0, NOT_JSON_SENTENCE);
}

#[test]
fn a_result_that_does_not_decode_is_refused_as_a_whole() {
    let mut other_schema = saved_result();
    other_schema["schemaVersion"] = json!(2);
    assert_eq!(
        refused(&other_schema),
        INVALID_SENTENCE,
        "only schemas 3 and 4 are read"
    );

    let mut empty_id = saved_result();
    empty_id["resultId"] = json!("");
    assert_eq!(refused(&empty_id), INVALID_SENTENCE);

    let mut no_revision = saved_result();
    no_revision["resultRevision"] = json!(0);
    assert_eq!(refused(&no_revision), INVALID_SENTENCE);

    let mut loop_revision = saved_result();
    loop_revision["loop"]["revision"] = json!("loop-1");
    assert_eq!(
        refused(&loop_revision),
        INVALID_SENTENCE,
        "a revision is a sha256 digest"
    );

    let mut bad_source = saved_result();
    bad_source["source"] = json!("Virtual");
    assert_eq!(refused(&bad_source), INVALID_SENTENCE);
}

#[test]
fn a_creation_time_must_be_an_iso_time_with_a_zone() {
    for stamp in [
        "2026-10-10 12:00:00Z",
        "2026-13-10T12:00:00Z",
        "2026-10-10T12:00:00",
        "2026-10-10T12:00:00.Z",
    ] {
        let mut value = saved_result();
        value["createdAt"] = json!(stamp);
        assert_eq!(refused(&value), INVALID_SENTENCE, "{stamp}");
    }
    let mut offset = saved_result();
    offset["createdAt"] = json!("2026-10-10T12:00:00+02:00");
    assert!(check(&offset).is_ok(), "an offset is a zone");
}

#[test]
fn a_credential_key_anywhere_in_the_result_refuses_it() {
    let mut with_token = saved_result();
    with_token["software"]["authToken"] = json!("not-for-a-file");
    assert_eq!(refused(&with_token), INVALID_SENTENCE);
}

#[test]
fn samples_out_of_order_or_missing_a_value_refuse_the_result() {
    let mut repeated_scan = saved_result();
    repeated_scan["recording"]["rawSamples"][2]["scan"] = json!(2);
    assert_eq!(
        refused(&repeated_scan),
        INVALID_SENTENCE,
        "scans increase strictly"
    );

    let mut earlier_time = saved_result();
    earlier_time["recording"]["rawSamples"][3]["elapsedSeconds"] = json!(0.5);
    assert_eq!(
        refused(&earlier_time),
        INVALID_SENTENCE,
        "elapsed time increases strictly"
    );

    let mut missing_pv = saved_result();
    missing_pv["recording"]["rawSamples"][0]
        .as_object_mut()
        .expect("a sample")
        .remove("pv");
    assert_eq!(
        refused(&missing_pv),
        INVALID_SENTENCE,
        "a required value must be present, though it may be null"
    );

    let mut null_pv = saved_result();
    null_pv["recording"]["rawSamples"][0]["pv"] = json!(null);
    assert!(
        check(&null_pv).is_ok(),
        "a null is a value the sample holds"
    );
}

#[test]
fn a_sample_of_other_units_refuses_the_result() {
    let mut other_units = saved_result();
    other_units["recording"]["rawSamples"][1]["units"]["pv"] = json!("degF");
    assert_eq!(refused(&other_units), INVALID_SENTENCE);
}

#[test]
fn an_aborted_experiment_cannot_supply_a_recording() {
    let mut aborted = saved_result();
    aborted["experiment"] = json!({ "status": "aborted" });
    assert_eq!(
        refused(&aborted),
        "An aborted or failed experiment cannot supply an Identify recording."
    );
}

#[test]
fn a_recording_in_other_units_than_the_loop_is_refused_with_no_conversion() {
    let mut other_units = saved_result();
    other_units["units"] = json!({ "sp": "degF", "pv": "degF", "mv": "degF" });
    for sample in other_units["recording"]["rawSamples"]
        .as_array_mut()
        .expect("samples")
    {
        sample["units"] = json!({ "sp": "degF", "pv": "degF", "mv": "degF" });
    }
    assert_eq!(
        refused(&other_units),
        "Recording input and output units must match this loop; no unit conversion was applied."
    );
}

#[test]
fn an_incomplete_acquisition_is_refused_with_the_identify_sentence() {
    let mut incomplete = saved_result();
    incomplete["recording"]["acquisition"]["completeness"] = json!("incomplete");
    assert_eq!(refused(&incomplete), SENTENCE);
}

#[test]
fn a_step_response_reference_is_refused_until_the_engine_reads_it() {
    let mut reference = saved_result();
    reference["identificationStudy"] = json!({
        "estimation": { "requestedKind": "fopdt", "stepResponseSettings": {}, "recording": {} }
    });
    assert_eq!(refused(&reference), REFERENCE_SENTENCE);
}

#[test]
fn an_open_loop_of_schema_four_has_an_empty_setpoint_and_no_setpoint_samples() {
    let mut open = saved_result();
    open["loop"]["topology"] = json!("open-loop");
    open["units"]["sp"] = json!("");
    for sample in open["recording"]["rawSamples"]
        .as_array_mut()
        .expect("samples")
    {
        sample["sp"] = json!(null);
        sample["units"]["sp"] = json!("");
    }
    assert!(check(&open).is_ok(), "an open loop carries no setpoint");

    let mut with_setpoint = open.clone();
    with_setpoint["recording"]["rawSamples"][0]["sp"] = json!(50.0);
    assert_eq!(refused(&with_setpoint), INVALID_SENTENCE);
}

#[test]
fn a_recording_copied_from_another_result_is_read_on_schema_four_only() {
    let mut copied = saved_result();
    copied["recording"]["provenance"] = provenance_json();
    assert!(check(&copied).is_ok(), "a schema 4 copy keeps its origin");

    let mut schema_three = saved_result();
    schema_three["schemaVersion"] = json!(3);
    assert!(check(&schema_three).is_ok(), "a schema 3 result is read");
    schema_three["recording"]["provenance"] = provenance_json();
    assert_eq!(
        refused(&schema_three),
        INVALID_SENTENCE,
        "provenance is schema 4"
    );
}

#[test]
fn a_copy_whose_origin_has_other_units_is_refused() {
    let mut copied = saved_result();
    let mut origin = provenance_json();
    origin["units"]["pv"] = json!("degF");
    copied["recording"]["provenance"] = origin;
    assert_eq!(refused(&copied), INVALID_SENTENCE);
}

#[test]
fn a_sample_of_another_evidence_source_than_the_result_or_before_the_origin_refuses_the_result() {
    // control-studio-validate-apply: "rejects mixed evidence sources and non-monotonic saved samples" (B10.2: simulated evidence is never
    // loaded as physical evidence; B10.4: a record with inconsistent sample order is rejected as a whole).
    let mut mixed = saved_result();
    mixed["recording"]["rawSamples"][1]["source"] = json!("Physical");
    assert_eq!(
        refused(&mixed),
        INVALID_SENTENCE,
        "a simulation result must not hold a physical sample"
    );
    let mut reversed = saved_result();
    reversed["recording"]["rawSamples"][1]["elapsedSeconds"] = json!(-0.1);
    assert_eq!(
        refused(&reversed),
        INVALID_SENTENCE,
        "saved samples stay in order of elapsed time, which starts at zero"
    );
}

#[test]
fn the_plant_of_a_result_keeps_its_evidence_source_and_an_experiment_ends_in_a_known_state() {
    // control-studio-validate-apply: "retains validated experiment and plant context without changing evidence class" (B10.2: simulated
    // plant evidence is never loaded as physical plant evidence). The import reads the plant's source and the experiment's status; the
    // plant's configuration and the experiment's restoration are not read by the engine.
    let mut contextual = saved_result();
    contextual["experiment"] = json!({
        "kind": "setpoint_step", "requestedValue": 1, "appliedValue": 1, "status": "completed",
        "stopReason": "duration_complete", "aborted": false,
        "restoration": { "value": 0, "status": "confirmed" }
    });
    contextual["plant"] = json!({
        "source": "Simulation", "id": "tank", "configurationRevision": "sha256:revision-1"
    });
    assert!(
        check(&contextual).is_ok(),
        "the context of a simulation result with a simulated plant and a completed experiment"
    );

    let mut physical_plant = contextual.clone();
    physical_plant["plant"]["source"] = json!("Physical");
    assert_eq!(
        refused(&physical_plant),
        INVALID_SENTENCE,
        "a simulation result must not carry a physical plant"
    );

    let mut unknown_end = contextual.clone();
    unknown_end["experiment"]["status"] = json!("unknown");
    assert_eq!(
        refused(&unknown_end),
        INVALID_SENTENCE,
        "an experiment ends completed, aborted, failed or failed to restore"
    );
}
