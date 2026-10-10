//! Tests of the rules that read a saved recording for identification (`identificationRecordingImport.ts`).

use serde_json::json;

use super::{
    check_saved_acquisition, check_saved_provenance, display_engineering_unit,
    import_failure_reply, route_import_message, ImportRoute, ImportScope, SavedProvenance,
    SavedRecordingData,
};
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
