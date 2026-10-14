//! Tests of the archived runs of a saved result: the pair a fit with independent validation reads, the isolated-simulation runs that
//! the file keeps (`decode_isolated_run`), and the recording an archived run is kept as (`archive.rs`; specification 35, B3.6, B5.2,
//! B10.2 and B10.4). The file-level checks of a saved result are tested with the import (`identify_import_tests.rs`), where a
//! complete saved result is built.

use std::collections::BTreeMap;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::*;
use crate::identification::connected_record::{
    connected_identification_record, RunConditions, RunEvidence, ValidationChannel, ValidationRun,
    ValidationSample, ValidationValue,
};
use crate::identification::independent_validation::RequirementStatus;
use crate::loop_definition::LoopDefinition;
use crate::recording::channels::{
    ChannelQuality, MANIPULATED_VARIABLE_BINDING, PROCESS_VARIABLE_BINDING, SETPOINT_BINDING,
};
use crate::recording::evidence::recording_evidence;
use crate::recording::fixtures::loop_definition;
use crate::recording::opened::opened_plan;
use crate::recording::{
    Acquisition, Completeness, RecordedRun, RecordedSample, RecordingFile, ScanRange, StopReason,
};
use crate::signals::{Quality, SignalBinding};
use crate::validation::isolated_run_sampling::recorded_sample_times;
use crate::validation::run_validation::{evaluate_recorded_run, RunCheck, RunRequirement, Window};

/// The symbols of the fixture's manipulated value and measured value, both in %.
const INPUT: &str = "ValveMV";
const OUTPUT: &str = "LevelPV";
/// The simulation configuration of a selected model, and its digest as the runtime names it.
const SIMULATION_TOML: &str = "[simulation]\nenabled = true\n";

/// A connected record of the fixture tank with `count` constant samples, recorded on `program_revision`.
fn record(id: &str, program_revision: &str, count: u64) -> ArchivedRun {
    let loop_definition = loop_definition();
    let bind = |id: &str, symbol: &str| SignalBinding {
        id: id.to_string(),
        address: None,
        symbol: Some(symbol.to_string()),
        iec_type: "LREAL".to_string(),
        unit: None,
    };
    let samples = (0..count)
        .map(|index| RecordedSample {
            scan: index + 1,
            runtime_time_ns: Some(
                i64::try_from(index * 1_000_000_000).expect("a runtime time in range"),
            ),
            captured_at_ms: 1_000 + index,
            values: vec![Some(50.0), Some(40.0), Some(30.0)],
            forced: vec![false; 3],
            quality: vec![Quality::Good; 3],
            gap: false,
        })
        .collect();
    let run = RecordedRun {
        recording_id: id.to_string(),
        loop_revision: loop_definition.revision.clone(),
        program_revision: Some(program_revision.to_string()),
        bindings: vec![
            bind(SETPOINT_BINDING, "LevelSP"),
            bind(PROCESS_VARIABLE_BINDING, "LevelPV"),
            bind(MANIPULATED_VARIABLE_BINDING, "ValveMV"),
        ],
        stop_reason: StopReason::Stopped,
        samples,
        acquisition: Acquisition {
            method: "runtime_scan_buffer".to_string(),
            completeness: Completeness::Complete,
            observed_sample_count: count,
            scan_range: Some(ScanRange {
                first: 1,
                last: count,
            }),
            missing_scan_ranges: Vec::new(),
            dropped_observed_samples: 0,
            actual_cycle_timing_available: true,
            freshness_available: false,
            reason: "Complete recording.".to_string(),
        },
    };
    let file = RecordingFile::new(
        &loop_definition.id,
        EvidenceSource::SimulatedPlant,
        run.clone(),
        recording_evidence(&run, None),
    );
    let plan = opened_plan(&file, &loop_definition);
    ArchivedRun::Connected(Box::new(
        connected_identification_record(
            &loop_definition,
            &run,
            None,
            &plan,
            0.0,
            None,
            evaluate_recorded_limits,
        )
        .expect("the connected record of the fixture run"),
    ))
}

