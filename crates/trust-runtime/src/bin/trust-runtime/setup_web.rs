//! Browser-based setup wizard for first-time configuration.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::setup::SetupDefaults;
use crate::wizard;

#[derive(Debug, Clone)]
pub struct SetupWebOptions {
    pub bundle_root: PathBuf,
    pub bind: String,
    pub port: u16,
    pub token_required: bool,
    pub token_ttl_minutes: u64,
    pub defaults: SetupDefaults,
}

#[derive(Debug)]
struct SetupState {
    options: SetupWebOptions,
    token: Option<String>,
    expires_at: Option<u64>,
    done: bool,
}

#[derive(Debug, Deserialize)]
struct SetupApplyRequest {
    #[serde(alias = "bundle_path")]
    project_path: Option<String>,
    resource_name: Option<String>,
    cycle_ms: Option<u64>,
    driver: Option<String>,
    write_system_io: Option<bool>,
    overwrite_system_io: Option<bool>,
    use_system_io: Option<bool>,
}

#[derive(Debug, Serialize)]
struct SetupDefaultsResponse {
    project_path: String,
    resource_name: String,
    cycle_ms: u64,
    driver: String,
    use_system_io: bool,
    write_system_io: bool,
    system_io_exists: bool,
    token_required: bool,
    token_expires_at: Option<u64>,
    needs_setup: bool,
}

const INDEX_HTML: &str = include_str!("../../web/ui/index.html");
const APP_JS: &str = include_str!("../../web/ui/app.js");
const APP_CSS: &str = include_str!("../../web/ui/styles.css");

pub fn run_setup_web(options: SetupWebOptions) -> anyhow::Result<()> {
    let listen = format!("{}:{}", options.bind, options.port);
    let token = if options.token_required {
        Some(generate_token())
    } else {
        None
    };
    let expires_at = token
        .as_ref()
        .map(|_| now_secs() + Duration::from_secs(options.token_ttl_minutes * 60).as_secs());
    let state = Arc::new(Mutex::new(SetupState {
        options,
        token,
        expires_at,
        done: false,
    }));
    print_setup_urls(&state);
    let server = Server::http(&listen).map_err(|err| anyhow::anyhow!("setup web bind: {err}"))?;
    for mut request in server.incoming_requests() {
        let path = request.url().to_string();
        let (path, query) = split_query(&path);
        let method = request.method().clone();
        if method == Method::Get && (path == "/" || path == "/setup") {
            if !authorize_setup(&request, query.as_deref(), &state) {
                let response = Response::from_string("setup token required").with_status_code(403);
                let _ = request.respond(response);
                continue;
            }
            let response = Response::from_string(INDEX_HTML)
                .with_header(Header::from_bytes("Content-Type", "text/html").unwrap());
            let _ = request.respond(response);
            continue;
        }
        if method == Method::Get && path == "/styles.css" {
            let response = Response::from_string(APP_CSS)
                .with_header(Header::from_bytes("Content-Type", "text/css").unwrap());
            let _ = request.respond(response);
            continue;
        }
        if method == Method::Get && path == "/app.js" {
            let response = Response::from_string(APP_JS)
                .with_header(Header::from_bytes("Content-Type", "application/javascript").unwrap());
            let _ = request.respond(response);
            continue;
        }
        if method == Method::Get && path == "/api/setup/defaults" {
            if !authorize_setup(&request, query.as_deref(), &state) {
                let response = Response::from_string("unauthorized").with_status_code(403);
                let _ = request.respond(response);
                continue;
            }
            let guard = state.lock().ok();
            let body = if let Some(guard) = guard {
                let defaults = SetupDefaultsResponse {
                    project_path: guard.options.bundle_root.display().to_string(),
                    resource_name: guard.options.defaults.resource_name.as_str().to_string(),
                    cycle_ms: guard.options.defaults.cycle_ms,
                    driver: guard.options.defaults.driver.clone(),
                    use_system_io: true,
                    write_system_io: false,
                    system_io_exists: trust_runtime::config::system_io_config_path().is_file(),
                    token_required: guard.options.token_required,
                    token_expires_at: guard.expires_at,
                    needs_setup: true,
                };
                serde_json::to_string(&defaults).unwrap_or_else(|_| "{}".to_string())
            } else {
                "{}".to_string()
            };
            let response = Response::from_string(body)
                .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());
            let _ = request.respond(response);
            continue;
        }
        if method == Method::Post && path == "/api/setup/apply" {
            if !authorize_setup(&request, query.as_deref(), &state) {
                let response = Response::from_string("unauthorized").with_status_code(403);
                let _ = request.respond(response);
                continue;
            }
            let mut body = String::new();
            if request.as_reader().read_to_string(&mut body).is_err() {
                let response = Response::from_string("invalid body").with_status_code(400);
                let _ = request.respond(response);
                continue;
            }
            let payload: SetupApplyRequest = match serde_json::from_str(&body) {
                Ok(value) => value,
                Err(_) => {
                    let response = Response::from_string("invalid json").with_status_code(400);
                    let _ = request.respond(response);
                    continue;
                }
            };
            let result = apply_setup(&state, payload);
            let response_body = match result {
                Ok(message) => {
                    let mut guard = state.lock().ok();
                    if let Some(guard) = guard.as_mut() {
                        guard.done = true;
                    }
                    message
                }
                Err(err) => format!("error: {err}"),
            };
            let response = Response::from_string(response_body)
                .with_header(Header::from_bytes("Content-Type", "text/plain").unwrap());
            let _ = request.respond(response);
            if state.lock().map(|guard| guard.done).unwrap_or(false) {
                break;
            }
            continue;
        }
        let response = Response::from_string("not found").with_status_code(StatusCode(404));
        let _ = request.respond(response);
    }
    println!("Setup web server stopped.");
    Ok(())
}

