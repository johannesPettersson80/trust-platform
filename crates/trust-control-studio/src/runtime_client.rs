//! Control Studio's runtime operations (runtime protocol annex, E1 to E4) over the shared control
//! client. Each request is checked before it is sent and each answer before it is used; a refusal
//! keeps the runtime's text, and its code decides the next action (E0.4, A11).

use std::collections::hash_map::RandomState;
use std::collections::BTreeMap;
use std::hash::BuildHasher;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use trust_control::commissioning::commissioning_result::TransferStatus;
use trust_control::commissioning::confirmation::Timing;
use trust_control::commissioning::parameter_application::{
    ApplyRequest, CancelResponse, ParameterOwner, RollbackRequest, RuntimeBinding,
};
use trust_control::commissioning::parameter_status::ParameterStatus;
use trust_control::commissioning::transfer::TransferBinding;
use trust_control::commissioning::transfer_protocol::{
    TransferApplyRequest, TransferCancelAnswer, TransferCancelRequest,
};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::reconcile::StatusRead;
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::{RecordedRun, RecordingRequest, RecordingStatus};
use trust_control::runtime::{RuntimeConnection, RuntimeStatus};
use trust_control::signals::{Inspection, InspectionRequest};
use trust_control::validation::isolated_run_protocol::{
    decode_status, ready_project_revision, IsolatedRunRequest, IsolatedRunStart, IsolatedRunStatus,
};

use trust_runtime_client::{ControlClient, ControlEndpoint, ControlError};

pub mod experiment;
pub mod experiment_recording;
pub mod experiment_status_poller;
pub mod parameters;
pub mod project_experiment_hold;
pub mod recording_coordinator;
pub mod runtime_recording;
pub mod test_experiment_guards;
pub mod transfers;

#[cfg(test)]
pub(crate) mod fake_runtime;
#[cfg(test)]
mod status_read_tests;

/// How long one request may wait for its answer.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// A connection to the runtime a study is bound to.
#[derive(Debug)]
pub struct RuntimeClient {
    client: ControlClient,
}

impl RuntimeClient {
    /// Connects to the runtime of `connection`.
    ///
    /// # Errors
    /// The endpoint is not an endpoint, or cannot be reached.
    pub fn connect(connection: &RuntimeConnection) -> Result<Self, Failure> {
        Self::connect_within(connection, REQUEST_TIMEOUT)
    }

    /// Connects to the runtime of `connection`, every request of the connection waiting at most `timeout`.
    ///
    /// # Errors
    /// The endpoint is not an endpoint, or cannot be reached.
    pub fn connect_within(
        connection: &RuntimeConnection,
        timeout: Duration,
    ) -> Result<Self, Failure> {
        open_client_within(connection, timeout)
            .map(|client| Self { client })
            .map_err(|reason| Failure::unreachable(&reason))
    }

    /// The runtime facts of E1.3.
    ///
    /// # Errors
    /// The runtime cannot be reached, refuses, or answers out of protocol.
    pub fn status(&mut self) -> Result<RuntimeStatus, Failure> {
        let answer = self.call("status", None)?;
        decode("status", answer)
    }

    /// One coherent scan of the request's signals (E3).
    ///
    /// # Errors
    /// The runtime cannot be reached, refuses, or answers out of protocol.
    pub fn inspect(&mut self, request: &InspectionRequest) -> Result<Inspection, Failure> {
        let answer = self.call("control.signals.inspect", Some(to_params(request)?))?;
        request.decode(&answer)
    }

    /// Starts a recording (E4.2); its identifier must already be journaled (E11.2, E11.3).
    ///
    /// # Errors
    /// The request breaks a limit, or the runtime refuses or answers out of protocol.
    pub fn start_recording(&mut self, request: &RecordingRequest) -> Result<(), Failure> {
        request.check()?;
        let answer = self.call("control.recording.start", Some(to_params(request)?))?;
        if answer != json!({ "recording_id": request.recording_id, "status": "recording" }) {
            return Err(Failure::invalid_response("control.recording.start"));
        }
        Ok(())
    }

