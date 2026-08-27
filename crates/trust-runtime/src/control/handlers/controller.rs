use super::{ControlRequest, ControlResponse, ControlState};
pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    super::super::controller_handlers::dispatch(
        request.id,
        request.r#type.as_str(),
        request.params.clone(),
        state,
    )
}
