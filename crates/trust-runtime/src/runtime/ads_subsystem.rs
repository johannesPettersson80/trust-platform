//! ADS client subsystem owned by the runtime scan cycle.

use smol_str::SmolStr;
use trust_ads_core::{AdsRoute, QualityState};

use crate::ads::diagnostics::{
    AdsConnectionStatus, AdsConnectionStatusState, AdsStatusOverall, AdsStatusReport,
    ADS_DIAGNOSTICS_SCHEMA_VERSION,
};
use crate::ads::onboarding::ActiveAdsDeviceSnapshot;
use crate::ads::{
    AdsBridgeError, AdsConnectionBridge, AdsConnectionState, AdsLiveValue, AdsWorkerThread,
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

    pub(super) fn prepare_rebind(
        &self,
        candidate: &crate::Runtime,
    ) -> Result<Vec<Vec<crate::ads::AdsBinding>>, RuntimeError> {
        self.connections
            .iter()
            .map(|connection| {
                crate::ads::resolve_declared_bindings(
                    candidate,
                    &crate::ads::AdsConnectionConfig {
                        route: connection.route.clone(),
                        points: connection.bridge.points(),
                    },
                )
                .map_err(|error| RuntimeError::InvalidConfig(error.to_string().into()))
            })
            .collect()
    }

    pub(super) fn commit_rebind(&mut self, bindings: Vec<Vec<crate::ads::AdsBinding>>) {
        for (connection, bindings) in self.connections.iter_mut().zip(bindings) {
            connection.bridge.replace_bindings(bindings);
        }
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

    #[cfg(test)]
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

    #[cfg(test)]
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

    pub(super) fn apply_inputs_with_cycle(
        &mut self,
        access: &mut trust_runtime_core::vm::CycleAccess<'_, '_>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        for connection in &mut self.connections {
            connection
                .bridge
                .apply_inputs_with_cycle(access, now_ms)
                .map_err(|err| ads_runtime_error(&connection.name, err))?;
        }
        Ok(())
    }

    pub(super) fn capture_outputs_with_cycle(
        &mut self,
        access: &mut trust_runtime_core::vm::CycleAccess<'_, '_>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        for connection in &mut self.connections {
            connection
                .bridge
                .capture_outputs_with_cycle(access, now_ms)
                .map_err(|err| ads_runtime_error(&connection.name, err))?;
        }
        Ok(())
    }

    pub(super) fn live_values(&self, storage: &VariableStorage) -> Vec<AdsLiveValue> {
        self.connections
            .iter()
            .flat_map(|connection| {
                connection.bridge.live_values(
                    storage,
                    connection.name.as_str(),
                    connection.route.ams_port,
                )
            })
            .collect()
    }

    /// Select once before any PLC mutation. No worker can mutate binding metadata
    /// while this owner-local bridge borrow remains live.
    pub(super) fn writable_live_bridge(
        &mut self,
        point_name: &str,
    ) -> Result<Option<&mut AdsConnectionBridge>, RuntimeError> {
        let mut found_read_only = false;
        for connection in &mut self.connections {
            match connection.bridge.live_value_writable(point_name) {
                Some(true) => return Ok(Some(&mut connection.bridge)),
                Some(false) => found_read_only = true,
                None => {}
            }
        }
        if found_read_only {
            Err(RuntimeError::InvalidConfig(format!("ADS tag '{point_name}' is read-only. Enable writes for this tag and restart the runtime.").into()))
        } else {
            Ok(None)
        }
    }

    pub(super) fn queue_live_write(
        &mut self,
        point_name: &str,
        value: crate::value::Value,
    ) -> bool {
        self.connections.iter_mut().any(|connection| {
            connection
                .bridge
                .queue_live_write(point_name, value.clone())
        })
    }

    pub(super) fn live_value_writable(&self, point_name: &str) -> Option<bool> {
        let mut found_read_only = false;
        for connection in &self.connections {
            match connection.bridge.live_value_writable(point_name) {
                Some(true) => return Some(true),
                Some(false) => found_read_only = true,
                None => {}
            }
        }
        found_read_only.then_some(false)
    }

    pub(super) fn shutdown(&mut self) -> Result<(), RuntimeError> {
        let mut first_error = None;
        for connection in &mut self.connections {
            if let Some(worker) = connection.worker.take()
                && let Err(err) = worker.shutdown()
            {
                first_error.get_or_insert_with(|| ads_runtime_error(&connection.name, err));
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
    if let Some(error) = err.execution_cause() {
        return error.clone();
    }
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
    let target_matches = match route.host.parse::<std::net::IpAddr>() {
        Ok(route_host) => {
            target
                .ip
                .parse::<std::net::IpAddr>()
                .is_ok_and(|target_host| route_host == target_host)
                && route.ams_port == target.ams_port
                && (target.ams_net_id.is_empty() || route.target_net_id.0 == target.ams_net_id)
        }
        Err(_) => {
            route.host == target.ip
                || route.host == target.name.as_deref().unwrap_or_default()
                || route.target_net_id.0 == target.ams_net_id
        }
    };
    let local_matches = match (route.local_net_id.as_ref(), local) {
        (Some(route_local), Some(local)) => route_local.0 == local.ams_net_id,
        (Some(_), None) => true,
        (None, _) => true,
    };
    target_matches && local_matches
}

#[cfg(test)]
mod tests {
    use trust_ads_core::{AdsRoute, AdsSecurityPolicy, AmsNetId, TransportSecurity};

    use super::*;

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
        let target = target();
        let local = local_identity();

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
    fn active_device_snapshot_rejects_partial_target_matches() {
        let cases = [
            (
                "wrong host",
                AdsRoute {
                    host: "192.168.10.99".to_string(),
                    ..route()
                },
            ),
            (
                "wrong target AMS Net ID",
                AdsRoute {
                    target_net_id: AmsNetId::new("5.23.91.12.9.9"),
                    ..route()
                },
            ),
            (
                "wrong AMS port",
                AdsRoute {
                    ams_port: 852,
                    ..route()
                },
            ),
        ];
        let target = target();
        let local = local_identity();

        for (case, route) in cases {
            let mut subsystem = AdsSubsystem::new();
            let bridge = AdsConnectionBridge::new(Vec::new()).expect("empty bridge");
            subsystem.add_connection_for_test(route, bridge);

            assert!(
                subsystem
                    .active_device_snapshot(&target, Some(&local))
                    .is_none(),
                "{case} must not identify the requested active ADS device"
            );
        }

        let mut subsystem = AdsSubsystem::new();
        let bridge = AdsConnectionBridge::new(Vec::new()).expect("empty bridge");
        subsystem.add_connection_for_test(route(), bridge);
        let mismatched_local = crate::ads::diagnostics::LocalIdentity {
            ams_net_id: "192.168.10.20.9.9".to_string(),
            ..local_identity()
        };
        assert!(
            subsystem
                .active_device_snapshot(&target, Some(&mismatched_local))
                .is_none(),
            "wrong both-present local AMS identity must reject active ADS device reuse"
        );
    }

    #[test]
    fn active_device_snapshot_skips_partial_match_and_selects_later_exact_route() {
        let mut subsystem = AdsSubsystem::new();
        let partial_route = AdsRoute {
            name: "partial".to_string(),
            host: "192.168.10.99".to_string(),
            ..route()
        };
        let exact_route = AdsRoute {
            name: "exact".to_string(),
            ..route()
        };
        subsystem.add_connection_for_test(
            partial_route,
            AdsConnectionBridge::new(Vec::new()).expect("partial bridge"),
        );
        subsystem.add_connection_for_test(
            exact_route,
            AdsConnectionBridge::new(Vec::new()).expect("exact bridge"),
        );
        let target = target();
        let local = local_identity();

        let snapshot = subsystem
            .active_device_snapshot(&target, Some(&local))
            .expect("exact active device");

        assert_eq!(snapshot.connection_name, "exact");
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

    fn management_runtime(access: trust_ads_core::PointAccess) -> crate::Runtime {
        let mut runtime = crate::harness::CompileSession::from_source(
            "VAR_GLOBAL point : INT := 1; ordinary : INT := 2; END_VAR PROGRAM Main END_PROGRAM",
        )
        .build_runtime()
        .unwrap();
        let reference = runtime.storage().ref_for_global("point").unwrap();
        let binding = crate::ads::AdsBinding {
            point: crate::ads::AdsPointConfig {
                point_name: "point".into(),
                address: crate::ads::AdsPointAddress::Symbol("Remote.Value".into()),
                data_type: trust_ads_core::AdsDataTypeDescriptor::scalar(
                    "INT",
                    trust_ads_core::IecDataType::Int,
                ),
                access,
                mode: trust_ads_core::UpdateMode::Poll,
                notification_mode: crate::ads::AdsNotificationMode::OnChange,
                allow_retain_read: false,
            },
            reference,
            quality_reference: None,
            type_id: trust_hir::TypeId::INT,
            retain: false,
        };
        // Two writable connections pin first-match behavior, not duplicate sends.
        for _ in 0..2 {
            runtime.ads.add_connection_for_test(
                route(),
                AdsConnectionBridge::new(vec![binding.clone()]).unwrap(),
            );
        }
        runtime
    }

    #[test]
    fn checked_management_write_rejects_before_queue_and_commits_one_selected_bridge() {
        use crate::value::Value;
        let mut runtime = management_runtime(trust_ads_core::PointAccess::ReadWrite);
        assert!(runtime
            .write_global_and_queue_ads("point", Value::Bool(true))
            .is_err());
        assert_eq!(runtime.storage().get_global("point"), Some(&Value::Int(1)));
        assert!(runtime
            .ads
            .connections
            .iter()
            .all(|connection| connection.bridge.pending_write("point").is_none()));
        runtime.set_execution_deadline(Some(std::time::Instant::now()));
        assert_eq!(
            runtime.write_global_and_queue_ads("point", Value::Int(8)),
            Err(RuntimeError::ExecutionTimeout)
        );
        assert_eq!(runtime.storage().get_global("point"), Some(&Value::Int(1)));
        assert!(runtime
            .ads
            .connections
            .iter()
            .all(|connection| connection.bridge.pending_write("point").is_none()));
        runtime.set_execution_deadline(None);
        runtime
            .write_global_and_queue_ads("point", Value::Int(8))
            .unwrap();
        assert_eq!(runtime.storage().get_global("point"), Some(&Value::Int(8)));
        assert_eq!(
            runtime.ads.connections[0].bridge.pending_write("point"),
            Some(Value::Int(8))
        );
        assert_eq!(
            runtime.ads.connections[1].bridge.pending_write("point"),
            None
        );
        runtime
            .write_global_and_queue_ads("ordinary", Value::Int(9))
            .unwrap();
        assert_eq!(
            runtime.storage().get_global("ordinary"),
            Some(&Value::Int(9))
        );
        assert!(runtime
            .ads
            .connections
            .iter()
            .all(|connection| connection.bridge.pending_write("ordinary").is_none()));
        assert!(!runtime.faulted());
    }

    #[test]
    fn readonly_management_write_preserves_value_and_queue() {
        let mut runtime = management_runtime(trust_ads_core::PointAccess::Read);
        let error = runtime
            .write_global_and_queue_ads("point", crate::value::Value::Int(8))
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("is read-only. Enable writes for this tag and restart the runtime."));
        assert_eq!(
            runtime.storage().get_global("point"),
            Some(&crate::value::Value::Int(1))
        );
        assert!(runtime
            .ads
            .connections
            .iter()
            .all(|connection| connection.bridge.pending_write("point").is_none()));
        assert!(!runtime.faulted());
    }

    #[test]
    fn management_queue_uses_canonical_admitted_width_for_wire_encoding() {
        use crate::value::Value;
        use trust_ads_core::{AdsDataTypeDescriptor, IecDataType, PointAccess, UpdateMode};
        for (declaration, wire, ty, expected, bytes) in [
            (
                "DINT",
                IecDataType::Dint,
                trust_hir::TypeId::DINT,
                Value::DInt(7),
                7i32.to_le_bytes().to_vec(),
            ),
            (
                "REAL",
                IecDataType::Real,
                trust_hir::TypeId::REAL,
                Value::Real(7.0),
                7f32.to_le_bytes().to_vec(),
            ),
            (
                "Alias",
                IecDataType::Dint,
                trust_hir::TypeId::DINT,
                Value::DInt(7),
                7i32.to_le_bytes().to_vec(),
            ),
        ] {
            let source = format!("TYPE Alias : DINT; END_TYPE VAR_GLOBAL point : {declaration}; END_VAR PROGRAM Main END_PROGRAM");
            let mut runtime = crate::harness::CompileSession::from_source(source)
                .build_runtime()
                .unwrap();
            let descriptor = AdsDataTypeDescriptor::scalar(declaration, wire);
            let binding = crate::ads::AdsBinding {
                point: crate::ads::AdsPointConfig {
                    point_name: "point".into(),
                    address: crate::ads::AdsPointAddress::Symbol("Remote.Value".into()),
                    data_type: descriptor.clone(),
                    access: PointAccess::ReadWrite,
                    mode: UpdateMode::Poll,
                    notification_mode: crate::ads::AdsNotificationMode::OnChange,
                    allow_retain_read: false,
                },
                reference: runtime.storage().ref_for_global("point").unwrap(),
                quality_reference: None,
                type_id: ty,
                retain: false,
            };
            runtime
                .ads
                .add_connection_for_test(route(), AdsConnectionBridge::new(vec![binding]).unwrap());
            runtime
                .write_global_and_queue_ads("point", Value::Int(7))
                .unwrap();
            assert_eq!(runtime.storage().get_global("point"), Some(&expected));
            let queued = runtime.ads.connections[0]
                .bridge
                .pending_write("point")
                .unwrap();
            assert_eq!(queued, expected);
            assert_eq!(
                trust_ads_core::ads_bytes_from_value(&descriptor, &queued).unwrap(),
                bytes
            );
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

    fn target() -> crate::ads::diagnostics::TargetIdentity {
        crate::ads::diagnostics::TargetIdentity {
            name: None,
            ip: "192.168.10.5".to_string(),
            ams_net_id: "5.23.91.12.1.1".to_string(),
            ams_port: 851,
            tc_version: None,
        }
    }

    fn local_identity() -> crate::ads::diagnostics::LocalIdentity {
        crate::ads::diagnostics::LocalIdentity {
            host_name: Some("line-controller-1".to_string()),
            chosen_ip: "192.168.10.20".to_string(),
            ams_net_id: "192.168.10.20.1.1".to_string(),
            nic: Some("eth0".to_string()),
            candidates: Vec::new(),
            classification: crate::ads::diagnostics::LocalNetworkClassification::Lan,
        }
    }
}
