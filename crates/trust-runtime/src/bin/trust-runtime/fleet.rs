use std::collections::BTreeSet;
use std::fs;
use std::net::TcpListener;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::TryRng;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use toml_edit::{value, DocumentMut, Item, Table};
use trust_runtime::bundle_template::{build_io_config_auto, render_io_toml, render_runtime_toml};

use crate::cli::{FleetAction, FleetRuntimeAction, FleetRuntimeTemplateArg};

const FLEET_MANIFEST_FILE: &str = "fleet.toml";
const FLEET_MANIFEST_SCHEMA_VERSION: u32 = 1;
const DEFAULT_CONTROL_PORT: u16 = 9900;
const DEFAULT_WEB_PORT: u16 = 18080;

pub fn run_fleet(action: FleetAction) -> anyhow::Result<()> {
    match action {
        FleetAction::Runtime { action } => match action {
            FleetRuntimeAction::Add {
                fleet_root,
                name,
                template,
                control_port,
                web_port,
                json,
            } => {
                let response = add_runtime(&fleet_root, &name, template, control_port, web_port)?;
                print_add_response(response, json)
            }
        },
        FleetAction::List { fleet_root, json } => {
            let response = list_runtimes(&fleet_root)?;
            print_list_response(response, json)
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct FleetManifest {
    #[serde(default)]
    runtime: Vec<FleetManifestRuntime>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct FleetManifestRuntime {
    name: String,
    path: String,
    control_endpoint: String,
    web_port: u16,
}

#[derive(Debug, Serialize)]
struct FleetRuntimeAddResponse {
    name: String,
    path: String,
    control_endpoint: String,
    web_port: u16,
}

#[derive(Debug, Serialize)]
struct FleetListResponse {
    schema_version: u32,
    fleet_root: String,
    runtimes: Vec<FleetManifestRuntime>,
}

fn add_runtime(
    fleet_root: &Path,
    name: &str,
    template: FleetRuntimeTemplateArg,
    control_port: Option<u16>,
    web_port: Option<u16>,
) -> anyhow::Result<FleetRuntimeAddResponse> {
    let name = validate_runtime_name(name)?;
    let manifest_path = fleet_root.join(FLEET_MANIFEST_FILE);
    let mut manifest = load_manifest(&manifest_path)?;
    if manifest.runtime.iter().any(|runtime| runtime.name == name) {
        anyhow::bail!(
            "fleet runtime '{name}' already exists in {}",
            manifest_path.display()
        );
    }

    let runtime_path = fleet_root.join(name.as_str());
    if runtime_path.exists() {
        anyhow::bail!(
            "runtime project path already exists: {}",
            runtime_path.display()
        );
    }

    let mut used_ports = manifest
        .runtime
        .iter()
        .filter_map(|runtime| control_port_from_endpoint(runtime.control_endpoint.as_str()))
        .collect::<BTreeSet<_>>();
    used_ports.extend(manifest.runtime.iter().map(|runtime| runtime.web_port));

    let control_port = select_port(
        control_port,
        DEFAULT_CONTROL_PORT,
        &mut used_ports,
        "control",
    )?;
    let web_port = select_port(web_port, DEFAULT_WEB_PORT, &mut used_ports, "web")?;

    fs::create_dir_all(fleet_root)
        .with_context(|| format!("failed to create fleet root {}", fleet_root.display()))?;
    write_runtime_project(
        &runtime_path,
        name.as_str(),
        template,
        control_port,
        web_port,
    )?;

    let control_endpoint = format!("tcp://127.0.0.1:{control_port}");
    manifest.runtime.push(FleetManifestRuntime {
        name: name.clone(),
        path: name.clone(),
        control_endpoint: control_endpoint.clone(),
        web_port,
    });
    manifest
        .runtime
        .sort_by(|left, right| left.name.cmp(&right.name));
    write_manifest(&manifest_path, &manifest)?;

    Ok(FleetRuntimeAddResponse {
        name,
        path: runtime_path.display().to_string(),
        control_endpoint,
        web_port,
    })
}

fn list_runtimes(fleet_root: &Path) -> anyhow::Result<FleetListResponse> {
    let manifest = load_manifest(&fleet_root.join(FLEET_MANIFEST_FILE))?;
    Ok(FleetListResponse {
        schema_version: FLEET_MANIFEST_SCHEMA_VERSION,
        fleet_root: fleet_root.display().to_string(),
        runtimes: manifest.runtime,
    })
}

fn load_manifest(path: &Path) -> anyhow::Result<FleetManifest> {
    match fs::read_to_string(path) {
        Ok(text) => toml::from_str(text.as_str())
            .with_context(|| format!("failed to parse {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(FleetManifest::default()),
        Err(error) => Err(error).with_context(|| format!("failed to read {}", path.display())),
    }
}

fn write_manifest(path: &Path, manifest: &FleetManifest) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = toml::to_string_pretty(manifest).context("failed to serialize fleet manifest")?;
    fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn write_runtime_project(
    runtime_path: &Path,
    name: &str,
    template: FleetRuntimeTemplateArg,
    control_port: u16,
    web_port: u16,
) -> anyhow::Result<()> {
    fs::create_dir_all(runtime_path)
        .with_context(|| format!("failed to create {}", runtime_path.display()))?;

    let resource_name = SmolStr::new(resource_name_from_runtime_name(name));
    let runtime_text = runtime_toml_for_runtime(&resource_name, control_port, web_port)?;
    trust_runtime::config::validate_runtime_toml_text(runtime_text.as_str())
        .map_err(anyhow::Error::msg)
        .with_context(|| format!("generated runtime.toml is invalid for '{name}'"))?;
    fs::write(runtime_path.join("runtime.toml"), runtime_text)
        .with_context(|| format!("failed to write {}/runtime.toml", runtime_path.display()))?;

    let io_driver = match template {
        FleetRuntimeTemplateArg::Simulate => "simulated",
        FleetRuntimeTemplateArg::Empty => "loopback",
    };
    let io_text = render_io_toml(&build_io_config_auto(io_driver)?);
    trust_runtime::config::validate_io_toml_text(io_text.as_str())
        .map_err(anyhow::Error::msg)
        .with_context(|| format!("generated io.toml is invalid for '{name}'"))?;
    fs::write(runtime_path.join("io.toml"), io_text)
        .with_context(|| format!("failed to write {}/io.toml", runtime_path.display()))?;

    let src_dir = runtime_path.join("src");
    fs::create_dir_all(&src_dir)
        .with_context(|| format!("failed to create {}", src_dir.display()))?;
    fs::write(src_dir.join("main.st"), render_main_source())
        .with_context(|| format!("failed to write {}/src/main.st", runtime_path.display()))?;
    fs::write(
        src_dir.join("config.st"),
        render_config_source(&resource_name, 100),
    )
    .with_context(|| format!("failed to write {}/src/config.st", runtime_path.display()))?;

    Ok(())
}

fn runtime_toml_for_runtime(
    resource_name: &SmolStr,
    control_port: u16,
    web_port: u16,
) -> anyhow::Result<String> {
    let mut doc = render_runtime_toml(resource_name, 100)
        .parse::<DocumentMut>()
        .context("failed to parse default runtime.toml template")?;
    let runtime = table_mut(doc.as_table_mut(), "runtime")?;
    let control = table_mut(runtime, "control")?;
    control["endpoint"] = value(format!("tcp://127.0.0.1:{control_port}"));
    control["auth_token"] = value(generate_control_token());

    let web = table_mut(runtime, "web")?;
    web["listen"] = value(format!("127.0.0.1:{web_port}"));

    let discovery = table_mut(runtime, "discovery")?;
    discovery["service_name"] = value(format!("truST-{resource_name}"));

    Ok(doc.to_string())
}

fn table_mut<'a>(parent: &'a mut Table, key: &str) -> anyhow::Result<&'a mut Table> {
    if !parent.contains_key(key) {
        parent.insert(key, Item::Table(Table::new()));
    }
    parent
        .get_mut(key)
        .and_then(Item::as_table_mut)
        .with_context(|| format!("expected [{key}] to be a TOML table"))
}

fn select_port(
    requested: Option<u16>,
    start: u16,
    used: &mut BTreeSet<u16>,
    label: &str,
) -> anyhow::Result<u16> {
    if let Some(port) = requested {
        validate_requested_port(port, used, label)?;
        used.insert(port);
        return Ok(port);
    }

    for port in start..=u16::MAX {
        if used.contains(&port) || port == 0 {
            continue;
        }
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            used.insert(port);
            return Ok(port);
        }
    }
    anyhow::bail!("no free {label} port found from {start}");
}

fn validate_requested_port(port: u16, used: &BTreeSet<u16>, label: &str) -> anyhow::Result<()> {
    if port == 0 {
        anyhow::bail!("{label}-port must be a concrete TCP port, not 0");
    }
    if used.contains(&port) {
        anyhow::bail!("{label}-port {port} is already used by this fleet manifest");
    }
    TcpListener::bind(("127.0.0.1", port))
        .with_context(|| format!("{label}-port {port} is not available on 127.0.0.1"))?;
    Ok(())
}

fn control_port_from_endpoint(endpoint: &str) -> Option<u16> {
    endpoint
        .strip_prefix("tcp://")
        .and_then(|rest| rest.rsplit_once(':').map(|(_, port)| port))
        .and_then(|port| port.parse::<u16>().ok())
}

fn validate_runtime_name(name: &str) -> anyhow::Result<String> {
    let name = name.trim();
    if name.is_empty() {
        anyhow::bail!("runtime name must not be empty");
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        anyhow::bail!("runtime name may only contain ASCII letters, digits, '-' and '_'");
    }
    if name == "." || name == ".." {
        anyhow::bail!("runtime name must be a folder name, not '{name}'");
    }
    Ok(name.to_string())
}

fn resource_name_from_runtime_name(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        }
    }
    if out.is_empty() {
        "Runtime".to_string()
    } else if out.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        format!("Runtime{out}")
    } else {
        out
    }
}

