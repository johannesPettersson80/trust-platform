//! Tests of the export (specification 35, B10.5): the CSV's columns and full-precision numbers, and the JSON export that reads back as
//! the result it wrote, without any key that names a secret.

use serde_json::{json, Value};

use super::*;
use crate::recording::context::{saved_context, RunContext};
use crate::recording::evidence::recording_evidence;
use crate::recording::fixtures::{binding, loop_with_disturbance, run, sample};
use crate::signals::Quality;

/// A recorded run of the fixture loop's setpoint, measurement, manipulated variable and disturbance (`feed-flow`), three samples
/// 10 ms apart. The measurement of the first sample is `0.1 + 0.2`, which only round-trips with its full digits; the disturbance of
/// the second sample is missing.
fn fixture_run() -> RecordedRun {
    let bindings = vec![
        binding("setpoint", "%ML0", "LREAL"),
        binding("processVariable", "%IL0", "LREAL"),
        binding("manipulatedVariable", "%QL0", "LREAL"),
        binding("feed-flow", "%QL8", "LREAL"),
    ];
    let good = [Quality::Good; 4];
    let samples = vec![
        sample(
            1,
            Some(0),
            &[Some(50.0), Some(0.1 + 0.2), Some(12.5), Some(3.0)],
            &good,
        ),
        sample(
            2,
            Some(10_000_000),
            &[Some(50.0), Some(1.0 / 3.0), Some(12.25), None],
            &good,
        ),
        sample(
            3,
            Some(20_000_000),
            &[Some(50.0), Some(7.0), Some(12.0), Some(3.5)],
            &good,
        ),
    ];
    run(bindings, samples)
}

fn csv_rows(text: &str) -> Vec<Vec<String>> {
    text.lines()
        .map(|line| line.split(',').map(str::to_owned).collect())
        .collect()
}

#[test]
fn the_csv_header_lists_the_b10_5_columns_then_five_columns_for_each_additional_channel() {
    let loop_definition = loop_with_disturbance();
    let text = samples_csv(
        &loop_definition,
        EvidenceSource::SimulatedPlant,
        &fixture_run(),
    );
    let header = text.lines().next().expect("the header line");
    let expected = format!(
        "{},additional.feed-flow.value,additional.feed-flow.unit,additional.feed-flow.quality,additional.feed-flow.channel,additional.feed-flow.role",
        CSV_COLUMNS.join(",")
    );
    assert_eq!(header, expected);
    assert_eq!(text.lines().count(), 4, "one header and one row per sample");
    assert!(text.ends_with('\n'));
}

#[test]
fn the_csv_rows_hold_the_saved_values_with_their_units_source_and_missing_cells_empty() {
    let loop_definition = loop_with_disturbance();
    let text = samples_csv(
        &loop_definition,
        EvidenceSource::SimulatedPlant,
        &fixture_run(),
    );
    let rows = csv_rows(&text);
    let first = &rows[1];
    let second = &rows[2];
    assert_eq!(first[0], "0", "the first sample is at the origin");
    assert_eq!(
        first[1], "1001",
        "the observation time is the sample's capture time"
    );
    assert_eq!(first[2], "", "the first sample has no cycle before it");
    assert_eq!(first[3], "50");
    assert_eq!(
        first[7],
        loop_definition
            .signals
            .setpoint
            .as_ref()
            .map_or("", |signal| signal.unit.as_str())
    );
    assert_eq!(first[17], "simulated-plant");
    assert_eq!(
        first[18], "",
        "the engine's recordings keep no event markers"
    );
    assert_eq!(second[0], "0.01", "10 ms of process time is 0.01 s");
    assert_eq!(
        second[2], "0.01",
        "the cycle is the time from the previous sample"
    );
    assert_eq!(second[19], "", "a missing additional value stays missing");
    assert_eq!(
        second[23], "disturbance",
        "the additional channel's role is written"
    );
    assert_eq!(
        second[22], "reactor",
        "the additional channel's channel is written"
    );
}

#[test]
fn the_csv_numbers_read_back_to_the_exact_values_that_were_recorded() {
    let text = samples_csv(
        &loop_with_disturbance(),
        EvidenceSource::PhysicalPlant,
        &fixture_run(),
    );
    let rows = csv_rows(&text);
    let measured: f64 = rows[1][4].parse().expect("the measurement is a number");
    let sum = 0.1_f64 + 0.2_f64;
    assert_eq!(
        measured.to_bits(),
        sum.to_bits(),
        "the first measurement reads back exactly"
    );
    let third: f64 = rows[2][4]
        .parse()
        .expect("the second measurement is a number");
    assert_eq!(third.to_bits(), (1.0_f64 / 3.0_f64).to_bits());
    assert_eq!(rows[1][17], "physical-plant");
}

#[test]
fn the_csv_holds_no_credential_because_its_inputs_are_the_loop_and_the_recording_only() {
    let text = samples_csv(
        &loop_with_disturbance(),
        EvidenceSource::SimulatedPlant,
        &fixture_run(),
    );
    let lower = text.to_ascii_lowercase();
    for word in ["token", "password", "credential", "secret", "authorization"] {
        assert!(!lower.contains(word), "the CSV names no {word}");
    }
}