    /// The recording's state and the page of samples from `from_sample` on (E4.6).
    ///
    /// # Errors
    /// The runtime cannot be reached, refuses, or answers out of protocol.
    pub fn recording_status(
        &mut self,
        request: &RecordingRequest,
        from_sample: Option<u64>,
    ) -> Result<RecordingStatus, Failure> {
        let mut params =
            json!({ "recording_id": request.recording_id, "loop_revision": request.loop_revision });
        if let Some(from_sample) = from_sample {
            params["from_sample"] = json!(from_sample);
        }
        let status: RecordingStatus = decode(
            "control.recording.status",
            self.call("control.recording.status", Some(params))?,
        )?;
        if status.recording_id != request.recording_id {
            return Err(Failure::invalid_response("control.recording.status"));
        }
        Ok(status)
    }

    /// Stops the recording and returns its run, checked against the request (E4.7). A stop is
    /// idempotent, so it may be retried with the same identifier (E11.4).
    ///
    /// # Errors
    /// The runtime cannot be reached, refuses, or answers out of protocol.
    pub fn stop_recording(&mut self, request: &RecordingRequest) -> Result<RecordedRun, Failure> {
        let params =
            json!({ "recording_id": request.recording_id, "loop_revision": request.loop_revision });
        let run: RecordedRun = decode(
            "control.recording.stop",
            self.call("control.recording.stop", Some(params))?,
        )?;
        run.check_answers(request)?;
        Ok(run)
    }

    /// Frees the recording's slot; only after its run is retained (E4.7, E11.6).
    ///
    /// # Errors
    /// The runtime cannot be reached, refuses, or answers out of protocol.
    pub fn release_recording(&mut self, request: &RecordingRequest) -> Result<(), Failure> {
        let params =
            json!({ "recording_id": request.recording_id, "loop_revision": request.loop_revision });
        let answer = self.call("control.recording.release", Some(params))?;
        if answer != json!({ "recording_id": request.recording_id, "status": "released" }) {
            return Err(Failure::invalid_response("control.recording.release"));
        }
        Ok(())
    }

    /// Starts an isolated test run (E8.4). The request is checked before it is sent, and the answer
    /// must name its identifier and revision. Studio journals the identifier before this call (E11.2);
    /// a lost acknowledgement is read by that identifier and never started again (E11.4).
    ///
    /// # Errors
    /// The request breaks a limit or rule of E8.4, the runtime refuses or cannot be reached, or its
    /// answer is not the start of this request.
    pub fn start_isolated_run(
        &mut self,
        request: &IsolatedRunRequest,
    ) -> Result<IsolatedRunStart, Failure> {
        request.check()?;
        let answer = self.call("control.simulation.run.start", Some(to_params(request)?))?;
        let start: IsolatedRunStart = decode("control.simulation.run.start", answer)?;
        start.check(request)?;
        Ok(start)
    }

    /// Reads the status of an isolated test run (E8.4). With `include_results` set to `Some(true)`
    /// the answer carries the run once the job has ended; a thin answer (`Some(false)` or none) never
    /// carries it. The run's samples are checked against the request's grid and conditions (E8.3).
    ///
    /// # Errors
    /// The request breaks a rule of E8.4, the runtime refuses or cannot be reached, or its answer does
    /// not describe this request's job and run.
    pub fn isolated_run_status(
        &mut self,
        request: &IsolatedRunRequest,
        include_results: Option<bool>,
    ) -> Result<IsolatedRunStatus, Failure> {
        request.check()?;
        let mut params = json!({
            "job_id": request.request_id,
            "expected_project_revision": request.expected_project_revision,
        });
        if let Some(include) = include_results {
            params["include_results"] = json!(include);
        }
        let status = decode_status(self.call("control.simulation.run.status", Some(params))?)?;
        status.check(request, include_results)?;
        Ok(status)
    }

    /// Cancels an isolated test run (E8.4) and returns its status. The answer never carries the run's
    /// samples; the job becomes `cancelled` once its child process has ended.
    ///
    /// # Errors
    /// As for [`Self::isolated_run_status`].
    pub fn cancel_isolated_run(
        &mut self,
        request: &IsolatedRunRequest,
    ) -> Result<IsolatedRunStatus, Failure> {
        request.check()?;
        let params = json!({
            "job_id": request.request_id,
            "expected_project_revision": request.expected_project_revision,
            "include_results": false,
        });
        let status = decode_status(self.call("control.simulation.run.cancel", Some(params))?)?;
        status.check(request, Some(false))?;
        Ok(status)
    }

