use super::{ControlRequest, ControlResponse, ControlState};

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    let context = super::super::plant_handlers::PlantHandlerContext::runtime(state);
    super::super::plant_handlers::dispatch(
        request.id,
        request.r#type.as_str(),
        request.params.clone(),
        &context,
    )
}