fn generate_control_token() -> String {
    let mut bytes = [0_u8; 32];
    let mut rng = rand::rngs::SysRng;
    if rng.try_fill_bytes(&mut bytes).is_err() {
        let fallback = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            ^ u128::from(std::process::id());
        bytes[..16].copy_from_slice(&fallback.to_le_bytes());
    }
    URL_SAFE_NO_PAD.encode(bytes)
}

fn render_main_source() -> &'static str {
    r#"PROGRAM Main
VAR
    Count : INT := 0;
END_VAR

VAR_EXTERNAL
    InSignal : BOOL;
    OutSignal : BOOL;
END_VAR

IF InSignal THEN
    Count := Count + 1;
END_IF;
OutSignal := (Count MOD 2) = 1;
END_PROGRAM
"#
}

fn render_config_source(resource_name: &SmolStr, cycle_ms: u64) -> String {
    format!(
        "CONFIGURATION Config\nVAR_GLOBAL\n    InSignal AT %IX0.0 : BOOL;\n    OutSignal AT %QX0.0 : BOOL;\nEND_VAR\nRESOURCE {resource_name} ON PLC\n    TASK MainTask (INTERVAL := T#{cycle_ms}ms, PRIORITY := 1);\n    PROGRAM P1 WITH MainTask : Main;\nEND_RESOURCE\nEND_CONFIGURATION\n"
    )
}

