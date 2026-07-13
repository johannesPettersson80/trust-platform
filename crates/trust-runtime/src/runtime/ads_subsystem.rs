//! ADS client subsystem owned by the runtime scan cycle.

use smol_str::SmolStr;
use trust_ads_core::{AdsRoute, QualityState};

use crate::ads::diagnostics::{
    AdsConnectionStatus, AdsConnectionStatusState, AdsStatusOverall, AdsStatusReport,
    ADS_DIAGNOSTICS_SCHEMA_VERSION,
};
use crate::ads::onboarding::ActiveAdsDeviceSnapshot;
use crate::ads::{
    AdsBridgeError, AdsConnectionBridge, AdsConnectionState, AdsLiveValuesSnapshot, AdsWorkerThread,
};
use crate::error::RuntimeError;
use crate::memory::VariableStorage;

pub(super) struct AdsSubsystem {
    connections: Vec<AdsRuntimeConnection>,
    deployed_ads_config_hash: Option<String>,
}

struct AdsRuntimeConnection {
    name: SmolStr,
    route: AdsRoute,
    bridge: AdsConnectionBridge,
    worker: Option<AdsWorkerThread>,
}

impl AdsSubsystem {
    pub(super) fn new() -> Self {
        Self {
            connections: Vec::new(),
            deployed_ads_config_hash: None,
        }
    }

    pub(super) fn set_deployed_ads_config_hash(&mut self, hash: Option<String>) {
        self.deployed_ads_config_hash = hash;
    }

    pub(super) fn add_connection(
        &mut self,
        route: AdsRoute,
        bridge: AdsConnectionBridge,
        worker: AdsWorkerThread,
    ) {
        let name = route.name.clone();
        self.connections.push(AdsRuntimeConnection {
            name: name.into(),
            route,
            bridge,
            worker: Some(worker),
        });
    }

    pub(super) fn connection_count(&self) -> usize {
        self.connections.len()
    }

    pub(super) fn status_report(&self) -> AdsStatusReport {
        let connections: Vec<AdsConnectionStatus> =
            self.connections.iter().map(connection_status).collect();
        let overall = ads_status_overall(&connections);
        AdsStatusReport {
            schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
            role: crate::ads::diagnostics::DoctorRole::Client,
            overall,
            runtime_identity_hash: None,
            deployed_ads_config_hash: self.deployed_ads_config_hash.clone(),
            connections,
            summary: ads_status_summary(overall),
        }
    }

    pub(super) fn live_values_snapshot(&self, scan: u64) -> AdsLiveValuesSnapshot {
        let entries = self
            .connections
            .iter()
            .flat_map(|connection| {
                connection
                    .bridge
                    .live_value_entries(connection.name.as_str())
            })
            .collect();
        AdsLiveValuesSnapshot::new(scan, entries)
    }

    pub(super) fn active_device_snapshot(
        &self,
        target: &crate::ads::diagnostics::TargetIdentity,
        local: Option<&crate::ads::diagnostics::LocalIdentity>,
    ) -> Option<ActiveAdsDeviceSnapshot> {
        self.connections
            .iter()
            .find(|connection| route_matches_target(&connection.route, target, local))
            .map(|connection| ActiveAdsDeviceSnapshot {
                connection_name: connection.name.to_string(),
                target: target_identity_from_route(&connection.route),
                local: local.cloned(),
                state: connection_state(connection.bridge.state()),
                point_statuses: connection.bridge.statuses(),
                symbol_version: None,
            })
    }

    pub(super) fn apply_inputs(
        &mut self,
        storage: &mut VariableStorage,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        for connection in &mut self.connections {
            connection
                .bridge
                .apply_inputs(storage, now_ms)
                .map_err(|err| ads_runtime_error(&connection.name, err))?;
        }
        Ok(())
    }

    pub(super) fn capture_outputs(
        &mut self,
        storage: &mut VariableStorage,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        for connection in &mut self.connections {
            connection
                .bridge
                .capture_outputs(storage, now_ms)
                .map_err(|err| ads_runtime_error(&connection.name, err))?;
        }
        Ok(())
    }