/// The digest of `text` as the runtime names a simulation configuration: `sha256:` and its hexadecimal SHA-256.
fn digest(text: &str) -> String {
    use std::fmt::Write;
    let mut hex = String::new();
    for byte in Sha256::digest(text.as_bytes()) {
        write!(hex, "{byte:02x}").expect("writing to a string cannot fail");
    }
    format!("sha256:{hex}")
}

/// The samples of an isolated run on `times`: a steady input of 50 % and a steady output of 10 %, both good.
fn isolated_samples(times: &[f64]) -> Vec<ValidationSample> {
    times
        .iter()
        .enumerate()
        .map(|(index, &time)| ValidationSample {
            time_seconds: time,
            sequence: u64::try_from(index + 1).expect("a sequence number"),
            values: BTreeMap::from([
                (
                    INPUT.to_string(),
                    ValidationValue {
                        value: Some(50.0),
                        quality: ChannelQuality::Good,
                    },
                ),
                (
                    OUTPUT.to_string(),
                    ValidationValue {
                        value: Some(10.0),
                        quality: ChannelQuality::Good,
                    },
                ),
            ]),
        })
        .collect()
}

/// An isolated simulation's run record of `revision`: 21 samples on the 100 ms grid over 2 s, with its frozen request and the verdict
/// of its output bounds, evaluated here as the runtime's record carries it. A `selected` record took a selected model's plant.
fn isolated_record(id: &str, revision: &str, selected: bool) -> Value {
    let times = recorded_sample_times(100, 2_000, 1);
    let samples = isolated_samples(&times);
    let run = ValidationRun {
        id: id.to_string(),
        evidence: RunEvidence::StExecuted,
        parameters: BTreeMap::new(),
        conditions: RunConditions {
            initial_state: BTreeMap::new(),
            events: Vec::new(),
            duration_seconds: 2.0,
            sample_times_seconds: times,
            seed: 1,
            model_revision: if selected {
                "selected-plant:v1".to_string()
            } else {
                "synthetic-plant".to_string()
            },
            program_revision: revision.to_string(),
            parameter_schema_revision: "synthetic-parameters".to_string(),
        },
        channels: vec![
            ValidationChannel {
                id: INPUT.to_string(),
                unit: "%".to_string(),
            },
            ValidationChannel {
                id: OUTPUT.to_string(),
                unit: "%".to_string(),
            },
        ],
        samples,
    };
    let requirement = RunRequirement {
        id: "output-bounds".to_string(),
        channel: OUTPUT.to_string(),
        window: Window {
            start: 0.0,
            end: 2.0,
        },
        check: RunCheck::Bounds {
            minimum: -20.0,
            maximum: 20.0,
        },
    };
    let evaluation = evaluate_recorded_run(&run, std::slice::from_ref(&requirement), false)
        .expect("the fixture run is valid");
    let mut request = json!({
        "execution": "isolated-simulation",
        "expected_project_revision": revision,
        "step_ms": 100,
        "duration_ms": 2_000,
        "seed": 1,
        "max_total_cycles": 21,
        "wall_budget_ms": 1_000,
        "initial_values": {},
        "events": [],
        "observations": [{ "symbol": INPUT, "unit": "%" }, { "symbol": OUTPUT, "unit": "%" }],
        "parameter_overrides": []
    });
    let mut stored_run = serde_json::to_value(&run).expect("the run as JSON");
    if selected {
        request["simulation_override"] = json!({
            "model_revision": "selected-plant:v1",
            "model_provenance": "declared-fitted-validated",
            "simulation_toml": SIMULATION_TOML
        });
        let conditions = stored_run["conditions"]
            .as_object_mut()
            .expect("the conditions are an object");
        conditions.insert(
            "modelProvenance".to_string(),
            json!("declared-fitted-validated"),
        );
        conditions.insert(
            "simulationConfigRevision".to_string(),
            json!(digest(SIMULATION_TOML)),
        );
    }
    json!({
        "projectRevision": revision,
        "request": request,
        "run": stored_run,
        "evaluation": serde_json::to_value(&evaluation).expect("the evaluation as JSON")
    })
}

