//! The archived runs of a saved result (`engineerStudy`; specification 35, §0.1a, B3.6, B10.2 and B10.4): the completed
//! identification runs a file carries, which an import keeps as recordings and fits with independent validation
//! (`openedRecordingIdentification.ts` and `selectedRecordingIdentification.ts`). The engine reads a connected runtime record
//! (`decode_connected_identification_record`, the same decoder as a stored record) and an isolated simulation's run
//! ([`decode_isolated_run`]). A run of another execution refuses the file.

mod isolated;

use serde_json::Value;

use super::{check_import_length, ImportCheck, REFERENCE_SENTENCE};
use crate::identification::connected_record::{
    connected_identification_record, decode_connected_identification_record, ConnectedAcquisition,
    RunEvidence, RuntimeVerificationResult, ValidationChannel, ValidationRun, ValidationSample,
    ValidationValue,
};
use crate::identification::independent_validation::RequirementStatus;
use crate::identification::recorded_limits::{evaluate_recorded_limits, RecordedLimitsEvaluation};
use crate::identification::recorded_model::{check_gain_sign, transfer_gain};
use crate::identification::refusal::Refusal;
use crate::identification::saved_result::{read_saved_result, INVALID_SENTENCE, NOT_JSON_SENTENCE};
use crate::loop_definition::LoopDefinition;
use crate::model::GeneralModel;
use crate::recording::channels::{
    ChannelQuality, ACTUATOR_BINDING, MANIPULATED_VARIABLE_BINDING, PROCESS_VARIABLE_BINDING,
    SETPOINT_BINDING,
};
use crate::recording::evidence::recording_evidence;
use crate::recording::opened::{nanoseconds, opened_plan};
use crate::recording::opened_channels::CatalogChannel;
use crate::recording::{
    Acquisition, Completeness, EvidenceSource, RecordedRun, RecordedSample, RecordingFile,
    ScanRange, StopReason,
};
use crate::signals::{Quality, SignalBinding};

pub use self::isolated::decode_isolated_run;
pub use self::isolated::IsolatedRun;

/// The sentence of an archived run whose units are not the loop's: no unit conversion is applied (A9).
const UNITS_SENTENCE: &str =
    "Recording input and output units must match this loop; no unit conversion was applied.";
/// The sentence of an archived run that lacks one of the loop's plant signals (B3.6).
const SIGNALS_SENTENCE: &str =
    "The recording does not contain the selected plant signals and units.";
/// The sentence of an archived run whose samples cannot be kept as runtime samples (E4.4).
const UNREADABLE_SENTENCE: &str =
    "The archived recording has invalid samples, so it cannot be kept as a recording.";
/// The acquisition method an isolated simulation's recording names: the runtime took no scan buffer of it (E8.4).
const ISOLATED_METHOD: &str = "isolated-simulation";
/// The acquisition reason an isolated simulation's recording keeps: its samples are its whole grid.
const ISOLATED_REASON: &str =
    "The isolated simulation ran its whole grid, so its samples are complete.";

/// One archived run of a saved result: a connected runtime record (B3.4), or an isolated simulation's run (E8.4).
#[derive(Debug, Clone)]
pub enum ArchivedRun {
    /// A connected runtime record, with the connected request it was taken with.
    Connected(Box<RuntimeVerificationResult<RecordedLimitsEvaluation>>),
    /// An isolated simulation's run, checked against its frozen request.
    Isolated(Box<IsolatedRun>),
}

impl ArchivedRun {
    /// The program revision the run was taken on.
    #[must_use]
    pub fn project_revision(&self) -> &str {
        match self {
            Self::Connected(result) => result.project_revision.as_str(),
            Self::Isolated(run) => run.project_revision.as_str(),
        }
    }

    /// The run as the evaluation reads it.
    #[must_use]
    pub fn run(&self) -> &ValidationRun {
        match self {
            Self::Connected(result) => &result.run,
            Self::Isolated(run) => &run.run,
        }
    }

