use super::{ControlRequest, ControlResponse, ControlState};
pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    super::super::signal_handlers::dispatch(
        request.id,
        request.r#type.as_str(),
        request.params.clone(),
        state,
    )
    .or_else(|| {
        super::super::search::dispatch(
            request.id,
            request.r#type.as_str(),
            request.params.clone(),
            state,
        )
    })
    .or_else(|| {
        super::super::parameter_handlers::dispatch(
            request.id,
            request.r#type.as_str(),
            request.params.clone(),
            state,
        )
    })
    .or_else(|| {
        super::super::transfer_handlers::dispatch(
            request.id,
            request.r#type.as_str(),
            request.params.clone(),
            state,
        )
    })
    .or_else(|| {
        super::super::recording_handlers::dispatch(
            request.id,
            request.r#type.as_str(),
            request.params.clone(),
            state,
        )
    })
    .or_else(|| {
        super::super::controller_handlers::dispatch(
            request.id,
            request.r#type.as_str(),
            request.params.clone(),
            state,
        )
    })
}
