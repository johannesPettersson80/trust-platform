use super::{ControlRequest, ControlResponse, ControlState};

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    let response = match request.r#type.as_str() {
        "hmi.read" => {
            super::super::hmi_handlers::handle_hmi_read(request.id, request.params.clone(), state)
        }
        _ => return None,
    };
    Some(response)
}
