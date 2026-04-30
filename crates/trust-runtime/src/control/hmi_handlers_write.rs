pub(super) fn handle_hmi_alarm_ack(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match params {
        Some(value) => match serde_json::from_value::<HmiAlarmAckParams>(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let result = match state.hmi_live.lock() {
        Ok(mut live) => {
            match crate::hmi::acknowledge_alarm(&mut live, params.id.as_str(), timestamp_ms) {
                Ok(()) => crate::hmi::build_alarm_view(&live, 100),
                Err(err) => return ControlResponse::error(id, err),
            }
        }
        Err(_) => return ControlResponse::error(id, "hmi state unavailable".into()),
    };
    ControlResponse::ok(
        id,
        serde_json::to_value(result).expect("serialize hmi.alarm.ack"),
    )
}

pub(super) fn handle_hmi_write(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match params {
        Some(value) => match serde_json::from_value::<HmiWriteParams>(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let target = params.id.trim();
    if target.is_empty() {
        return ControlResponse::error(id, "missing params.id".into());
    }

    let queued = match queue_hmi_runtime_write_port(state, target, &params.value) {
        Ok(queued) => queued,
        Err(err) => return ControlResponse::error(id, err),
    };

    ControlResponse::ok(
        id,
        json!({
            "status": "queued",
            "id": queued.id,
            "path": queued.path,
        }),
    )
}
