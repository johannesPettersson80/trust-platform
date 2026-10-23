//! Endpoint and token precedence of the console (spec 11 section 6.8.1).
//!
//! The merged `--token` and `TRUST_CTL_TOKEN` value comes from the caller, so these tests call
//! the resolver directly and never depend on the environment of the machine that runs them.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use smol_str::SmolStr;

use super::resolve_endpoint;
use crate::bundle_template::render_runtime_toml;
use crate::control::ControlEndpoint;

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
        let runtime = render_runtime_toml(&SmolStr::new("console-test"), 10).replacen(
            "endpoint = \"unix:///tmp/trust-runtime.sock\"",
            &format!("endpoint = \"{endpoint}\"\nauth_token = \"{token}\""),
            1,
        );
        std::fs::write(project.0.join("runtime.toml"), runtime).expect("write runtime config");
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

#[test]
fn endpoint_override_keeps_the_project_token() {
    let project = ScratchProject::with_target("tcp://127.0.0.1:1", "project-token");

    let (endpoint, token, root) = resolve_endpoint(
        Some(project.path()),
        Some("tcp://127.0.0.1:9".to_string()),
        None,
    )
    .expect("resolve console target with endpoint override");

    assert!(
        matches!(endpoint, ControlEndpoint::Tcp(addr) if addr.port() == 9),
        "the override endpoint is used"
    );
    assert_eq!(token.as_deref(), Some("project-token"));
    assert_eq!(root, Some(project.path()));
}

#[test]
fn explicit_token_wins_and_the_project_is_not_read_for_it() {
    let project = ScratchProject::unreadable();

    let (_, token, _) = resolve_endpoint(
        Some(project.path()),
        Some("tcp://127.0.0.1:9".to_string()),
        Some("explicit-token".to_string()),
    )
    .expect("explicit token needs no project read");

    assert_eq!(token.as_deref(), Some("explicit-token"));
}

#[test]
fn endpoint_override_without_a_project_reads_no_project() {
    let (_, token, root) = resolve_endpoint(None, Some("tcp://127.0.0.1:9".to_string()), None)
        .expect("resolve endpoint-only console target");

    assert!(token.is_none());
    assert!(root.is_none());
}
