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

const SETUP_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8"/>
  <meta name="viewport" content="width=device-width, initial-scale=1"/>
  <title>truST Setup</title>
  <link rel="stylesheet" href="/styles.css"/>
</head>
<body>
  <main class="setup-wrap">
    <h1>truST Setup</h1>
    <p class="muted">Initialize runtime.toml and io.toml for this project.</p>
    <form id="setupForm" class="setup-form">
      <label>Project Path <input id="projectPath" type="text"/></label>
      <label>Resource Name <input id="resourceName" type="text"/></label>
      <label>Cycle (ms) <input id="cycleMs" type="number" min="1"/></label>
      <label>Driver <input id="driver" type="text"/></label>
      <label class="row"><input id="useSystemIo" type="checkbox"/> Use system I/O config</label>
      <label class="row"><input id="writeSystemIo" type="checkbox"/> Write system I/O config</label>
      <label class="row"><input id="overwriteSystemIo" type="checkbox"/> Overwrite existing system I/O config</label>
      <div class="actions">
        <button id="applyBtn" type="submit">Apply Setup</button>
      </div>
    </form>
    <pre id="status" class="status">Loading setup defaults...</pre>
  </main>
  <script src="/app.js"></script>
</body>
</html>
"#;

const SETUP_JS: &str = r#"(() => {
  const qs = window.location.search || "";
  const withQuery = (path) => `${path}${qs}`;
  const $ = (id) => document.getElementById(id);
  const statusEl = $("status");
  const form = $("setupForm");

  const setStatus = (text) => {
    if (statusEl) statusEl.textContent = String(text || "");
  };

  async function fetchDefaults() {
    const response = await fetch(withQuery("/api/setup/defaults"));
    if (!response.ok) {
      const body = await response.text();
      throw new Error(body || `defaults failed (${response.status})`);
    }
    return response.json();
  }

  function applyDefaults(defaults) {
    $("projectPath").value = defaults.project_path || "";
    $("resourceName").value = defaults.resource_name || "";
    $("cycleMs").value = String(defaults.cycle_ms || 100);
    $("driver").value = defaults.driver || "loopback";
    $("useSystemIo").checked = Boolean(defaults.use_system_io);
    $("writeSystemIo").checked = Boolean(defaults.write_system_io);
    $("overwriteSystemIo").checked = Boolean(defaults.system_io_exists);
  }

  async function submitSetup(event) {
    event.preventDefault();
    const payload = {
      project_path: $("projectPath").value.trim(),
      resource_name: $("resourceName").value.trim(),
      cycle_ms: Number($("cycleMs").value || 100),
      driver: $("driver").value.trim(),
      use_system_io: $("useSystemIo").checked,
      write_system_io: $("writeSystemIo").checked,
      overwrite_system_io: $("overwriteSystemIo").checked,
    };
    setStatus("Applying setup...");
    const response = await fetch(withQuery("/api/setup/apply"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    const body = await response.text();
    if (!response.ok || body.startsWith("error:")) {
      throw new Error(body || `apply failed (${response.status})`);
    }
    setStatus(body || "Setup applied.");
  }

  form?.addEventListener("submit", (event) => {
    submitSetup(event).catch((error) => setStatus(`error: ${error.message || error}`));
  });

  fetchDefaults()
    .then((defaults) => {
      applyDefaults(defaults || {});
      setStatus("Ready.");
    })
    .catch((error) => {
      setStatus(`error: ${error.message || error}`);
    });
})();
"#;

