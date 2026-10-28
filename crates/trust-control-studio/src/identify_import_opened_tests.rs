//! The engine tests of the kept recordings that `openRecording` answers at once: an isolated run's pair with the kept first run, an extra
//! output selected on the first file and paired under the second, and a kept connected pair (`identify_import::open_kept_recording`,
//! B3.6, B5.2 and B10.2). The fixtures are those of `identify_import_tests.rs`, and no runtime runs here.

use super::*;

#[test]
fn a_second_file_with_one_isolated_run_pairs_with_the_kept_isolated_run_of_the_first_and_is_fitted()
{
    let project = Project::new("isolated-second");
    let (studio, _updates) = engine();
    let loop_definition = project.loop_definition();
    let first_file = archive_result(
        &loop_definition,
        &[isolated_record_json(
            &loop_definition,
            "isolated-first",
            1,
            10,
            5.0,
            240,
        )],
    );
    let first = identify(
        &studio,
        project.request(json!({ "action": "importRecording", "json": first_file.to_string() })),
    );
    assert_eq!(first["status"], "recording-needed", "{first}");
    let second_file = archive_result(
        &loop_definition,
        &[isolated_record_json(
            &loop_definition,
            "isolated-second",
            7,
            20,
            -5.0,
            240,
        )],
    );
    let second = identify(
        &studio,
        project.request(json!({ "action": "importRecording", "json": second_file.to_string() })),
    );
    assert_eq!(second["status"], "checked", "{second}");
    assert_eq!(
        second["recordingId"], first["recordingId"],
        "the kept isolated run is the estimation (B5.2)"
    );
    assert_eq!(
        second["identification"]["provisional"], false,
        "the isolated run pairs with the kept one, so the fit is checked on an independent run (B5.2)"
    );
    assert_eq!(
        project.working_study().recordings.len(),
        2,
        "both runs are kept"
    );
    assert!(nothing_started(&studio), "an import starts no operation");
}

/// An isolated run of the example loop with one more output, `symbol` in `unit`, that the loop does not bind: its value is the measured
/// output plus `offset`, and the request names it as an observation (E8.4). The verdict judges the measured output only, so it holds.
fn with_extra_output(
    loop_definition: &LoopDefinition,
    mut record: Value,
    symbol: &str,
    unit: &str,
    offset: f64,
) -> Value {
    let measured = loop_definition.signals.process_variable.symbol.clone();
    record["run"]["channels"]
        .as_array_mut()
        .expect("the run's channels")
        .push(json!({ "id": symbol, "unit": unit }));
    // An isolated run names its observed channels in its request; a connected record names none.
    if let Some(observations) = record["request"]["observations"].as_array_mut() {
        observations.push(json!({ "symbol": symbol, "unit": unit }));
    }
    for sample in record["run"]["samples"]
        .as_array_mut()
        .expect("the run's samples")
    {
        let value = sample["values"][&measured]["value"]
            .as_f64()
            .expect("the measured output");
        sample["values"][symbol] = json!({ "value": value + offset, "quality": "good" });
    }
    record
}

/// A saved result of one isolated run of the example loop with the archived output `extra` in °C, 0.25 above the measured output.
fn extra_output_file(
    loop_definition: &LoopDefinition,
    run: &str,
    seed: u64,
    step_at: u32,
    size: f64,
    extra: &str,
) -> Value {
    archive_result(
        loop_definition,
        &[with_extra_output(
            loop_definition,
            isolated_record_json(loop_definition, run, seed, step_at, size, 240),
            extra,
            "°C",
            0.25,
        )],
    )
}

/// The measured outputs of the recording selection the study keeps.
fn kept_selection_outputs(project: &Project) -> Option<Vec<String>> {
    project
        .working_study()
        .section::<InputsSection>()
        .expect("the study reads")
        .expect("the study keeps its inputs")
        .recording_selection
        .map(|selection| selection.outputs)
}

