//! Web server lifecycle and top-level request loop.

#![allow(missing_docs)]

use super::request_dispatch::{RequestHandler, WebRequestDispatcher};
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
    control_state
        .web_listener_bound
        .store(true, Ordering::Relaxed);
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
    let hmi_sessions = Arc::new(crate::hmi::access::sessions::SessionStore::new(
        config.hmi.clone(),
    ));
    let hmi_access = Arc::new(config.hmi.clone());
    crate::hmi::remote_operations::configure_owner(
        &crate::control::hmi_ports::hmi_exchange(&control_state),
        hmi_sessions.clone(),
        hmi_access.clone(),
        control_state.audit_tx.clone(),
    );
    let hmi_store = Arc::new(crate::hmi::bundle::store::BundleStore::new(
        super::default_bundle_root(&bundle_root),
    ));
    let hmi_generation = Arc::new(AtomicU64::new(0));
    // The runtime keeps the active bundle's trended values from the start (§5.9 item 1).
    super::hmi_display::trends::share_trended_paths(
        &hmi_store,
        &crate::control::hmi_ports::hmi_exchange(&control_state),
    );
    let hmi_alarms =
        super::hmi_alarms::AlarmService::start(crate::control::hmi_ports::resource(&control_state));
    let hmi_commands = Arc::new(super::hmi_command::Commands {
        registry: Arc::new(crate::hmi::commands::registry::CommandRegistry::default()),
        access: hmi_access.clone(),
        control_state: control_state.clone(),
        store: hmi_store.clone(),
        alarms: hmi_alarms.clone(),
    });
    let request_handler: RequestHandler = Arc::new(move |mut request| {
        let method = request.method().clone();
        let url = request.url().to_string();
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
            ConfigUiRouteOutcome::Handled => return,
            ConfigUiRouteOutcome::NotHandled(request) => request,
        };
        request = match handle_ui_route(request, &method, url.as_str()) {
            UiRouteOutcome::Handled => return,
            UiRouteOutcome::NotHandled(request) => request,
        };
        request = match handle_hmi_deploy_route(
            request,
            &method,
            url.as_str(),
            HmiDeployContext {
                auth_mode: auth,
                auth_token: &auth_token,
                pairing: pairing.as_deref(),
                sessions: &hmi_sessions,
                store: &hmi_store,
                access: &hmi_access,
                control_state: &control_state,
                generation: &hmi_generation,
            },
        ) {
            HmiDeployOutcome::Handled => return,
            HmiDeployOutcome::NotHandled(request) => request,
        };
        request = match handle_hmi_operate_route(
            request,
            &method,
            url.as_str(),
            HmiOperateContext {
                auth_mode: auth,
                auth_token: &auth_token,
                pairing: pairing.as_deref(),
                sessions: &hmi_sessions,
                commands: &hmi_commands,
                secure: web_tls_enabled,
            },
        ) {
            HmiOperateOutcome::Handled => return,
            HmiOperateOutcome::NotHandled(request) => request,
        };
        request = match super::hmi_alarms::handle_hmi_alarm_route(
            request,
            &method,
            url.as_str(),
            super::hmi_alarms::HmiAlarmContext {
                auth_mode: auth,
                auth_token: &auth_token,
                pairing: pairing.as_deref(),
                sessions: &hmi_sessions,
                registry: &hmi_commands.registry,
                control_state: &control_state,
                alarms: &hmi_alarms,
            },
        ) {
            super::hmi_alarms::HmiAlarmOutcome::Handled => return,
            super::hmi_alarms::HmiAlarmOutcome::NotHandled(request) => request,
        };
        request = match handle_hmi_route(
            request,
            &method,
            url.as_str(),
            HmiRouteContext {
                auth_mode: auth,
                auth_token: &auth_token,
                pairing: pairing.as_deref(),
                control_state: &control_state,
                sessions: &hmi_sessions,
                commands: &hmi_commands.registry,
                store: &hmi_store,
                generation: &hmi_generation,
                alarms: &hmi_alarms,
                access: &hmi_access,
            },
        ) {
            HmiRouteOutcome::Handled => return,
            HmiRouteOutcome::NotHandled(request) => request,
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
            AuxRouteOutcome::Handled => return,
            AuxRouteOutcome::NotHandled(request) => request,
        };
        request = match handle_ads_route(
            request,
            &method,
            url.as_str(),
            AdsRouteContext {
                auth_mode: auth,
                auth_token: &auth_token,
                pairing: pairing.as_deref(),
                web_tls_enabled,
                control_state: &control_state,
            },
        ) {
            AdsRouteOutcome::Handled => return,
            AdsRouteOutcome::NotHandled(request) => request,
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
                RuntimeCloudRouteOutcome::Handled => return,
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
            IdeRouteOutcome::Handled => return,
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
                hmi_store: &hmi_store,
                hmi_access: &hmi_access,
                hmi_generation: &hmi_generation,
            },
        ) {
            OpsRouteOutcome::Handled => return,
            OpsRouteOutcome::NotHandled(request) => request,
        };
        let response = Response::from_string("not found").with_status_code(StatusCode(404));
        let _ = request.respond(response);
    });
    let dispatcher = WebRequestDispatcher::new(request_handler);
    let handle = thread::spawn(move || {
        for request in server.incoming_requests() {
            dispatcher.dispatch(request);
        }
    });

    Ok(WebServer { handle, listen })
}
