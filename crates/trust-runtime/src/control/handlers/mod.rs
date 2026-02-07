use super::{ControlRequest, ControlResponse, ControlState};

mod debug;
mod io;
mod program;
mod status;
mod variables;

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    status::dispatch(request, state)
        .or_else(|| io::dispatch(request, state))
        .or_else(|| debug::dispatch(request, state))
        .or_else(|| variables::dispatch(request, state))
        .or_else(|| program::dispatch(request, state))
}
