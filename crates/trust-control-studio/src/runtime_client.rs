//! Control Studio's runtime operations (runtime protocol annex, E1 to E4) over the shared control
//! client. Each request is checked before it is sent and each answer before it is used; a refusal
//! keeps the runtime's text, and its code decides the next action (E0.4, A11).

use std::collections::BTreeMap;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
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
pub mod project_experiment_hold;
pub mod recording_coordinator;
pub mod runtime_recording;
pub mod test_experiment_guards;

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
        open_client(connection)
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
        self.client
            .request(operation, params)
            .map_err(|error| match error {
                ControlError::Transport(reason) => Failure::unreachable(&reason),
                ControlError::Refused { error, code } => {
                    Failure::from_runtime(code.as_deref().unwrap_or_default(), &error)
                }
                ControlError::Malformed(_) => Failure::invalid_response(operation),
            })
    }
}

/// The values of the loop's ST parameters in a `control.parameters.status` answer (E7.2). The rest of that answer belongs to
/// the parameter transactions.
#[derive(Deserialize)]
struct ParameterValues {
    values: BTreeMap<String, f64>,
}

/// Opens the control connection of `connection`. The reason says why it could not be opened.
fn open_client(connection: &RuntimeConnection) -> Result<ControlClient, String> {
    let endpoint = ControlEndpoint::parse(&connection.endpoint)
        .ok_or_else(|| format!("{} is not a runtime endpoint.", connection.endpoint))?;
    ControlClient::connect(&endpoint, connection.token.as_deref(), REQUEST_TIMEOUT)
        .map_err(|error| error.to_string())
}

fn to_params(request: &impl serde::Serialize) -> Result<Value, Failure> {
    serde_json::to_value(request).map_err(|error| Failure::invalid_request(&error.to_string()))
}

fn decode<T: DeserializeOwned>(operation: &str, answer: Value) -> Result<T, Failure> {
    trust_control::json::decode_value(answer).map_err(|_| Failure::invalid_response(operation))
}
