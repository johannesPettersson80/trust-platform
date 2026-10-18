//! `trust-runtime discover`: the one search, on this computer or inside a runtime (specification
//! 11, "One search, device identity, and presence"). The search is `trust_runtime::search`; this
//! command builds its request, follows its progress and prints it.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use serde_json::{json, Map, Value};
use trust_runtime::search::project_choices::{self, Memory};
use trust_runtime::search::{SearchHost, SearchRegistry, SearchRequest, SourceId};
use trust_runtime_client::{ControlClient, ControlEndpoint, RESPONSE_LINE_LIMIT};

use crate::cli::DiscoverArgs;

/// How often the command reads the search's progress.
const POLL_PERIOD: Duration = Duration::from_millis(250);
/// How long one control request to a runtime may wait.
const CONTROL_TIMEOUT: Duration = Duration::from_secs(2);

pub fn run_discover(args: DiscoverArgs) -> anyhow::Result<()> {
    let memory = remembered(&args)?;
    let params = request_params(&args, &memory)?;
    let mut search = RunningSearch::start(&args, params)?;
    let mut printer = Printer::new(
        args.json,
        args.endpoint.as_deref(),
        memory.ignored,
        args.show_ignored,
    );
    let mut since = None;
    loop {
        let status = search.status(since)?;
        printer.progress(&status)?;
        since = status.get("cursor").and_then(Value::as_u64);
        if status.get("state").and_then(Value::as_str) != Some("running") {
            return printer.end(&status);
        }
        std::thread::sleep(POLL_PERIOD);
    }
}

/// What the project of `--project` remembers; nothing without it.
fn remembered(args: &DiscoverArgs) -> anyhow::Result<Memory> {
    match &args.project {
        None => Ok(Memory::default()),
        Some(project) => project_choices::load(project).map_err(|reason| anyhow!(reason)),
    }
}

/// The sources the search runs: `--only`; without it, with a project, the remembered sources that
/// the command line runs (specification 11, "One search"). `None` runs every source.
fn sources_to_run(args: &DiscoverArgs, memory: &Memory) -> anyhow::Result<Option<Vec<String>>> {
    if !args.only.is_empty() {
        return Ok(Some(args.only.clone()));
    }
    let Some(remembered) = &memory.protocols else {
        return Ok(None);
    };
    // A protocol that a runtime advertises is scanned from the panel, not by this command.
    let sources: Vec<String> = remembered
        .iter()
        .filter(|name| {
            SourceId::ALL
                .iter()
                .any(|source| source.as_str() == name.as_str())
        })
        .cloned()
        .collect();
    if sources.is_empty() {
        bail!("the project remembers no source that the command line runs; choose one with --only");
    }
    Ok(Some(sources))
}

/// The `comm.search.start` parameters that the arguments name; the search checks them.
fn request_params(args: &DiscoverArgs, memory: &Memory) -> anyhow::Result<Value> {
    let mut params = Map::new();
    if let Some(only) = sources_to_run(args, memory)? {
        params.insert("only".to_string(), json!(only));
    }
    let mut targets = Map::new();
    if !args.hosts.is_empty() {
        targets.insert("hosts".to_string(), json!(args.hosts));
    }
    if let Some(cidr) = &args.cidr {
        targets.insert("cidr".to_string(), json!(cidr));
    }
    if !targets.is_empty() {
        params.insert("targets".to_string(), Value::Object(targets));
    }
    if let Some(deadline_ms) = args.deadline_ms {
        params.insert("deadline_ms".to_string(), json!(deadline_ms));
    }
    if !args.confirm_fieldbus.is_empty() {
        let mut fieldbus = Vec::new();
        for item in &args.confirm_fieldbus {
            match item.split_once(':') {
                Some((bus, adapter)) => fieldbus.push(json!({ "bus": bus, "adapter": adapter })),
                None if item.trim().is_empty() => {
                    bail!("--confirm-fieldbus takes a bus, for example ethercat or ethercat:eth1")
                }
                None => fieldbus.push(json!({ "bus": item })),
            }
        }
        params.insert("confirm_fieldbus_scan".to_string(), json!(true));
        params.insert("fieldbus".to_string(), Value::Array(fieldbus));
    }
    Ok(Value::Object(params))
}

/// A search on this computer, or inside the runtime at `--endpoint`.
enum RunningSearch {
    Local {
        registry: SearchRegistry,
        search_id: String,
    },
    Remote {
        client: ControlClient,
        search_id: String,
    },
}