const SETUP_CSS: &str = r#"
:root { color-scheme: light dark; font-family: "Sora", sans-serif; }
body { margin: 0; background: #0b1220; color: #e2e8f0; }
.setup-wrap { max-width: 840px; margin: 24px auto; padding: 20px; background: #111827; border: 1px solid #334155; border-radius: 12px; }
h1 { margin: 0 0 8px; font-size: 24px; }
.muted { color: #94a3b8; margin: 0 0 16px; }
.setup-form { display: grid; gap: 12px; }
label { display: grid; gap: 6px; font-size: 13px; color: #cbd5e1; }
input[type="text"], input[type="number"] { padding: 9px 10px; border-radius: 8px; border: 1px solid #475569; background: #0f172a; color: #f8fafc; }
.row { display: flex; align-items: center; gap: 8px; }
.actions { display: flex; justify-content: flex-end; }
button { padding: 10px 14px; border-radius: 8px; border: 1px solid #0f766e; background: #0d9488; color: #f0fdfa; cursor: pointer; font-weight: 600; }
.status { margin: 16px 0 0; padding: 10px; min-height: 48px; border-radius: 8px; background: #020617; border: 1px solid #334155; color: #bae6fd; white-space: pre-wrap; }
"#;

pub fn run_setup_web(options: SetupWebOptions) -> anyhow::Result<()> {
    let listen = format!("{}:{}", options.bind, options.port);
    let token = if options.token_required {
        Some(generate_token()?)
    } else {
        None
    };
    let expires_at = token
        .as_ref()
        .map(|_| token_expiry(now_secs(), options.token_ttl_minutes))
        .transpose()?;
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
            let response = Response::from_string(SETUP_HTML)
                .with_header(Header::from_bytes("Content-Type", "text/html").unwrap());
            let _ = request.respond(response);
            continue;
        }
        if method == Method::Get && path == "/styles.css" {
            let response = Response::from_string(SETUP_CSS)
                .with_header(Header::from_bytes("Content-Type", "text/css").unwrap());
            let _ = request.respond(response);
            continue;
        }
        if method == Method::Get && path == "/app.js" {
            let response = Response::from_string(SETUP_JS)
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
    let write_system_io = payload.write_system_io.unwrap_or(false);
    let overwrite_system_io = payload.overwrite_system_io.unwrap_or(false);
    let use_system_io = payload.use_system_io.unwrap_or(true);
    drop(guard);

    let resource_name = SmolStr::new(resource_name);
    wizard::validate_runtime_generation_inputs(&resource_name, cycle_ms)?;
    wizard::create_bundle_auto(Some(bundle_root.clone()))?;
    let runtime_path = bundle_root.join("runtime.toml");
    wizard::write_runtime_toml(&runtime_path, &resource_name, cycle_ms)?;
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

fn generate_token() -> anyhow::Result<String> {
    let mut buf = [0u8; 16];
    let mut rng = rand::rngs::SysRng;
    generate_token_with_rng(&mut rng, &mut buf)
}

fn generate_token_with_rng<R: rand::TryRng>(
    rng: &mut R,
    bytes: &mut [u8; 16],
) -> anyhow::Result<String> {
    rng.try_fill_bytes(bytes).map_err(|error| {
        anyhow::anyhow!("operating-system secure random source failed: {error}")
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn token_expiry(now_unix: u64, token_ttl_minutes: u64) -> anyhow::Result<u64> {
    let ttl_seconds = token_ttl_minutes
        .checked_mul(Duration::from_secs(60).as_secs())
        .ok_or_else(|| anyhow::anyhow!("setup token TTL is too large"))?;
    now_unix
        .checked_add(ttl_seconds)
        .ok_or_else(|| anyhow::anyhow!("setup token TTL is too large"))
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
    fn setup_query_parsing_preserves_token_parameter() {
        assert_eq!(
            split_query("/api/setup/defaults?other=1&token=secret"),
            (
                "/api/setup/defaults".to_string(),
                Some("other=1&token=secret".to_string())
            )
        );
        assert_eq!(
            extract_query_param("other=1&token=secret", "token"),
            Some("secret".to_string())
        );
        assert_eq!(extract_query_param("token", "token"), None);
    }

    #[test]
    fn remote_setup_token_expiry_uses_checked_minutes() {
        assert_eq!(token_expiry(1_000, 15).expect("normal TTL"), 1_900);
        assert!(token_expiry(u64::MAX - 30, 1).is_err());
    }

    #[test]
    fn remote_setup_ttl_overflow_returns_error_instead_of_panicking() {
        let root = unique_temp_dir("setup-web-ttl-overflow");
        let defaults = SetupDefaults::from_bundle(&root);
        let outcome = std::panic::catch_unwind(|| {
            run_setup_web(SetupWebOptions {
                bundle_root: root,
                bind: "127.0.0.1".to_string(),
                port: 0,
                token_required: true,
                token_ttl_minutes: u64::MAX,
                defaults,
            })
        });

        let result = outcome.expect("TTL overflow must not panic");
        let error = result.expect_err("TTL overflow must fail before server bind");
        assert!(error.to_string().contains("token TTL is too large"));
    }

    #[test]
    fn remote_setup_token_generation_fails_closed_when_secure_randomness_fails() {
        #[derive(Debug)]
        struct FailingRng;

        impl rand::TryRng for FailingRng {
            type Error = std::io::Error;

            fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
                Err(std::io::Error::other("entropy unavailable"))
            }

            fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
                Err(std::io::Error::other("entropy unavailable"))
            }

            fn try_fill_bytes(&mut self, _dst: &mut [u8]) -> Result<(), Self::Error> {
                Err(std::io::Error::other("entropy unavailable"))
            }
        }

        let mut bytes = [0_u8; 16];
        let error = generate_token_with_rng(&mut FailingRng, &mut bytes)
            .expect_err("secure random failure must abort setup token generation");

        assert!(
            error
                .to_string()
                .contains("operating-system secure random source failed"),
            "unexpected error: {error:#}"
        );
        assert_eq!(bytes, [0_u8; 16], "no fallback token may be synthesized");
    }

    #[test]
    fn remote_setup_token_contains_exact_secure_random_bytes() {
        #[derive(Debug)]
        struct FixedRng;

        impl rand::TryRng for FixedRng {
            type Error = std::io::Error;

            fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
                unreachable!("token generation uses try_fill_bytes")
            }

            fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
                unreachable!("token generation uses try_fill_bytes")
            }

            fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
                for (index, byte) in destination.iter_mut().enumerate() {
                    *byte = index as u8;
                }
                Ok(())
            }
        }

        let mut bytes = [0_u8; 16];
        let token = generate_token_with_rng(&mut FixedRng, &mut bytes)
            .expect("encode deterministic secure bytes");

        assert_eq!(token, "000102030405060708090a0b0c0d0e0f");
        assert_eq!(token.len(), 32, "16 random bytes require 32 hex digits");
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
            cycle_ms: Some(25),
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
        assert!(root.join("src").join("main.st").is_file());
        assert!(root.join("src").join("config.st").is_file());
        let runtime = trust_runtime::config::RuntimeConfig::load(root.join("runtime.toml"))
            .expect("load configured runtime.toml");
        assert_eq!(runtime.resource_name, "setup_web_plc");
        assert_eq!(runtime.cycle_interval.as_millis(), 25);
        let config_source =
            std::fs::read_to_string(root.join("src").join("config.st")).expect("read config.st");
        assert!(config_source.contains("RESOURCE setup_web_plc ON PLC"));
        assert!(config_source.contains("INTERVAL := T#25ms"));
        let bundle = trust_runtime::config::RuntimeBundle::load(&root).expect("load setup bundle");
        let module = trust_runtime::bytecode::BytecodeModule::decode(&bundle.bytecode)
            .expect("decode setup bytecode");
        module.validate().expect("validate setup bytecode");
        let metadata = module.metadata().expect("load setup metadata");
        let resource = metadata
            .primary_resource()
            .expect("setup bytecode resource metadata");
        assert_eq!(resource.name, "setup_web_plc");
        assert_eq!(resource.tasks.len(), 1);
        assert_eq!(resource.tasks[0].interval.as_millis(), 25);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn apply_setup_omitted_flags_do_not_write_system_io() {
        let root = unique_temp_dir("setup-web-safe-defaults");
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
            project_path: None,
            resource_name: Some("SafeDefaults".to_string()),
            cycle_ms: Some(20),
            driver: Some("definitely-not-a-system-driver".to_string()),
            write_system_io: None,
            overwrite_system_io: None,
            use_system_io: None,
        };

        let message = apply_setup(&state, payload)
            .expect("omitted write flag must retain the advertised false default");

        assert!(message.contains("Setup complete"));
        assert!(root.join("runtime.toml").is_file());
        assert!(root.join("program.stbc").is_file());
        assert!(
            !root.join("io.toml").exists(),
            "default use_system_io must remove project io.toml"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