    /// The verdict of the run's requirements (N8.5).
    fn status(&self) -> RequirementStatus {
        match self {
            Self::Connected(result) => result.evaluation.status,
            Self::Isolated(run) => run.status,
        }
    }

    /// Whether the import archives the run: the evidence of the ST program, a Pass verdict and no selected model
    /// (`openedRecordingIdentification.ts`).
    fn is_archivable(&self) -> bool {
        self.run().evidence == RunEvidence::StExecuted
            && self.status() == RequirementStatus::Pass
            && !matches!(self, Self::Isolated(run) if run.selected_model)
    }
}

/// The archived runs of a saved result that an import fits (`archivedRuns` in `openedRecordingIdentification.ts`): the identity of
/// the result, its source, the completed runs with `ST-executed` evidence and a Pass verdict in file order, and the channels every
/// one of them holds.
#[derive(Debug, Clone)]
pub struct ArchivedRuns {
    /// The identity of the saved result, as the check of the import names it.
    pub check: ImportCheck,
    /// The source of the file's recordings (A9).
    pub source: EvidenceSource,
    /// The archived runs, in the order the file holds them.
    pub runs: Vec<ArchivedRun>,
    /// The channels every archived run holds with one unit, in the order of the first run.
    pub channels: Vec<CatalogChannel>,
}

/// The keys a campaign of an engineer's study holds (`decodeCampaign`).
const CAMPAIGN_KEYS: [&str; 4] = ["id", "status", "reason", "runs"];

/// A campaign of an engineer's study, decoded: whether it completed, and the runs it holds.
struct Campaign {
    completed: bool,
    runs: Vec<ArchivedRun>,
}

/// Reads the archived runs of a saved result (`engineerStudy` of a schema 4 result, B10.2). `Ok(None)` when the file holds no
/// engineer's study, or when it holds no completed run with `ST-executed` evidence and a Pass verdict.
///
/// # Errors
/// The text is too large, is not JSON, or does not decode as a saved result (B10.4); the file keeps a step-response reference; its
/// engineer's study does not decode as a whole (B10.4: a campaign or run with an unknown key, a duplicate identifier, a completed
/// campaign whose runs were recorded on different project revisions, or a run that is not a valid record of its execution); or an
/// archived run lacks a plant signal of the loop, or holds one in other units than the loop's (A9).
pub fn archived_runs(
    json: &str,
    loop_definition: &LoopDefinition,
) -> Result<Option<ArchivedRuns>, Refusal> {
    check_import_length(json)?;
    let value: Value = serde_json::from_str(json).map_err(|_| Refusal::new(NOT_JSON_SENTENCE))?;
    let Some(study) = value.get("engineerStudy") else {
        return Ok(None);
    };
    let saved = read_saved_result(json)?;
    if saved.step_response_reference {
        return Err(Refusal::new(REFERENCE_SENTENCE));
    }
    if value.get("schemaVersion").and_then(Value::as_u64) != Some(4) {
        return Err(invalid());
    }
    let archived: Vec<ArchivedRun> = campaigns(study)?
        .into_iter()
        .filter(|campaign| campaign.completed)
        .flat_map(|campaign| campaign.runs)
        .filter(ArchivedRun::is_archivable)
        .collect();
    if archived.is_empty() {
        return Ok(None);
    }
    let signals = &loop_definition.signals;
    for run in &archived {
        for signal in [&signals.manipulated_variable, &signals.process_variable] {
            match run
                .run()
                .channels
                .iter()
                .find(|channel| channel.id == signal.symbol)
            {
                None => return Err(Refusal::new(SIGNALS_SENTENCE)),
                Some(channel) if channel.unit != signal.unit => {
                    return Err(Refusal::new(UNITS_SENTENCE));
                }
                Some(_) => {}
            }
        }
    }
    let channels = shared_channels(&archived);
    let source = match saved.source.as_str() {
        "Simulation" => EvidenceSource::SimulatedPlant,
        "Physical" => EvidenceSource::PhysicalPlant,
        _ => return Err(invalid()),
    };
    let check = ImportCheck {
        status: "checked",
        result_id: saved.result_id,
        result_revision: saved.result_revision,
        project_id: saved.project_id,
        source: saved.source,
        samples: saved.samples,
    };
    Ok(Some(ArchivedRuns {
        check,
        source,
        runs: archived,
        channels,
    }))
}

