//! Attach resolution (spec 13 attach arguments; spec 11, sections 6.8.0a and 6.8.1): the
//! `runtimeRoot` project's endpoint file, its configured endpoint, the token precedence, and the
//! messages for a missing target and for a runtime that is not running.
//!
//! `TRUST_CTL_TOKEN` is passed in, so the tests never depend on the environment of the machine
//! that runs them.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use serde_json::{json, Value};
use trust_runtime::control::endpoint::{EndpointSource, PublishedEndpoint};
use trust_runtime::control::ControlEndpoint;

use super::remote::{attach_with_env_token, resolve_attach_target, RemoteEndpoint};
use crate::protocol::AttachArguments;

static PROJECT_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A scratch project folder, removed when the test ends.
struct ScratchProject(PathBuf);

impl ScratchProject {
    fn new(label: &str) -> Self {
        let sequence = PROJECT_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "trust-debug-attach-{label}-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create attach test project");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// Writes a runtime.toml whose `[runtime.control]` table holds `control_lines`.
    fn write_runtime_toml(&self, control_lines: &str) {
        let text = format!(
            "[bundle]\nversion = 1\n\n[resource]\nname = \"attach-test\"\ncycle_interval_ms = 10\n\n[runtime]\nexecution_backend = \"vm\"\n\n[runtime.control]\n{control_lines}\nmode = \"debug\"\ndebug_enabled = true\n\n[runtime.log]\nlevel = \"info\"\n\n[runtime.retain]\nmode = \"none\"\nsave_interval_ms = 1000\n\n[runtime.watchdog]\nenabled = false\ntimeout_ms = 5000\naction = \"halt\"\n\n[runtime.fault]\npolicy = \"halt\"\n"
        );
        std::fs::write(self.0.join("runtime.toml"), text).expect("write runtime.toml");
    }
}

impl Drop for ScratchProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Attach arguments from a JSON object, as the DAP `attach` request carries them.
fn attach_args(value: Value) -> AttachArguments {
    let additional = value
        .as_object()
        .expect("attach arguments object")
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    AttachArguments { additional }
}

/// A control server that answers every request successfully and records the `auth` of each.
struct FakeRuntime {
    address: SocketAddr,
    auth: Arc<Mutex<Vec<Option<String>>>>,
    server: JoinHandle<()>,
}

impl FakeRuntime {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake runtime");
        let address = listener.local_addr().expect("fake runtime address");
        let auth = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&auth);
        let server = std::thread::spawn(move || {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut writer = stream.try_clone().expect("clone fake runtime stream");
            for line in BufReader::new(stream).lines() {
                let Ok(line) = line else {
                    break;
                };
                let request: Value = serde_json::from_str(&line).expect("request JSON");
                recorded
                    .lock()
                    .expect("auth record")
                    .push(request["auth"].as_str().map(str::to_string));
                let response = json!({
                    "id": request["id"].as_u64().unwrap_or(0),
                    "ok": true,
                    "result": {"paused": false, "stops": []},
                });
                if writeln!(writer, "{response}").is_err() {
                    break;
                }
            }
        });
        Self {
            address,
            auth,
            server,
        }
    }

    /// The `auth` of each request, once the session that sent them has closed its connection.
    fn finish(self) -> Vec<Option<String>> {
        self.server.join().expect("join fake runtime");
        let auth = self.auth.lock().expect("auth record");
        auth.clone()
    }
}

#[test]
fn neither_endpoint_nor_runtime_root_is_refused_with_the_attach_message() {
    let error =
        resolve_attach_target(&attach_args(json!({})), None).expect_err("attach needs a target");

    assert_eq!(error, "attach requires endpoint or runtimeRoot");
}

