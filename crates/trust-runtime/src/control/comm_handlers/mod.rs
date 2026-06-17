use std::sync::mpsc;
use std::time::Duration;

use crate::ads::diagnostics::{AdsStatusOverall, AdsStatusReport};
use crate::config::RuntimeCloudProfile;
use crate::io::{IoDriverHealth, IoDriverStatus};
use crate::scheduler::ResourceCommand;

use super::{ControlResponse, ControlState};

mod apply;
mod contract;
mod probe;
mod schema;

const ADS_STATUS_TIMEOUT: Duration = Duration::from_millis(250);

use contract::{
    action, CommCapabilitiesResponse, CommHealth, CommId, CommNextAction, CommNextActionKind,
    CommPlatform, RuntimeCapabilityStatus, COMM_SCHEMA_VERSION,
};

pub(super) fn handle_comm_capabilities(id: u64, state: &ControlState) -> ControlResponse {
    let response = build_capabilities_response(state);
    match serde_json::to_value(response) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error(
            id,
            format!("Communication capabilities serialization failed: {error}"),
        ),
    }
}

pub(super) fn handle_comm_schema(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    schema::handle_comm_schema(id, params, state)
}

pub(super) fn handle_comm_apply(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    apply::handle_comm_apply(id, params, state)
}

pub(super) fn handle_comm_test(id: u64, params: Option<serde_json::Value>) -> ControlResponse {
    probe::handle_comm_test(id, params)
}

pub(super) fn comm_test_probe_value(
    protocol: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    probe::probe_value(protocol, params)
}

pub(super) fn audit_details_for_comm_request(
    kind: &str,
    params: Option<&serde_json::Value>,
) -> Option<serde_json::Value> {
    if kind == "comm.apply" || kind == "comm.test" {
        params.map(apply::sanitized_apply_audit_details)
    } else {
        None
    }
}

fn build_capabilities_response(state: &ControlState) -> CommCapabilitiesResponse {
    let settings = state.settings.lock().ok().map(|guard| guard.clone());
    let io_health = state
        .io_health
        .lock()
        .ok()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    let ads_status = ads_status_report(state);

    let capabilities = vec![
        ads_client_capability(ads_status.as_ref()),
        ads_server_capability(state),
        opcua_capability(settings.as_ref()),
        io_driver_capability(CommId::ModbusTcp, "modbus-tcp", true, None, &io_health),
        io_driver_capability(CommId::Mqtt, "mqtt", true, None, &io_health),
        openot_capability(),
        discovery_capability(settings.as_ref()),
        mesh_capability(settings.as_ref()),
        realtime_capability(state, settings.as_ref()),
        runtime_cloud_capability(settings.as_ref()),
        io_driver_capability(
            CommId::Ethercat,
            "ethercat",
            cfg!(feature = "ethercat-wire"),
            Some(CommPlatform::Unix),
            &io_health,
        ),
        io_driver_capability(
            CommId::Gpio,
            "gpio",
            cfg!(target_os = "linux"),
            Some(CommPlatform::Linux),
            &io_health,
        ),
        io_driver_capability(CommId::Simulated, "simulated", true, None, &io_health),
        io_driver_capability(CommId::Loopback, "loopback", true, None, &io_health),
    ];

    CommCapabilitiesResponse {
        schema_version: COMM_SCHEMA_VERSION,
        capabilities,
    }
}

