use super::{ControlRequest, ControlResponse, ControlState};

// `status`, `health`, `tasks.stats`, `events.tail`, `events` and `faults` are answered by the
// shared request pipeline (`trust_runtime_services::status`).

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    let response = match request.r#type.as_str() {
        "config.get" => super::super::config_handlers::handle_config_get(request.id, state),
        "config.set" => super::super::config_handlers::handle_config_set(
            request.id,
            request.params.clone(),
            state,
        ),
        "historian.query" => super::super::status_handlers::handle_historian_query(
            request.id,
            request.params.clone(),
            state,
        ),
        "historian.alerts" => super::super::status_handlers::handle_historian_alerts(
            request.id,
            request.params.clone(),
            state,
        ),
        _ => return None,
    };
    Some(response)
}
