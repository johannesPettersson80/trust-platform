//! Identification experiments (specification 35, A6.5 and B3; annex E5): the plan of an experiment and its review
//! (`plan`), the status the runtime reports and the stored recovery a reopened panel reattaches to (`status`), the
//! Test records Identify consumes (`identification_record`), and where a recorded Test's data begins
//! (`recorded_experiment_start`).
//!
//! The review is split by concern: `automatic` chooses an excitation for an automatic request, `trajectory` writes
//! the points an experiment runs, `duration` holds the durations and their limits, and `number_text` prints the
//! numbers of the engineer's sentences.
//!
//! The runtime calls of the experiment client (`ControlExperimentClient` in `experiment.ts`) are not here: they are
//! the transport of `trust-lsp`, which calls the functions of this module for every decision.

mod automatic;
mod duration;
pub mod identification_record;
pub(crate) mod number_text;
pub mod plan;
pub mod recorded_experiment_start;
pub mod start;
pub mod status;
mod trajectory;

pub use duration::{experiment_recording_window_ms, project_step_recording_ms};
pub use identification_record::{
    identification_record_for_role, identification_samples_from_runtime_recording,
    retained_identification_samples_from_runtime_recording, ExperimentIdentificationLedger,
    ExperimentIdentificationRecord, ExperimentIdentificationSample,
    ExperimentIdentificationTerminal, ExperimentLedgerError, ExperimentTerminalStatus,
    IdentificationOutcome, IdentificationQualification, IdentificationRole,
    IdentificationSamplesError, SampleQuality, TerminalRestoration,
};
pub use plan::{
    build_control_experiment_plan, control_experiment_choices, control_experiment_defaults,
    ControlExperimentChoice, ControlExperimentDefaults, ControlExperimentKind,
    ControlExperimentPlan, ControlExperimentPlanResult, ControlExperimentPoint,
    ControlExperimentRequestKind, ControlRelayFeedbackPlan, ControllerMode, ExperimentModeBinding,
    ExperimentSample, ExperimentValue, PlanAbortBinding, PlanAuxiliarySignal, PlanInput,
    PlanSignal, PlanSource,
};
pub use recorded_experiment_start::{baseline_end_seconds, experiment_start_seconds};
pub use start::{checked_experiment_start, ExperimentStart};
pub use status::{
    check_recording_owner_scope, classify_control_experiment_status,
    decode_control_experiment_recovery, ControlEvidenceSource, ControlExperimentRecovery,
    ControlExperimentRecoveryScope, ControlExperimentRuntimePhase,
    DecodedControlExperimentRecovery, RecordingRecoveryBinding, RecordingRecoveryOwner,
    RestorationState,
};
