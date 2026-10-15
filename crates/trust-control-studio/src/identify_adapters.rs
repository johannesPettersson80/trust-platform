//! The production ports of an identification run (specification 35, A8.3, A9 and B3.4; runtime protocol annex E3, E4.7 and
//! E11.6): the loop's live sample, read from the runtime, and the study, which keeps each stopped recording as a recording
//! file before the runtime buffer is released. The conversion of an inspection into the run's sample is trust-control's
//! (`identification::loop_sample`), and the files are written only through `storage`, so these ports decide nothing.

use std::path::Path;

use trust_control::experiment::ExperimentSample;
use trust_control::failure::Failure;
use trust_control::identification::loop_sample::experiment_sample;
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::evidence::RecordingEvidence;
use trust_control::recording::{EvidenceSource, RecordedRun, RecordingFile};
use trust_control::runtime::RuntimeConnection;
use trust_control::signals::{
    loop_inspection_bindings, Inspection, InspectionRequest, InspectionSample,
};
use trust_control::study::Study;

use crate::runtime_client::RuntimeClient;
use crate::storage;
use crate::workflow::identify::{LoopSampler, RecordingRetention};

/// The loop's live sample on the runtime the loop is bound to (`ports.sample()`): one coherent scan of the loop's signals
/// (E3), which trust-control turns into the run's sample. A scan that is not available yet is no sample.
pub struct RuntimeSampler<'a> {
    connection: &'a RuntimeConnection,
    loop_definition: &'a LoopDefinition,
    source: EvidenceSource,
}

impl<'a> RuntimeSampler<'a> {
    /// The sampler of `loop_definition` on the runtime of `connection`, whose values come from `source` (A9): the source read
    /// from the runtime when the run starts.
    #[must_use]
    pub fn new(
        connection: &'a RuntimeConnection,
        loop_definition: &'a LoopDefinition,
        source: EvidenceSource,
    ) -> Self {
        Self {
            connection,
            loop_definition,
            source,
        }
    }
}

impl RuntimeSampler<'_> {
    /// One coherent scan of the loop's signals as the runtime answers it (E3), or `None` while no scan is available. A gain-scheduled
    /// loop's signals include its scheduling variable (`loop_inspection_bindings`).
    ///
    /// # Errors
    /// The runtime cannot be reached, refuses, or answers out of protocol.
    pub fn scan(&self) -> Result<Option<InspectionSample>, Failure> {
        let request = InspectionRequest::new(
            &self.loop_definition.revision,
            None,
            loop_inspection_bindings(self.loop_definition),
        )?;
        let inspection = RuntimeClient::connect(self.connection)?.inspect(&request)?;
        Ok(match inspection {
            Inspection::Pending { .. } => None,
            Inspection::Available(sample) => Some(sample),
        })
    }

    /// The sample that Identify reviews and records, converted from one scan (`experiment_sample`).
    #[must_use]
    pub fn sample_of(&self, scan: &InspectionSample) -> Option<ExperimentSample> {
        experiment_sample(self.loop_definition, self.source, scan)
    }
}

impl LoopSampler for RuntimeSampler<'_> {
    fn sample(&mut self) -> Result<Option<ExperimentSample>, Failure> {
        Ok(self.scan()?.and_then(|scan| self.sample_of(&scan)))
    }
}

/// The runtime recording a retention kept, and the recording file it became (A8.3).
struct KeptRecording {
    /// The recording's identifier on the runtime.
    recording_id: String,
    /// The identifier of its recording file in the study.
    file_id: String,
}

/// The loop's working study as the place a run keeps its stopped recordings (`ports.retain`): each run becomes a recording
/// file with the verdict that labels it (B10.3), written through `storage` before the runtime buffer is released (E4.7,
/// E11.6). The study it holds is always the one the last keep wrote.
pub struct StudyRetention<'a> {
    project_root: &'a Path,
    loop_id: &'a str,
    source: EvidenceSource,
    study: Study,
    kept: Vec<KeptRecording>,
}

impl<'a> StudyRetention<'a> {
    /// The working `study` of the loop `loop_id` in `project_root`, keeping recordings whose values come from `source` (A9).
    #[must_use]
    pub fn new(
        project_root: &'a Path,
        loop_id: &'a str,
        source: EvidenceSource,
        study: Study,
    ) -> Self {
        Self {
            project_root,
            loop_id,
            source,
            study,
            kept: Vec::new(),
        }
    }

    /// The study as the last keep wrote it.
    #[must_use]
    pub fn study(&self) -> &Study {
        &self.study
    }

    /// The identifier of the recording file that the runtime recording `recording_id` became, once it was kept.
    #[must_use]
    pub fn kept_file_id(&self, recording_id: &str) -> Option<&str> {
        self.kept
            .iter()
            .find(|kept| kept.recording_id == recording_id)
            .map(|kept| kept.file_id.as_str())
    }
}

impl RecordingRetention for StudyRetention<'_> {
    fn retain(&mut self, run: &RecordedRun, evidence: &RecordingEvidence) -> Result<(), Failure> {
        let file = RecordingFile::new(self.loop_id, self.source, run.clone(), evidence.clone());
        let file_id = file.id()?;
        self.study = storage::save_recording(self.project_root, &self.study, &file)?;
        self.kept.push(KeptRecording {
            recording_id: run.recording_id.clone(),
            file_id,
        });
        Ok(())
    }
}