impl RunningSearch {
    fn start(args: &DiscoverArgs, mut params: Value) -> anyhow::Result<Self> {
        let Some(endpoint) = args.endpoint.as_deref() else {
            let registry = SearchRegistry::new();
            let search_id = SearchRequest::from_params(Some(params))
                .and_then(|request| registry.start(request, SearchHost::ThisHost))
                .map_err(|error| anyhow!("invalid search request: {error}"))?;
            return Ok(Self::Local {
                registry,
                search_id,
            });
        };
        let parsed = ControlEndpoint::parse(endpoint).with_context(|| {
            format!("'{endpoint}' is not a control endpoint (tcp://host:port or unix:///path)")
        })?;
        let token = args
            .token
            .clone()
            .or_else(|| std::env::var("TRUST_CTL_TOKEN").ok());
        let mut client = ControlClient::connect(&parsed, token.as_deref(), CONTROL_TIMEOUT)
            .map_err(|error| anyhow!("connect control endpoint {endpoint}: {error}"))?
            .with_response_limit(RESPONSE_LINE_LIMIT);
        params["origin"] = json!("runtime");
        let started = client
            .request("comm.search.start", Some(params))
            .map_err(|error| anyhow!("comm.search.start failed: {error}"))?;
        let Some(search_id) = started.get("search_id").and_then(Value::as_str) else {
            bail!("the runtime's answer to comm.search.start has no search_id");
        };
        Ok(Self::Remote {
            search_id: search_id.to_string(),
            client,
        })
    }

    /// The search's progress, as `comm.search.status` answers it.
    fn status(&mut self, since: Option<u64>) -> anyhow::Result<Value> {
        match self {
            Self::Local {
                registry,
                search_id,
            } => Ok(serde_json::to_value(registry.status(search_id, since)?)?),
            Self::Remote { client, search_id } => {
                let mut params = json!({ "search_id": search_id });
                if let Some(since) = since {
                    params["since"] = json!(since);
                }
                client
                    .request("comm.search.status", Some(params))
                    .map_err(|error| anyhow!("comm.search.status failed: {error}"))
            }
        }
    }
}

/// Prints each source's progress and each device when it is found or changes.
struct Printer {
    json: bool,
    origin_text: String,
    started: bool,
    sources: BTreeMap<String, Value>,
    devices: BTreeSet<String>,
    /// The devices that the project ignores (`devices.toml`).
    ignored: BTreeSet<String>,
    /// Whether the devices that the project ignores are printed too.
    show_ignored: bool,
}

impl Printer {
    fn new(
        json: bool,
        endpoint: Option<&str>,
        ignored: BTreeSet<String>,
        show_ignored: bool,
    ) -> Self {
        let origin_text = endpoint.map_or_else(
            || "this computer".to_string(),
            |endpoint| format!("the runtime at {endpoint}"),
        );
        Self {
            json,
            origin_text,
            started: false,
            sources: BTreeMap::new(),
            devices: BTreeSet::new(),
            ignored,
            show_ignored,
        }
    }

    /// Whether a device is left out: the project ignores it, and `--show-ignored` is not given.
    fn left_out(&self, device: &Value) -> bool {
        !self.show_ignored
            && device
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| self.ignored.contains(id))
    }

    fn progress(&mut self, status: &Value) -> anyhow::Result<()> {
        if !self.started {
            self.started = true;
            if !self.json {
                let deadline_ms = status
                    .get("deadline_ms")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                println!(
                    "Searching from {} for up to {:.1} s.",
                    self.origin_text,
                    seconds(deadline_ms)
                );
            }
        }
        for merged in array(status, "merged") {
            if let Some(id) = merged.get("id").and_then(Value::as_str) {
                self.devices.remove(id);
            }
            if self.json {
                let mut line = merged.clone();
                line["event"] = json!("merged");
                print_line(&line)?;
            }
        }
        for device in array(status, "devices") {
            if self.left_out(device) {
                continue;
            }
            if let Some(id) = device.get("id").and_then(Value::as_str) {
                self.devices.insert(id.to_string());
            }
            if self.json {
                let mut line = device.clone();
                line["event"] = json!("device");
                print_line(&line)?;
            } else {
                println!("{}", device_text(device));
            }
        }
        for source in array(status, "sources") {
            let Some(id) = source.get("id").and_then(Value::as_str) else {
                continue;
            };
            if self.sources.get(id) == Some(source) {
                continue;
            }
            self.sources.insert(id.to_string(), source.clone());
            if self.json {
                let mut line = source.clone();
                line["event"] = json!("source");
                print_line(&line)?;
            } else if let Some(text) = source_text(source) {
                println!("{text}");
            }
        }
        Ok(())
    }

    fn end(&self, status: &Value) -> anyhow::Result<()> {
        let state = status
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("done");
        let elapsed_ms = status
            .get("elapsed_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if self.json {
            return print_line(&json!({
                "event": "end",
                "search_id": status.get("search_id"),
                "origin": status.get("origin"),
                "state": state,
                "elapsed_ms": elapsed_ms,
                "devices": self.devices.len(),
                "warnings": status.get("warnings").cloned().unwrap_or_else(|| json!([])),
            }));
        }
        for warning in array(status, "warnings") {
            if let Some(warning) = warning.as_str() {
                println!("Warning: {warning}");
            }
        }
        let ended = if state == "cancelled" {
            "Search cancelled"
        } else {
            "Search done"
        };
        let count = self.devices.len();
        let noun = if count == 1 { "device" } else { "devices" };
        println!("{ended}: {count} {noun} in {:.1} s.", seconds(elapsed_ms));
        Ok(())
    }
}