/// An isolated run of `revision` read as an archived run, for the tests that follow it.
fn isolated(id: &str, revision: &str) -> ArchivedRun {
    ArchivedRun::Isolated(Box::new(
        decode_isolated_run(&isolated_record(id, revision, false))
            .expect("the isolated run answers its request and its verdict holds"),
    ))
}

#[test]
fn the_pair_is_the_last_two_eligible_runs_of_the_latest_project_revision() {
    let selection = [("ValveMV", "%"), ("LevelPV", "%")];
    let runs = [
        record("rec-old", "sha256:old", 20),
        record("rec-new-a", "sha256:new", 20),
        record("rec-new-b", "sha256:new", 20),
    ];
    let refs: Vec<&ArchivedRun> = runs.iter().collect();
    assert_eq!(
        independent_pair(&refs, &selection),
        vec![1, 2],
        "the two runs of the latest project revision are the estimation and the validation"
    );
    let one = [
        record("rec-old", "sha256:old", 20),
        record("rec-new", "sha256:new", 20),
    ];
    let refs: Vec<&ArchivedRun> = one.iter().collect();
    assert_eq!(
        independent_pair(&refs, &selection),
        vec![1],
        "one eligible run leaves the pair short, so a second recording is needed"
    );
}

#[test]
fn an_archived_run_is_kept_as_an_opened_recording_of_the_loop_with_its_samples() {
    let loop_definition = loop_definition();
    let result = record("rec-kept", "sha256:new", 12);
    let file = archived_recording(&loop_definition, EvidenceSource::SimulatedPlant, &result)
        .expect("the archived run is kept");
    assert!(file.opened, "the recording carries the opened marker (A9)");
    assert_eq!(file.run.recording_id, "rec-kept");
    assert_eq!(file.run.samples.len(), 12, "every sample is kept");
    assert!(
        file.run
            .bindings
            .iter()
            .any(|binding| binding.id == PROCESS_VARIABLE_BINDING),
        "the measurement is bound to the loop's signal"
    );
    assert_eq!(file.run.acquisition.completeness, Completeness::Complete);
    assert_eq!(file.run.program_revision.as_deref(), Some("sha256:new"));
    assert_eq!(file.source, EvidenceSource::SimulatedPlant);
}

#[test]
fn an_isolated_simulation_run_is_read_when_its_request_and_its_verdict_hold() {
    let read = decode_isolated_run(&isolated_record("iso-first", "sha256:iso", false))
        .expect("the run answers its frozen request and its verdict holds");
    assert_eq!(read.project_revision, "sha256:iso");
    assert_eq!(read.run.id, "iso-first");
    assert_eq!(read.status, RequirementStatus::Pass);
    assert!(!read.selected_model, "the run took the loop's own plant");
    assert!(
        ArchivedRun::Isolated(Box::new(read)).is_archivable(),
        "an ST-executed run with a Pass verdict is archived"
    );
}

#[test]
fn an_isolated_run_whose_verdict_or_request_does_not_hold_is_refused_as_a_whole() {
    let mut tampered_verdict = isolated_record("iso-first", "sha256:iso", false);
    tampered_verdict["evaluation"]["status"] = json!("Fail");
    assert!(
        decode_isolated_run(&tampered_verdict).is_none(),
        "a stored verdict that the requirements do not give again is refused (B10.4)"
    );
    let mut other_grid = isolated_record("iso-first", "sha256:iso", false);
    other_grid["request"]["step_ms"] = json!(200);
    assert!(
        decode_isolated_run(&other_grid).is_none(),
        "a run on another grid than its frozen request is refused"
    );
    let mut other_revision = isolated_record("iso-first", "sha256:iso", false);
    other_revision["request"]["expected_project_revision"] = json!("sha256:other");
    assert!(
        decode_isolated_run(&other_revision).is_none(),
        "a run requested on another project revision is refused"
    );
}

