use super::{ControlRequest, ControlResponse, ControlState};

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    let response = match request.r#type.as_str() {
        "io.list" => super::super::io_handlers::handle_io_list(request.id, state),
        "io.read" => super::super::io_handlers::handle_io_read(request.id, state),
        "io.write" => {
            super::super::io_handlers::handle_io_write(request.id, request.params.clone(), state)
        }
        "io.force" => {
            super::super::io_handlers::handle_io_force(request.id, request.params.clone(), state)
        }
        "io.unforce" => {
            super::super::io_handlers::handle_io_unforce(request.id, request.params.clone(), state)
        }
        _ => return None,
    };
    Some(response)
}
