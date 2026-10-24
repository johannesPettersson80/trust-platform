use super::{ControlRequest, ControlResponse, ControlState};

mod ads;
mod comm;
mod connectors;
mod debug;
mod fleet;
mod io;
mod program;
mod status;
mod variables;

/// Answers the operations only the host has: the shared request pipeline hands every operation
/// it does not answer itself to this chain (the host's `ExtensionDispatch` port).
pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    debug::dispatch_compiled(request, state)
        .or_else(|| status::dispatch(request, state))
        .or_else(|| connectors::dispatch(request, state))
        .or_else(|| ads::dispatch(request, state))
        .or_else(|| comm::dispatch(request, state))
        .or_else(|| fleet::dispatch(request, state))
        .or_else(|| io::dispatch(request, state))
        .or_else(|| debug::dispatch(request, state))
        .or_else(|| variables::dispatch(request, state))
        .or_else(|| program::dispatch(request, state))
}