    pub(super) fn shutdown(&mut self) -> Result<(), RuntimeError> {
        let mut first_error = None;
        for connection in &mut self.connections {
            if let Some(worker) = connection.worker.take() {
                if let Err(err) = worker.shutdown() {
                    first_error.get_or_insert_with(|| ads_runtime_error(&connection.name, err));
                }
            }
        }
        if let Some(error) = first_error {
            return Err(error);
        }
        Ok(())
    }
}

impl Drop for AdsSubsystem {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn ads_runtime_error(connection: &SmolStr, err: AdsBridgeError) -> RuntimeError {
    RuntimeError::IoTransport(format!("ADS connection '{connection}': {err}").into())
}

fn connection_status(connection: &AdsRuntimeConnection) -> AdsConnectionStatus {
    let point_statuses = connection.bridge.statuses();
    let point_count = point_statuses.len();
    let degraded_points = point_statuses
        .iter()
        .filter(|status| status.quality.state != QualityState::Good)
        .count();
    let last_good_value_ms = point_statuses
        .iter()
        .filter(|status| status.quality.state == QualityState::Good)
        .filter_map(|status| status.quality.last_update_ms)
        .max();
    let state = connection_state(connection.bridge.state());

    AdsConnectionStatus {
        name: connection.name.to_string(),
        target: Some(target_identity_from_route(&connection.route)),
        state,
        point_count,
        degraded_points,
        last_good_value_ms,
        symbol_version: None,
        summary: format!(
            "{} point(s), {} degraded, state {:?}",
            point_count, degraded_points, state
        ),
    }
}

fn connection_state(state: AdsConnectionState) -> AdsConnectionStatusState {
    match state {
        AdsConnectionState::Connected => AdsConnectionStatusState::Connected,
        AdsConnectionState::Connecting | AdsConnectionState::Reconnecting => {
            AdsConnectionStatusState::Reconnecting
        }
        AdsConnectionState::Faulted => AdsConnectionStatusState::Faulted,
        AdsConnectionState::Disconnected => AdsConnectionStatusState::Stale,
    }
}

fn ads_status_overall(connections: &[AdsConnectionStatus]) -> AdsStatusOverall {
    if connections.is_empty() {
        return AdsStatusOverall::Disabled;
    }
    if connections
        .iter()
        .any(|connection| connection.state == AdsConnectionStatusState::Faulted)
    {
        return AdsStatusOverall::Faulted;
    }
    if connections.iter().any(|connection| {
        connection.state != AdsConnectionStatusState::Connected || connection.degraded_points > 0
    }) {
        return AdsStatusOverall::Degraded;
    }
    AdsStatusOverall::Healthy
}

fn ads_status_summary(overall: AdsStatusOverall) -> String {
    match overall {
        AdsStatusOverall::Healthy => "ADS connections healthy.".to_string(),
        AdsStatusOverall::Degraded => "One or more ADS connections are degraded.".to_string(),
        AdsStatusOverall::NotReady => "ADS is not ready.".to_string(),
        AdsStatusOverall::Faulted => "One or more ADS connections are faulted.".to_string(),
        AdsStatusOverall::Disabled => "ADS is not configured.".to_string(),
        AdsStatusOverall::Unknown => "ADS status is unknown.".to_string(),
    }
}

fn target_identity_from_route(route: &AdsRoute) -> crate::ads::diagnostics::TargetIdentity {
    crate::ads::diagnostics::TargetIdentity {
        name: Some(route.name.clone()),
        ip: route.host.clone(),
        ams_net_id: route.target_net_id.0.clone(),
        ams_port: route.ams_port,
        tc_version: None,
    }
}

fn route_matches_target(
    route: &AdsRoute,
    target: &crate::ads::diagnostics::TargetIdentity,
    local: Option<&crate::ads::diagnostics::LocalIdentity>,
) -> bool {
    let target_matches = route.host == target.ip
        || route.host == target.name.as_deref().unwrap_or_default()
        || route.target_net_id.0 == target.ams_net_id;
    let local_matches = match (route.local_net_id.as_ref(), local) {
        (Some(route_local), Some(local)) => route_local.0 == local.ams_net_id,
        (Some(_), None) => true,
        (None, _) => true,
    };
    target_matches && local_matches
}

#[cfg(test)]
mod tests {
    use trust_ads_core::{
        AdsDataTypeDescriptor, AdsRoute, AdsSecurityPolicy, AmsNetId, IecDataType, PointAccess,
        PointQuality, QualityState, SymbolDescriptor, SymbolFlag, TransportSecurity, UpdateMode,
    };
    use trust_hir::TypeId;

