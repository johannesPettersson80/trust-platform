//! Project folders for the tests, removed when dropped, and the texts of their files.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FOLDER: AtomicU64 = AtomicU64::new(1);

/// A folder of its own under the system's temporary folder.
pub(crate) struct TempFolder {
    pub(crate) path: PathBuf,
}

impl TempFolder {
    pub(crate) fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "trust-devices-{label}-{}-{}",
            std::process::id(),
            NEXT_FOLDER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).expect("create a test folder");
        Self { path }
    }

    /// Writes `text` to `relative`, creating its folders.
    pub(crate) fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create the file's folder");
        }
        fs::write(&path, text).expect("write a test file");
        path
    }

    /// The path of `relative` in the folder.
    pub(crate) fn join(&self, relative: &str) -> PathBuf {
        self.path.join(relative)
    }
}

impl Drop for TempFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// A valid `runtime.toml` of the resource `name` with the control endpoint `endpoint`, and the
/// token `token`, which a TCP endpoint needs.
pub(crate) fn runtime_toml(name: &str, endpoint: &str, token: Option<&str>) -> String {
    let token = token.map_or_else(String::new, |token| format!("auth_token = \"{token}\"\n"));
    format!(
        r#"[bundle]
version = 1

[resource]
name = "{name}"
cycle_interval_ms = 100

[runtime.control]
endpoint = "{endpoint}"
{token}mode = "production"
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

/// A `fleet.toml` that lists `runtimes` as their name, control endpoint and web port. Each
/// runtime's folder is its name.
pub(crate) fn fleet_toml(runtimes: &[(&str, &str, u16)]) -> String {
    use std::fmt::Write as _;
    runtimes
        .iter()
        .fold(String::new(), |mut toml, (name, endpoint, web_port)| {
            // Writing into a String cannot fail.
            let _ = write!(
                toml,
                "[[runtime]]\nname = \"{name}\"\npath = \"{name}\"\ncontrol_endpoint = \"{endpoint}\"\nweb_port = {web_port}\n\n"
            );
            toml
        })
}

/// Writes a managed runtime `name` with the control endpoint `endpoint` below `fleet_root`.
pub(crate) fn managed_runtime(fleet_root: &Path, name: &str, endpoint: &str) {
    let folder = fleet_root.join(name);
    fs::create_dir_all(&folder).expect("create the runtime's folder");
    fs::write(
        folder.join("runtime.toml"),
        runtime_toml(name, endpoint, Some("managed-token")),
    )
    .expect("write the runtime's runtime.toml");
}
