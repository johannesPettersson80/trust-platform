use super::{ControlRequest, ControlResponse, ControlState};

mod ads;
mod comm;
mod debug;
mod fleet;
mod io;
mod program;
mod status;
mod variables;

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    status::dispatch(request, state)
        .or_else(|| ads::dispatch(request, state))
        .or_else(|| comm::dispatch(request, state))
        .or_else(|| fleet::dispatch(request, state))
        .or_else(|| io::dispatch(request, state))
        .or_else(|| debug::dispatch(request, state))
        .or_else(|| variables::dispatch(request, state))
        .or_else(|| program::dispatch(request, state))
}
