use super::{ControlResponse, ControlState};
use serde::Deserialize;
use serde_json::{json, Value};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    recording_id: String,
    loop_revision: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatusRequest {
    recording_id: String,
    loop_revision: String,
    #[serde(default)]
    from_sample: Option<usize>,
}

pub(super) fn dispatch(
    id: u64,
    operation: &str,
    params: Option<Value>,
    state: &ControlState,
) -> Option<ControlResponse> {
    if !matches!(
        operation,
        "control.recording.start"
            | "control.recording.stop"
            | "control.recording.release"
            | "control.recording.status"
    ) {
        return None;
    }
    let execute = || -> Result<Value, String> {
        let params = params.ok_or("Recording parameters are required.")?;
        if operation == "control.recording.start" {
            let spec: crate::debug::IoRecordingSpec =
                serde_json::from_value(params).map_err(|error| error.to_string())?;
            let identity = spec.recording_id.clone();
            let metadata = state
                .metadata
                .lock()
                .map_err(|_| "runtime metadata unavailable")?;
            // A task recording samples a task the program runs; another would record nothing.
            if let Some(task) = spec.task.as_deref()
                && !metadata
                    .tasks()
                    .iter()
                    .any(|configured| configured.name.eq_ignore_ascii_case(task))
            {
                return Err(format!("The program runs no task named {task}."));
            }
            state
                .debug
                .start_io_recording_for_program(spec, &metadata)?;
            return Ok(json!({"recording_id": identity, "status": "recording"}));
        }
        if operation == "control.recording.status" {
            let request: StatusRequest =
                serde_json::from_value(params).map_err(|error| error.to_string())?;
            return serde_json::to_value(state.debug.io_recording_status(
                &request.recording_id,
                &request.loop_revision,
                request.from_sample,
            )?)
            .map_err(|error| error.to_string());
        }
        let identity: Identity =
            serde_json::from_value(params).map_err(|error| error.to_string())?;
        if operation == "control.recording.stop" {
            serde_json::to_value(
                state
                    .debug
                    .stop_io_recording(&identity.recording_id, &identity.loop_revision)?,
            )
            .map_err(|error| error.to_string())
        } else {
            state
                .debug
                .release_io_recording(&identity.recording_id, &identity.loop_revision)?;
            Ok(json!({"recording_id": identity.recording_id, "status": "released"}))
        }
    };
    Some(match execute() {
        Ok(value) => ControlResponse::ok(id, value),
        Err(reason) => ControlResponse::error_with_code(id, reason, "recording_unavailable"),
    })
}
