//! The runtime a run works on, over the shared control connection (runtime protocol annex E4, E5.3, E5.6 and E6): its recordings
//! through the runtime's recording session, the campaign's experiment client, the run's own start and status reads, and the
//! connection its mode change transfers the loop on.

use std::sync::Arc;

use serde_json::Value;
use trust_control::experiment::{ControlExperimentPlan, RecordingRecoveryOwner};
use trust_control::failure::Failure;
use trust_control::identification::connected_run::StartRead;
use trust_control::identification::reconcile::StatusRead;
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::evidence::RecordingEvidence;
use trust_control::recording::RecordedRun;
use trust_control::runtime::RuntimeConnection;

use super::{IdentifyRecording, IdentifyRuntime};
use crate::runtime_client::runtime_recording::{
    PendingReleases, RecordingOwnerPersistence, RecordingStartError, RecordingStartPlan,
    RuntimeRecordingSession,
};
use crate::runtime_client::RuntimeClient;
use crate::workflow::identify_campaign::ConnectedExperiment;

/// The runtime of a study's connection (E11.1), with the releases its recordings left pending.
pub struct ConnectedRuntime<'a> {
    connection: &'a RuntimeConnection,
    pending: &'a Arc<PendingReleases>,
}

impl<'a> ConnectedRuntime<'a> {
    /// The runtime of `connection`; a recording whose release fails stays in `pending` for the next start.
    #[must_use]
    pub fn new(connection: &'a RuntimeConnection, pending: &'a Arc<PendingReleases>) -> Self {
        Self {
            connection,
            pending,
        }
    }
}

impl IdentifyRuntime for ConnectedRuntime<'_> {
    type Recording = RuntimeRecordingSession;
    type Experiment = ConnectedExperiment;

    fn start_recording(
        &self,
        loop_definition: &LoopDefinition,
        plan: &RecordingStartPlan,
        persistence: &dyn RecordingOwnerPersistence,
    ) -> Result<RuntimeRecordingSession, RecordingStartError<RuntimeRecordingSession>> {
        RuntimeRecordingSession::start(
            self.connection,
            loop_definition,
            plan,
            Some(persistence),
            self.pending,
        )
    }

    fn recover_recording(
        &self,
        loop_definition: &LoopDefinition,
        owner: &RecordingRecoveryOwner,
    ) -> Result<RuntimeRecordingSession, Failure> {
        RuntimeRecordingSession::from_recovery_owner(
            self.connection,
            loop_definition,
            owner,
            self.pending,
        )
    }

    fn experiment(&self) -> ConnectedExperiment {
        ConnectedExperiment::new(self.connection.clone())
    }

    /// The start keeps the runtime's refusal code, because the code decides whether the start is reconciled (E0.4, E11.4).
    fn start_experiment(&self, plan: &ControlExperimentPlan) -> StartRead {
        RuntimeClient::start_experiment_read(self.connection, plan)
    }

    fn experiment_status(&self, experiment_id: &str) -> StatusRead {
        RuntimeClient::read_experiment_status(self.connection, experiment_id)
    }

    /// The study's connection: the mode change transfers the loop on the runtime the run works on (E11.1, annex E6).
    fn transfer_connection(&self) -> Result<RuntimeConnection, Failure> {
        Ok(self.connection.clone())
    }

    fn io_list(&self) -> Result<Value, Failure> {
        RuntimeClient::read_io_list(self.connection)
    }
}

/// The runtime's recording session as a run uses it. Its owner and evidence are the session's own, called by its type because
/// the methods share their names.
impl IdentifyRecording for RuntimeRecordingSession {
    fn owner(&self) -> RecordingRecoveryOwner {
        RuntimeRecordingSession::recovery_owner(self)
    }

    fn evidence(&self, run: &RecordedRun) -> RecordingEvidence {
        RuntimeRecordingSession::evidence(self, run)
    }
}
