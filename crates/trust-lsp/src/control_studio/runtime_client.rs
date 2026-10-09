//! Control Studio's runtime operations (runtime protocol annex, E1 to E4) over the shared control
//! client. Each request is checked before it is sent and each answer before it is used; a refusal
//! keeps the runtime's text, and its code decides the next action (E0.4, A11).

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use trust_control::failure::Failure;
use trust_control::recording::{RecordedRun, RecordingRequest, RecordingStatus};
use trust_control::runtime::{RuntimeConnection, RuntimeStatus};
use trust_control::signals::{Inspection, InspectionRequest};

use crate::runtime_control::{ControlClient, ControlEndpoint, ControlError};

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
        let endpoint = ControlEndpoint::parse(&connection.endpoint).ok_or_else(|| {
            Failure::unreachable(&format!(
                "{} is not a runtime endpoint.",
                connection.endpoint
            ))
        })?;
        let client =
            ControlClient::connect(&endpoint, connection.token.as_deref(), REQUEST_TIMEOUT)
                .map_err(|error| Failure::unreachable(&error.to_string()))?;
        Ok(Self { client })
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

fn to_params(request: &impl serde::Serialize) -> Result<Value, Failure> {
    serde_json::to_value(request).map_err(|error| Failure::invalid_request(&error.to_string()))
}

fn decode<T: DeserializeOwned>(operation: &str, answer: Value) -> Result<T, Failure> {
    serde_json::from_value(answer).map_err(|_| Failure::invalid_response(operation))
}