fn ads_client_capability(status: Option<&AdsStatusReport>) -> RuntimeCapabilityStatus {
    let built = cfg!(feature = "ads-wire");
    if !built {
        return capability(
            CommId::Ads,
            false,
            false,
            false,
            None,
            CommHealth::NotInBuild,
            "This runtime was not built with ADS client support.",
            action(
                CommNextActionKind::GetBuildWithFeature,
                "Get a build with ads-wire",
            ),
        );
    }
    let Some(status) = status else {
        return capability(
            CommId::Ads,
            true,
            false,
            false,
            None,
            CommHealth::Degraded,
            "ADS client status is unavailable from the selected runtime.",
            action(CommNextActionKind::DiagnoseAds, "Diagnose ADS"),
        );
    };
    let configured = !matches!(status.overall, AdsStatusOverall::Disabled);
    let health = match status.overall {
        AdsStatusOverall::Healthy => CommHealth::Connected,
        AdsStatusOverall::Degraded | AdsStatusOverall::Unknown => CommHealth::Degraded,
        AdsStatusOverall::Faulted => CommHealth::Error,
        AdsStatusOverall::Disabled => CommHealth::NotConfigured,
    };
    capability(
        CommId::Ads,
        true,
        configured,
        matches!(health, CommHealth::Connected),
        None,
        health,
        status.summary.as_str(),
        action_for_health(health, true),
    )
}

fn ads_server_capability(state: &ControlState) -> RuntimeCapabilityStatus {
    let built = cfg!(feature = "ads-server");
    if !built {
        return capability(
            CommId::AdsServer,
            false,
            false,
            false,
            None,
            CommHealth::NotInBuild,
            "This runtime was not built with ADS server support.",
            action(
                CommNextActionKind::GetBuildWithFeature,
                "Get a build with ads-server",
            ),
        );
    }
    let config = state
        .ads_server_config
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
        .unwrap_or_default();
    let configured = config.enabled
        || config.listen.is_some()
        || config.ams_net_id.is_some()
        || !config.expose.is_empty()
        || !config.clients.is_empty();
    if !configured {
        return capability(
            CommId::AdsServer,
            true,
            false,
            false,
            None,
            CommHealth::NotConfigured,
            "ADS server is not configured.",
            action(CommNextActionKind::Setup, "Set up ADS server"),
        );
    }
    #[cfg(feature = "ads-server")]
    let running = state
        .ads_server_runtime
        .lock()
        .ok()
        .is_some_and(|guard| guard.is_some());
    #[cfg(not(feature = "ads-server"))]
    let running = false;
    let health = if running {
        CommHealth::Connected
    } else if config.enabled {
        CommHealth::Error
    } else {
        CommHealth::NotConfigured
    };
    let detail = if running {
        "ADS server is listening and serving configured symbols."
    } else if config.enabled {
        "ADS server is configured but no running listener is reported."
    } else {
        "ADS server has partial config but is disabled."
    };
    capability(
        CommId::AdsServer,
        true,
        configured,
        running,
        None,
        health,
        detail,
        action_for_health(health, true),
    )
}

fn opcua_capability(
    settings: Option<&crate::settings::RuntimeSettings>,
) -> RuntimeCapabilityStatus {
    let built = cfg!(feature = "opcua-wire");
    let configured = settings.is_some_and(|settings| {
        settings.opcua.enabled || !settings.opcua.expose.is_empty() || settings.opcua.username_set
    });
    if !built {
        return capability(
            CommId::Opcua,
            false,
            configured,
            false,
            None,
            CommHealth::NotInBuild,
            "This runtime was not built with OPC UA support.",
            action(
                CommNextActionKind::GetBuildWithFeature,
                "Get a build with opcua-wire",
            ),
        );
    }
    if !configured {
        return capability(
            CommId::Opcua,
            true,
            false,
            false,
            None,
            CommHealth::NotConfigured,
            "OPC UA is not configured.",
            action(CommNextActionKind::Setup, "Set up OPC UA"),
        );
    }
    capability(
        CommId::Opcua,
        true,
        true,
        false,
        None,
        CommHealth::Degraded,
        "OPC UA is configured; dedicated live health is not reported yet.",
        action(CommNextActionKind::Setup, "Review OPC UA setup"),
    )
}

