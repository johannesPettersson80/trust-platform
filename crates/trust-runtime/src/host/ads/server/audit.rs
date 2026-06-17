//! ADS server audit integration.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;

use serde_json::json;
use smol_str::SmolStr;
use trust_ads_server::{AdsServerAuditEvent, AdsServerAuditKind, AuditSink};

use crate::control::ControlAuditEvent;

static ADS_AUDIT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Audit sink that forwards ADS server write/security records to the runtime audit channel.
#[derive(Clone)]
pub struct AdsServerRuntimeAuditSink {
    tx: Option<Sender<ControlAuditEvent>>,
}

impl AdsServerRuntimeAuditSink {
    /// Creates a runtime ADS audit sink.
    #[must_use]
    pub fn new(tx: Option<Sender<ControlAuditEvent>>) -> Self {
        Self { tx }
    }
}

impl AuditSink for AdsServerRuntimeAuditSink {
    fn record(&self, event: &AdsServerAuditEvent) {
        let Some(tx) = &self.tx else {
            return;
        };
        let ok = event.result.is_ok();
        let error = event
            .result
            .as_ref()
            .err()
            .map(|err| SmolStr::new(err.to_string()));
        let audit = ControlAuditEvent {
            event_id: next_ads_audit_id(event.timestamp_ms),
            timestamp_ms: u128::from(event.timestamp_ms),
            request_id: 0,
            request_type: SmolStr::new(request_type(event.kind)),
            correlation_id: None,
            ok,
            error,
            auth_present: false,
            client: Some(SmolStr::new(format!(
                "ams_net_id={} source_ip={}",
                event.client.ams_net_id.0,
                event.client.source_ip.as_deref().unwrap_or("<unknown>")
            ))),
            details: Some(json!({
                "kind": audit_kind(event.kind),
                "client_ams_net_id": event.client.ams_net_id.0,
                "source_ip": event.client.source_ip,
                "symbol": event.symbol,
                "value_type": event.value_type,
                "result": if ok { "ok" } else { "error" },
                "ads_error": event.result.as_ref().err().map(|err| {
                    json!({
                        "code": err.code().value(),
                        "message": err.message(),
                    })
                }),
            })),
        };
        let _ = tx.send(audit);
    }
}

fn next_ads_audit_id(timestamp_ms: u64) -> SmolStr {
    let sequence = ADS_AUDIT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    SmolStr::new(format!("ads-audit-{timestamp_ms}-{sequence}"))
}

fn request_type(kind: AdsServerAuditKind) -> &'static str {
    match kind {
        AdsServerAuditKind::WriteAccepted | AdsServerAuditKind::WriteRejected => "ads.server.write",
        AdsServerAuditKind::PolicyRejected => "ads.server.policy",
    }
}

fn audit_kind(kind: AdsServerAuditKind) -> &'static str {
    match kind {
        AdsServerAuditKind::WriteAccepted => "write_accepted",
        AdsServerAuditKind::WriteRejected => "write_rejected",
        AdsServerAuditKind::PolicyRejected => "policy_rejected",
    }
}