/// The campaigns of an engineer's study, each decoded as a whole (`decodeEngineerStudy`, `decodeCampaign`).
fn campaigns(study: &Value) -> Result<Vec<Campaign>, Refusal> {
    let items = study
        .get("campaigns")
        .and_then(Value::as_array)
        .ok_or_else(invalid)?;
    let mut ids: Vec<&str> = Vec::with_capacity(items.len());
    let mut campaigns = Vec::with_capacity(items.len());
    for item in items {
        let object = item.as_object().ok_or_else(invalid)?;
        if object
            .keys()
            .any(|key| !CAMPAIGN_KEYS.contains(&key.as_str()))
        {
            return Err(invalid());
        }
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(invalid)?;
        if ids.contains(&id) {
            return Err(invalid());
        }
        ids.push(id);
        let completed = match object.get("status").and_then(Value::as_str) {
            Some("completed") => true,
            Some("failed" | "cancelled") => false,
            _ => return Err(invalid()),
        };
        let reason = object.get("reason");
        if reason.is_some_and(|reason| reason.as_str().is_none_or(str::is_empty)) {
            return Err(invalid());
        }
        if !completed && reason.is_none() {
            return Err(invalid());
        }
        let runs = object
            .get("runs")
            .and_then(Value::as_array)
            .ok_or_else(invalid)?;
        let decoded = runs
            .iter()
            .map(decode_run)
            .collect::<Result<Vec<_>, Refusal>>()?;
        if completed
            && decoded
                .windows(2)
                .any(|pair| pair[0].project_revision() != pair[1].project_revision())
        {
            return Err(invalid());
        }
        campaigns.push(Campaign {
            completed,
            runs: decoded,
        });
    }
    Ok(campaigns)
}

/// Reads one run of an engineer's study as its execution says: a connected runtime record, or an isolated simulation's run. A run of
/// another execution, or one that does not decode as its execution's record, refuses the file (B10.4).
fn decode_run(run: &Value) -> Result<ArchivedRun, Refusal> {
    match run.pointer("/request/execution").and_then(Value::as_str) {
        Some("connected-runtime") => {
            decode_connected_identification_record(&engine_form(run), evaluate_recorded_limits)
                .map(|record| ArchivedRun::Connected(Box::new(record)))
                .ok_or_else(invalid)
        }
        Some(isolated::EXECUTION) => decode_isolated_run(run)
            .map(|isolated| ArchivedRun::Isolated(Box::new(isolated)))
            .ok_or_else(invalid),
        _ => Err(invalid()),
    }
}

/// The record as the engine reads it. A record the TypeScript writes lists its observed signals in its request and repeats its run in
/// its evaluation (`connectedIdentificationRecord.ts`); the engine's record keeps neither, so both are taken out before it is read.
fn engine_form(run: &Value) -> Value {
    let mut form = run.clone();
    if let Some(request) = form.get_mut("request").and_then(Value::as_object_mut) {
        request.remove("observations");
    }
    if let Some(evaluation) = form.get_mut("evaluation").and_then(Value::as_object_mut) {
        evaluation.remove("run");
    }
    form
}

/// The channels every archived run holds with one unit, in the order of the first run (`openedRecordingCatalog`).
fn shared_channels(runs: &[ArchivedRun]) -> Vec<CatalogChannel> {
    let Some((first, rest)) = runs.split_first() else {
        return Vec::new();
    };
    first
        .run()
        .channels
        .iter()
        .filter(|channel| {
            rest.iter().all(|run| {
                run.run()
                    .channels
                    .iter()
                    .any(|other| other.id == channel.id && other.unit == channel.unit)
            })
        })
        .map(|channel| CatalogChannel {
            id: channel.id.clone(),
            unit: channel.unit.clone(),
        })
        .collect()
}

