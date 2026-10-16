//! Endpoint and token precedence of the control helper (spec 11 section 6.8.1).
//!
//! The environment token is merged by `call_control_request`, so these tests call the resolver
//! directly and never depend on the `TRUST_CTL_TOKEN` of the machine that runs them.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use smol_str::SmolStr;
use trust_runtime::bundle_template::render_runtime_toml;

use super::resolve_control_target;

static PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A scratch project directory, removed when the test ends.
struct ScratchProject(PathBuf);

impl ScratchProject {
    fn new() -> Self {
        let sequence = PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("trust-dev-ctl-{}-{sequence}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create ctl test project");
        Self(path)
    }

    /// A project whose `runtime.toml` names the endpoint and the control token.
    fn with_target(endpoint: &str, token: &str) -> Self {
        let project = Self::new();
        let runtime = render_runtime_toml(&SmolStr::new("ctl-test"), 10).replacen(
            "endpoint = \"unix:///tmp/trust-runtime.sock\"",
            &format!("endpoint = \"{endpoint}\"\nauth_token = \"{token}\""),
            1,
        );
        std::fs::write(project.path().join("runtime.toml"), runtime).expect("write runtime config");
        project
    }

    /// A project whose `runtime.toml` cannot be parsed, so any read of it fails.
    fn unreadable() -> Self {
        let project = Self::new();
        std::fs::write(project.path().join("runtime.toml"), "not = [valid")
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

    let target = resolve_control_target(
        Some(project.path()),
        Some("tcp://127.0.0.1:9".to_string()),
        None,
    )
    .expect("resolve target with endpoint override");

    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:9");
    assert_eq!(target.auth_token.as_deref(), Some("project-token"));
}

#[test]
fn explicit_token_wins_and_the_project_is_not_read_for_it() {
    let project = ScratchProject::unreadable();

    let target = resolve_control_target(
        Some(project.path()),
        Some("tcp://127.0.0.1:9".to_string()),
        Some("explicit-token".to_string()),
    )
    .expect("explicit token needs no project read");

    assert_eq!(target.auth_token.as_deref(), Some("explicit-token"));
}

#[test]
fn project_supplies_endpoint_and_token_without_an_override() {
    let project = ScratchProject::with_target("tcp://127.0.0.1:1", "project-token");

    let target =
        resolve_control_target(Some(project.path()), None, None).expect("resolve project target");

    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:1");
    assert_eq!(target.auth_token.as_deref(), Some("project-token"));
}

#[test]
fn endpoint_override_without_a_project_sends_no_token() {
    let target = resolve_control_target(None, Some("tcp://127.0.0.1:9".to_string()), None)
        .expect("resolve endpoint-only target");

    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:9");
    assert!(target.auth_token.is_none());
}

#[test]
fn a_target_is_required() {
    let Err(err) = resolve_control_target(None, None, None) else {
        panic!("a target without endpoint or project must be rejected");
    };

    assert!(
        err.to_string().contains("--endpoint or --project required"),
        "{err}"
    );
}