/// The Identify answer `controlStudio/open` gives the project's study (A8.6): its `identification`, which is `null` when the study
/// holds no current model for the loop.
fn opened_identification(studio: &ControlStudio, project: &Project) -> Value {
    let request = json!({
        "projectRoot": project.root.display().to_string(),
        "runtimeId": RUNTIME_ID,
        "loopId": LOOP_ID,
        "storageFolder": project.storage().display().to_string(),
    });
    let opened = studio.execute(OPEN, &[request]).expect("answered");
    opened["result"]["identification"].clone()
}

/// A fit of the recorded channel `extra`, which the loop does not bind, as the loop's measured output (B3.6): the section keeps the
/// loop's signals as its signals and the recorded channel beside them (A8.4), it is current for the loop, and `controlStudio/open`
/// answers the fit with that channel as its measurement.
fn assert_fit_reads_the_extra_output(
    studio: &ControlStudio,
    project: &Project,
    section: &IdentifySection,
    extra: &str,
) {
    let loop_definition = project.loop_definition();
    assert_eq!(
        section.signals.outputs,
        std::slice::from_ref(&loop_definition.signals.process_variable.symbol),
        "the fit keeps the loop's measured output as its signals (A8.4)"
    );
    assert_eq!(
        section
            .recorded_channels
            .as_ref()
            .map(|read| read.outputs.clone()),
        Some(vec![extra.to_string()]),
        "beside them, the recording's channel the fit read its measurement from (B3.6)"
    );
    let measured = &section.selected_model.identity().outputs[0];
    assert_eq!(
        measured.id, loop_definition.signals.process_variable.symbol,
        "the model names the loop's measured output, not the recorded channel (B3.6)"
    );
    assert_eq!(
        measured.unit, "°C",
        "the model keeps the recorded channel's unit"
    );
    assert!(
        section.is_current_for(&loop_definition),
        "the fit is current for the loop (A8.4)"
    );
    let identification = opened_identification(studio, project);
    let measurement = identification["plot"]["traces"]
        .as_array()
        .and_then(|traces| traces.iter().find(|trace| trace["role"] == "measurement"));
    assert!(
        measurement.is_some_and(|trace| trace["symbol"] == extra),
        "controlStudio/open answers the fit with the recorded channel as its measurement: {identification}"
    );
}

#[test]
fn an_extra_output_is_selected_on_the_first_file_and_the_second_file_pairs_under_it() {
    let project = Project::new("archive-extra-output");
    let (studio, _updates) = engine();
    let loop_definition = project.loop_definition();
    let extra = "P1.RecordedTemperature";
    let first_file = extra_output_file(&loop_definition, "isolated-first", 1, 10, 5.0, extra);
    let first = identify(
        &studio,
        project.request(json!({ "action": "importRecording", "json": first_file.to_string() })),
    );
    assert_eq!(first["status"], "recording-needed", "{first}");
    let offered = first["recordingCatalog"]["channels"]
        .as_array()
        .is_some_and(|channels| {
            channels
                .iter()
                .any(|channel| channel["id"] == extra && channel["unit"] == "°C")
        });
    assert!(
        offered,
        "the archived output is offered with its unit: {first}"
    );
    let first_id = first["recordingId"]
        .as_str()
        .expect("the kept recording")
        .to_string();

    // The engineer selects the archived output on the first file: the study keeps the selection, and the file still needs a second
    // recording, because an isolated run is not fitted alone (B3.6, B5.2).
    let selected = identify(
        &studio,
        project.request(json!({
            "action": "openRecording",
            "recordingId": first_id,
            "outputs": [extra]
        })),
    );
    assert_eq!(selected["status"], "recording-needed", "{selected}");
    assert_eq!(
        selected["reason"], "Open an independent recording to check the plant model.",
        "{selected}"
    );
    assert_eq!(
        selected["recordingCatalog"]["selection"]["outputs"],
        json!([extra]),
        "{selected}"
    );
    assert_eq!(
        kept_selection_outputs(&project),
        Some(vec![extra.to_string()]),
        "the study keeps the selection the engineer named"
    );
    assert!(
        nothing_started(&studio),
        "an answer from the study starts no operation"
    );

    let second_file = extra_output_file(&loop_definition, "isolated-second", 7, 20, -5.0, extra);
    let second = identify(
        &studio,
        project.request(json!({ "action": "importRecording", "json": second_file.to_string() })),
    );
    assert_eq!(second["status"], "checked", "{second}");
    assert_eq!(
        second["recordingId"],
        first_id.as_str(),
        "the first file's run is the estimation (B5.2)"
    );
    assert_eq!(second["identification"]["provisional"], false, "{second}");
    let measurement = second["identification"]["plot"]["traces"]
        .as_array()
        .and_then(|traces| traces.iter().find(|trace| trace["role"] == "measurement"));
    assert!(
        measurement.is_some_and(|trace| trace["symbol"] == extra),
        "the fit's measurement is the selected archived output: {second}"
    );
    let stored = storage::read_identify(&project.root, &project.working_study())
        .expect("the study reads")
        .expect("the fit is kept");
    assert_fit_reads_the_extra_output(&studio, &project, &stored.section, extra);
    assert_eq!(
        kept_selection_outputs(&project),
        Some(vec![extra.to_string()]),
        "reopening shows the selection the study keeps"
    );
}

