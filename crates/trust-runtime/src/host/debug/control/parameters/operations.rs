//! Fences late parameter writes using identities assigned before dispatch.
use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CancelRequest {
    loop_id: String,
    transaction_id: String,
    expected_program_revision: String,
    expected_binding_revision: String,
}
impl DebugControl {
    pub(crate) fn parameter_cancel(
        &self,
        params: JsonValue,
        metadata: &RuntimeMetadata,
    ) -> Result<JsonValue, String> {
        let request: CancelRequest =
            serde_json::from_value(params).map_err(|error| error.to_string())?;
        if request.transaction_id.is_empty() || request.transaction_id.len() > 128 {
            return Err("invalid parameter transaction identity".into());
        }
        let mut state = self.state.0.lock().map_err(|_| "debug state unavailable")?;
        if state.parameters.program_revision(metadata) != request.expected_program_revision {
            return Err("parameter program epoch changed".into());
        }
        let record = state
            .parameters
            .records
            .get(&request.loop_id)
            .ok_or("read parameter status before cancellation")?;
        if record.binding_revision != request.expected_binding_revision {
            return Err("parameter binding changed".into());
        }
        let id = &request.transaction_id;
        if state.parameters.cancelled_operations.contains(id)
            || !state.parameters.used_operations.contains(id)
        {
            if !state.parameters.used_operations.contains(id)
                && state.parameters.used_operations.len() >= 1024
            {
                return Err("parameter identity capacity reached".into());
            }
            state.parameters.used_operations.insert(id.clone());
            state.parameters.cancelled_operations.insert(id.clone());
            return Ok(
                json!({"transaction_id":id,"program_revision":request.expected_program_revision,
                "binding_revision":request.expected_binding_revision,"confirmed":true,"before_dispatch":true}),
            );
        }
        if record.transaction_id.as_ref() != Some(id) {
            return Err("parameter transaction history no longer matches".into());
        }
        if let Some(pending) = state
            .parameters
            .pending
            .as_ref()
            .filter(|pending| pending.request.transaction_id.as_ref() == Some(id))
        {
            if pending.committed {
                return Ok(
                    json!({"confirmed":false,"reason":"parameter commit requires post-ST observation"}),
                );
            }
            state.parameters.pending = None;
            let record = state
                .parameters
                .records
                .get_mut(&request.loop_id)
                .ok_or("parameter record unavailable")?;
            record.status = "rejected".into();
            record.reason =
                Some("parameter operation cancelled before task-boundary commit".into());
        }
        Ok(
            json!({"transaction_id":id,"program_revision":request.expected_program_revision,
            "binding_revision":request.expected_binding_revision,"confirmed":true,"before_dispatch":false}),
        )
    }
}
