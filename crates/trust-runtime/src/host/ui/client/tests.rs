//! Endpoint and token precedence of the console (spec 11 section 6.8.1), its reconnection to a
//! runtime that restarted, and a refused connection (spec 11 section 6.8.0a).
//!
//! The merged `--token` and `TRUST_CTL_TOKEN` value comes from the caller, so these tests call
//! the resolver directly and never depend on the environment of the machine that runs them.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::ConsoleTarget;
use crate::control::endpoint::{ControlTarget, EndpointSource, PublishedEndpoint};
use crate::control::ControlEndpoint;

/// Resolves a console target once, as the console does before each connection attempt.
fn resolve_endpoint(
    bundle: Option<PathBuf>,
    endpoint: Option<String>,
    token: Option<String>,
) -> anyhow::Result<(ControlTarget, Option<PathBuf>)> {
    let target = ConsoleTarget::new(bundle, endpoint, token)?;
    let project = target.project().map(std::path::Path::to_path_buf);
    Ok((target.resolve()?, project))
}

static PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A scratch project directory, removed when the test ends.
struct ScratchProject(PathBuf);

impl ScratchProject {
    fn new() -> Self {
        let sequence = PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("trust-ui-client-{}-{sequence}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create console test project");
        Self(path)
    }

    /// A project whose `runtime.toml` names the endpoint and the control token.
    fn with_target(endpoint: &str, token: &str) -> Self {
        let project = Self::new();
        let control = format!("endpoint = \"{endpoint}\"\nauth_token = \"{token}\"");
        std::fs::write(project.0.join("runtime.toml"), runtime_toml(&control))
            .expect("write runtime config");
        project
    }

    /// A project whose `runtime.toml` cannot be parsed, so any read of it fails.
    fn unreadable() -> Self {
        let project = Self::new();
        std::fs::write(project.0.join("runtime.toml"), "not = [valid")
            .expect("write malformed runtime config");
        project
    }

    fn path(&self) -> PathBuf {
        self.0.clone()
    }
}

impl Drop for ScratchProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A complete runtime.toml, valid for the loader, whose `[runtime.control]` table holds
/// `control_lines`.
fn runtime_toml(control_lines: &str) -> String {
    format!(
        r#"
[bundle]
version = 1

[resource]
name = "console-test"
cycle_interval_ms = 10

[runtime]
execution_backend = "vm"

[runtime.control]
{control_lines}
mode = "production"
debug_enabled = false

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"
"#
    )
}

#[test]
fn endpoint_override_keeps_the_project_token() {
    let project = ScratchProject::with_target("tcp://127.0.0.1:1", "project-token");

    let (target, root) = resolve_endpoint(
        Some(project.path()),
        Some("tcp://127.0.0.1:9".to_string()),
        None,
    )
    .expect("resolve console target with endpoint override");

    assert!(
        matches!(target.endpoint, ControlEndpoint::Tcp(addr) if addr.port() == 9),
        "the override endpoint is used"
    );
    assert_eq!(target.source, EndpointSource::Explicit);
    assert_eq!(target.auth_token.as_deref(), Some("project-token"));
    assert_eq!(root, Some(project.path()));
}

#[test]
fn explicit_token_wins_and_the_project_is_not_read_for_it() {
    let project = ScratchProject::unreadable();

    let (target, _) = resolve_endpoint(
        Some(project.path()),
        Some("tcp://127.0.0.1:9".to_string()),
        Some("explicit-token".to_string()),
    )
    .expect("explicit token needs no project read");

    assert_eq!(target.auth_token.as_deref(), Some("explicit-token"));
}

#[test]
fn endpoint_override_without_a_project_reads_no_project() {
    let (target, root) = resolve_endpoint(None, Some("tcp://127.0.0.1:9".to_string()), None)
        .expect("resolve endpoint-only console target");

    assert!(target.auth_token.is_none());
    assert!(root.is_none());
}

#[test]
fn the_project_supplies_endpoint_and_token_without_an_override() {
    let project = ScratchProject::with_target("tcp://127.0.0.1:1", "project-token");

    let (target, root) =
        resolve_endpoint(Some(project.path()), None, None).expect("resolve project target");

    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:1");
    assert_eq!(target.source, EndpointSource::Configured);
    assert_eq!(target.auth_token.as_deref(), Some("project-token"));
    assert_eq!(root, Some(project.path()));
}

/// A loopback listener that accepts nothing on its own, so a test can see which listener a client
/// reached.
fn listener() -> (std::net::TcpListener, std::net::SocketAddr) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = listener.local_addr().expect("listener address");
    (listener, address)
}

/// Whether a connection reached `listener`.
fn reached(listener: &std::net::TcpListener) -> bool {
    listener
        .set_nonblocking(true)
        .expect("nonblocking listener");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match listener.accept() {
            Ok(_) => return true,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() > deadline {
                    return false;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(error) => panic!("accept: {error}"),
        }
    }
}

#[test]
fn each_connection_attempt_follows_the_current_endpoint_file() {
    let project = ScratchProject::new();
    std::fs::write(project.0.join("runtime.toml"), runtime_toml("")).expect("write runtime.toml");
    let target = ConsoleTarget::new(Some(project.path()), None, None).expect("console target");

    let (first, first_address) = listener();
    let published = PublishedEndpoint::publish(
        &project.0,
        &ControlEndpoint::Tcp(first_address),
        Some("first-token"),
    )
    .expect("publish the first start");
    let client = target.connect().expect("connect to the first start");
    assert_eq!(client.token.as_deref(), Some("first-token"));
    assert!(reached(&first));
    drop(published);

    // The runtime restarted on another port with another token.
    let (second, second_address) = listener();
    let republished = PublishedEndpoint::publish(
        &project.0,
        &ControlEndpoint::Tcp(second_address),
        Some("second-token"),
    )
    .expect("publish the second start");
    let client = target.connect().expect("reconnect to the second start");
    assert_eq!(client.token.as_deref(), Some("second-token"));
    assert!(reached(&second));
    drop(republished);
}

#[test]
fn a_refused_endpoint_from_the_endpoint_file_means_the_runtime_is_not_running() {
    let project = ScratchProject::new();
    std::fs::write(project.0.join("runtime.toml"), runtime_toml("")).expect("write runtime.toml");
    let (closed, address) = listener();
    drop(closed);
    let published = PublishedEndpoint::publish(&project.0, &ControlEndpoint::Tcp(address), None)
        .expect("publish a stale endpoint");
    let target = ConsoleTarget::new(Some(project.path()), None, None).expect("console target");

    let Err(error) = target.connect() else {
        panic!("nothing listens at the published endpoint");
    };

    assert!(
        error.to_string().starts_with("runtime is not running"),
        "{error:#}"
    );
    drop(published);
}
