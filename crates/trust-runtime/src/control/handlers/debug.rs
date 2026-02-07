use super::{ControlRequest, ControlResponse, ControlState};

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    let response = match request.r#type.as_str() {
        "pause" => super::super::handle_pause(request.id, state),
        "resume" => super::super::handle_resume(request.id, state),
        "step_in" => super::super::handle_step(request.id, state, super::super::StepKind::In),
        "step_over" => super::super::handle_step(request.id, state, super::super::StepKind::Over),
        "step_out" => super::super::handle_step(request.id, state, super::super::StepKind::Out),
        "debug.state" => super::super::handle_debug_state(request.id, state),
        "debug.stops" => super::super::handle_debug_stops(request.id, state),
        "debug.stack" => super::super::handle_debug_stack(request.id, state),
        "debug.scopes" => {
            super::super::handle_debug_scopes(request.id, request.params.clone(), state)
        }
        "debug.variables" => {
            super::super::handle_debug_variables(request.id, request.params.clone(), state)
        }
        "debug.evaluate" => {
            super::super::handle_debug_evaluate(request.id, request.params.clone(), state)
        }
        "debug.breakpoint_locations" => super::super::handle_debug_breakpoint_locations(
            request.id,
            request.params.clone(),
            state,
        ),
        "breakpoints.set" => {
            super::super::handle_breakpoints_set(request.id, request.params.clone(), state)
        }
        "breakpoints.clear" => {
            super::super::handle_breakpoints_clear(request.id, request.params.clone(), state)
        }
        "breakpoints.list" => super::super::handle_breakpoints_list(request.id, state),
        "breakpoints.clear_all" => super::super::handle_breakpoints_clear_all(request.id, state),
        "breakpoints.clear_id" => {
            super::super::handle_breakpoints_clear_id(request.id, request.params.clone(), state)
        }
        _ => return None,
    };
    Some(response)
}
