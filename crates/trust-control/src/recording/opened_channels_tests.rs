use serde_json::{json, Value};

use super::{
    configured_recording_selection, decode_recording_catalog, opened_recording_catalog,
    selected_recording_channels, CatalogKind, RecordingSignalCatalog, RecordingSignalSelection,
    WorkflowUnits,
};
use crate::loop_definition::LoopDefinition;
use crate::recording::fixtures::{binding, loop_definition, run, sample};
use crate::recording::RecordedRun;
use crate::signals::{Quality, SignalBinding};

fn units() -> WorkflowUnits {
    WorkflowUnits {
        mv: "%".to_string(),
        pv: "%".to_string(),
    }
}

fn three_bindings() -> Vec<SignalBinding> {
    vec![
        binding("setpoint", "%ML0", "LREAL"),
        binding("processVariable", "%IL0", "LREAL"),
        binding("manipulatedVariable", "%QL0", "LREAL"),
    ]
}

/// A recorded run with the three signals of the fixture loop and one good sample per entry of `values`, each
/// `[setpoint, process variable, manipulated variable]`.
fn archive(values: &[[f64; 3]]) -> RecordedRun {
    let samples = values
        .iter()
        .enumerate()
        .map(|(index, [setpoint, process, manipulated])| {
            let index = u64::try_from(index).expect("a few samples");
            sample(
                index + 1,
                Some(i64::try_from(index).expect("a few samples") * 100_000_000),
                &[Some(*setpoint), Some(*process), Some(*manipulated)],
                &[Quality::Good; 3],
            )
        })
        .collect();
    run(three_bindings(), samples)
}

fn loop_with_units(process_unit: &str) -> LoopDefinition {
    let mut loop_definition = loop_definition();
    loop_definition.signals.process_variable.unit = process_unit.to_string();
    loop_definition
}

fn decoded(value: &Value) -> Option<RecordingSignalCatalog> {
    decode_recording_catalog(value)
}

fn refusal(text: Result<RecordingSignalCatalog, crate::failure::Failure>) -> String {
    text.expect_err("the selection is refused").message
}

#[test]
fn a_legacy_file_offers_the_applied_input_and_the_measurement_with_the_file_units() {
    let catalog =
        opened_recording_catalog(&units(), &loop_definition(), &[], None).expect("the file opens");
    assert_eq!(catalog.kind, CatalogKind::Legacy);
    assert_eq!(
        catalog
            .channels
            .iter()
            .map(|channel| (channel.id.as_str(), channel.unit.as_str()))
            .collect::<Vec<_>>(),
        [("appliedMv", "%"), ("pv", "%")]
    );
    assert_eq!(
        catalog.selection,
        RecordingSignalSelection {
            inputs: vec!["appliedMv".to_string()],
            outputs: vec!["pv".to_string()],
        }
    );
}

#[test]
fn an_archive_names_its_channels_by_loop_symbol_and_selects_the_loop_default() {
    let runs = [archive(&[[50.0, 48.0, 32.0]])];
    let catalog = opened_recording_catalog(&units(), &loop_definition(), &runs, None)
        .expect("the archive opens");
    assert_eq!(catalog.kind, CatalogKind::Archive);
    assert_eq!(
        catalog
            .channels
            .iter()
            .map(|channel| channel.id.as_str())
            .collect::<Vec<_>>(),
        ["LevelSP", "LevelPV", "ValveMV"],
        "the channels are the loop's signals, named by their symbols"
    );
    assert_eq!(
        configured_recording_selection(&loop_definition()),
        RecordingSignalSelection {
            inputs: vec!["ValveMV".to_string()],
            outputs: vec!["LevelPV".to_string()],
        }
    );
    assert_eq!(catalog.selection.inputs, ["ValveMV"]);
    assert_eq!(catalog.selection.outputs, ["LevelPV"]);
}

#[test]
fn a_requested_selection_must_name_present_and_distinct_channels() {
    let runs = [archive(&[[50.0, 48.0, 32.0]])];
    let loop_definition = loop_definition();
    let requested = json!({ "inputs": ["LevelSP"], "outputs": ["LevelPV"] });
    let chosen = opened_recording_catalog(&units(), &loop_definition, &runs, Some(&requested))
        .expect("present channels");
    assert_eq!(chosen.selection.inputs, ["LevelSP"]);
    assert_eq!(
        refusal(opened_recording_catalog(
            &units(),
            &loop_definition,
            &runs,
            Some(&json!({ "inputs": ["Missing"], "outputs": ["LevelPV"] }))
        )),
        "Choose input and output signals present in this recording."
    );
    assert_eq!(
        refusal(opened_recording_catalog(
            &units(),
            &loop_definition,
            &runs,
            Some(&json!({ "inputs": ["LevelPV"], "outputs": ["LevelPV"] }))
        )),
        "Choose distinct plant input and measured output signals from the recording."
    );
    assert_eq!(
        refusal(opened_recording_catalog(
            &units(),
            &loop_definition,
            &runs,
            Some(&json!({ "inputs": ["LevelSP"], "outputs": ["LevelPV"], "extra": 1 }))
        )),
        "Choose distinct plant input and measured output signals from the recording."
    );
}

