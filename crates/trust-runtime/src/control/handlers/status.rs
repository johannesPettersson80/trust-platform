use super::{ControlRequest, ControlResponse, ControlState};

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    let response = match request.r#type.as_str() {
        "status" => super::super::handle_status(request.id, state),
        "health" => super::super::handle_health(request.id, state),
        "tasks.stats" => super::super::handle_task_stats(request.id, state),
        "events.tail" | "events" => {
            super::super::handle_events_tail(request.id, request.params.clone(), state)
        }
        "faults" => super::super::handle_faults(request.id, request.params.clone(), state),
        "config.get" => super::super::handle_config_get(request.id, state),
        "config.set" => super::super::handle_config_set(request.id, request.params.clone(), state),
        _ => return None,
    };
    Some(response)
}
