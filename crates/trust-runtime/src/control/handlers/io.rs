use super::{ControlRequest, ControlResponse, ControlState};

// `io.list`, `io.read`, `io.write`, `io.force` and `io.unforce` are answered by the shared request
// pipeline (`trust_runtime_services::live_values::io_requests`).

pub(super) fn dispatch(request: &ControlRequest, state: &ControlState) -> Option<ControlResponse> {
    let response = match request.r#type.as_str() {
        "hmi.schema.get" => super::super::hmi_handlers::handle_hmi_schema_get(request.id, state),
        "hmi.values.get" => super::super::hmi_handlers::handle_hmi_values_get(
            request.id,
            request.params.clone(),
            state,
        ),
        "hmi.trends.get" => super::super::hmi_handlers::handle_hmi_trends_get(
            request.id,
            request.params.clone(),
            state,
        ),
        "hmi.alarms.get" => super::super::hmi_handlers::handle_hmi_alarms_get(
            request.id,
            request.params.clone(),
            state,
        ),
        "hmi.descriptor.get" => {
            super::super::hmi_handlers::handle_hmi_descriptor_get(request.id, state)
        }
        "hmi.descriptor.update" => super::super::hmi_handlers::handle_hmi_descriptor_update(
            request.id,
            request.params.clone(),
            state,
        ),
        "hmi.scaffold.reset" => super::super::hmi_handlers::handle_hmi_scaffold_reset(
            request.id,
            request.params.clone(),
            state,
        ),
        "hmi.alarm.ack" => super::super::hmi_handlers::handle_hmi_alarm_ack(
            request.id,
            request.params.clone(),
            state,
        ),
        "hmi.write" => {
            super::super::hmi_handlers::handle_hmi_write(request.id, request.params.clone(), state)
        }
        _ => return None,
    };
    Some(response)
}