/// The two recordings a fit with independent validation reads, as indices into `runs` (`openedRecordingIdentification.ts`, B5.2).
/// The candidates are the study's kept recordings, then the file's runs, in order. The eligible candidates are those of the latest
/// candidate's project revision and model revision that hold each selected signal with its unit; the last two distinct recordings
/// among them are the estimation and the validation, in that order. Fewer than two indices means that a second recording is needed.
#[must_use]
pub fn independent_pair(runs: &[&ArchivedRun], selection: &[(&str, &str)]) -> Vec<usize> {
    let Some(&latest) = runs.last() else {
        return Vec::new();
    };
    // A recording that appears twice keeps its first place and its later run, as a `Map` keeps them.
    let mut eligible: Vec<(&str, usize)> = Vec::new();
    for (index, &run) in runs.iter().enumerate() {
        let same_revisions = run.project_revision() == latest.project_revision()
            && run.run().conditions.model_revision == latest.run().conditions.model_revision;
        let holds_selection = selection.iter().all(|(id, unit)| {
            run.run()
                .channels
                .iter()
                .any(|channel| channel.id == *id && channel.unit == *unit)
        });
        if !(same_revisions && holds_selection) {
            continue;
        }
        match eligible
            .iter_mut()
            .find(|(id, _)| *id == run.run().id.as_str())
        {
            Some(slot) => slot.1 = index,
            None => eligible.push((run.run().id.as_str(), index)),
        }
    }
    let first = eligible.len().saturating_sub(2);
    eligible[first..].iter().map(|(_, index)| *index).collect()
}

/// The archived run an opened recording the study keeps is read as. A recording kept from an isolated simulation names its model
/// revision, and reads back as that isolated run (`kept_isolated_run`, B3.6, B5.2). Any other recording reads back as its connected
/// record of the loop, built as the engine builds the record of a recording it fits (`connected_identification_record`,
/// `opened_plan`), with its extra channels added (`with_extra_channels`). A recording the record builder refuses has no run here.
#[must_use]
pub fn opened_archived_run(
    loop_definition: &LoopDefinition,
    file: &RecordingFile,
) -> Option<ArchivedRun> {
    if file.model_revision.is_some() {
        return isolated::kept_isolated_run(loop_definition, file)
            .map(|run| ArchivedRun::Isolated(Box::new(run)));
    }
    let plan = opened_plan(file, loop_definition);
    let mut record = connected_identification_record(
        loop_definition,
        &file.run,
        None,
        &plan,
        0.0,
        None,
        evaluate_recorded_limits,
    )
    .ok()?;
    with_extra_channels(&mut record.run, file);
    Some(ArchivedRun::Connected(Box::new(record)))
}

/// Adds the extra channels of a kept recording to the run of its connected record, sample by sample. The record builder names the
/// loop's own signals only, so without them a recording with an extra output would not pair under that output, nor be fitted with it
/// as the measured output (B3.6).
pub fn with_extra_channels(run: &mut ValidationRun, file: &RecordingFile) {
    let extras = file
        .run
        .bindings
        .iter()
        .enumerate()
        .filter_map(|(index, binding)| {
            Some((index, binding.symbol.as_ref()?, binding.unit.as_ref()?))
        });
    for (index, symbol, unit) in extras {
        run.channels.push(ValidationChannel {
            id: symbol.clone(),
            unit: unit.clone(),
        });
        for (sample, source) in run.samples.iter_mut().zip(&file.run.samples) {
            let value = source.values.get(index).copied().flatten();
            let quality = source.quality.get(index).copied().unwrap_or(Quality::Bad);
            sample
                .values
                .insert(symbol.clone(), validation_value(value, quality));
        }
    }
}

