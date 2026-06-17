use super::{ControlRequest, ControlResponse, ControlState};

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    let response = match request.r#type.as_str() {
        "fleet.topology" => super::super::fleet_handlers::handle_fleet_topology(request.id, state),
        _ => return None,
    };
    Some(response)
}
