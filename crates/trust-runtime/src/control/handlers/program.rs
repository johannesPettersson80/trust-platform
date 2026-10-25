use super::{ControlRequest, ControlResponse, ControlState};

// `shutdown` and `restart` are answered by the shared request pipeline
// (`trust_runtime_services::resource::operations`), and so are the pairing requests `pair.*`
// (`trust_runtime_services::access::pairing`), through the store `ControlState::pairing`.

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    let response = match request.r#type.as_str() {
        "bytecode.reload" => super::super::program_handlers::handle_bytecode_reload(
            request.id,
            request.params.clone(),
            state,
        ),
        _ => return None,
    };
    Some(response)
}