/// A channel value of a recording sample as a run holds it (`ValidationValue`): a missing value is bad, and otherwise the sample's
/// quality is kept (A4.2). It is the inverse of the mapping `recorded_sample` makes.
fn validation_value(value: Option<f64>, quality: Quality) -> ValidationValue {
    let quality = match quality {
        Quality::Good if value.is_some() => ChannelQuality::Good,
        Quality::Forced if value.is_some() => ChannelQuality::Forced,
        _ => ChannelQuality::Bad,
    };
    ValidationValue { value, quality }
}

/// The recording file an archived run is kept as (B10.2, A9): its samples bound to the loop's signals, its acquisition, and the
/// source of the file it came from, marked as opened. Every channel of the run is kept (B3.6): a channel the loop binds keeps the
/// loop's binding, and any other channel, an extra output, keeps its name with its unit (`archived_binding`). The archive holds no
/// wall-clock capture time, so each sample keeps `0` for it; the time is shown only for display (`RecordedSample`). An isolated
/// simulation's run names no loop revision, so its recording is kept for the loop the file is imported into.
///
/// # Errors
/// A sample's time is not a finite runtime time.
pub fn archived_recording(
    loop_definition: &LoopDefinition,
    source: EvidenceSource,
    run: &ArchivedRun,
) -> Result<RecordingFile, Refusal> {
    let channels = &run.run().channels;
    let symbols: Vec<&str> = channels.iter().map(|channel| channel.id.as_str()).collect();
    let samples = run
        .run()
        .samples
        .iter()
        .map(|sample| recorded_sample(sample, &symbols))
        .collect::<Result<Vec<_>, Refusal>>()?;
    let (loop_revision, acquisition) = match run {
        ArchivedRun::Connected(result) => (
            result.request.loop_revision.clone(),
            acquisition_of(&result.request.acquisition),
        ),
        ArchivedRun::Isolated(_) => (
            loop_definition.revision.clone(),
            isolated_acquisition(&samples),
        ),
    };
    let recording = RecordedRun {
        recording_id: run.run().id.clone(),
        loop_revision,
        program_revision: Some(run.project_revision().to_string()),
        bindings: channels
            .iter()
            .map(|channel| archived_binding(loop_definition, channel))
            .collect(),
        stop_reason: StopReason::Stopped,
        samples,
        acquisition,
    };
    let evidence = recording_evidence(&recording, None);
    let mut file = RecordingFile::new(&loop_definition.id, source, recording, evidence);
    file.opened = true;
    Ok(file)
}

/// The binding of an archived channel (B3.6, B10.2). A channel the loop binds keeps the loop's binding and no unit, so a file without
/// extra channels keeps its bytes and its identifier; any other channel is kept under its name with its unit.
fn archived_binding(
    loop_definition: &LoopDefinition,
    channel: &ValidationChannel,
) -> SignalBinding {
    let (id, unit) = match binding_id(loop_definition, &channel.id) {
        Some(id) => (id, None),
        None => (channel.id.clone(), Some(channel.unit.clone())),
    };
    SignalBinding {
        id,
        address: None,
        symbol: Some(channel.id.clone()),
        // The archive names no IEC type; an analogue signal of the loop is read as REAL.
        iec_type: "REAL".to_string(),
        unit,
    }
}

/// The checks of a fitted transfer function's gain against the loop's controller action (B3.9, N6.3): a zero gain and a gain whose sign
/// contradicts the action are refused. A model that is not a transfer function has no such gain and passes.
///
/// # Errors
/// The gain is zero, or its sign contradicts the controller action; the refusal is the sentence to show.
pub fn check_fitted_gain(
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
) -> Result<(), Refusal> {
    let GeneralModel::TransferFunction { transfer, .. } = model else {
        return Ok(());
    };
    if let Some((gain, integrating)) = transfer_gain(&transfer.numerator, &transfer.denominator) {
        check_gain_sign(loop_definition, gain, integrating)
    } else {
        Ok(())
    }
}

