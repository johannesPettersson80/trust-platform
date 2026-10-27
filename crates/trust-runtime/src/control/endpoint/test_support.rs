//! Helpers shared by the endpoint module's tests. Each test works in its own scratch folder, so
//! the tests can run in parallel.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::file::{
    read_endpoint_file, write_endpoint_file, EndpointFile, ENDPOINT_FILE_SCHEMA_VERSION,
};
use crate::control::ControlEndpoint;

static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A scratch folder that is removed when the test ends. Folder permissions are restored first, so
/// a test may make the folder read-only.
pub(super) struct Scratch(PathBuf);

impl Scratch {
    pub(super) fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        Self::under(
            &std::env::temp_dir(),
            &format!("trust-endpoint-{label}-{nanos}"),
        )
    }

    /// A scratch folder `<parent>/<label>-<pid>-<n>`, for a test that needs a short path.
    pub(super) fn under(parent: &Path, label: &str) -> Self {
        let sequence = SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!("{label}-{}-{sequence}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create scratch folder");
        Self(path)
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }

    /// Writes a complete runtime.toml whose `[runtime.control]` table holds `control_lines`.
    pub(super) fn write_runtime_toml(&self, control_lines: &str) {
        std::fs::write(self.0.join("runtime.toml"), runtime_toml(control_lines))
            .expect("write runtime.toml");
    }

    /// Publishes an endpoint file as a running runtime with process id `pid` would on a Unix-like
    /// system, with the token in the file.
    pub(super) fn publish_file(&self, endpoint: &str, pid: u32, token: Option<&str>) {
        let file = EndpointFile {
            schema_version: ENDPOINT_FILE_SCHEMA_VERSION,
            endpoint: endpoint.to_string(),
            pid,
            auth_token: token.map(str::to_string),
        };
        write_endpoint_file(&self.0, &file).expect("write endpoint file");
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        #[cfg(unix)]
        restore_permissions(&self.0);
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Gives the owner full access to `path` and everything below it again, so it can be removed.
#[cfg(unix)]
fn restore_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        return;
    }
    let mode = if metadata.is_dir() { 0o700 } else { 0o600 };
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    if metadata.is_dir()
        && let Ok(entries) = std::fs::read_dir(path)
    {
        for entry in entries.flatten() {
            restore_permissions(&entry.path());
        }
    }
}

/// Sets the Unix permission bits of `path`.
#[cfg(unix)]
pub(super) fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("set mode");
}

/// Whether this process can still read `path`. A privileged user reads files whose mode forbids
/// it, so a test that needs an unreadable file skips itself then.
#[cfg(unix)]
pub(super) fn readable_despite_mode(path: &Path) -> bool {
    std::fs::read(path).is_ok()
}

/// A complete runtime.toml, valid for the loader, whose `[runtime.control]` table holds
/// `control_lines`. It names no web, discovery or mesh service, so nothing can bind.
pub(super) fn runtime_toml(control_lines: &str) -> String {
    format!(
        r#"
[bundle]
version = 1

[resource]
name = "main"
cycle_interval_ms = 100

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

pub(super) fn tcp(text: &str) -> ControlEndpoint {
    ControlEndpoint::Tcp(text.parse::<SocketAddr>().expect("socket address"))
}

pub(super) fn read_file(root: &Path) -> EndpointFile {
    read_endpoint_file(root)
        .expect("read endpoint file")
        .expect("endpoint file present")
}
