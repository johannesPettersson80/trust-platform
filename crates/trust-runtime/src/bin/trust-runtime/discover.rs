//! `trust-runtime discover`: the one search, on this computer or inside a runtime (specification
//! 11, "One search, device identity, and presence"). The search is `trust_runtime::search`; this
//! command builds its request, follows its progress and prints it. With `--compare` it compares the
//! search with a project instead of printing the devices (specification 11, "Compare with the
//! project").

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use serde_json::{json, Map, Value};
use trust_runtime::search::compare::{compare, searched, Comparison, Reason};
use trust_runtime::search::configured::ConfiguredDevice;
use trust_runtime::search::project_choices::{self, Memory};
use trust_runtime::search::project_devices;
use trust_runtime::search::{SearchHost, SearchRegistry, SearchRequest, SourceId};
use trust_runtime_client::{ControlClient, ControlEndpoint, RESPONSE_LINE_LIMIT};

use crate::cli::DiscoverArgs;

/// How often the command reads the search's progress.
const POLL_PERIOD: Duration = Duration::from_millis(250);
/// How long one control request to a runtime may wait.
const CONTROL_TIMEOUT: Duration = Duration::from_secs(2);
/// How long one control request to a chip's serial port may wait (specification 11, 6.8.1).
const SERIAL_CONTROL_TIMEOUT: Duration = Duration::from_secs(5);

pub fn run_discover(args: DiscoverArgs) -> anyhow::Result<()> {
    let plan = Plan::of(&args)?;
    let mut search = RunningSearch::start(&args, plan.params.clone())?;
    let mut printer = Printer::new(
        args.json,
        args.endpoint.as_deref(),
        plan.memory.ignored.clone(),
        args.show_ignored,
    );
    if args.compare {
        printer = printer.without_devices();
    }
    let mut since = None;
    loop {
        let status = search.status(since)?;
        printer.progress(&status)?;
        since = status.get("cursor").and_then(Value::as_u64);
        if status.get("state").and_then(Value::as_str) != Some("running") {
            if let Some(project) = args.project.as_deref().filter(|_| args.compare) {
                let (search_id, comparison) = compare_finished(&search, &plan)?;
                printer.compare(project, &search_id, &comparison)?;
            }
            return printer.end(&status);
        }
        std::thread::sleep(POLL_PERIOD);
    }
}

/// What the arguments and the project decide before the search starts.
struct Plan {
    /// What the project remembers of the Discover pane.
    memory: Memory,
    /// The devices of the project that `--compare` compares with; none without `--compare`.
    configured: Vec<ConfiguredDevice>,
    /// The `comm.search.start` parameters.
    params: Value,
}

impl Plan {
    /// Reads the project's choices and, with `--compare`, its devices, and builds the request.
    fn of(args: &DiscoverArgs) -> anyhow::Result<Self> {
        let memory = remembered(args)?;
        let mut params = request_params(args, &memory)?;
        let configured = match args.project.as_deref().filter(|_| args.compare) {
            Some(project) => project_devices::load(project).map_err(|reason| anyhow!(reason))?,
            None => Vec::new(),
        };
        direct_at_project(&mut params, &configured);
        Ok(Self {
            memory,
            configured,
            params,
        })
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

/// Directs the search at the devices of the project, so that a device that is not found is a device
/// that was asked for and did not answer (specification 11, "Command line"): their addresses join the
/// known addresses, and the Modbus unit IDs that a search can probe become `modbus.unit_ids`.
fn direct_at_project(params: &mut Value, configured: &[ConfiguredDevice]) {
    let Some(request) = params.as_object_mut() else {
        return;
    };
    let mut hosts: Vec<String> = request
        .get("targets")
        .and_then(|targets| targets.get("hosts"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    for target in configured.iter().filter_map(ConfiguredDevice::target) {
        if !hosts.contains(&target) {
            hosts.push(target);
        }
    }
    if !hosts.is_empty() {
        let targets = request.entry("targets").or_insert_with(|| json!({}));
        targets["hosts"] = json!(hosts);
    }
    let unit_ids: BTreeSet<u8> = configured
        .iter()
        .filter_map(ConfiguredDevice::probe_unit_id)
        .collect();
    if !unit_ids.is_empty() {
        request.insert("modbus".to_string(), json!({ "unit_ids": unit_ids }));
    }
}

/// The search ID and the comparison of the finished search with the project's devices.
fn compare_finished(search: &RunningSearch, plan: &Plan) -> anyhow::Result<(String, Comparison)> {
    let RunningSearch::Local {
        registry,
        search_id,
    } = search
    else {
        bail!("--compare compares a search that ran on this computer");
    };
    let status = registry.finished_status(Some(search_id.as_str()))?;
    let comparison = compare(
        &searched(&status),
        &status.devices,
        &plan.configured,
        &plan.memory.ignored,
    );
    Ok((search_id.clone(), comparison))
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
            format!(
                "'{endpoint}' is not a control endpoint (tcp://host:port, unix:///path or \
                 serial:///dev/ttyACM0)"
            )
        })?;
        let token = args
            .token
            .clone()
            .or_else(|| std::env::var("TRUST_CTL_TOKEN").ok());
        let timeout = if matches!(parsed, ControlEndpoint::Serial(_)) {
            SERIAL_CONTROL_TIMEOUT
        } else {
            CONTROL_TIMEOUT
        };
        let mut client = ControlClient::connect(&parsed, token.as_deref(), timeout)
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
    /// Whether each device is printed when it is found; `--compare` prints the comparison instead.
    show_devices: bool,
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
            show_devices: true,
        }
    }