    use super::*;
    use crate::ads::{
        resolve_declared_bindings, AdsConnectionConfig, AdsNotificationMode, AdsPointAddress,
        AdsPointConfig, MockAdsTransport,
    };
    use crate::value::Value;
    use crate::{GlobalInitValue, RetainPolicy, Runtime};

    #[test]
    fn empty_ads_subsystem_reports_disabled_status() {
        let subsystem = AdsSubsystem::new();

        let report = subsystem.status_report();

        assert_eq!(report.overall, AdsStatusOverall::Disabled);
        assert!(report.connections.is_empty());
    }

    #[test]
    fn status_report_includes_deployed_ads_config_hash() {
        let mut subsystem = AdsSubsystem::new();
        subsystem.set_deployed_ads_config_hash(Some("sha256:ads-config".to_string()));

        let report = subsystem.status_report();

        assert_eq!(
            report.deployed_ads_config_hash.as_deref(),
            Some("sha256:ads-config")
        );
    }

    #[test]
    fn status_overall_reports_fault_before_degraded_or_healthy() {
        let status = AdsConnectionStatus {
            name: "line1".to_string(),
            target: None,
            state: AdsConnectionStatusState::Faulted,
            point_count: 1,
            degraded_points: 0,
            last_good_value_ms: None,
            symbol_version: None,
            summary: "Faulted.".to_string(),
        };

        assert_eq!(ads_status_overall(&[status]), AdsStatusOverall::Faulted);
    }

    #[test]
    fn status_overall_reports_degraded_for_stale_or_degraded_points() {
        let stale = AdsConnectionStatus {
            name: "line1".to_string(),
            target: None,
            state: AdsConnectionStatusState::Stale,
            point_count: 1,
            degraded_points: 0,
            last_good_value_ms: None,
            symbol_version: None,
            summary: "Stale.".to_string(),
        };
        let degraded = AdsConnectionStatus {
            state: AdsConnectionStatusState::Connected,
            degraded_points: 1,
            summary: "Degraded.".to_string(),
            ..stale.clone()
        };

        assert_eq!(ads_status_overall(&[stale]), AdsStatusOverall::Degraded);
        assert_eq!(ads_status_overall(&[degraded]), AdsStatusOverall::Degraded);
    }

    #[test]
    fn active_device_snapshot_matches_configured_route_without_socket_io() {
        let mut subsystem = AdsSubsystem::new();
        let bridge = AdsConnectionBridge::new(Vec::new()).expect("empty bridge");
        subsystem.add_connection_for_test(route(), bridge);
        let target = crate::ads::diagnostics::TargetIdentity {
            name: None,
            ip: "192.168.10.5".to_string(),
            ams_net_id: "5.23.91.12.1.1".to_string(),
            ams_port: 851,
            tc_version: None,
        };
        let local = crate::ads::diagnostics::LocalIdentity {
            host_name: Some("line-controller-1".to_string()),
            chosen_ip: "192.168.10.20".to_string(),
            ams_net_id: "192.168.10.20.1.1".to_string(),
            nic: Some("eth0".to_string()),
            candidates: Vec::new(),
            classification: crate::ads::diagnostics::LocalNetworkClassification::Lan,
        };

        let snapshot = subsystem
            .active_device_snapshot(&target, Some(&local))
            .expect("active device");

        assert_eq!(snapshot.connection_name, "line1");
        assert_eq!(snapshot.target.ams_net_id, "5.23.91.12.1.1");
        assert_eq!(
            snapshot
                .local
                .as_ref()
                .map(|local| local.ams_net_id.as_str()),
            Some("192.168.10.20.1.1")
        );
    }

