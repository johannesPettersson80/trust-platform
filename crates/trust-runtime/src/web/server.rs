//! Web server lifecycle and top-level request loop.

#![allow(missing_docs)]

use super::*;
pub struct WebServer {
    // Retained to keep the web thread alive for the lifetime of the server handle.
    #[allow(dead_code)]
    handle: thread::JoinHandle<()>,
    pub listen: String,
}

pub fn start_web_server(
    config: &WebConfig,
    control_state: Arc<ControlState>,
    discovery: Option<Arc<DiscoveryState>>,
    pairing: Option<Arc<PairingStore>>,
    bundle_root: Option<PathBuf>,
    tls_materials: Option<Arc<TlsMaterials>>,
) -> Result<WebServer, RuntimeError> {
    start_web_server_with_mode(
        config,
        control_state,
        discovery,
        pairing,
        bundle_root,
        tls_materials,
        WebServerMode::Runtime,
    )
}

pub fn start_web_server_with_mode(
    config: &WebConfig,
    control_state: Arc<ControlState>,
    discovery: Option<Arc<DiscoveryState>>,
    pairing: Option<Arc<PairingStore>>,
    bundle_root: Option<PathBuf>,
    tls_materials: Option<Arc<TlsMaterials>>,
    mode: WebServerMode,
) -> Result<WebServer, RuntimeError> {
    if !config.enabled {
        return Err(RuntimeError::ControlError("web disabled".into()));
    }
    let listen = config.listen.to_string();
    let server = if config.tls {
        let materials = tls_materials.as_ref().ok_or_else(|| {
            RuntimeError::ControlError(
                "web tls enabled but runtime.tls certificate settings are unavailable".into(),
            )
        })?;
        Server::https(&listen, materials.tiny_http_ssl_config())
            .map_err(|err| RuntimeError::ControlError(format!("web tls bind: {err}").into()))?
    } else {
        Server::http(&listen)
            .map_err(|err| RuntimeError::ControlError(format!("web bind: {err}").into()))?
    };
    let auth = config.auth;
    let web_url = format_web_url(&listen, config.tls);
    let auth_token = control_state.auth_token.clone();
    let discovery = discovery.unwrap_or_else(|| Arc::new(DiscoveryState::new()));
    let pairing = pairing.or_else(|| {
        bundle_root
            .as_ref()
            .map(|root| Arc::new(PairingStore::load(root.join("pairings.json"))))
    });
    let ide_state = Arc::new(WebIdeState::new(bundle_root.clone()));
    let ide_task_store: Arc<Mutex<HashMap<u64, IdeTaskJob>>> = Arc::new(Mutex::new(HashMap::new()));
    let ide_task_seq = Arc::new(AtomicU64::new(1));
    let bundle_root = bundle_root.clone();
    let runtime_cloud_profile = control_state
        .settings
        .lock()
        .ok()
        .map(|settings| settings.runtime_cloud.profile)
        .unwrap_or(RuntimeCloudProfile::Dev);
    let runtime_cloud_wan_allow_write = control_state
        .settings
        .lock()
        .ok()
        .map(|settings| settings.runtime_cloud.wan_allow_write.clone())
        .unwrap_or_default();
    let runtime_cloud_link_preferences = control_state
        .settings
        .lock()
        .ok()
        .map(|settings| settings.runtime_cloud.link_preferences.clone())
        .unwrap_or_default();
    let runtime_cloud_config_path = runtime_cloud_config_state_path(bundle_root.as_ref());
    let runtime_cloud_link_transport_path = runtime_cloud_links_state_path(bundle_root.as_ref());
    let runtime_cloud_rollouts_path = runtime_cloud_rollouts_state_path(bundle_root.as_ref());
    let runtime_cloud_config = Arc::new(Mutex::new(runtime_cloud_config_load_state(
        runtime_cloud_config_path.as_deref(),
    )));
    let mut runtime_cloud_link_transport_state =
        runtime_cloud_links_load_state(runtime_cloud_link_transport_path.as_deref())?;
    if runtime_cloud_seed_link_transport_preferences(
        &mut runtime_cloud_link_transport_state,
        runtime_cloud_link_preferences.as_slice(),
        "runtime.toml",
    ) {
        runtime_cloud_links_store_state(
            runtime_cloud_link_transport_path.as_deref(),
            &runtime_cloud_link_transport_state,
        );
    }
    let runtime_cloud_link_transport = Arc::new(Mutex::new(runtime_cloud_link_transport_state));
    let runtime_cloud_rollouts = Arc::new(Mutex::new(runtime_cloud_rollouts_load_state(
        runtime_cloud_rollouts_path.as_deref(),
    )?));
    let runtime_cloud_ha = Arc::new(Mutex::new(RuntimeCloudHaCoordinator::default()));
    let web_tls_enabled = config.tls;
    let handle = thread::spawn(move || {
        for mut request in server.incoming_requests() {
            let method = request.method().clone();
            let url = request.url().to_string();
            let url_path = url.split('?').next().unwrap_or(url.as_str());
            request = match handle_config_ui_route(
                request,
                &method,
                url.as_str(),
                ConfigUiRouteContext {
                    mode,
                    auth_mode: auth,
                    auth_token: &auth_token,
                    pairing: pairing.as_deref(),
                    control_state: &control_state,
                    bundle_root: &bundle_root,
                },
            ) {
                ConfigUiRouteOutcome::Handled => continue,
                ConfigUiRouteOutcome::NotHandled(request) => request,
            };
            request = match handle_ui_route(
                request,
                &method,
                url.as_str(),
                url_path,
                UiRouteContext {
                    auth_mode: auth,
                    auth_token: &auth_token,
                    pairing: pairing.as_deref(),
                    control_state: &control_state,
                    hmi_asset_root: bundle_root.clone().or_else(|| {
                        crate::control::hmi_asset_project_root_port(control_state.as_ref())
                    }),
                },
            ) {
                UiRouteOutcome::Handled => continue,
                UiRouteOutcome::NotHandled(request) => request,
            };
            request = match handle_aux_route(
                request,
                &method,
                url.as_str(),
                AuxRouteContext {
                    mode,
                    auth_mode: auth,
                    auth_token: &auth_token,
                    pairing: pairing.as_deref(),
                    web_tls_enabled,
                    control_state: &control_state,
                    discovery: &discovery,
                    bundle_root: &bundle_root,
                },
            ) {
                AuxRouteOutcome::Handled => continue,
                AuxRouteOutcome::NotHandled(request) => request,
            };
            if mode == WebServerMode::Runtime {
                request = match handle_runtime_cloud_route(
                    request,
                    &method,
                    url.as_str(),
                    RuntimeCloudRouteContext {
                        auth_mode: auth,
                        auth_token: &auth_token,
                        pairing: pairing.as_deref(),
                        web_tls_enabled,
                        control_state: &control_state,
                        discovery: &discovery,
                        bundle_root: &bundle_root,
                        profile: runtime_cloud_profile,
                        wan_allow_write: runtime_cloud_wan_allow_write.as_slice(),
                        config_state: &runtime_cloud_config,
                        config_path: runtime_cloud_config_path.as_deref(),
                        link_transport_state: &runtime_cloud_link_transport,
                        link_transport_path: runtime_cloud_link_transport_path.as_deref(),
                        rollouts_state: &runtime_cloud_rollouts,
                        rollouts_path: runtime_cloud_rollouts_path.as_deref(),
                        ha_state: &runtime_cloud_ha,
                    },
                ) {
                    RuntimeCloudRouteOutcome::Handled => continue,
                    RuntimeCloudRouteOutcome::NotHandled(request) => request,
                };
            }
            request = match handle_ide_route(
                request,
                &method,
                url.as_str(),
                IdeRouteContext {
                    auth_mode: auth,
                    auth_token: &auth_token,
                    pairing: pairing.as_deref(),
                    control_state: &control_state,
                    bundle_root: &bundle_root,
                    ide_state: &ide_state,
                    ide_task_store: &ide_task_store,
                    ide_task_seq: &ide_task_seq,
                },
            ) {
                IdeRouteOutcome::Handled => continue,
                IdeRouteOutcome::NotHandled(request) => request,
            };
            request = match handle_ops_route(
                request,
                &method,
                url.as_str(),
                OpsRouteContext {
                    auth_mode: auth,
                    auth_token: &auth_token,
                    pairing: pairing.as_deref(),
                    web_url: web_url.as_str(),
                    control_state: &control_state,
                    bundle_root: &bundle_root,
                    web_tls_enabled,
                },
            ) {
                OpsRouteOutcome::Handled => continue,
                OpsRouteOutcome::NotHandled(request) => request,
            };
            let response = Response::from_string("not found").with_status_code(StatusCode(404));
            let _ = request.respond(response);
        }
    });

    Ok(WebServer { handle, listen })
}
