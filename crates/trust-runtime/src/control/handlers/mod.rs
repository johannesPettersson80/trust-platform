use super::{ControlRequest, ControlResponse, ControlState};

mod ads;
mod comm;
mod connectors;
mod controller;
mod debug;
mod experiment;
mod fleet;
mod hmi;
mod io;
mod plant;
mod program;
mod status;
mod variables;

/// Answers the operations only the host has: the shared request pipeline hands every operation
/// it does not answer itself to this chain (the host's `ExtensionDispatch` port).
pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    debug::dispatch_compiled(request, state)
        .or_else(|| status::dispatch(request, state))
        .or_else(|| controller::dispatch(request, state))
        .or_else(|| connectors::dispatch(request, state))
        .or_else(|| ads::dispatch(request, state))
        .or_else(|| comm::dispatch(request, state))
        .or_else(|| fleet::dispatch(request, state))
        .or_else(|| hmi::dispatch(request, state))
        .or_else(|| plant::dispatch(request, state))
        .or_else(|| experiment::dispatch(request, state))
        .or_else(|| variables::dispatch(request, state))
        .or_else(|| program::dispatch(request, state))
        .or_else(|| unsupported_plant_operation(request))
}

/// A `plant.*` operation that no handler answers is refused with the plant contract's
/// `unsupported_operation` (specification 47 §20, §20.1), not as an unknown request.
fn unsupported_plant_operation(request: &ControlRequest) -> Option<ControlResponse> {
    request.r#type.starts_with("plant.").then(|| {
        ControlResponse::error_with_code(
            request.id,
            "unsupported plant operation".into(),
            "unsupported_operation",
        )
    })
}