#[test]
fn runtime_root_attaches_through_the_endpoint_file_with_its_token() {
    let project = ScratchProject::new("file");
    let runtime = FakeRuntime::start();
    let published = PublishedEndpoint::publish(
        project.path(),
        &ControlEndpoint::Tcp(runtime.address),
        Some("file-token"),
    )
    .expect("publish endpoint file");
    let args = attach_args(json!({"runtimeRoot": project.path()}));

    let target = resolve_attach_target(&args, None).expect("resolve attach target");
    assert_eq!(target.source, EndpointSource::EndpointFile);
    assert_eq!(target.endpoint_text, format!("tcp://{}", runtime.address));

    let (session, _) = attach_with_env_token(&args, None).expect("attach");
    assert!(matches!(session.endpoint(), RemoteEndpoint::Tcp(addr) if *addr == runtime.address));
    drop(session);

    let auth = runtime.finish();
    assert!(!auth.is_empty(), "attach sends its first requests");
    assert!(
        auth.iter()
            .all(|value| value.as_deref() == Some("file-token")),
        "{auth:?}"
    );
    drop(published);
}

#[test]
fn an_explicit_auth_token_beats_the_environment_and_the_endpoint_file_token() {
    let project = ScratchProject::new("auth-token");
    let runtime = FakeRuntime::start();
    let published = PublishedEndpoint::publish(
        project.path(),
        &ControlEndpoint::Tcp(runtime.address),
        Some("file-token"),
    )
    .expect("publish endpoint file");
    let args = attach_args(json!({
        "runtimeRoot": project.path(),
        "authToken": "explicit-token",
    }));

    let (session, _) = attach_with_env_token(&args, Some("env-token".to_string())).expect("attach");
    drop(session);

    let auth = runtime.finish();
    assert!(
        auth.iter()
            .all(|value| value.as_deref() == Some("explicit-token")),
        "{auth:?}"
    );
    drop(published);
}

#[test]
fn the_environment_token_beats_the_endpoint_file_token() {
    let project = ScratchProject::new("env-token");
    project.write_runtime_toml("");
    let published = PublishedEndpoint::publish(
        project.path(),
        &ControlEndpoint::Tcp("127.0.0.1:9".parse().expect("address")),
        Some("file-token"),
    )
    .expect("publish endpoint file");

    let target = resolve_attach_target(
        &attach_args(json!({"runtimeRoot": project.path()})),
        Some("env-token".to_string()),
    )
    .expect("resolve attach target");

    assert_eq!(target.auth_token.as_deref(), Some("env-token"));
    drop(published);
}

#[test]
fn runtime_root_without_an_endpoint_file_uses_the_configured_endpoint_and_token() {
    let project = ScratchProject::new("configured");
    project.write_runtime_toml(
        "endpoint = \"tcp://127.0.0.1:9123\"\nauth_token = \"configured-token\"",
    );

    let target = resolve_attach_target(&attach_args(json!({"runtimeRoot": project.path()})), None)
        .expect("resolve attach target");

    assert_eq!(target.source, EndpointSource::Configured);
    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:9123");
    assert_eq!(target.auth_token.as_deref(), Some("configured-token"));
}

#[test]
fn an_explicit_endpoint_is_used_with_the_project_token() {
    let project = ScratchProject::new("explicit");
    project.write_runtime_toml(
        "endpoint = \"tcp://127.0.0.1:9123\"\nauth_token = \"configured-token\"",
    );

    let target = resolve_attach_target(
        &attach_args(json!({
            "endpoint": "tcp://127.0.0.1:9124",
            "runtimeRoot": project.path(),
        })),
        None,
    )
    .expect("resolve attach target");

    assert_eq!(target.source, EndpointSource::Explicit);
    assert_eq!(target.endpoint_text, "tcp://127.0.0.1:9124");
    assert_eq!(target.auth_token.as_deref(), Some("configured-token"));
}

#[test]
fn an_endpoint_file_whose_runtime_does_not_answer_means_the_runtime_is_not_running() {
    let project = ScratchProject::new("refused");
    let closed = TcpListener::bind("127.0.0.1:0").expect("bind listener");
    let address = closed.local_addr().expect("listener address");
    drop(closed);
    let published =
        PublishedEndpoint::publish(project.path(), &ControlEndpoint::Tcp(address), None)
            .expect("publish a stale endpoint file");

    let Err(error) =
        attach_with_env_token(&attach_args(json!({"runtimeRoot": project.path()})), None)
    else {
        panic!("nothing listens at the published endpoint");
    };

    assert!(error.starts_with("runtime is not running"), "{error}");
    assert!(error.contains(&address.to_string()), "{error}");
    drop(published);
}