    #[test]
    fn live_values_snapshot_preserves_good_value_and_metadata_when_quality_turns_stale() {
        let mut runtime = Runtime::new();
        let point_name = SmolStr::new("line1_temp");
        runtime
            .storage_mut()
            .set_global(point_name.clone(), Value::Real(0.0));
        runtime.register_global_meta(
            point_name,
            TypeId::REAL,
            RetainPolicy::NonRetain,
            GlobalInitValue::Value(Value::Real(0.0)),
        );
        let point = AdsPointConfig {
            point_name: "line1_temp".to_string(),
            address: AdsPointAddress::Symbol("MAIN.Temperature".to_string()),
            data_type: AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            access: PointAccess::Read,
            mode: UpdateMode::Poll,
            notification_mode: AdsNotificationMode::OnChange,
            allow_retain_read: false,
        };
        let connection = AdsConnectionConfig {
            route: route(),
            points: vec![point],
        };
        let bindings = resolve_declared_bindings(&runtime, &connection).expect("ADS bindings");
        let symbol = SymbolDescriptor::new(
            "MAIN.Temperature",
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            0x4020,
            0,
            4,
        )
        .with_flag(SymbolFlag::Read);
        let mut transport = MockAdsTransport::new(vec![symbol]);
        transport.set_value("line1_temp", Value::Real(42.5), PointQuality::good(10));
        let (mut bridge, mut worker) =
            AdsConnectionBridge::with_transport(transport, bindings).expect("ADS bridge");
        bridge.initialize_live_values(runtime.storage());
        worker.tick(0).expect("initial ADS poll");

        let mut subsystem = AdsSubsystem::new();
        subsystem.add_connection_for_test(connection.route, bridge);
        let before_scan = subsystem.live_values_snapshot(6);
        let entry = &before_scan.entries[0];
        assert_eq!(entry.value, "0.0");
        assert_eq!(entry.quality.state, QualityState::Stale);

        subsystem
            .apply_inputs(runtime.storage_mut(), 11)
            .expect("apply ADS input");
        let good = subsystem.live_values_snapshot(7);

        assert_eq!(good.schema_version, 1);
        assert_eq!(good.scan, 7);
        assert_eq!(good.entries.len(), 1);
        let entry = &good.entries[0];
        assert_eq!(entry.connection, "line1");
        assert_eq!(entry.name, "line1_temp");
        assert_eq!(entry.remote_symbol, "MAIN.Temperature");
        assert_eq!(entry.value, "42.5");
        assert_eq!(entry.value_type, "REAL");
        assert_eq!(entry.access, "read");
        assert_eq!(entry.quality.state, QualityState::Good);
        assert_eq!(entry.quality.last_update_ms, Some(10));

        worker.mark_reconnecting(20, "route lost");
        let before_stale_scan = subsystem.live_values_snapshot(7);
        assert_eq!(
            before_stale_scan.entries[0].quality.state,
            QualityState::Good
        );
        subsystem
            .apply_inputs(runtime.storage_mut(), 21)
            .expect("commit stale ADS quality");
        let stale = subsystem.live_values_snapshot(8);
        let entry = &stale.entries[0];
        assert_eq!(entry.value, "42.5", "stale keeps the last scan value");
        assert_eq!(entry.quality.state, QualityState::Stale);
        assert_eq!(entry.quality.last_update_ms, Some(10));
        assert!(entry
            .quality
            .detail
            .as_deref()
            .is_some_and(|detail| detail.contains("route lost")));
    }

    impl AdsSubsystem {
        fn add_connection_for_test(&mut self, route: AdsRoute, bridge: AdsConnectionBridge) {
            let name = route.name.clone();
            self.connections.push(AdsRuntimeConnection {
                name: name.into(),
                route,
                bridge,
                worker: None,
            });
        }
    }

    fn route() -> AdsRoute {
        AdsRoute {
            name: "line1".to_string(),
            target_net_id: AmsNetId::new("5.23.91.12.1.1"),
            host: "192.168.10.5".to_string(),
            ams_port: 851,
            local_net_id: Some(AmsNetId::new("192.168.10.20.1.1")),
            security: AdsSecurityPolicy {
                transport: TransportSecurity::Plain,
                auto_add_route: false,
            },
        }
    }
}