#[test]
fn opening_a_kept_member_of_a_connected_pair_answers_the_pair_fit_at_once() {
    let project = Project::new("pair-open");
    let (studio, _updates) = engine();
    let loop_definition = project.loop_definition();
    let file = archive_result(
        &loop_definition,
        &[
            connected_record_json(&loop_definition, "recording-estimation", 10, 5.0, 240),
            connected_record_json(&loop_definition, "recording-validation", 20, -5.0, 240),
        ],
    );
    let pair = identify(
        &studio,
        project.request(json!({ "action": "importRecording", "json": file.to_string() })),
    );
    assert_eq!(pair["status"], "checked", "{pair}");
    let estimation = pair["recordingId"]
        .as_str()
        .expect("the estimation recording")
        .to_string();
    let reopened = identify(&studio, project.request(open_recording(&estimation)));
    assert_eq!(reopened["status"], "checked", "{reopened}");
    assert!(
        reopened.get("operationId").is_none(),
        "the pair is fitted at once, with no operation (B5.2): {reopened}"
    );
    assert_eq!(reopened["recordingId"], estimation.as_str(), "{reopened}");
    assert_eq!(
        reopened["identification"]["provisional"], false,
        "{reopened}"
    );
    assert!(
        nothing_started(&studio),
        "an answer from the study starts no operation"
    );
}

#[test]
fn a_lone_opened_connected_recording_with_an_extra_output_is_fitted_with_that_output_as_the_measurement(
) {
    let project = Project::new("lone-extra-output");
    let (studio, updates) = engine();
    let loop_definition = project.loop_definition();
    let extra = "P1.RecordedTemperature";
    let file = archive_result(
        &loop_definition,
        &[with_extra_output(
            &loop_definition,
            connected_record_json(&loop_definition, "recording-extra", 10, 5.0, 240),
            extra,
            "°C",
            0.25,
        )],
    );
    let imported = identify(
        &studio,
        project.request(json!({ "action": "importRecording", "json": file.to_string() })),
    );
    assert_eq!(imported["status"], "recording-needed", "{imported}");
    let recording_id = imported["recordingId"]
        .as_str()
        .expect("the kept recording")
        .to_string();
    // The lone connected recording offers its extra output, and the engineer selects it (B3.6).
    let started = identify(
        &studio,
        project.request(json!({
            "action": "openRecording",
            "recordingId": recording_id,
            "outputs": [extra]
        })),
    );
    assert!(
        started["operationId"].is_string(),
        "a lone connected recording is fitted by the operation: {started}"
    );
    let update = final_update(&updates);
    assert_eq!(
        update.state,
        OperationState::Finished,
        "{:?}",
        update.failure
    );
    let result = update.result.expect("the fit of the lone recording");
    assert_eq!(result["recordingId"], recording_id.as_str(), "{result}");
    assert_eq!(
        result["provisional"], true,
        "a fit from one recording is provisional (B3.5): {result}"
    );
    let measurement = result["plot"]["traces"]
        .as_array()
        .and_then(|traces| traces.iter().find(|trace| trace["role"] == "measurement"));
    assert!(
        measurement.is_some_and(|trace| trace["symbol"] == extra),
        "the fit's measurement is the selected extra output: {result}"
    );
    let stored = storage::read_identify(&project.root, &project.working_study())
        .expect("the study reads")
        .expect("the fit is kept");
    assert_fit_reads_the_extra_output(&studio, &project, &stored.section, extra);
    assert_eq!(
        stored.section.signals.inputs,
        ["P1.ManipulatedValue".to_string()],
        "the loop's own plant input is the input of the fit"
    );
}