/// The acquisition an isolated simulation's recording keeps (E8.4): the run is complete on its whole grid, so its scan range is its
/// first and last samples, and it names no runtime scan buffer.
fn isolated_acquisition(samples: &[RecordedSample]) -> Acquisition {
    Acquisition {
        method: ISOLATED_METHOD.to_string(),
        completeness: Completeness::Complete,
        observed_sample_count: u64::try_from(samples.len()).unwrap_or(u64::MAX),
        scan_range: samples
            .first()
            .zip(samples.last())
            .map(|(first, last)| ScanRange {
                first: first.scan,
                last: last.scan,
            }),
        missing_scan_ranges: Vec::new(),
        dropped_observed_samples: 0,
        actual_cycle_timing_available: true,
        freshness_available: false,
        reason: ISOLATED_REASON.to_string(),
    }
}

/// The binding identifier of the loop signal named `symbol`, or `None` when the loop has no signal of that name.
fn binding_id(loop_definition: &LoopDefinition, symbol: &str) -> Option<String> {
    let signals = &loop_definition.signals;
    if signals.process_variable.symbol == symbol {
        return Some(PROCESS_VARIABLE_BINDING.to_string());
    }
    if signals.manipulated_variable.symbol == symbol {
        return Some(MANIPULATED_VARIABLE_BINDING.to_string());
    }
    if signals
        .setpoint
        .as_ref()
        .is_some_and(|signal| signal.symbol == symbol)
    {
        return Some(SETPOINT_BINDING.to_string());
    }
    if signals
        .actuator
        .as_ref()
        .is_some_and(|signal| signal.symbol == symbol)
    {
        return Some(ACTUATOR_BINDING.to_string());
    }
    signals
        .additional
        .iter()
        .flatten()
        .find(|signal| signal.symbol == symbol)
        .map(|signal| signal.id.clone())
}

/// One sample of an archived run as a recording sample, with one value and quality per channel, in the order of `symbols`.
fn recorded_sample(sample: &ValidationSample, symbols: &[&str]) -> Result<RecordedSample, Refusal> {
    let runtime_time_ns = nanoseconds(sample.time_seconds).ok_or_else(unreadable)?;
    let mut values: Vec<Option<f64>> = Vec::with_capacity(symbols.len());
    let mut quality: Vec<Quality> = Vec::with_capacity(symbols.len());
    for symbol in symbols {
        let reading = sample.values.get(*symbol);
        values.push(reading.and_then(|reading| reading.value));
        quality.push(match reading {
            Some(ValidationValue {
                value: Some(_),
                quality: ChannelQuality::Good,
            }) => Quality::Good,
            Some(ValidationValue {
                value: Some(_),
                quality: ChannelQuality::Forced,
            }) => Quality::Forced,
            _ => Quality::Bad,
        });
    }
    let forced: Vec<bool> = quality
        .iter()
        .map(|quality| *quality == Quality::Forced)
        .collect();
    Ok(RecordedSample {
        scan: sample.sequence,
        runtime_time_ns: Some(runtime_time_ns),
        captured_at_ms: 0,
        values,
        forced,
        quality,
        gap: false,
    })
}

/// The acquisition evidence of a connected record, as the recording keeps it (E4.5).
fn acquisition_of(acquisition: &ConnectedAcquisition) -> Acquisition {
    Acquisition {
        method: acquisition.method.clone(),
        completeness: acquisition.completeness,
        observed_sample_count: acquisition.observed_sample_count,
        scan_range: acquisition.scan_range,
        missing_scan_ranges: acquisition.missing_scan_ranges.clone(),
        dropped_observed_samples: acquisition.dropped_observed_samples,
        actual_cycle_timing_available: acquisition.actual_cycle_timing_available,
        freshness_available: acquisition.freshness_available,
        reason: acquisition.reason.clone(),
    }
}

fn invalid() -> Refusal {
    Refusal::new(INVALID_SENTENCE)
}

fn unreadable() -> Refusal {
    Refusal::new(UNREADABLE_SENTENCE)
}

#[cfg(test)]
#[path = "archive_tests.rs"]
mod tests;