    /// The project revision an isolated run starts on, from the runtime's readiness answer: `control.simulation.run.status`
    /// with no job (E8.2). A program that is not ready for an isolated run is refused with the runtime's own reason.
    ///
    /// # Errors
    /// The runtime cannot be reached or refuses, its answer is out of protocol, the program is not ready (the reason is the
    /// failure's text), or the runtime does not support the request identifiers that the journal needs (E0.2, E11.2).
    pub fn isolated_run_readiness(&mut self) -> Result<String, Failure> {
        let answer = self.call("control.simulation.run.status", Some(json!({})))?;
        ready_project_revision(answer)
    }

    /// The configured plant the runtime reports (`plant.read`, as `readConfiguredPlantBinding` reads it). The answer is
    /// decoded by trust-control (`decode_configured_plant_binding`), so a plant the saved configuration does not describe
    /// is the caller's to read as no binding (B5.3).
    ///
    /// # Errors
    /// The runtime cannot be reached or refuses the request.
    pub fn plant_read(&mut self) -> Result<Value, Failure> {
        self.call(
            "plant.read",
            Some(json!({ "contract": "trust.plant", "schema_version": 1 })),
        )
    }

    /// The plant status of `project_id` (`plant.status`, specification 34, §20.4), as the runtime answers it, with the refusal
    /// code of a refused request. Trust-control decides what the answer means.
    ///
    /// # Errors
    /// The runtime cannot be reached, or refuses the request.
    pub fn plant_status(&mut self, project_id: &str) -> Result<Value, CallFailure> {
        self.call_coded(
            "plant.status",
            Some(
                json!({ "contract": "trust.plant", "schema_version": 1, "project_id": project_id }),
            ),
        )
    }

    /// The runtime's I/O list (`io.list`), which gives the address of each variable by its name: the address of a plant
    /// input's Manual command is read from it (C7).
    ///
    /// # Errors
    /// The runtime cannot be reached or refuses the request.
    pub fn read_io_list(connection: &RuntimeConnection) -> Result<Value, Failure> {
        let mut client = Self::connect(connection)?;
        client.call("io.list", None)
    }

    /// The values of the loop's bound ST parameters, as the runtime reports them (`values` of `control.parameters.status`,
    /// E7.2). These are the settings the controller runs with now (B5.3).
    ///
    /// # Errors
    /// The loop declares no parameter binding (its setup is the next action), the runtime cannot be reached or refuses, or its
    /// answer is out of protocol.
    pub fn parameter_values(
        &mut self,
        loop_definition: &LoopDefinition,
    ) -> Result<BTreeMap<String, f64>, Failure> {
        let binding = loop_definition.parameter_binding.as_ref().ok_or_else(|| {
            Failure::new(
                "Declare the ST parameter binding before application.",
                Cause::Settings,
                NextAction::OpenLoopSetup,
            )
        })?;
        let answer = self.call(
            "control.parameters.status",
            Some(json!({ "binding": binding })),
        )?;
        let status: ParameterValues = decode("control.parameters.status", answer)?;
        Ok(status.values)
    }

    /// The transfer status of the loop's binding, as the runtime reports it (`control.transfer.status`, E6.3). The caller checks it.
    ///
    /// # Errors
    /// The runtime cannot be reached, refuses, or answers out of protocol.
    pub fn transfer_status(
        &mut self,
        binding: &TransferBinding,
    ) -> Result<TransferStatus, Failure> {
        let answer = self.call(
            "control.transfer.status",
            Some(json!({ "binding": binding })),
        )?;
        decode("control.transfer.status", answer)
    }

    /// Sends a transfer apply (`control.transfer.apply`, E6.5). The identifier must already be journaled (E11.2). See
    /// [`Self::transfer_apply_coded`] for the refusal code, which the transfer session needs.
    ///
    /// # Errors
    /// The runtime refuses the request, cannot be reached, or answers out of protocol.
    pub fn transfer_apply(&mut self, request: &TransferApplyRequest) -> Result<Value, Failure> {
        self.transfer_apply_coded(request)
            .map_err(|refusal| refusal.failure)
    }

