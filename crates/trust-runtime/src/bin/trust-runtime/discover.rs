//! `trust-runtime discover`: the one search, on this computer or inside a runtime (specification
//! 11, "One search, device identity, and presence"). The search is `trust_runtime::search`; this
//! command builds its request, follows its progress and prints it.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use serde_json::{json, Map, Value};
use trust_runtime::search::{SearchHost, SearchRegistry, SearchRequest};
use trust_runtime_client::{ControlClient, ControlEndpoint, RESPONSE_LINE_LIMIT};

use crate::cli::DiscoverArgs;

/// How often the command reads the search's progress.
const POLL_PERIOD: Duration = Duration::from_millis(250);
/// How long one control request to a runtime may wait.
const CONTROL_TIMEOUT: Duration = Duration::from_secs(2);

pub fn run_discover(args: DiscoverArgs) -> anyhow::Result<()> {
    let params = request_params(&args)?;
    let mut search = RunningSearch::start(&args, params)?;
    let mut printer = Printer::new(args.json, args.endpoint.as_deref());
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

/// The `comm.search.start` parameters that the arguments name; the search checks them.
fn request_params(args: &DiscoverArgs) -> anyhow::Result<Value> {
    let mut params = Map::new();
    if !args.only.is_empty() {
        params.insert("only".to_string(), json!(args.only));
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
}

impl Printer {
    fn new(json: bool, endpoint: Option<&str>) -> Self {
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
        }
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