/// One line for a source whose search ended or did not run; none while it searches.
fn source_text(source: &Value) -> Option<String> {
    let id = source.get("id").and_then(Value::as_str).unwrap_or_default();
    let count = source.get("count").and_then(Value::as_u64).unwrap_or(0);
    let state = match source.get("state").and_then(Value::as_str)? {
        "found" => format!("{count} found"),
        "timed_out" => format!("timed out, {count} found"),
        "cancelled" => format!("cancelled, {count} found"),
        "skipped" => format!(
            "skipped: {}",
            source
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or_default()
        ),
        _ => return None,
    };
    let mut lines = vec![format!("{:<15} {state}", source_label(id))];
    for warning in array(source, "warnings") {
        if let Some(warning) = warning.as_str() {
            lines.push(format!("{:<15} {warning}", ""));
        }
    }
    Some(lines.join("\n"))
}

/// One line for a device: its addresses and every protocol found on it.
fn device_text(device: &Value) -> String {
    let addresses: Vec<&str> = array(device, "addresses")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let name = if addresses.is_empty() {
        device
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    } else {
        addresses.join(", ")
    };
    let finds: Vec<String> = array(device, "finds")
        .iter()
        .map(|find| {
            let protocol = find
                .get("protocol")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let confidence = find
                .get("confidence")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .replace('_', " ");
            let simulated = if find.get("simulated").and_then(Value::as_bool) == Some(true) {
                ", simulated"
            } else {
                ""
            };
            format!("{} ({confidence}{simulated})", source_label(protocol))
        })
        .collect();
    format!("Found {name}: {}", finds.join(", "))
}

fn source_label(id: &str) -> &str {
    match id {
        "trust_runtime" => "truST runtimes",
        "ads" => "ADS",
        "modbus_tcp" => "Modbus TCP",
        "mqtt" => "MQTT",
        "opcua" => "OPC UA",
        "ethercat" => "EtherCAT",
        "gpio" => "GPIO",
        "serial" => "Serial",
        "usb" => "USB",
        "bluetooth" => "Bluetooth",
        other => other,
    }
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

fn print_line(value: &Value) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

fn seconds(millis: u64) -> f64 {
    Duration::from_millis(millis).as_secs_f64()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;

    /// The arguments of `trust-runtime discover` for `project`, with `--only` set to `only`.
    fn args(project: Option<PathBuf>, show_ignored: bool, only: &[&str]) -> DiscoverArgs {
        DiscoverArgs {
            endpoint: None,
            token: None,
            only: only.iter().map(|source| (*source).to_string()).collect(),
            hosts: Vec::new(),
            cidr: None,
            deadline_ms: None,
            confirm_fieldbus: Vec::new(),
            json: false,
            project,
            show_ignored,
        }
    }

    #[test]
    fn the_cli_leaves_out_ignored_finds_and_uses_the_remembered_protocols() {
        // Specification 11 ("One search", `trust-runtime discover --project`) and 25 §2: without `--only`,
        // the remembered `protocols` are the sources that run, and the devices that the project ignores
        // are left out unless `--show-ignored` is given. The expected values are literal. A wrong
        // implementation runs every source, runs `mydevice` (a protocol that a runtime advertises, which
        // this command cannot run), lets the remembered list win over `--only`, or prints the ignored device.
        let memory = Memory {
            protocols: Some(vec![
                "ads".to_owned(),
                "modbus_tcp".to_owned(),
                "mydevice".to_owned(),
            ]),
            ignored: BTreeSet::from(["dev:ip:192.168.1.20".to_owned()]),
        };
        let project = Some(PathBuf::from("/projects/cell"));
        let remembered_run = request_params(&args(project.clone(), false, &[]), &memory)
            .expect("the remembered sources are a valid request");
        assert_eq!(remembered_run["only"], json!(["ads", "modbus_tcp"]));

        let named = request_params(&args(project, false, &["gpio"]), &memory)
            .expect("--only is a valid request");
        assert_eq!(
            named["only"],
            json!(["gpio"]),
            "--only wins over the remembered list"
        );

        let status = json!({ "devices": [
            { "id": "dev:ip:192.168.1.20" },
            { "id": "dev:ip:192.168.1.21" },
        ]});
        let devices = array(&status, "devices");
        let hidden = Printer::new(false, None, memory.ignored.clone(), false);
        let shown: Vec<&Value> = devices
            .iter()
            .filter(|device| !hidden.left_out(device))
            .collect();
        assert_eq!(shown, vec![&devices[1]], "the ignored device is left out");
        let all = Printer::new(false, None, memory.ignored, true);
        assert!(
            devices.iter().all(|device| !all.left_out(device)),
            "--show-ignored prints the ignored device too"
        );
    }
}
