use std::sync::atomic::{AtomicU64, Ordering};

use rand::TryRng;
use smol_str::SmolStr;

use super::{ControlAuditEvent, ControlState};

static CONTROL_AUDIT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(super) struct ControlAuditRecord<'a> {
    pub request_id: u64,
    pub request_type: SmolStr,
    pub correlation_id: Option<&'a str>,
    pub ok: bool,
    pub error: Option<SmolStr>,
    pub auth_present: bool,
    pub client: Option<&'a str>,
}

pub(super) fn record_audit(
    state: &ControlState,
    record: ControlAuditRecord<'_>,
) -> Option<SmolStr> {
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let sequence = CONTROL_AUDIT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let mut nonce_bytes = [0_u8; 4];
    let mut rng = rand::rngs::SysRng;
    let _ = rng.try_fill_bytes(&mut nonce_bytes);
    let nonce = u32::from_le_bytes(nonce_bytes);
    let event_id = SmolStr::new(format!("audit-{timestamp_ms}-{sequence}-{nonce:08x}"));
    let event = ControlAuditEvent {
        event_id: event_id.clone(),
        timestamp_ms,
        request_id: record.request_id,
        request_type: record.request_type.clone(),
        correlation_id: record.correlation_id.map(SmolStr::new),
        ok: record.ok,
        error: record.error,
        auth_present: record.auth_present,
        client: record.client.map(SmolStr::new),
    };
    if let Some(sender) = &state.audit_tx {
        if let Err(err) = sender.send(event) {
            if let Ok(mut events) = state.events.lock() {
                events.push_back(crate::debug::RuntimeEvent::AuditDropped {
                    request_id: record.request_id,
                    request_type: record.request_type,
                    error: err.to_string(),
                    time: crate::value::Duration::from_millis(
                        timestamp_ms.min(i64::MAX as u128) as i64
                    ),
                });
            }
        }
    }
    Some(event_id)
}
