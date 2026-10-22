use serde_json::{json, Value};

use super::{
    check_selection, configured_recording_selection, decode_recording_catalog,
    opened_recording_catalog, selected_recording_channels, selection_loop, CatalogKind,
    RecordingSignalCatalog, RecordingSignalSelection, WorkflowUnits,
};
use crate::loop_definition::{Direction, LoopDefinition};
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
fn a_legacy_file_offers_each_selector_its_own_channel() {
    let catalog =
        opened_recording_catalog(&units(), &loop_definition(), &[], None).expect("the file opens");
    let names = |channels: &[super::CatalogChannel]| {
        channels
            .iter()
            .map(|channel| channel.id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&catalog.inputs), ["appliedMv"]);
    assert_eq!(names(&catalog.outputs), ["pv"]);
}

#[test]
fn a_legacy_file_words_its_two_channels_and_an_archive_words_none() {
    // Wrong behaviour caught: the page knowing the legacy identifiers `appliedMv` and `pv` to put words to them (B3.6). The engine
    // gives the words, a catalog read back from the study gives them again, and an archive's channels, named by their symbols, have none.
    let legacy =
        opened_recording_catalog(&units(), &loop_definition(), &[], None).expect("the file opens");
    let words = |channels: &[super::CatalogChannel]| {
        channels
            .iter()
            .map(|channel| (channel.id.clone(), channel.label.clone()))
            .collect::<Vec<_>>()
    };
    let expected = [
        ("appliedMv".to_string(), Some("Applied input".to_string())),
        ("pv".to_string(), Some("Measured output".to_string())),
    ];
    assert_eq!(words(&legacy.channels), expected);
    assert_eq!(words(&legacy.inputs), expected[..1]);
    assert_eq!(words(&legacy.outputs), expected[1..]);
    let stored = json!({
        "kind": "legacy",
        "channels": [{ "id": "appliedMv", "unit": "%" }, { "id": "pv", "unit": "%" }],
        "selection": { "inputs": ["appliedMv"], "outputs": ["pv"] }
    });
    let read_back = decoded(&stored).expect("the stored legacy catalog reads");
    assert_eq!(words(&read_back.channels), expected);

    let opened = opened_recording_catalog(
        &units(),
        &loop_definition(),
        &[archive(&[[50.0, 48.0, 32.0]])],
        None,
    )
    .expect("the archive opens");
    assert!(
        opened
            .channels
            .iter()
            .all(|channel| channel.label.is_none()),
        "an archive's channels are named by their symbols"
    );
}

