use super::{ControlRequest, ControlResponse, ControlState};

// `pause` and `resume` are answered by the shared request pipeline
// (`trust_runtime_services::resource::operations`).

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    let response = match request.r#type.as_str() {
        "step_in" => super::super::program_handlers::handle_step(
            request.id,
            state,
            super::super::program_handlers::StepKind::In,
        ),
        "step_over" => super::super::program_handlers::handle_step(
            request.id,
            state,
            super::super::program_handlers::StepKind::Over,
        ),
        "step_out" => super::super::program_handlers::handle_step(
            request.id,
            state,
            super::super::program_handlers::StepKind::Out,
        ),
        "debug.state" => super::super::debug_handlers::handle_debug_state(request.id, state),
        "debug.stops" => super::super::debug_handlers::handle_debug_stops(request.id, state),
        "debug.stack" => super::super::debug_handlers::handle_debug_stack(request.id, state),
        "debug.scopes" => super::super::debug_handlers::handle_debug_scopes(
            request.id,
            request.params.clone(),
            state,
        ),
        "debug.variables" => super::super::debug_handlers::handle_debug_variables(
            request.id,
            request.params.clone(),
            state,
        ),
        "debug.evaluate" => super::super::debug_handlers::handle_debug_evaluate(
            request.id,
            request.params.clone(),
            state,
        ),
        "debug.breakpoint_locations" => {
            super::super::debug_handlers::handle_debug_breakpoint_locations(
                request.id,
                request.params.clone(),
                state,
            )
        }
        "breakpoints.set" => super::super::breakpoint_handlers::handle_breakpoints_set(
            request.id,
            request.params.clone(),
            state,
        ),
        "breakpoints.clear" => super::super::breakpoint_handlers::handle_breakpoints_clear(
            request.id,
            request.params.clone(),
            state,
        ),
        "breakpoints.list" => {
            super::super::breakpoint_handlers::handle_breakpoints_list(request.id, state)
        }
        "breakpoints.clear_all" => {
            super::super::breakpoint_handlers::handle_breakpoints_clear_all(request.id, state)
        }
        "breakpoints.clear_id" => super::super::breakpoint_handlers::handle_breakpoints_clear_id(
            request.id,
            request.params.clone(),
            state,
        ),
        _ => return None,
    };
    Some(response)
}
