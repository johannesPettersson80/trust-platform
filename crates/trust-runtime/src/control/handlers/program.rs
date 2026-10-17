use super::{ControlRequest, ControlResponse, ControlState};

// `shutdown` and `restart` are answered by the shared request pipeline
// (`trust_runtime_services::resource::operations`).

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    let response = match request.r#type.as_str() {
        "bytecode.reload" => super::super::program_handlers::handle_bytecode_reload(
            request.id,
            request.params.clone(),
            state,
        ),
        "pair.start" => super::super::program_handlers::handle_pair_start(request.id, state),
        "pair.claim" => super::super::program_handlers::handle_pair_claim(
            request.id,
            request.params.clone(),
            state,
        ),
        "pair.list" => super::super::program_handlers::handle_pair_list(request.id, state),
        "pair.revoke" => super::super::program_handlers::handle_pair_revoke(
            request.id,
            request.params.clone(),
            state,
        ),
        _ => return None,
    };
    Some(response)
}
