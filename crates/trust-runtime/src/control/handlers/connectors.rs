use super::{ControlRequest, ControlResponse, ControlState};

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    let response = match request.r#type.as_str() {
        "connectors.status" => {
            super::super::connectors_handlers::handle_connectors_status(request.id, state)
        }
        _ => return None,
    };
    Some(response)
}
