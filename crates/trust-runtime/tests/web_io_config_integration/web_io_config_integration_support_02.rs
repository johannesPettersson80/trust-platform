use super::*;

pub(super) fn start_test_server_with_options_and_profile(
    state: Arc<ControlState>,
    project_root: PathBuf,
    discovery: Option<Arc<DiscoveryState>>,
    pairing: Option<Arc<PairingStore>>,
    auth: WebAuthMode,
    profile: RuntimeCloudProfile,
) -> String {
    if let Ok(mut settings) = state.settings.lock() {
        settings.runtime_cloud.profile = profile;
    }
    let port = reserve_loopback_port();
    let listen = format!("127.0.0.1:{port}");
    let config = WebConfig {
        enabled: true,
        listen: SmolStr::new(listen.clone()),
        auth,
        tls: false,
    };
    let _server = start_web_server(&config, state, discovery, pairing, Some(project_root), None)
        .expect("start web server");
    let base = format!("http://{listen}");
    wait_for_server(&base);
    base
}

pub(super) fn start_test_server_config_ui(
    state: Arc<ControlState>,
    project_root: PathBuf,
) -> String {
    let port = reserve_loopback_port();
    let listen = format!("127.0.0.1:{port}");
    let config = WebConfig {
        enabled: true,
        listen: SmolStr::new(listen.clone()),
        auth: WebAuthMode::Local,
        tls: false,
    };
    let _server = trust_runtime::web::start_web_server_with_mode(
        &config,
        state,
        Some(Arc::new(DiscoveryState::new())),
        None,
        Some(project_root),
        None,
        trust_runtime::web::WebServerMode::StandaloneIde,
    )
    .expect("start config-ui web server");
    let base = format!("http://{listen}");
    for _ in 0..80 {
        let mode = ureq::get(&format!("{base}/api/ui/mode")).call();
        if let Ok(mut response) = mode {
            let body = response.body_mut().read_to_string().unwrap_or_default();
            let json: Value = serde_json::from_str(&body).unwrap_or_else(|_| json!({}));
            if json.get("mode").and_then(Value::as_str) == Some("standalone-ide") {
                return base;
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("config-ui web server did not become reachable at {base}");
}

pub(super) fn wait_for_server(base: &str) {
    for _ in 0..80 {
        // Any answer, a refusal for a missing token included, means the server is listening.
        let answer = ureq::get(&format!("{base}/api/io/config"))
            .config()
            .http_status_as_error(false)
            .build()
            .call();
        if answer.is_ok() {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!("web server did not become reachable at {base}");
}

pub(super) fn make_project(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("trust-runtime-web-io-{name}-{stamp}"));
    std::fs::create_dir_all(root.join("src")).expect("create src");
    std::fs::write(
        root.join("src/main.st"),
        "PROGRAM Main\nVAR\nx : INT := 0;\nEND_VAR\nEND_PROGRAM\n",
    )
    .expect("write source");
    root
}

pub(super) fn parse_base_port(base: &str) -> u16 {
    let without_scheme = base
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    without_scheme
        .rsplit(':')
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .expect("parse test server port")
}

pub(super) fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos() as u64
}

pub(super) fn recv_audit_event(rx: &Receiver<ControlAuditEvent>) -> ControlAuditEvent {
    rx.recv_timeout(Duration::from_secs(2))
        .expect("receive control audit event")
}

pub(super) fn create_pairing_token(path: PathBuf, role: AccessRole) -> (Arc<PairingStore>, String) {
    let store = Arc::new(PairingStore::load(path));
    let code = store.start_pairing();
    let token = store
        .claim(code.code.as_str(), Some(role))
        .expect("claim pairing token");
    (store, token)
}