fn io_driver_capability(
    id: CommId,
    driver_name: &str,
    built: bool,
    platform: Option<CommPlatform>,
    health: &[IoDriverStatus],
) -> RuntimeCapabilityStatus {
    if !built {
        return capability(
            id,
            false,
            false,
            false,
            platform,
            CommHealth::NotInBuild,
            "This runtime/platform does not include this driver.",
            action(
                CommNextActionKind::GetBuildWithFeature,
                "Get a compatible build",
            ),
        );
    }
    let driver = health
        .iter()
        .find(|entry| entry.name.as_str().eq_ignore_ascii_case(driver_name));
    let Some(driver) = driver else {
        return capability(
            id,
            true,
            false,
            false,
            platform,
            CommHealth::NotConfigured,
            "No live driver instance is configured for this protocol.",
            action(CommNextActionKind::Setup, "Set up driver"),
        );
    };
    let (comm_health, detail) = match &driver.health {
        IoDriverHealth::Ok => (CommHealth::Connected, "Driver is healthy.".to_string()),
        IoDriverHealth::Degraded { error } => {
            (CommHealth::Degraded, format!("Driver is degraded: {error}"))
        }
        IoDriverHealth::Faulted { error } => {
            (CommHealth::Error, format!("Driver faulted: {error}"))
        }
    };
    capability(
        id,
        true,
        true,
        matches!(comm_health, CommHealth::Connected),
        platform,
        comm_health,
        detail,
        action_for_health(comm_health, false),
    )
}

fn openot_capability() -> RuntimeCapabilityStatus {
    let built = cfg!(unix);
    capability(
        CommId::Openot,
        built,
        false,
        false,
        Some(CommPlatform::Unix),
        if built {
            CommHealth::NotConfigured
        } else {
            CommHealth::NotInBuild
        },
        if built {
            "OpenOT telemetry config is not exposed to Communication capabilities yet."
        } else {
            "OpenOT telemetry requires a Unix shared-memory runtime build."
        },
        if built {
            action(CommNextActionKind::Setup, "Set up OpenOT")
        } else {
            action(
                CommNextActionKind::GetBuildWithFeature,
                "Use a Unix runtime",
            )
        },
    )
}

fn discovery_capability(
    settings: Option<&crate::settings::RuntimeSettings>,
) -> RuntimeCapabilityStatus {
    let configured = settings.is_some_and(|settings| settings.discovery.enabled);
    if configured {
        capability(
            CommId::Discovery,
            true,
            true,
            false,
            None,
            CommHealth::Degraded,
            "Discovery is configured; dedicated live health is not reported yet.",
            action(CommNextActionKind::Setup, "Review discovery setup"),
        )
    } else {
        capability(
            CommId::Discovery,
            true,
            false,
            false,
            None,
            CommHealth::NotConfigured,
            "Runtime discovery is not configured.",
            action(CommNextActionKind::Setup, "Set up discovery"),
        )
    }
}

fn mesh_capability(settings: Option<&crate::settings::RuntimeSettings>) -> RuntimeCapabilityStatus {
    let configured = settings.is_some_and(|settings| {
        settings.mesh.enabled
            || !settings.mesh.connect.is_empty()
            || !settings.mesh.publish.is_empty()
    });
    if configured {
        capability(
            CommId::Mesh,
            true,
            true,
            false,
            None,
            CommHealth::Degraded,
            "Mesh/Zenoh is configured; dedicated live health is not reported yet.",
            action(CommNextActionKind::Setup, "Review mesh setup"),
        )
    } else {
        capability(
            CommId::Mesh,
            true,
            false,
            false,
            None,
            CommHealth::NotConfigured,
            "Mesh/Zenoh is not configured.",
            action(CommNextActionKind::Setup, "Set up mesh"),
        )
    }
}

