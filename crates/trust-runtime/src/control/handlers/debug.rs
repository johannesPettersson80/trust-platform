use super::{ControlRequest, ControlResponse, ControlState};
use trust_runtime_services::live_values::io_requests::IoQueuePort;

/// The debug operations of the program, which its debugger answers with the shared operations, as
/// a device answers them (specification 44 §13.2, §8.8); none without a debugger.
pub(super) fn dispatch_compiled(
    request: &ControlRequest,
    state: &ControlState,
) -> Option<ControlResponse> {
    if !trust_runtime_services::debug::is_debug_operation(request.r#type.as_str()) {
        return None;
    }
    let debugger = state.debug.compiled()?;
    Some(debugger.answer(request, state.io_snapshot()))
}

// `pause` and `resume` are answered by the shared request pipeline
// (`trust_runtime_services::resource::operations`).