    /// The same printer, which counts the devices without printing them.
    fn without_devices(mut self) -> Self {
        self.show_devices = false;
        self
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
            if self.json && self.show_devices {
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
            if !self.show_devices {
                continue;
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

    /// Prints the comparison of the search with `project` (specification 11, "Command line").
    fn compare(
        &self,
        project: &Path,
        search_id: &str,
        comparison: &Comparison,
    ) -> anyhow::Result<()> {
        if self.json {
            return print_line(&compare_event(project, search_id, comparison));
        }
        println!("Compared with the project {}.", project.display());
        for line in comparison_text(&comparison.report(search_id)) {
            println!("{line}");
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

/// The `compare` line of `--json` (specification 11, "Command line"): the reply of `comm.compare` with
/// the name of the event and the project's folder.
fn compare_event(project: &Path, search_id: &str, comparison: &Comparison) -> Value {
    let mut event = comparison.report(search_id);
    event["event"] = json!("compare");
    event["project"] = json!(project.display().to_string());
    event
}

/// The four sections of a comparison (specification 11, "Command line"): a heading with its number of
/// entries, then one indented line for each entry. Read from the reply, so that it prints what
/// `comm.compare` and `--json` give.
fn comparison_text(report: &Value) -> Vec<String> {
    let text = |entry: &Value, key: &str| {
        entry
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let mut lines = Vec::new();

    let missing = array(report, "missing");
    lines.push(format!("Missing: {}", missing.len()));
    for entry in missing {
        lines.push(format!(
            "  {}: {}",
            source_label(&text(entry, "protocol")),
            facts_text(&entry["configured"])
        ));
    }

    let unused = array(report, "unused");
    lines.push(format!("Not in the project: {}", unused.len()));
    for entry in unused {
        let protocols: Vec<&str> = array(entry, "protocols")
            .iter()
            .filter_map(Value::as_str)
            .map(source_label)
            .collect();
        lines.push(format!(
            "  {}: {}",
            text(entry, "device_id"),
            protocols.join(", ")
        ));
    }

    let differs = array(report, "differs");
    lines.push(format!("Differs from the project: {}", differs.len()));
    for entry in differs {
        let fields: Vec<String> = array(entry, "fields")
            .iter()
            .filter_map(Value::as_str)
            .map(|field| {
                format!(
                    "{field} project {}, found {}",
                    fact_text(&entry["configured"][field]),
                    fact_text(&entry["found"][field])
                )
            })
            .collect();
        lines.push(format!(
            "  {}: {}: {}",
            text(entry, "device_id"),
            source_label(&text(entry, "protocol")),
            fields.join("; ")
        ));
    }

    let skipped = array(report, "not_compared");
    lines.push(format!("Not compared: {}", skipped.len()));
    for entry in skipped {
        lines.push(format!(
            "  {}: {} ({})",
            source_label(&text(entry, "protocol")),
            facts_text(&entry["configured"]),
            reason_text(&text(entry, "reason"))
        ));
    }
    lines
}

/// The facts of a device as `name value` pairs in the order of their names, separated by `, `.
fn facts_text(facts: &Value) -> String {
    let mut pairs: Vec<(&String, &Value)> = facts
        .as_object()
        .map(|facts| facts.iter().collect())
        .unwrap_or_default();
    pairs.sort_by_key(|(name, _)| *name);
    pairs
        .into_iter()
        .map(|(name, value)| format!("{name} {}", fact_text(value)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// One fact: its text, or `none` for a fact the device did not report.
fn fact_text(fact: &Value) -> String {
    match fact {
        Value::Null => "none".to_string(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Why a configured device was not compared, in words; a reason this build does not know is
/// printed as the reply names it.
fn reason_text(reason: &str) -> &str {
    for known in Reason::ALL {
        if known.as_str() == reason {
            return known.words();
        }
    }
    reason
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
    use std::io::Read;
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;
    use std::{fs, thread};

    use clap::Parser;
    use serde_json::json;

    use super::*;
    use crate::cli::{Cli, Command};

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
            compare: false,
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

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// A reachable TCP port that speaks no protocol: it takes each connection, reads one byte and
    /// closes it.
    fn silent_port() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let address = listener.local_addr().expect("listener address").to_string();
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else {
                    continue;
                };
                let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                let mut byte = [0_u8; 1];
                let _ = stream.read(&mut byte);
            }
        });
        address
    }

    /// A port on this computer that nothing listens on.
    fn closed_port() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        listener.local_addr().expect("listener address").to_string()
    }

    /// A new project folder: a valid `runtime.toml` and the given `io.toml`.
    fn project(io_toml: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!(
            "trust-discover-compare-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&folder).expect("create the project folder");
        fs::write(
            folder.join("runtime.toml"),
            trust_runtime::bundle_template::render_runtime_toml(
                &smol_str::SmolStr::new("cell"),
                100,
            ),
        )
        .expect("write runtime.toml");
        fs::write(folder.join("io.toml"), io_toml).expect("write io.toml");
        folder
    }

    /// The arguments of `discover --compare --project <folder>` that search Modbus TCP only.
    fn compare_args(folder: &Path) -> DiscoverArgs {
        DiscoverArgs {
            only: vec!["modbus_tcp".to_owned()],
            deadline_ms: Some(5_000),
            compare: true,
            ..args(Some(folder.to_path_buf()), false, &[])
        }
    }

    /// Runs the search of `args` to its end, as `--compare` does, and answers the printed sections, the
    /// reply they are read from and the `compare` line of `--json`.
    fn finished_comparison(args: &DiscoverArgs) -> (Vec<String>, Value, Value) {
        let plan = Plan::of(args).expect("the project's devices and the request");
        let mut search = RunningSearch::start(args, plan.params.clone()).expect("start the search");
        let give_up = Instant::now() + Duration::from_secs(30);
        loop {
            let status = search.status(None).expect("the search's status");
            if status["state"] != json!("running") {
                break;
            }
            assert!(Instant::now() < give_up, "the search did not end in time");
            thread::sleep(Duration::from_millis(50));
        }
        let (search_id, comparison) = compare_finished(&search, &plan).expect("the comparison");
        let report = comparison.report(&search_id);
        let project = args.project.as_deref().expect("--compare has a project");
        let event = compare_event(project, &search_id, &comparison);
        (comparison_text(&report), report, event)
    }

    #[test]
    fn cli_discover_compare_project() {
        // Specification 11, "Command line" (`--compare`) and "Compare with the project": the search is
        // directed at the project's devices, and the command prints the four sections. First run: the
        // project's Modbus device is at a closed port (missing), the silent port given with --host is a
        // device the project does not use, and the EtherCAT module cannot be searched from this computer
        // (not compared). Second run: the project names the silent port's device and expects a product
        // code that it does not report (differs, found none). A wrong implementation searches only
        // the default networks and calls every configured device missing, prints the found devices instead
        // of the comparison, or calls the EtherCAT module missing.
        let silent = silent_port();
        let closed = closed_port();
        let folder = project(&format!(
            "[[io.drivers]]\nname = \"modbus-tcp\"\nparams = {{ address = \"{closed}\" }}\n\n\
             [[io.drivers]]\nname = \"ethercat\"\nparams = {{ adapter = \"eth9\", modules = [{{ model = \"EL2008\", slot = 2 }}] }}\n"
        ));
        let mut first = compare_args(&folder);
        first.hosts = vec![silent.clone()];

        let (lines, report, _) = finished_comparison(&first);

        assert_eq!(
            lines,
            vec![
                "Missing: 1".to_string(),
                format!("  Modbus TCP: address {closed}, unit_id 1"),
                "Not in the project: 1".to_string(),
                "  dev:ip:127.0.0.1: Modbus TCP".to_string(),
                "Differs from the project: 0".to_string(),
                "Not compared: 1".to_string(),
                "  EtherCAT: adapter eth9, model EL2008, position 2 (its source did not run in this search)"
                    .to_string(),
            ]
        );
        assert_eq!(
            report["missing"],
            json!([{ "protocol": "modbus_tcp", "configured": { "address": closed, "unit_id": 1 } }])
        );

        let folder = project(&format!(
            "[[io.drivers]]\nname = \"modbus-tcp\"\nparams = {{ address = \"{silent}\", product_code = \"DRV-9\" }}\n"
        ));
        let (lines, report, event) = finished_comparison(&compare_args(&folder));

        assert_eq!(
            lines,
            vec![
                "Missing: 0".to_string(),
                "Not in the project: 0".to_string(),
                "Differs from the project: 1".to_string(),
                "  dev:ip:127.0.0.1: Modbus TCP: product_code project DRV-9, found none"
                    .to_string(),
                "Not compared: 0".to_string(),
            ]
        );
        assert_eq!(report["differs"][0]["fields"], json!(["product_code"]));
        assert_eq!(report["differs"][0]["found"]["product_code"], Value::Null);

        // `--json` prints the same reply on one `compare` line, with the event's name and the folder.
        assert_eq!(event["event"], json!("compare"));
        assert_eq!(event["project"], json!(folder.display().to_string()));
        assert_eq!(event["search_id"], json!("s-1"));
        assert_eq!(event["differs"], report["differs"]);
        assert_eq!(event["missing"], json!([]));
    }

    #[test]
    fn discover_compare_directs_the_search_at_the_projects_devices() {
        // Specification 11, "Command line": the addresses of the Modbus TCP, MQTT, OPC UA and ADS devices join
        // the known addresses after the user's own, and the Modbus unit IDs from 1 to 247 become
        // `modbus.unit_ids`; an EtherCAT module has no address, and unit 255 cannot be probed. A wrong
        // implementation drops the user's host, repeats a host, or sends unit 255, which the request refuses.
        let devices = |protocol: &str, params: Value| {
            ConfiguredDevice::from_connection(protocol, &params).expect("devices")
        };
        let mut configured = devices(
            "modbus_tcp",
            json!({ "address": "10.0.0.9:502", "unit_id": 5 }),
        );
        configured.extend(devices(
            "modbus_tcp",
            json!({ "address": "10.0.0.10:502", "unit_id": 255 }),
        ));
        configured.extend(devices("mqtt", json!({ "broker": "broker.plant:1883" })));
        configured.extend(devices(
            "opcua_client",
            json!({ "endpoint_url": "opc.tcp://plc01/UA", "security_policy": "none", "security_mode": "none" }),
        ));
        configured.extend(devices(
            "ads",
            json!({ "target_net_id": "5.1.2.3.1.1", "host": "10.0.0.7" }),
        ));
        configured.extend(devices("ethercat", json!({ "adapter": "eth1" })));

        let mut params = json!({ "only": ["modbus_tcp"], "targets": { "hosts": ["192.168.1.5", "10.0.0.9:502"] } });
        direct_at_project(&mut params, &configured);
        assert_eq!(
            params,
            json!({
                "only": ["modbus_tcp"],
                "targets": { "hosts": [
                    "192.168.1.5", "10.0.0.9:502", "10.0.0.10:502", "broker.plant:1883", "plc01:4840", "10.0.0.7"
                ] },
                "modbus": { "unit_ids": [5] }
            })
        );

        let mut bare = json!({});
        direct_at_project(&mut bare, &configured[..1]);
        assert_eq!(
            bare,
            json!({ "targets": { "hosts": ["10.0.0.9:502"] }, "modbus": { "unit_ids": [5] } })
        );
        let mut untouched = json!({ "only": ["usb"] });
        direct_at_project(&mut untouched, &[]);
        assert_eq!(untouched, json!({ "only": ["usb"] }));
    }

    #[test]
    fn discover_compare_needs_a_project_and_no_endpoint() {
        // Specification 11, "Command line": `--compare` needs `--project <dir>` and runs on this computer, so
        // it excludes `--endpoint`. A wrong implementation compares with nothing, or searches inside a runtime
        // and compares with a project of another computer.
        let parse = |arguments: &[&str]| {
            Cli::try_parse_from(
                ["trust-runtime", "discover"]
                    .into_iter()
                    .chain(arguments.iter().copied()),
            )
        };
        assert!(parse(&["--compare"]).is_err());
        assert!(parse(&[
            "--compare",
            "--project",
            "cell",
            "--endpoint",
            "tcp://10.0.0.5:5680"
        ])
        .is_err());
        let accepted =
            parse(&["--compare", "--project", "cell"]).expect("a project to compare with");
        let Some(Command::Discover(args)) = accepted.command else {
            panic!("expected the discover command");
        };
        assert!(args.compare);
        assert_eq!(args.project, Some(PathBuf::from("cell")));
    }
}