fn apply_setup(
    state: &Arc<Mutex<SetupState>>,
    payload: SetupApplyRequest,
) -> anyhow::Result<String> {
    let guard = state
        .lock()
        .map_err(|_| anyhow::anyhow!("setup state unavailable"))?;
    let bundle_root = payload
        .project_path
        .map(PathBuf::from)
        .unwrap_or_else(|| guard.options.bundle_root.clone());
    let resource_name = payload
        .resource_name
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| guard.options.defaults.resource_name.to_string());
    let cycle_ms = payload.cycle_ms.unwrap_or(guard.options.defaults.cycle_ms);
    let driver = payload
        .driver
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| guard.options.defaults.driver.clone());
    let write_system_io = payload.write_system_io.unwrap_or(true);
    let overwrite_system_io = payload.overwrite_system_io.unwrap_or(false);
    let use_system_io = payload.use_system_io.unwrap_or(true);
    drop(guard);

    wizard::create_bundle_auto(Some(bundle_root.clone()))?;
    let runtime_path = bundle_root.join("runtime.toml");
    wizard::write_runtime_toml(&runtime_path, &SmolStr::new(resource_name), cycle_ms)?;
    let io_path = bundle_root.join("io.toml");
    if use_system_io {
        wizard::remove_io_toml(&io_path)?;
    } else {
        wizard::write_io_toml_with_driver(&io_path, driver.trim())?;
    }
    if write_system_io {
        let options = trust_runtime::setup::SetupOptions {
            driver: Some(SmolStr::new(driver)),
            backend: None,
            force: overwrite_system_io,
            path: None,
        };
        trust_runtime::setup::run_setup(options)?;
    }
    Ok(format!(
        "✓ Setup complete. Start runtime with: trust-runtime --project {}",
        bundle_root.display()
    ))
}

fn authorize_setup(
    request: &tiny_http::Request,
    query: Option<&str>,
    state: &Arc<Mutex<SetupState>>,
) -> bool {
    let header_token = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("X-Setup-Token"))
        .map(|header| header.value.as_str().to_string());
    let guard = match state.lock() {
        Ok(guard) => guard,
        Err(_) => return false,
    };
    authorize_setup_access(
        guard.token.as_deref(),
        guard.expires_at,
        header_token.as_deref(),
        query,
        now_secs(),
    )
}