#[test]
fn open_recording_without_an_id_fits_the_studys_last_opened_recording() {
    let project = Project::new("last-opened");
    let (studio, updates) = engine();
    let loop_definition = project.loop_definition();
    let imported = identify(
        &studio,
        project.request(json!({
            "action": "importRecording",
            "json": plant_result(&loop_definition, 240).to_string()
        })),
    );
    let opened = imported["recordingId"]
        .as_str()
        .expect("the kept recording")
        .to_string();
    // The engine fits the checked recording within the import (B3.6); the fit ends before the page's own fit can start.
    let _import_fit = final_update(&updates);
    // A recording the study keeps after it, which the engine did not open, is not the one fitted (B3.6).
    let mut later =
        storage::read_recording(&project.root, LOOP_ID, &opened).expect("the kept file reads back");
    later.opened = false;
    storage::save_recording(&project.root, &project.working_study(), &later)
        .expect("the later recording is kept");
    assert_eq!(
        project.working_study().recordings.len(),
        2,
        "the study keeps both recordings"
    );
    let started = identify(
        &studio,
        project.request(json!({ "action": "openRecording" })),
    );
    assert!(started["operationId"].is_string(), "{started}");
    let update = final_update(&updates);
    assert_eq!(
        update.state,
        OperationState::Finished,
        "{:?}",
        update.failure
    );
    let result = update.result.expect("the fit of the last opened recording");
    assert_eq!(
        result["recordingId"],
        opened.as_str(),
        "the last opened recording is fitted, not the later recording the engine did not open: {result}"
    );
}

#[test]
fn open_recording_without_an_id_refuses_a_study_that_keeps_no_opened_recording() {
    let project = Project::new("no-opened");
    let (studio, _updates) = engine();
    // `Failure::new` keeps the first sentence as the message and the whole text as the detail.
    let sentence = "The study keeps no opened recording to fit.";
    let detail = "The study keeps no opened recording to fit. Open a recording first.";
    let refused = identify(
        &studio,
        project.request(json!({ "action": "openRecording" })),
    );
    assert_eq!(refusal(&refused), (sentence, "none"), "{refused}");
    assert_eq!(refused["failure"]["detail"], detail, "{refused}");
    assert!(
        nothing_started(&studio),
        "a refused fit starts no operation"
    );
    // A study that keeps only a recording the engine did not open refuses the same way (B3.6).
    storage::save_recording(&project.root, &project.working_study(), &recording())
        .expect("the runtime recording is kept");
    let refused = identify(
        &studio,
        project.request(json!({ "action": "openRecording" })),
    );
    assert_eq!(refusal(&refused), (sentence, "none"), "{refused}");
    assert_eq!(refused["failure"]["detail"], detail, "{refused}");
    assert!(
        nothing_started(&studio),
        "a refused fit starts no operation"
    );
}