fn realtime_capability(
    state: &ControlState,
    settings: Option<&crate::settings::RuntimeSettings>,
) -> RuntimeCapabilityStatus {
    let configured = settings.is_some_and(|settings| settings.realtime.enabled);
    if !configured {
        return capability(
            CommId::RealtimeT0,
            true,
            false,
            false,
            Some(CommPlatform::Linux),
            CommHealth::NotConfigured,
            "Realtime T0 is not configured.",
            action(CommNextActionKind::Setup, "Set up realtime"),
        );
    }
    let status = state
        .realtime_status
        .lock()
        .ok()
        .map(|guard| guard.clone())
        .unwrap_or_else(|| crate::linux_rt::LinuxRtRuntimeStatus::from_config(Default::default()));
    let health = if !status.errors.is_empty() {
        CommHealth::Error
    } else if !status.active || !status.warnings.is_empty() {
        CommHealth::Degraded
    } else {
        CommHealth::Connected
    };
    let detail = if !status.errors.is_empty() {
        format!("Realtime has errors: {}", status.errors.join("; "))
    } else if !status.warnings.is_empty() {
        format!("Realtime has warnings: {}", status.warnings.join("; "))
    } else if status.active {
        "Realtime posture is active.".to_string()
    } else {
        "Realtime is configured but not active.".to_string()
    };
    capability(
        CommId::RealtimeT0,
        true,
        true,
        matches!(health, CommHealth::Connected),
        Some(CommPlatform::Linux),
        health,
        detail,
        action_for_health(health, false),
    )
}

fn runtime_cloud_capability(
    settings: Option<&crate::settings::RuntimeSettings>,
) -> RuntimeCapabilityStatus {
    let configured = settings.is_some_and(|settings| {
        !matches!(settings.runtime_cloud.profile, RuntimeCloudProfile::Dev)
            || !settings.runtime_cloud.link_preferences.is_empty()
            || !settings.runtime_cloud.wan_allow_write.is_empty()
    });
    if configured {
        capability(
            CommId::RuntimeCloud,
            true,
            true,
            false,
            None,
            CommHealth::ConfiguredPolicy,
            "Runtime cloud/federation policy is configured; it is not an operational live link.",
            action(CommNextActionKind::Setup, "Review federation policy"),
        )
    } else {
        capability(
            CommId::RuntimeCloud,
            true,
            false,
            false,
            None,
            CommHealth::NotConfigured,
            "Runtime cloud/federation policy is not configured.",
            action(CommNextActionKind::Setup, "Set up federation policy"),
        )
    }
}

fn ads_status_report(state: &ControlState) -> Option<AdsStatusReport> {
    let (tx, rx) = mpsc::channel();
    state
        .resource
        .send_command(ResourceCommand::AdsStatus { respond_to: tx })
        .ok()?;
    rx.recv_timeout(ADS_STATUS_TIMEOUT).ok()
}

#[allow(clippy::too_many_arguments)]
fn capability(
    id: CommId,
    built: bool,
    configured: bool,
    operational: bool,
    platform: Option<CommPlatform>,
    health: CommHealth,
    detail: impl Into<String>,
    next_action: CommNextAction,
) -> RuntimeCapabilityStatus {
    RuntimeCapabilityStatus {
        id,
        built,
        configured,
        operational,
        platform,
        health,
        detail: detail.into(),
        next_action,
    }
}

fn action_for_health(health: CommHealth, ads: bool) -> CommNextAction {
    match health {
        CommHealth::Connected => action(CommNextActionKind::None, "Status"),
        CommHealth::NotInBuild => action(
            CommNextActionKind::GetBuildWithFeature,
            "Get a compatible build",
        ),
        CommHealth::Simulate => action(CommNextActionKind::SwitchToOnline, "Switch to online"),
        CommHealth::RuntimeUnreachable => {
            action(CommNextActionKind::OpenRuntimePane, "Open Runtime pane")
        }
        CommHealth::NotConfigured | CommHealth::ConfiguredPolicy => {
            action(CommNextActionKind::Setup, "Set up")
        }
        CommHealth::Degraded | CommHealth::Error if ads => {
            action(CommNextActionKind::DiagnoseAds, "Diagnose ADS")
        }
        CommHealth::Degraded | CommHealth::Error => {
            action(CommNextActionKind::Setup, "Review setup")
        }
    }
}