#[test]
fn a_cell_with_a_comma_or_a_quote_is_quoted_and_its_quotes_are_doubled() {
    assert_eq!(csv_cell("plain"), "plain");
    assert_eq!(csv_cell("a,b"), "\"a,b\"");
    assert_eq!(csv_cell("say \"hi\""), "\"say \"\"hi\"\"\"");
}

#[test]
fn the_json_export_reads_back_as_the_result_it_wrote_and_drops_every_secret() {
    let result = json!({
        "stages": [{ "stage": "Identify", "status": "complete" }],
        "runtime": { "endpoint": "tcp://127.0.0.1:0", "authToken": "s3cret-token-value" },
        "password": "hunter2",
        "nested": [{ "credentials": { "user": "operator" }, "ratio": 0.300_000_000_000_000_04 }],
        "ratio": 0.300_000_000_000_000_04
    });
    let text = workflow_json(&result).expect("the result writes as JSON");
    assert!(
        !text.contains("s3cret-token-value"),
        "a token is never exported"
    );
    assert!(!text.contains("hunter2"), "a password is never exported");
    assert!(
        !text.contains("authToken") && !text.contains("credentials"),
        "the keys that name a secret are dropped"
    );
    let read_back: Value = serde_json::from_str(&text).expect("the export reads back as JSON");
    assert_eq!(
        read_back,
        json!({
            "stages": [{ "stage": "Identify", "status": "complete" }],
            "runtime": { "endpoint": "tcp://127.0.0.1:0" },
            "nested": [{ "ratio": 0.300_000_000_000_000_04 }],
            "ratio": 0.300_000_000_000_000_04
        })
    );
}

/// A saved result in the form the extension wrote, whose run was an aborted manual-output step on a simulated target (B10.3).
fn aborted_step() -> Value {
    json!({
        "source": "Simulation",
        "experiment": {
            "kind": "mv_step", "affectedSignal": "ValveMV", "address": "%QL0", "requestedValue": 25, "appliedValue": 25,
            "durationMs": 10_000, "trajectory": [{ "atMs": 0, "value": 25 }], "stopConditions": ["Abort"],
            "minimum": 0, "maximum": 100, "approval": "Simulation confirmation", "status": "aborted",
            "stopReason": "user_abort", "aborted": true,
            "restoration": { "value": 20, "status": "confirmed" }
        },
        "recording": { "rawSamples": [] }
    })
}

/// The recording file of the fixture run with `context` as the context of its run.
fn kept_with(context: Option<RunContext>) -> RecordingFile {
    let recorded = fixture_run();
    let evidence = recording_evidence(&recorded, None);
    let mut file = RecordingFile::new(
        "loop:tank",
        EvidenceSource::SimulatedPlant,
        recorded,
        evidence,
    );
    file.context = context;
    file
}

#[test]
fn the_json_export_writes_the_run_context_of_each_recording_that_has_one() {
    // B10.2, B10.3, B10.5: a recording from an aborted test keeps its stop reason, its abort state and its restoration, and the
    // export writes them beside the recording's identity and verdict. A recording whose run states no context has no entry.
    let stated = saved_context(&aborted_step(), EvidenceSource::SimulatedPlant);
    assert!(stated.is_some(), "the fixture states its experiment");
    let with = format!("rec-{}", "a".repeat(32));
    let without = format!("rec-{}", "b".repeat(32));
    let recordings = [
        (with.clone(), kept_with(stated)),
        (without, kept_with(None)),
    ];
    let entries: Vec<RecordingContextEntry> = recordings
        .iter()
        .filter_map(|(id, file)| recording_context(id, file))
        .collect();
    assert_eq!(entries.len(), 1, "only the recording with a context");
    let result = json!({ "kind": "opened", "runtime": { "authToken": "s3cret-token-value" } });
    let text = workflow_json_with_recordings(&result, &entries).expect("the result writes as JSON");
    assert!(
        !text.contains("s3cret-token-value"),
        "a token is never exported"
    );
    let read_back: Value = serde_json::from_str(&text).expect("the export reads back as JSON");
    let entry = &read_back["recordings"][0];
    assert_eq!(entry["recordingId"], json!(with));
    assert_eq!(entry["source"], "simulated-plant");
    assert_eq!(entry["opened"], false);
    assert_eq!(entry["evidence"]["completeness"], "complete");
    let experiment = &entry["experiments"][0];
    assert_eq!(experiment["status"], "aborted");
    assert_eq!(experiment["stopReason"], "user_abort");
    assert_eq!(experiment["aborted"], true);
    assert_eq!(experiment["restoration"]["status"], "confirmed");
    assert_eq!(experiment["trajectory"][0]["value"], 25.0);
    assert!(
        entry.get("plant").is_none(),
        "a run with no plant to state writes none"
    );
    assert_eq!(read_back["recordings"].as_array().map(Vec::len), Some(1));
}

#[test]
fn a_study_with_no_run_context_exports_the_result_unchanged() {
    let result = json!({ "kind": "opened", "stages": [] });
    let text = workflow_json_with_recordings(&result, &[]).expect("the result writes as JSON");
    assert_eq!(
        text,
        workflow_json(&result).expect("the same result writes as JSON")
    );
    assert!(!text.contains("recordings"), "{text}");
}