#[test]
fn each_selector_offers_only_what_the_loop_does_not_declare_on_the_other_side() {
    let loop_definition = loop_definition();
    let process = loop_definition.signals.process_variable.symbol.clone();
    let manipulated = loop_definition.signals.manipulated_variable.symbol.clone();
    let catalog = opened_recording_catalog(
        &units(),
        &loop_definition,
        &[archive(&[[50.0, 48.0, 32.0]])],
        None,
    )
    .expect("the archive opens");
    let inputs: Vec<&str> = catalog.inputs.iter().map(|item| item.id.as_str()).collect();
    let outputs: Vec<&str> = catalog
        .outputs
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    assert!(
        inputs.contains(&manipulated.as_str()),
        "the plant input is offered"
    );
    assert!(
        !inputs.contains(&process.as_str()),
        "a measured output is no plant input"
    );
    assert!(
        outputs.contains(&process.as_str()),
        "the measured output is offered"
    );
    assert!(
        !outputs.contains(&manipulated.as_str()),
        "a plant input is no measured output"
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

#[test]
fn the_selected_signals_take_the_plant_roles_of_the_fit_of_one_recording() {
    // The fit of a selected output (B3.6) reads the selected signals as the loop's plant input and measured output.
    let loop_definition = loop_definition();
    let recorded = archive(&[[50.0, 48.0, 32.0]]);
    let view = selection_loop(&loop_definition, &recorded, "LevelSP", "ValveMV")
        .expect("both signals are the loop's");
    assert_eq!(view.signals.manipulated_variable.symbol, "LevelSP");
    assert_eq!(view.signals.process_variable.symbol, "ValveMV");
    assert!(
        selection_loop(&loop_definition, &recorded, "NOPE", "ValveMV").is_err(),
        "a signal the loop does not have and the recording does not hold is not a plant signal"
    );
}

#[test]
fn a_stored_selection_refuses_an_input_that_is_also_an_output() {
    let selection = RecordingSignalSelection {
        inputs: vec!["ValveMV".to_string()],
        outputs: vec!["ValveMV".to_string()],
    };
    assert!(
        check_selection(&selection).is_err(),
        "the study keeps only a selection of distinct channels"
    );
    let valid = RecordingSignalSelection {
        inputs: vec!["ValveMV".to_string()],
        outputs: vec!["LevelPV".to_string()],
    };
    assert!(check_selection(&valid).is_ok());
}

#[test]
fn an_extra_channel_of_an_archive_is_offered_with_its_unit_and_selected_by_its_name() {
    let mut run = archive(&[[50.0, 48.0, 32.0]]);
    // An output the loop does not bind keeps its name and its unit (`archived_recording`, B3.6); a mode binding keeps neither.
    run.bindings.push(SignalBinding {
        id: "P1.RecordedTemperature".to_string(),
        address: None,
        symbol: Some("P1.RecordedTemperature".to_string()),
        iec_type: "REAL".to_string(),
        unit: Some("%".to_string()),
    });
    run.bindings.push(SignalBinding {
        id: "controllerMode".to_string(),
        address: None,
        symbol: Some("LevelAuto".to_string()),
        iec_type: "BOOL".to_string(),
        unit: None,
    });
    for sample in &mut run.samples {
        sample.values.push(Some(47.0));
        sample.values.push(Some(1.0));
        sample.quality.push(Quality::Good);
        sample.quality.push(Quality::Good);
        sample.forced.push(false);
        sample.forced.push(false);
    }
    let requested = json!({ "inputs": ["ValveMV"], "outputs": ["P1.RecordedTemperature"] });
    let catalog = opened_recording_catalog(
        &units(),
        &loop_definition(),
        std::slice::from_ref(&run),
        Some(&requested),
    )
    .expect("the archive opens with the extra output selected");
    assert!(
        catalog
            .channels
            .iter()
            .any(|channel| channel.id == "P1.RecordedTemperature" && channel.unit == "%"),
        "the extra channel is offered with its unit"
    );
    assert!(
        !catalog
            .channels
            .iter()
            .any(|channel| channel.id == "LevelAuto"),
        "a mode binding is not a channel"
    );
    assert_eq!(catalog.selection.outputs, ["P1.RecordedTemperature"]);
}

/// `recorded` with one more binding, `symbol` in `unit`, that holds `readings` at its samples: an extra output the loop does not bind.
fn with_extra_output(
    mut recorded: RecordedRun,
    symbol: &str,
    unit: &str,
    readings: &[(f64, Quality)],
) -> RecordedRun {
    recorded.bindings.push(SignalBinding {
        id: symbol.to_string(),
        address: None,
        symbol: Some(symbol.to_string()),
        iec_type: "REAL".to_string(),
        unit: Some(unit.to_string()),
    });
    for (sample, (value, quality)) in recorded.samples.iter_mut().zip(readings) {
        sample.values.push(Some(*value));
        sample.quality.push(*quality);
        sample.forced.push(false);
    }
    recorded
}

#[test]
fn a_selected_extra_output_is_the_measured_output_of_the_view_read_only_in_its_observed_range() {
    let loop_definition = loop_definition();
    let recorded = with_extra_output(
        archive(&[[50.0, 48.0, 32.0], [50.0, 49.0, 32.0], [50.0, 47.0, 32.0]]),
        "P1.RecordedTemperature",
        "degC",
        &[
            (47.5, Quality::Good),
            (49.0, Quality::Good),
            (48.25, Quality::Good),
        ],
    );
    let view = selection_loop(
        &loop_definition,
        &recorded,
        "ValveMV",
        "P1.RecordedTemperature",
    )
    .expect("the extra output is a channel of the recording");
    let output = &view.signals.process_variable;
    assert_eq!(output.symbol, "P1.RecordedTemperature");
    assert_eq!(output.unit, "degC", "with the unit the channel holds");
    assert_eq!(output.address, None, "a recorded channel has no address");
    assert_eq!(output.iec_type, "REAL");
    assert_eq!(
        output.direction,
        Direction::Read,
        "a recorded signal is only read"
    );
    assert!(
        (output.range.minimum - 47.5).abs() < 1e-9 && (output.range.maximum - 49.0).abs() < 1e-9,
        "the range is the one its samples took, which grants no excitation limit: {:?}",
        output.range
    );
    assert_eq!(
        view.signals.manipulated_variable.symbol, "ValveMV",
        "the loop's own plant input stays"
    );
}

#[test]
fn a_selected_name_that_neither_the_loop_nor_the_recording_holds_is_refused() {
    let loop_definition = loop_definition();
    let recorded = archive(&[[50.0, 48.0, 32.0]]);
    let refused = selection_loop(&loop_definition, &recorded, "ValveMV", "P1.Missing")
        .expect_err("the recording holds no such channel");
    assert_eq!(
        refused.message,
        "Choose input and output signals present in this recording."
    );
}

#[test]
fn an_extra_output_with_a_sample_that_is_not_good_is_refused_as_an_invalid_signal() {
    let loop_definition = loop_definition();
    let recorded = with_extra_output(
        archive(&[[50.0, 48.0, 32.0], [50.0, 49.0, 32.0]]),
        "P1.RecordedTemperature",
        "degC",
        &[(47.0, Quality::Good), (47.5, Quality::Bad)],
    );
    let refused = selection_loop(
        &loop_definition,
        &recorded,
        "ValveMV",
        "P1.RecordedTemperature",
    )
    .expect_err("one sample of the output is not good");
    assert_eq!(
        refused.message,
        "Recording signal P1.RecordedTemperature has incomplete or invalid samples."
    );
}

#[test]
fn the_view_of_an_extra_output_names_it_with_the_loops_measured_output_and_keeps_its_input_name() {
    let mut loop_definition = loop_definition();
    loop_definition.channels = Some(
        serde_json::from_value(json!({
            "inputs": [{ "signalId": "manipulatedVariable", "modelChannelId": "u",
                "actuator": { "kind": "continuous", "minimum": 0, "maximum": 100 } }],
            "outputs": [{ "signalId": "processVariable", "modelChannelId": "y" }],
        }))
        .expect("typed channels"),
    );
    let recorded = with_extra_output(
        archive(&[[50.0, 48.0, 32.0], [50.0, 49.0, 32.0]]),
        "P1.RecordedTemperature",
        "degC",
        &[(47.0, Quality::Good), (47.2, Quality::Good)],
    );
    let view = selection_loop(
        &loop_definition,
        &recorded,
        "ValveMV",
        "P1.RecordedTemperature",
    )
    .expect("the extra output is a channel of the recording");
    assert_eq!(
        view.signals.process_variable.symbol, "P1.RecordedTemperature",
        "the fit reads the recorded channel"
    );
    let channels = view.channels.as_ref().expect("the loop's channel names");
    assert_eq!(
        output_names(channels),
        [("processVariable", "y")],
        "the model names the loop's measured output, not the recorded channel (B3.6)"
    );
    assert_eq!(
        input_names(channels),
        [("manipulatedVariable", "u")],
        "the plant input is the loop's own, so it keeps its model name"
    );
}

#[test]
fn an_extra_output_of_a_loop_without_channels_is_named_with_its_measured_output_symbol() {
    let loop_definition = loop_definition();
    let recorded = with_extra_output(
        archive(&[[50.0, 48.0, 32.0], [50.0, 49.0, 32.0]]),
        "P1.RecordedTemperature",
        "degC",
        &[(47.0, Quality::Good), (47.2, Quality::Good)],
    );
    let view = selection_loop(
        &loop_definition,
        &recorded,
        "ValveMV",
        "P1.RecordedTemperature",
    )
    .expect("the extra output is a channel of the recording");
    let channels = view.channels.as_ref().expect("the view names its outputs");
    assert_eq!(
        output_names(channels),
        [("processVariable", "LevelPV")],
        "the loop's own measured output has no model name, so its symbol is the model's name"
    );
    assert_eq!(
        input_names(channels),
        Vec::<(&str, &str)>::new(),
        "the plant input is the loop's own and is not renamed"
    );
}

#[test]
fn a_selected_signal_the_loop_binds_to_no_role_takes_the_roles_model_name() {
    // The setpoint is selected as the measured output. It is not the loop's measured output, so the model names it with the
    // identifier the loop's measured output has (B3.6).
    let loop_definition = loop_definition();
    let recorded = archive(&[[50.0, 48.0, 32.0]]);
    let view = selection_loop(&loop_definition, &recorded, "ValveMV", "LevelSP")
        .expect("both signals are the loop's");
    assert_eq!(view.signals.process_variable.symbol, "LevelSP");
    assert_eq!(
        view.channels.as_ref().map(output_names),
        Some(vec![("processVariable", "LevelPV")]),
        "the setpoint is not the measured output, so the measured output's name is the model's"
    );
}

/// The (signal, model name) pairs of the view's measured-output channels.
fn output_names(channels: &crate::loop_definition::LoopChannels) -> Vec<(&str, &str)> {
    channels
        .outputs
        .iter()
        .map(|entry| (entry.signal_id.as_str(), entry.model_channel_id.as_str()))
        .collect()
}

/// The (signal, model name) pairs of the view's plant-input channels.
fn input_names(channels: &crate::loop_definition::LoopChannels) -> Vec<(&str, &str)> {
    channels
        .inputs
        .iter()
        .map(|entry| (entry.signal_id.as_str(), entry.model_channel_id.as_str()))
        .collect()
}
