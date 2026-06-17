use super::{ControlRequest, ControlResponse, ControlState};

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    let response = match request.r#type.as_str() {
        "comm.capabilities" => {
            super::super::comm_handlers::handle_comm_capabilities(request.id, state)
        }
        "comm.schema" => super::super::comm_handlers::handle_comm_schema(
            request.id,
            request.params.clone(),
            state,
        ),
        "comm.apply" => super::super::comm_handlers::handle_comm_apply(
            request.id,
            request.params.clone(),
            state,
        ),
        "comm.test" => {
            super::super::comm_handlers::handle_comm_test(request.id, request.params.clone())
        }
        _ => return None,
    };
    Some(response)
}
