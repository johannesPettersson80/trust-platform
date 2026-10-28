use super::{ControlRequest, ControlResponse, ControlState};

// `set`, `var.force`, `var.unforce` and `var.forced` are answered by the shared request pipeline
// (`trust_runtime_services::live_values::var_requests`).

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    let response = match request.r#type.as_str() {
        "eval" => {
            super::super::variable_handlers::handle_eval(request.id, request.params.clone(), state)
        }
        _ => return None,
    };
    Some(response)
}
