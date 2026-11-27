//! The copy's runtime.toml as `runtime_process.rs` rewrites it before a start: the listen ports, the control
//! endpoint and token, and the settings a test asks for. Split from `runtime_process.rs`.

use std::fs;
use std::path::Path;

/// Fixed listen addresses in the example projects, by runtime.toml table and key, in the order of
/// `listen_reserved`. Each one moves to a loopback port in the copy. The examples' io.toml files
/// need no change: their only `address` keys are process-image addresses such as `%QX0.0`, not
/// network ports.
pub(super) const LISTEN_KEYS: [(&str, &str); 3] = [
    ("runtime.web", "listen"),
    ("runtime.mesh", "listen"),
    ("runtime.opcua", "listen"),
];

/// Which of `LISTEN_KEYS` take a reserved port rather than port 0. The web interface takes port 0,
/// and the runtime reports the address it bound in its fleet topology (specification 11), where
/// `Runtime::web_base` reads it; only a test that occupies the web port needs it fixed. The mesh
/// takes port 0 unless a test reserves it for another runtime to connect to
/// (`ExampleRuntime::reserve_mesh_port`). OPC UA cannot take port 0, because its endpoint URL
/// names its port, so it keeps a reserved one, and a start that finds it taken starts again on
/// fresh ports (checklist G98).
pub(super) fn listen_reserved(occupy_web_port: bool, reserve_mesh_port: bool) -> [bool; 3] {
    [occupy_web_port, reserve_mesh_port, true]
}

/// Rewrites the copied runtime.toml with the toml crate. The control endpoint becomes
/// `tcp://127.0.0.1:0`, a port the runtime chooses when it binds, and each `LISTEN_KEYS` entry that
/// exists takes the reserved port in the same place of `ports`, or port 0 where that place is
/// `None`. The copy's own token is set, the mode and debug flag are set when requested, discovery
/// is disabled unless `kept` keeps it, and so is the web interface. Every other setting
/// is kept.
pub(super) fn configure_runtime_toml(
    path: &Path,
    ports: &[Option<u16>],
    token: &str,
    mode: Option<&str>,
    debug_enabled: Option<bool>,
    start_paused: Option<bool>,
    kept: Kept,
) {
    let text = fs::read_to_string(path).expect("read runtime.toml");
    let mut document: toml::Table = toml::from_str(&text).expect("parse runtime.toml");
    let runtime = table_section(&mut document, "runtime");
    let control = table_section(runtime, "control");
    control.insert(
        "endpoint".into(),
        toml::Value::String("tcp://127.0.0.1:0".into()),
    );
    control.insert("auth_token".into(), toml::Value::String(token.into()));
    if let Some(mode) = mode {
        control.insert("mode".into(), toml::Value::String(mode.into()));
    }
    if let Some(enabled) = debug_enabled {
        control.insert("debug_enabled".into(), toml::Value::Boolean(enabled));
    }
    if let Some(paused) = start_paused {
        control.insert("start_paused".into(), toml::Value::Boolean(paused));
    }
    if let Some(web) = runtime.get_mut("web").and_then(toml::Value::as_table_mut) {
        web.insert("enabled".into(), toml::Value::Boolean(kept.web));
    }
    if !kept.discovery
        && let Some(discovery) = runtime
            .get_mut("discovery")
            .and_then(toml::Value::as_table_mut)
    {
        discovery.insert("enabled".into(), toml::Value::Boolean(false));
    }
    for (&(table, key), &port) in LISTEN_KEYS.iter().zip(ports) {
        if let Some(section) = table_path_mut(&mut document, table)
            && section.contains_key(key)
        {
            let port = port.unwrap_or(0);
            section.insert(key.into(), toml::Value::String(format!("127.0.0.1:{port}")));
        }
    }
    fs::write(
        path,
        toml::to_string(&document).expect("render runtime.toml"),
    )
    .expect("write runtime.toml");
}

/// The settings a copy keeps from its project's runtime.toml, which it otherwise turns off: the web
/// interface (`ExampleRuntime::web`) and discovery (`ExampleRuntime::keep_discovery`).
#[derive(Clone, Copy)]
pub(super) struct Kept {
    pub(super) web: bool,
    pub(super) discovery: bool,
}

/// The table under `key`, created when absent.
fn table_section<'a>(table: &'a mut toml::Table, key: &str) -> &'a mut toml::Table {
    if !table.contains_key(key) {
        table.insert(key.to_string(), toml::Value::Table(toml::Table::new()));
    }
    table
        .get_mut(key)
        .and_then(toml::Value::as_table_mut)
        .expect("runtime.toml section is a table")
}

/// The table at a dotted path such as `runtime.web`, when every segment exists.
fn table_path_mut<'a>(document: &'a mut toml::Table, path: &str) -> Option<&'a mut toml::Table> {
    let mut table = document;
    for segment in path.split('.') {
        table = table.get_mut(segment)?.as_table_mut()?;
    }
    Some(table)
}