#[test]
fn a_stored_catalog_is_read_only_when_it_is_well_formed() {
    let stored = json!({
        "kind": "archive",
        "channels": [{ "id": "LevelPV", "unit": "%" }, { "id": "ValveMV", "unit": "%" }],
        "selection": { "inputs": ["ValveMV"], "outputs": ["LevelPV"] }
    });
    let catalog = decoded(&stored).expect("the catalog is well formed");
    assert_eq!(catalog.kind, CatalogKind::Archive);
    assert_eq!(catalog.channels.len(), 2);
    let mut extra_key = stored.clone();
    extra_key["note"] = json!("not part of the format");
    assert!(
        decoded(&extra_key).is_none(),
        "only kind, channels and selection are stored"
    );
    let mut unknown_kind = stored.clone();
    unknown_kind["kind"] = json!("ordinary");
    assert!(decoded(&unknown_kind).is_none());
    let mut duplicate = stored.clone();
    duplicate["channels"] =
        json!([{ "id": "LevelPV", "unit": "%" }, { "id": "LevelPV", "unit": "%" }]);
    assert!(
        decoded(&duplicate).is_none(),
        "channel identifiers are unique"
    );
    let mut missing = stored.clone();
    missing["selection"] = json!({ "inputs": ["Gone"], "outputs": ["LevelPV"] });
    assert!(
        decoded(&missing).is_none(),
        "the selection names channels that exist"
    );
    let mut long_unit = stored.clone();
    long_unit["channels"][0]["unit"] = json!("x".repeat(65));
    assert!(
        decoded(&long_unit).is_none(),
        "a unit is at most 64 UTF-16 units"
    );
    let mut crowded = stored.clone();
    crowded["channels"] = json!((0..257)
        .map(|index| json!({ "id": format!("s{index}"), "unit": "%" }))
        .collect::<Vec<_>>());
    assert!(
        decoded(&crowded).is_none(),
        "a catalog keeps at most 256 channels"
    );
}

#[test]
fn selected_channels_span_the_values_the_runs_took_and_a_single_value_is_padded() {
    let runs = [archive(&[
        [50.0, 48.0, 32.0],
        [50.0, 48.5, 35.0],
        [50.0, 49.0, 35.0],
    ])];
    let channels = selected_recording_channels(
        &runs,
        &loop_definition(),
        &RecordingSignalSelection {
            inputs: vec!["ValveMV".to_string()],
            outputs: vec!["LevelPV".to_string()],
        },
    )
    .expect("the selected channels have good values");
    assert_eq!(channels.inputs[0].id, "ValveMV");
    assert_eq!(channels.inputs[0].unit, "%");
    assert_eq!(channels.inputs[0].operating_range.minimum, 32.0);
    assert_eq!(channels.inputs[0].operating_range.maximum, 35.0);
    assert_eq!(channels.outputs[0].operating_range.minimum, 48.0);
    assert_eq!(channels.outputs[0].operating_range.maximum, 49.0);

    let single = [archive(&[[50.0, 50.0, 50.0]])];
    let padded = selected_recording_channels(
        &single,
        &loop_definition(),
        &RecordingSignalSelection {
            inputs: vec!["LevelSP".to_string()],
            outputs: vec!["LevelPV".to_string()],
        },
    )
    .expect("a single value is a channel too");
    // max(1e-9, 50 · 1e-9) = 5e-8 on each side.
    assert!((padded.inputs[0].operating_range.minimum - (50.0 - 5e-8)).abs() < 1e-15);
    assert!((padded.inputs[0].operating_range.maximum - (50.0 + 5e-8)).abs() < 1e-15);
}

#[test]
fn selected_channels_refuse_bad_values_missing_channels_and_empty_runs() {
    let loop_definition = loop_definition();
    let selection = RecordingSignalSelection {
        inputs: vec!["ValveMV".to_string()],
        outputs: vec!["LevelPV".to_string()],
    };
    let mut bad = archive(&[[50.0, 48.0, 32.0]]);
    bad.samples[0].quality[2] = Quality::Bad;
    let failure = selected_recording_channels(&[bad], &loop_definition, &selection)
        .expect_err("bad values are refused");
    assert_eq!(
        failure.message,
        "Recording signal ValveMV has incomplete or invalid samples."
    );
    let empty = archive(&[]);
    let failure = selected_recording_channels(&[empty], &loop_definition, &selection)
        .expect_err("no samples are refused");
    assert_eq!(failure.message, "Recording signal ValveMV has no samples.");
    let missing = RecordingSignalSelection {
        inputs: vec!["Missing".to_string()],
        outputs: vec!["LevelPV".to_string()],
    };
    let failure = selected_recording_channels(
        &[archive(&[[50.0, 48.0, 32.0]])],
        &loop_definition,
        &missing,
    )
    .expect_err("a missing channel is refused");
    assert_eq!(
        failure.message,
        "Recording signal Missing is missing or has inconsistent units."
    );
}

#[test]
fn the_units_of_an_archive_come_from_the_loop_definition() {
    // A run carries no units (annex E4.7): the unit of a channel is the loop signal's unit.
    let loop_definition = loop_with_units("°C");
    let runs = [archive(&[[50.0, 48.0, 32.0]])];
    let catalog = opened_recording_catalog(&units(), &loop_definition, &runs, None)
        .expect("the archive opens");
    assert_eq!(catalog.channels[1].unit, "°C");
    let channels = selected_recording_channels(
        &runs,
        &loop_definition,
        &RecordingSignalSelection {
            inputs: vec!["ValveMV".to_string()],
            outputs: vec!["LevelPV".to_string()],
        },
    )
    .expect("the channels resolve");
    assert_eq!(channels.outputs[0].unit, "°C");
}