fn authorize_setup_access(
    expected_token: Option<&str>,
    expires_at: Option<u64>,
    header_token: Option<&str>,
    query: Option<&str>,
    now_unix: u64,
) -> bool {
    if let Some(expires_at) = expires_at {
        if now_unix > expires_at {
            return false;
        }
    }
    let Some(token) = expected_token else {
        return true;
    };
    if header_token == Some(token) {
        return true;
    }
    query
        .and_then(|value| extract_query_param(value, "token"))
        .map(|value| value == token)
        .unwrap_or(false)
}

fn split_query(path: &str) -> (String, Option<String>) {
    match path.split_once('?') {
        Some((base, query)) => (base.to_string(), Some(query.to_string())),
        None => (path.to_string(), None),
    }
}

fn extract_query_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=')?;
        if k == key {
            return Some(v.to_string());
        }
    }
    None
}

fn print_setup_urls(state: &Arc<Mutex<SetupState>>) {
    let guard = state.lock().ok();
    if guard.is_none() {
        return;
    }
    let guard = guard.unwrap();
    let token = guard.token.as_deref();
    let bind = guard.options.bind.as_str();
    let port = guard.options.port;
    let base = if bind == "0.0.0.0" {
        format!("http://trust.local:{port}/setup")
    } else {
        format!("http://localhost:{port}/setup")
    };
    if let Some(token) = token {
        println!("Setup URL (mDNS): {base}?token={token}");
        println!("If mDNS is unavailable, open the device IP in your browser.");
        println!(
            "Token expires in {} minutes",
            guard.options.token_ttl_minutes
        );
        println!("No browser available? Run: trust-runtime setup and choose CLI setup.");
    } else {
        println!("Setup URL: {base}");
        println!("No browser available? Run: trust-runtime setup and choose CLI setup.");
    }
}

fn generate_token() -> String {
    use rand::RngCore;
    let mut buf = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect::<String>()
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "trust-runtime-{prefix}-{}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn setup_access_allows_when_token_not_required() {
        assert!(authorize_setup_access(None, None, None, None, 100));
    }

    #[test]
    fn setup_access_requires_matching_token() {
        assert!(!authorize_setup_access(
            Some("secret"),
            None,
            None,
            None,
            100
        ));
        assert!(!authorize_setup_access(
            Some("secret"),
            None,
            Some("wrong"),
            None,
            100
        ));
        assert!(authorize_setup_access(
            Some("secret"),
            None,
            Some("secret"),
            None,
            100
        ));
        assert!(authorize_setup_access(
            Some("secret"),
            None,
            None,
            Some("token=secret"),
            100
        ));
    }

    #[test]
    fn setup_access_enforces_expiry() {
        assert!(authorize_setup_access(
            Some("secret"),
            Some(100),
            Some("secret"),
            None,
            100
        ));
        assert!(!authorize_setup_access(
            Some("secret"),
            Some(100),
            Some("secret"),
            None,
            101
        ));
    }

    #[test]
    fn apply_setup_persists_runtime_artifacts() {
        let root = unique_temp_dir("setup-web-artifacts");
        std::fs::create_dir_all(&root).expect("create root");
        let defaults = SetupDefaults::from_bundle(&root);
        let state = Arc::new(Mutex::new(SetupState {
            options: SetupWebOptions {
                bundle_root: root.clone(),
                bind: "127.0.0.1".to_string(),
                port: 8080,
                token_required: false,
                token_ttl_minutes: 0,
                defaults,
            },
            token: None,
            expires_at: None,
            done: false,
        }));
        let payload = SetupApplyRequest {
            project_path: Some(root.display().to_string()),
            resource_name: Some("setup_web_plc".to_string()),
            cycle_ms: Some(100),
            driver: Some("loopback".to_string()),
            write_system_io: Some(false),
            overwrite_system_io: Some(false),
            use_system_io: Some(false),
        };
        let message = apply_setup(&state, payload).expect("apply setup");
        assert!(message.contains("Setup complete"));
        assert!(root.join("runtime.toml").is_file());
        assert!(root.join("io.toml").is_file());
        assert!(root.join("program.stbc").is_file());
        assert!(root.join("sources").join("main.st").is_file());
        assert!(root.join("sources").join("config.st").is_file());
        let _ = std::fs::remove_dir_all(root);
    }
}
