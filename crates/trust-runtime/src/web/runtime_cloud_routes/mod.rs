//! Runtime-cloud HTTP route handlers for web server.

#![allow(missing_docs)]

use super::*;

mod actions;
mod config;
mod control_proxy;
mod io_proxy;
mod links;
mod rollouts;
mod state;

pub(super) struct RuntimeCloudRouteContext<'a> {
    pub auth_mode: WebAuthMode,
    pub auth_token: &'a Arc<Mutex<Option<SmolStr>>>,
    pub pairing: Option<&'a PairingStore>,
    pub web_tls_enabled: bool,
    pub control_state: &'a Arc<ControlState>,
    pub discovery: &'a Arc<DiscoveryState>,
    pub bundle_root: &'a Option<PathBuf>,
    pub profile: RuntimeCloudProfile,
    pub wan_allow_write: &'a [RuntimeCloudWanAllowRule],
    pub config_state: &'a Arc<Mutex<RuntimeCloudConfigAgentState>>,
    pub config_path: Option<&'a Path>,
    pub link_transport_state: &'a Arc<Mutex<RuntimeCloudLinkTransportState>>,
    pub link_transport_path: Option<&'a Path>,
    pub rollouts_state: &'a Arc<Mutex<RuntimeCloudRolloutManagerState>>,
    pub rollouts_path: Option<&'a Path>,
    pub ha_state: &'a Arc<Mutex<RuntimeCloudHaCoordinator>>,
}

pub(super) enum RuntimeCloudRouteOutcome {
    Handled,
    NotHandled(tiny_http::Request),
}

pub(super) fn local_runtime_id(ctx: &RuntimeCloudRouteContext<'_>) -> String {
    crate::control::runtime_resource_name_port(ctx.control_state.as_ref()).to_string()
}

pub(super) fn handle_runtime_cloud_route(
    request: tiny_http::Request,
    method: &Method,
    url: &str,
    ctx: RuntimeCloudRouteContext<'_>,
) -> RuntimeCloudRouteOutcome {
    if *method == Method::Get && url == "/api/runtime-cloud/config" {
        config::handle_get_config(request, &ctx);
        return RuntimeCloudRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/runtime-cloud/config/desired" {
        config::handle_post_config_desired(request, &ctx);
        return RuntimeCloudRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/runtime-cloud/links/transport" {
        links::handle_post_link_transport(request, &ctx);
        return RuntimeCloudRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/api/runtime-cloud/rollouts" {
        rollouts::handle_get_rollouts(request, &ctx);
        return RuntimeCloudRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/runtime-cloud/rollouts" {
        rollouts::handle_post_rollouts(request, &ctx);
        return RuntimeCloudRouteOutcome::Handled;
    }
    if *method == Method::Post && url.starts_with("/api/runtime-cloud/rollouts/") {
        rollouts::handle_post_rollout_action(request, url, &ctx);
        return RuntimeCloudRouteOutcome::Handled;
    }
    if *method == Method::Get && url == "/api/runtime-cloud/state" {
        state::handle_get_state(request, &ctx);
        return RuntimeCloudRouteOutcome::Handled;
    }
    if *method == Method::Get && url.starts_with("/api/runtime-cloud/io/config") {
        io_proxy::handle_get_io_config(request, url, &ctx);
        return RuntimeCloudRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/runtime-cloud/io/config" {
        io_proxy::handle_post_io_config(request, &ctx);
        return RuntimeCloudRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/runtime-cloud/actions/preflight" {
        actions::handle_post_preflight(request, &ctx);
        return RuntimeCloudRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/runtime-cloud/actions/dispatch" {
        actions::handle_post_dispatch(request, &ctx);
        return RuntimeCloudRouteOutcome::Handled;
    }
    if *method == Method::Post && url == "/api/runtime-cloud/control/proxy" {
        control_proxy::handle_post_control_proxy(request, &ctx);
        return RuntimeCloudRouteOutcome::Handled;
    }
    RuntimeCloudRouteOutcome::NotHandled(request)
}
