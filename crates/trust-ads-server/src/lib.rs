//! Runtime-neutral Beckhoff ADS server boundary.
//!
//! This crate owns the future ADS server wire engine and the public trait
//! boundary used by runtime integrations. It deliberately exposes only
//! `trust-ads-core` domain types and raw ADS bytes. Runtime values, storage,
//! scan-cycle handles, HMI policy, and control/web concerns stay outside.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

mod ams;
mod commands;
mod error;
mod identify;
mod listener;
mod notify;
mod symbols;
mod system_symbols;
mod traits;
mod types;

pub use ams::{
    ams_net_id_bytes_to_text, ams_net_id_text_to_bytes, AmsHeader, AmsNetIdBytes,
    AmsNetIdTextError, AmsParseError, AmsState, AmsTcpFrame, CommandId, AMS_HEADER_LEN,
    AMS_TCP_HEADER_LEN, STATE_FLAGS_REQUEST, STATE_FLAGS_RESPONSE,
};
pub use commands::{
    build_device_notification_payload, ActiveNotification, AdsDeviceState, CommandContext,
    CommandDispatcher, DeviceInfo, NotificationReceiver, NotificationSample, NotificationStamp,
    NotificationTarget, ADSIGRP_SUMUP_READ, ADSIGRP_SUMUP_READWRITE, ADSIGRP_SUMUP_READ_EX,
    ADSIGRP_SUMUP_WRITE, ADSIGRP_SYM_DT_UPLOAD, ADSIGRP_SYM_HNDBYNAME, ADSIGRP_SYM_RELEASEHND,
    ADSIGRP_SYM_UPLOAD, ADSIGRP_SYM_UPLOADINFO, ADSIGRP_SYM_UPLOADINFO2, ADSIGRP_SYM_VALBYHND,
    ADSIGRP_SYM_VALBYNAME, ADSIGRP_SYM_VERSION, ADSTRANS_SERVERCYCLE, ADSTRANS_SERVERONCHA,
};
pub use error::{AdsErrorCode, AdsServerError};
pub use identify::{AdsIdentifyConfig, AdsIdentifyError, AdsIdentifyServer};
pub use listener::{AdsTcpServer, AdsTcpServerConfig, AdsTcpServerError, AdsTcpServerServices};
pub use notify::{filetime_from_unix_millis, NotificationSampler};
pub use symbols::{
    build_server_symbol_snapshot, ServerSymbolError, ServerSymbolSpec, SymbolVersionCounter,
    DEFAULT_SERVER_SYMBOL_INDEX_GROUP,
};
pub use traits::{AuditSink, ClientPolicy, Clock, RuntimeWritePort, SymbolSource, ValueIo};
pub use types::{AdsServerAuditEvent, AdsServerAuditKind, ClientId};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use trust_ads_core::{AmsNetId, PointQuality, SymbolSnapshot};

    use super::{
        AdsErrorCode, AdsServerAuditEvent, AdsServerAuditKind, AdsServerError, AuditSink, ClientId,
        ClientPolicy, Clock, RuntimeWritePort, SymbolSource, ValueIo,
    };

    struct BoundaryHarness {
        snapshot: SymbolSnapshot,
    }

    impl SymbolSource for BoundaryHarness {
        fn snapshot(&self) -> Arc<SymbolSnapshot> {
            Arc::new(self.snapshot.clone())
        }

        fn version(&self) -> u32 {
            7
        }
    }

    impl ValueIo for BoundaryHarness {
        fn read(
            &self,
            _symbol: &trust_ads_core::SymbolDescriptor,
        ) -> Result<(Vec<u8>, PointQuality), AdsServerError> {
            Err(AdsServerError::device(
                AdsErrorCode::SymbolNotFound,
                "missing test symbol",
            ))
        }
    }

    impl RuntimeWritePort for BoundaryHarness {
        fn write(
            &self,
            _symbol: &trust_ads_core::SymbolDescriptor,
            _bytes: &[u8],
            _client: &ClientId,
        ) -> Result<(), AdsServerError> {
            Err(AdsServerError::device(
                AdsErrorCode::AccessDenied,
                "test write denied",
            ))
        }
    }

    impl ClientPolicy for BoundaryHarness {
        fn permits(&self, client: &ClientId) -> bool {
            client.ams_net_id.0.as_str() == "127.0.0.1.1.1"
        }
    }

    impl AuditSink for BoundaryHarness {
        fn record(&self, _event: &AdsServerAuditEvent) {}
    }

    impl Clock for BoundaryHarness {
        fn now_ms(&self) -> u64 {
            123
        }
    }

    #[test]
    fn boundary_traits_use_core_types_and_raw_bytes() {
        let harness = BoundaryHarness {
            snapshot: SymbolSnapshot {
                schema_version: trust_ads_core::SYMBOL_SNAPSHOT_SCHEMA_VERSION,
                route_name: "test".into(),
                symbols: Vec::new(),
            },
        };
        let client = ClientId::new(AmsNetId::new("127.0.0.1.1.1"));

        assert_eq!(harness.version(), 7);
        assert_eq!(harness.snapshot().symbols.len(), 0);
        assert!(harness.permits(&client));
        assert_eq!(harness.now_ms(), 123);
    }

    #[test]
    fn audit_event_records_policy_failures_without_runtime_types() {
        let client = ClientId::new(AmsNetId::new("5.23.91.12.1.1")).with_source_ip("192.168.10.50");
        let error = AdsServerError::device(AdsErrorCode::AccessDenied, "client not allowed");
        let event = AdsServerAuditEvent::new(
            AdsServerAuditKind::PolicyRejected,
            client,
            Some("global.setpoint".into()),
            Err(error.clone()),
            42,
        );

        assert_eq!(event.kind, AdsServerAuditKind::PolicyRejected);
        assert_eq!(event.client.source_ip.as_deref(), Some("192.168.10.50"));
        assert_eq!(event.result, Err(error));
    }
}