#[test]
fn an_isolated_run_of_a_selected_model_is_read_but_not_archived() {
    let read = decode_isolated_run(&isolated_record("iso-selected", "sha256:iso", true))
        .expect("a run of the selected model answers its frozen request");
    assert!(read.selected_model, "the run names its selected model");
    assert!(
        !ArchivedRun::Isolated(Box::new(read)).is_archivable(),
        "the import does not archive a run of a selected model (openedRecordingIdentification.ts)"
    );
}

#[test]
fn the_second_isolated_run_pairs_with_the_first_of_the_same_project_revision() {
    let first = isolated("iso-first", "sha256:iso");
    let second = isolated("iso-second", "sha256:iso");
    let other = isolated("iso-other", "sha256:other");
    let selection = [(INPUT, "%"), (OUTPUT, "%")];
    assert_eq!(
        independent_pair(&[&first, &second], &selection),
        vec![0, 1],
        "the first run is the estimation and the second the validation, in order"
    );
    assert_eq!(
        independent_pair(&[&first, &other], &selection),
        vec![1],
        "a run of another project revision is not paired with it"
    );
}

#[test]
fn an_isolated_run_is_kept_as_an_opened_recording_with_its_grid_and_its_own_acquisition() {
    let loop_definition = loop_definition();
    let run = isolated("iso-kept", "sha256:iso");
    let file = archived_recording(&loop_definition, EvidenceSource::SimulatedPlant, &run)
        .expect("the isolated run is kept");
    assert!(file.opened, "the recording carries the opened marker (A9)");
    assert_eq!(file.run.recording_id, "iso-kept");
    assert_eq!(file.run.samples.len(), 21, "every grid point is kept");
    assert_eq!(file.run.acquisition.method, "isolated-simulation");
    assert_eq!(file.run.acquisition.completeness, Completeness::Complete);
    assert_eq!(file.run.program_revision.as_deref(), Some("sha256:iso"));
    assert!(
        file.run
            .bindings
            .iter()
            .any(|binding| binding.id == MANIPULATED_VARIABLE_BINDING),
        "the input is bound to the loop's signal"
    );
}

/// `run` with one more channel, `symbol` in `unit`, whose value at each sample is `values[index]`: an extra output that the loop does
/// not bind, as an archived run of the journey holds it.
fn with_extra_channel(
    mut run: ArchivedRun,
    symbol: &str,
    unit: &str,
    values: &[f64],
) -> ArchivedRun {
    let inner: &mut ValidationRun = match &mut run {
        ArchivedRun::Connected(result) => &mut result.run,
        ArchivedRun::Isolated(isolated) => &mut isolated.run,
    };
    inner.channels.push(ValidationChannel {
        id: symbol.to_string(),
        unit: unit.to_string(),
    });
    for (sample, value) in inner.samples.iter_mut().zip(values) {
        sample.values.insert(
            symbol.to_string(),
            ValidationValue {
                value: Some(*value),
                quality: ChannelQuality::Good,
            },
        );
    }
    run
}

/// The recording an archived run is kept as, as `identify_import` keeps it: an isolated simulation's run names its model revision.
fn kept(loop_definition: &LoopDefinition, run: &ArchivedRun) -> RecordingFile {
    let mut file = archived_recording(loop_definition, EvidenceSource::SimulatedPlant, run)
        .expect("the run is kept");
    if let ArchivedRun::Isolated(isolated) = run {
        file.model_revision = Some(isolated.run.conditions.model_revision.clone());
    }
    file
}

