//! The terminal evidence of a finished Test (`testTerminalEvidence.ts`; specification 35, A4.4, A9 and B3). A finished Test
//! keeps its runtime answer whole, together with the identification record of its samples and its recording. This happens
//! only when the answer names the reviewed plan and reports a terminal status and a restoration. The samples are the
//! recording's own. When the recording is not complete, they are the samples the recording kept as evidence. Such a record
//! never fits a model.
//!
//! Left out: stopping the recording and reading the study store, which are I/O. The caller stops the recording only when
//! [`terminal_result`] accepts the answer, as the `TypeScript` does, and then passes the stopped run in.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::experiment::{
    identification_samples_from_runtime_recording,
    retained_identification_samples_from_runtime_recording, ControlExperimentPlan,
    ExperimentIdentificationLedger, ExperimentIdentificationRecord,
    ExperimentIdentificationTerminal, ExperimentLedgerError, ExperimentTerminalStatus,
    IdentificationSamplesError, TerminalRestoration,
};
use crate::loop_definition::LoopDefinition;
use crate::recording::{Acquisition, RecordedRun};

/// A finished Test with its evidence: the `TypeScript` result object with `identification`, `recordingEvidence` and
/// `recording` added to it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalTestRecord {
    /// The runtime's answer, with every key the runtime sent.
    #[serde(flatten)]
    pub result: Map<String, Value>,
    /// The Test's record in the ledger.
    pub identification: ExperimentIdentificationRecord,
    /// The acquisition of the recording that the samples come from.
    pub recording_evidence: Acquisition,
    /// The stopped recording.
    pub recording: RecordedRun,
}

/// Why a finished Test's evidence cannot be recorded: its samples are refused on both paths, or the ledger refuses the
/// record (the answer names another experiment, or the plan already has a record).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TerminalEvidenceError {
    /// The recording's samples are refused, the complete ones and the kept ones alike.
    #[error("{0}")]
    Samples(IdentificationSamplesError),
    /// The ledger refuses the record.
    #[error("{0}")]
    Record(ExperimentLedgerError),
}

/// The runtime's answer to a Test, read for its terminal fields. Its other keys are the runtime's own and are kept by the
/// record, so they are not read here.
#[derive(Deserialize)]
struct TerminalAnswer {
    experiment_id: String,
    status: ExperimentTerminalStatus,
    restoration: TerminalRestoration,
    #[serde(default)]
    stop_reason: Value,
    #[serde(default)]
    restoration_reason: Value,
}

/// The terminal answer of a finished Test (`terminalResult`). The answer names the reviewed plan, reports a status that
/// has ended (`completed`, `aborted`, `failed` or `failed_to_restore`), and reports a restoration (`restored`,
/// `not_required` or `failed`). A reason is kept when it is text and is absent otherwise. Any other answer gives `None`.
#[must_use]
pub fn terminal_result(
    plan: &ControlExperimentPlan,
    result: &Value,
) -> Option<ExperimentIdentificationTerminal> {
    let answer: TerminalAnswer = crate::json::decode_value(result.clone()).ok()?;
    if answer.experiment_id != plan.id {
        return None;
    }
    Some(ExperimentIdentificationTerminal {
        experiment_id: answer.experiment_id,
        status: answer.status,
        stop_reason: answer.stop_reason.as_str().map(str::to_owned),
        restoration: answer.restoration,
        restoration_reason: answer.restoration_reason.as_str().map(str::to_owned),
    })
}

/// The evidence of a finished Test (`buildTerminalTestEvidence`), decided from the values the caller holds once the
/// recording has stopped. It returns `Ok(None)` when there is no plan, no loop or no terminal answer of the plan. The
/// answer then stays as the runtime sent it.
///
/// Otherwise the samples are those of the recording, or, when the recording is not complete, the samples it kept. This
/// fall-back is `Result::or_else`: the kept samples are read only when the complete ones are refused, as the `TypeScript`
/// tries the one and catches into the other. The record is written to the ledger once, and the answer returns with its
/// identification, the recording's acquisition evidence and the recording itself.
///
/// # Errors
/// The samples are refused on both paths, or the ledger refuses the record.
pub fn terminal_test_evidence(
    ledger: &mut ExperimentIdentificationLedger,
    plan: Option<&ControlExperimentPlan>,
    loop_definition: Option<&LoopDefinition>,
    result: &Value,
    run: &RecordedRun,
    start_failure: Option<&str>,
) -> Result<Option<TerminalTestRecord>, TerminalEvidenceError> {
    let (Some(plan), Some(loop_definition), Some(fields)) =
        (plan, loop_definition, result.as_object())
    else {
        return Ok(None);
    };
    let Some(terminal) = terminal_result(plan, result) else {
        return Ok(None);
    };
    let samples = identification_samples_from_runtime_recording(
        plan.kind,
        run,
        loop_definition,
        start_failure,
    )
    .or_else(|_| {
        retained_identification_samples_from_runtime_recording(plan.kind, run, loop_definition)
    })
    .map_err(TerminalEvidenceError::Samples)?;
    let identification = ledger
        .record(plan, &terminal, &samples, Some(&run.acquisition))
        .map_err(TerminalEvidenceError::Record)?;
    Ok(Some(TerminalTestRecord {
        result: fields.clone(),
        identification,
        recording_evidence: run.acquisition.clone(),
        recording: run.clone(),
    }))
}

#[cfg(test)]
#[path = "test_terminal_evidence_tests.rs"]
mod tests;