fn print_add_response(response: FleetRuntimeAddResponse, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!(
            "Added runtime '{}' at {} ({}, web port {})",
            response.name, response.path, response.control_endpoint, response.web_port
        );
    }
    Ok(())
}

fn print_list_response(response: FleetListResponse, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
        return Ok(());
    }
    if response.runtimes.is_empty() {
        println!("No runtimes registered in {}", response.fleet_root);
        return Ok(());
    }
    for runtime in response.runtimes {
        println!(
            "{}\t{}\t{}\t{}",
            runtime.name, runtime.path, runtime.control_endpoint, runtime.web_port
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn fleet_runtime_add_creates_simulated_project_and_manifest() {
        let root = temp_dir("fleet-runtime-add");
        let response = add_runtime(
            &root,
            "line_1",
            FleetRuntimeTemplateArg::Simulate,
            Some(19910),
            Some(19980),
        )
        .expect("add runtime");

        assert_eq!(response.name, "line_1");
        assert_eq!(response.control_endpoint, "tcp://127.0.0.1:19910");
        assert_eq!(response.web_port, 19980);
        assert!(root.join("line_1/runtime.toml").is_file());
        assert!(root.join("line_1/io.toml").is_file());
        assert!(root.join("line_1/src/main.st").is_file());
        assert!(root.join("fleet.toml").is_file());

        let runtime_text =
            fs::read_to_string(root.join("line_1/runtime.toml")).expect("read runtime.toml");
        assert!(runtime_text.contains("endpoint = \"tcp://127.0.0.1:19910\""));
        assert!(runtime_text.contains("auth_token = "));
        trust_runtime::config::validate_runtime_toml_text(runtime_text.as_str())
            .expect("runtime.toml validates");

        let io_text = fs::read_to_string(root.join("line_1/io.toml")).expect("read io.toml");
        assert!(io_text.contains("driver = \"simulated\""));
        trust_runtime::config::validate_io_toml_text(io_text.as_str()).expect("io.toml validates");

        let listed = list_runtimes(&root).expect("list runtimes");
        assert_eq!(listed.runtimes.len(), 1);
        assert_eq!(listed.runtimes[0].name, "line_1");
        assert_eq!(listed.runtimes[0].path, "line_1");

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn fleet_runtime_add_rejects_duplicate_name_without_rewriting() {
        let root = temp_dir("fleet-runtime-duplicate");
        add_runtime(
            &root,
            "cell",
            FleetRuntimeTemplateArg::Empty,
            Some(19911),
            Some(19981),
        )
        .expect("first runtime");
        let err = add_runtime(
            &root,
            "cell",
            FleetRuntimeTemplateArg::Empty,
            Some(19912),
            Some(19982),
        )
        .expect_err("duplicate should fail");
        assert!(err.to_string().contains("already exists"), "{err}");

        let listed = list_runtimes(&root).expect("list runtimes");
        assert_eq!(listed.runtimes.len(), 1);
        assert_eq!(listed.runtimes[0].control_endpoint, "tcp://127.0.0.1:19911");

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn empty_template_uses_valid_loopback_io() {
        let root = temp_dir("fleet-runtime-empty");
        add_runtime(
            &root,
            "empty_runtime",
            FleetRuntimeTemplateArg::Empty,
            Some(19913),
            Some(19983),
        )
        .expect("add runtime");

        let io_text = fs::read_to_string(root.join("empty_runtime/io.toml")).expect("read io.toml");
        assert!(io_text.contains("driver = \"loopback\""));
        trust_runtime::config::validate_io_toml_text(io_text.as_str())
            .expect("loopback io.toml validates");

        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn fleet_runtime_add_rejects_any_manifest_port_collision() {
        let root = temp_dir("fleet-runtime-port-collision");
        add_runtime(
            &root,
            "first",
            FleetRuntimeTemplateArg::Simulate,
            Some(19914),
            Some(19984),
        )
        .expect("first runtime");

        let control_err = add_runtime(
            &root,
            "second",
            FleetRuntimeTemplateArg::Simulate,
            Some(19984),
            Some(19985),
        )
        .expect_err("control port colliding with existing web port should fail");
        assert!(
            control_err
                .to_string()
                .contains("control-port 19984 is already used"),
            "{control_err}"
        );

        let web_err = add_runtime(
            &root,
            "third",
            FleetRuntimeTemplateArg::Simulate,
            Some(19915),
            Some(19914),
        )
        .expect_err("web port colliding with existing control port should fail");
        assert!(
            web_err
                .to_string()
                .contains("web-port 19914 is already used"),
            "{web_err}"
        );

        fs::remove_dir_all(root).ok();
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "{prefix}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create temp dir");
        root
    }
}