#[test]
fn an_extra_channel_of_an_archived_run_is_kept_with_its_unit_and_the_loop_channels_keep_none() {
    let loop_definition = loop_definition();
    let run = with_extra_channel(
        record("rec-extra", "sha256:new", 3),
        "LevelRecorded",
        "degC",
        &[20.0, 21.0, 22.0],
    );
    let file = archived_recording(&loop_definition, EvidenceSource::SimulatedPlant, &run)
        .expect("the archived run is kept");
    let extra = file
        .run
        .bindings
        .iter()
        .find(|binding| binding.symbol.as_deref() == Some("LevelRecorded"))
        .expect("the extra channel is kept");
    assert_eq!(
        extra.id, "LevelRecorded",
        "the extra channel is bound under its name"
    );
    assert_eq!(extra.unit.as_deref(), Some("degC"), "with its unit (B3.6)");
    let measured = file
        .run
        .bindings
        .iter()
        .find(|binding| binding.id == PROCESS_VARIABLE_BINDING)
        .expect("the measurement is bound");
    assert_eq!(
        measured.unit, None,
        "a loop channel keeps no unit, so its file keeps its bytes"
    );
    let index = file
        .run
        .bindings
        .iter()
        .position(|binding| binding.id == "LevelRecorded")
        .expect("the extra channel's index");
    assert!(
        matches!(file.run.samples[2].values[index], Some(value) if (value - 22.0).abs() < 1e-9),
        "its value at each sample is kept"
    );
    assert_eq!(file.run.samples[2].quality[index], Quality::Good);
}

#[test]
fn a_file_without_an_extra_channel_writes_no_unit_so_its_bytes_are_those_it_had() {
    let loop_definition = loop_definition();
    let file = archived_recording(
        &loop_definition,
        EvidenceSource::SimulatedPlant,
        &record("rec-plain", "sha256:new", 12),
    )
    .expect("the archived run is kept");
    let text = serde_json::to_string(&file.run.bindings).expect("the bindings as JSON");
    assert!(
        !text.contains("\"unit\""),
        "no loop channel writes a unit, so the file's identifier stays as it was: {text}"
    );
}

#[test]
fn a_kept_connected_recording_reads_back_with_its_extra_channel() {
    let loop_definition = loop_definition();
    let run = with_extra_channel(
        record("rec-kept", "sha256:new", 12),
        "LevelRecorded",
        "degC",
        &[20.0; 12],
    );
    let file = archived_recording(&loop_definition, EvidenceSource::SimulatedPlant, &run)
        .expect("the archived run is kept");
    let read_back =
        opened_archived_run(&loop_definition, &file).expect("the kept recording reads back");
    assert!(
        matches!(read_back, ArchivedRun::Connected(_)),
        "a connected recording reads back as its connected record"
    );
    assert!(
        read_back
            .run()
            .channels
            .iter()
            .any(|channel| channel.id == "LevelRecorded" && channel.unit == "degC"),
        "the record holds the extra channel with its unit, so it pairs under that output (B3.6)"
    );
    assert_eq!(read_back.run().samples.len(), 12);
}

#[test]
fn a_kept_isolated_run_reads_back_as_an_isolated_run_and_pairs_with_the_next_one() {
    let loop_definition = loop_definition();
    let first = isolated("iso-first", "sha256:iso");
    let second = isolated("iso-second", "sha256:iso");
    let read_back = opened_archived_run(&loop_definition, &kept(&loop_definition, &first))
        .expect("the kept isolated run reads back");
    assert!(
        matches!(read_back, ArchivedRun::Isolated(_)),
        "a kept isolated run reads back as an isolated run (B5.2)"
    );
    assert_eq!(
        read_back.project_revision(),
        "sha256:iso",
        "its project revision is the program revision the file keeps"
    );
    assert_eq!(
        read_back.run().conditions.model_revision,
        "synthetic-plant",
        "its model revision is the one the file keeps"
    );
    assert_eq!(
        read_back.run().samples.len(),
        21,
        "every grid point is read back"
    );
    let selection = [(INPUT, "%"), (OUTPUT, "%")];
    assert_eq!(
        independent_pair(&[&read_back, &second], &selection),
        vec![0, 1],
        "the kept first run is the estimation and the second run is the validation"
    );
}