    /// Sends a transfer apply and keeps the runtime's refusal code with its failure (E0.4). A refusal with `invalid_transfer_request`
    /// means the transaction was never accepted, which decides whether its owner is released.
    ///
    /// # Errors
    /// The runtime refuses the request, cannot be reached, or answers out of protocol; the failure carries the refusal code.
    pub fn transfer_apply_coded(
        &mut self,
        request: &TransferApplyRequest,
    ) -> Result<Value, CallFailure> {
        let params = to_params(request).map_err(CallFailure::without_code)?;
        self.call_coded("control.transfer.apply", Some(params))
    }

    /// Cancels a transfer by its identifier (`control.transfer.cancel`, E6.10). A lost answer is read by the caller, never by a resend.
    ///
    /// # Errors
    /// The runtime refuses the request, cannot be reached, or answers out of protocol.
    pub fn transfer_cancel(
        &mut self,
        request: &TransferCancelRequest,
    ) -> Result<TransferCancelAnswer, Failure> {
        let answer = self.call("control.transfer.cancel", Some(to_params(request)?))?;
        decode("control.transfer.cancel", answer)
    }

    /// The parameter record of the binding, as the runtime reports it (`control.parameters.status`, E7.2). The caller checks it.
    ///
    /// # Errors
    /// The runtime cannot be reached, refuses, or answers out of protocol.
    pub fn parameter_status(
        &mut self,
        binding: &RuntimeBinding,
    ) -> Result<ParameterStatus, Failure> {
        let answer = self.call(
            "control.parameters.status",
            Some(json!({ "binding": binding })),
        )?;
        decode("control.parameters.status", answer)
    }

    /// Sends a parameter apply (`control.parameters.apply`, E7.3). The identifier must already be journaled (E11.2).
    ///
    /// # Errors
    /// The runtime refuses the request, cannot be reached, or answers out of protocol.
    pub fn parameter_apply(&mut self, request: &ApplyRequest) -> Result<Value, Failure> {
        self.call("control.parameters.apply", Some(to_params(request)?))
    }

    /// Sends a parameter rollback (`control.parameters.rollback`, E7.5).
    ///
    /// # Errors
    /// The runtime refuses the request, cannot be reached, or answers out of protocol.
    pub fn parameter_rollback(&mut self, request: &RollbackRequest) -> Result<Value, Failure> {
        self.call("control.parameters.rollback", Some(to_params(request)?))
    }

    /// Cancels a parameter operation by its identity (`control.parameters.cancel`, E7.6).
    ///
    /// # Errors
    /// The runtime refuses the request, cannot be reached, or answers out of protocol.
    pub fn parameter_cancel(
        &mut self,
        owner: &ParameterOwner,
        loop_id: &str,
    ) -> Result<CancelResponse, Failure> {
        let answer = self.call(
            "control.parameters.cancel",
            Some(json!({
                "loop_id": loop_id,
                "transaction_id": owner.id,
                "expected_program_revision": owner.program,
                "expected_binding_revision": owner.binding,
            })),
        )?;
        decode("control.parameters.cancel", answer)
    }

    /// The confirmation timing of a change on `task_name`, from the runtime's status (`confirmationTiming`, A7). A status that cannot be
    /// read counts as an interval of zero, so the window is the floor of ten seconds.
    #[must_use]
    pub fn confirmation_timing(&mut self, task_name: &str) -> Timing {
        let status = self.call("status", None).ok();
        trust_control::commissioning::confirmation::confirmation_timing(status.as_ref(), task_name)
    }

