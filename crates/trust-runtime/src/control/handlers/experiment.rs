use super::{ControlRequest, ControlResponse, ControlState};

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    let response = match request.r#type.as_str() {
        "control.experiment.start" => super::super::experiment_handlers::handle_start(
            request.id,
            request.params.clone(),
            state,
        ),
        "control.experiment.status" => super::super::experiment_handlers::handle_status(
            request.id,
            request.params.clone(),
            state,
        ),
        "control.experiment.abort" => super::super::experiment_handlers::handle_abort(
            request.id,
            request.params.clone(),
            state,
        ),
        _ => return None,
    };
    Some(response)
}
