pub(super) fn handle_hmi_schema_get(id: u64, state: &ControlState) -> ControlResponse {
    let result = match read_hmi_schema_port(state) {
        Ok(result) => result,
        Err(err) => return ControlResponse::error(id, err),
    };
    ControlResponse::ok(
        id,
        serde_json::to_value(result).expect("serialize hmi.schema.get"),
    )
}

pub(super) fn handle_hmi_values_get(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match params {
        Some(value) => match serde_json::from_value::<HmiValuesParams>(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => HmiValuesParams { ids: None },
    };
    let read = match read_hmi_values_port(state, params.ids.as_deref()) {
        Ok(read) => read,
        Err(err) => return ControlResponse::error(id, err),
    };
    if let Ok(mut live) = state.hmi_live.lock() {
        crate::hmi::update_live_state(&mut live, &read.schema, &read.values);
    }
    ControlResponse::ok(
        id,
        serde_json::to_value(read.values).expect("serialize hmi.values.get"),
    )
}

pub(super) fn handle_hmi_trends_get(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match params {
        Some(value) => match serde_json::from_value::<HmiTrendsParams>(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => HmiTrendsParams::default(),
    };
    let read = match read_hmi_values_port(state, params.ids.as_deref()) {
        Ok(read) => read,
        Err(err) => return ControlResponse::error(id, err),
    };
    let result = match state.hmi_live.lock() {
        Ok(mut live) => {
            crate::hmi::update_live_state(&mut live, &read.schema, &read.values);
            crate::hmi::build_trends(
                &live,
                &read.schema,
                params.ids.as_deref(),
                params.duration_ms.unwrap_or(10 * 60 * 1_000),
                params.buckets.unwrap_or(120),
            )
        }
        Err(_) => return ControlResponse::error(id, "hmi state unavailable".into()),
    };
    ControlResponse::ok(
        id,
        serde_json::to_value(result).expect("serialize hmi.trends.get"),
    )
}

pub(super) fn handle_hmi_alarms_get(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match params {
        Some(value) => match serde_json::from_value::<HmiAlarmsParams>(value) {
            Ok(parsed) => parsed,
            Err(err) => return ControlResponse::error(id, format!("invalid params: {err}")),
        },
        None => HmiAlarmsParams::default(),
    };
    let read = match read_hmi_values_port(state, None) {
        Ok(read) => read,
        Err(err) => return ControlResponse::error(id, err),
    };
    let result = match state.hmi_live.lock() {
        Ok(mut live) => {
            crate::hmi::update_live_state(&mut live, &read.schema, &read.values);
            crate::hmi::build_alarm_view(&live, params.limit.unwrap_or(100))
        }
        Err(_) => return ControlResponse::error(id, "hmi state unavailable".into()),
    };
    ControlResponse::ok(
        id,
        serde_json::to_value(result).expect("serialize hmi.alarms.get"),
    )
}
