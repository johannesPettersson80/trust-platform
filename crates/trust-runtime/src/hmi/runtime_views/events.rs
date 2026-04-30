#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HmiEventStreamEvent {
    #[serde(rename = "type")]
    pub event_type: &'static str,
    pub result: serde_json::Value,
}

#[derive(Debug, Default)]
pub struct HmiEventStreamState {
    last_schema_revision: u64,
    widget_ids: Vec<String>,
    last_values: serde_json::Map<String, serde_json::Value>,
    last_alarm_payload: Option<serde_json::Value>,
}

impl HmiEventStreamState {
    pub fn prime_schema(&mut self, schema: &serde_json::Value) {
        self.last_schema_revision = hmi_schema_revision(schema).unwrap_or(0);
        self.widget_ids = hmi_widget_ids(schema);
    }

    pub fn values_request_params(&self) -> Option<serde_json::Value> {
        if self.widget_ids.is_empty() {
            return None;
        }
        Some(serde_json::json!({ "ids": self.widget_ids }))
    }

    pub fn observe_schema(
        &mut self,
        schema: &serde_json::Value,
    ) -> Option<HmiEventStreamEvent> {
        let revision = hmi_schema_revision(schema).unwrap_or(self.last_schema_revision);
        if revision == self.last_schema_revision {
            return None;
        }
        self.last_schema_revision = revision;
        self.widget_ids = hmi_widget_ids(schema);
        Some(HmiEventStreamEvent {
            event_type: "hmi.schema.revision",
            result: serde_json::json!({ "schema_revision": revision }),
        })
    }

    pub fn observe_values(
        &mut self,
        values_result: &serde_json::Value,
    ) -> Option<HmiEventStreamEvent> {
        hmi_values_delta(values_result, &mut self.last_values).map(|result| HmiEventStreamEvent {
            event_type: "hmi.values.delta",
            result,
        })
    }

    pub fn observe_alarms(
        &mut self,
        alarms_result: &serde_json::Value,
    ) -> Option<HmiEventStreamEvent> {
        if self.last_alarm_payload.as_ref() == Some(alarms_result) {
            return None;
        }
        self.last_alarm_payload = Some(alarms_result.clone());
        Some(HmiEventStreamEvent {
            event_type: "hmi.alarms.event",
            result: alarms_result.clone(),
        })
    }
}

fn hmi_schema_revision(schema: &serde_json::Value) -> Option<u64> {
    schema
        .get("schema_revision")
        .and_then(serde_json::Value::as_u64)
}

fn hmi_widget_ids(schema: &serde_json::Value) -> Vec<String> {
    schema
        .get("widgets")
        .and_then(serde_json::Value::as_array)
        .map(|widgets| {
            widgets
                .iter()
                .filter_map(|widget| widget.get("id").and_then(serde_json::Value::as_str))
                .map(std::string::ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn hmi_values_delta(
    values_result: &serde_json::Value,
    last_values: &mut serde_json::Map<String, serde_json::Value>,
) -> Option<serde_json::Value> {
    let values = values_result.get("values")?.as_object()?;
    let mut delta = serde_json::Map::new();
    for (id, entry) in values {
        if last_values.get(id) != Some(entry) {
            delta.insert(id.clone(), entry.clone());
        }
    }
    last_values.retain(|id, _| values.contains_key(id));
    for (id, entry) in values {
        last_values.insert(id.clone(), entry.clone());
    }
    if delta.is_empty() {
        return None;
    }
    Some(serde_json::json!({
        "connected": values_result
            .get("connected")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        "timestamp_ms": values_result
            .get("timestamp_ms")
            .cloned()
            .unwrap_or(serde_json::Value::Null),
        "values": delta,
    }))
}