    /// Reads the status of `experiment_id` over a connection of its own and says what the read gave:
    /// the status record as the runtime sent it, the runtime's refusal with its code and text, or
    /// no answer that can be read (E5.6, E11.4). Nothing is written.
    ///
    /// The record is not checked here, because the rules in `trust-control` decide what it means.
    /// The refusal keeps its code, because an identifier that the runtime does not hold
    /// (`experiment_not_found`) is absent, and not merely unreadable (E0.4). `call` cannot give
    /// this, since it turns a refusal into a [`Failure`] without its code.
    #[must_use]
    pub fn read_experiment_status(
        connection: &RuntimeConnection,
        experiment_id: &str,
    ) -> StatusRead {
        let mut client = match open_client(connection) {
            Ok(client) => client,
            Err(reason) => return StatusRead::Unreadable(reason),
        };
        let reply = client.request(
            "control.experiment.status",
            Some(json!({ "experiment_id": experiment_id })),
        );
        match reply {
            Ok(answer) => StatusRead::Answer(answer),
            Err(ControlError::Refused { error, code }) => StatusRead::Refused { code, text: error },
            Err(error) => StatusRead::Unreadable(error.to_string()),
        }
    }

    fn call(&mut self, operation: &str, params: Option<Value>) -> Result<Value, Failure> {
        self.call_coded(operation, params)
            .map_err(|refusal| refusal.failure)
    }

    /// The same call as [`Self::call`], which keeps the refusal code of the runtime's answer (E0.4).
    fn call_coded(&mut self, operation: &str, params: Option<Value>) -> Result<Value, CallFailure> {
        self.client
            .request(operation, params)
            .map_err(|error| match error {
                ControlError::Transport(reason) => {
                    CallFailure::without_code(Failure::unreachable(&reason))
                }
                ControlError::Refused { error, code } => {
                    let failure =
                        Failure::from_runtime(code.as_deref().unwrap_or_default(), &error);
                    CallFailure {
                        failure,
                        refusal_code: code,
                    }
                }
                ControlError::Malformed(_) => {
                    CallFailure::without_code(Failure::invalid_response(operation))
                }
            })
    }
}

/// A call that the runtime did not complete: its failure, and the code of the refusal when the runtime refused the request (E0.4). The
/// code decides whether a transaction was refused before any effect.
#[derive(Debug)]
pub struct CallFailure {
    /// The failure that the caller reports.
    pub failure: Failure,
    /// The runtime's refusal code; `None` when the call was not refused with one.
    pub refusal_code: Option<String>,
}

impl CallFailure {
    /// A failure that carries no refusal code: the runtime was not reached, or its answer was out of protocol.
    fn without_code(failure: Failure) -> Self {
        Self {
            failure,
            refusal_code: None,
        }
    }
}

/// A new transaction identifier: `transaction-` and 32 hex digits drawn from the random seeds of the standard hash map, the same way as a
/// recording identifier (E0.2). Studio journals it before its request is sent (E11.2).
#[must_use]
pub fn new_transaction_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let state = RandomState::new();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "transaction-{:016x}{:016x}",
        state.hash_one(count),
        state.hash_one(!count)
    )
}

/// The values of the loop's ST parameters in a `control.parameters.status` answer (E7.2). The rest of that answer belongs to
/// the parameter transactions.
#[derive(Deserialize)]
struct ParameterValues {
    values: BTreeMap<String, f64>,
}

/// Opens the control connection of `connection`. The reason says why it could not be opened.
fn open_client(connection: &RuntimeConnection) -> Result<ControlClient, String> {
    open_client_within(connection, REQUEST_TIMEOUT)
}

/// Opens the control connection of `connection`, every read and write of it waiting at most `timeout`.
fn open_client_within(
    connection: &RuntimeConnection,
    timeout: Duration,
) -> Result<ControlClient, String> {
    let endpoint = ControlEndpoint::parse(&connection.endpoint)
        .ok_or_else(|| format!("{} is not a runtime endpoint.", connection.endpoint))?;
    ControlClient::connect(&endpoint, connection.token.as_deref(), timeout)
        .map_err(|error| error.to_string())
}

fn to_params(request: &impl serde::Serialize) -> Result<Value, Failure> {
    serde_json::to_value(request).map_err(|error| Failure::invalid_request(&error.to_string()))
}

fn decode<T: DeserializeOwned>(operation: &str, answer: Value) -> Result<T, Failure> {
    trust_control::json::decode_value(answer).map_err(|_| Failure::invalid_response(operation))
}
