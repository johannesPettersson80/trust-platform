//! The tests of `trust-runtime discover`: its request, the devices it leaves out, its printed lines, and
//! searches against reachable ports that speak no protocol.

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

/// A port on this computer that nothing listens on, held without listening (specification 22):
/// the caller keeps the holder as long as it uses the address, so no other socket takes the
/// port and answers.
fn closed_port() -> (trust_test_support::RefusedPort, String) {
    let held = trust_test_support::refused_loopback_port();
    let address = format!("127.0.0.1:{}", held.port());
    (held, address)
}

/// A new project folder: a valid `runtime.toml` and the given `io.toml`.
fn project(io_toml: &str) -> PathBuf {
    let folder = trust_test_support::unique_temp_path(&format!(
        "trust-discover-compare-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&folder).expect("create the project folder");
    fs::write(
        folder.join("runtime.toml"),
        trust_runtime::bundle_template::render_runtime_toml(&smol_str::SmolStr::new("cell"), 100),
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
    let (_held, closed) = closed_port();
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
            "  dev:ip:127.0.0.1: Modbus TCP: product_code project DRV-9, found none".to_string(),
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

    let mut params =
        json!({ "only": ["modbus_tcp"], "targets": { "hosts": ["192.168.1.5", "10.0.0.9:502"] } });
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
    let accepted = parse(&["--compare", "--project", "cell"]).expect("a project to compare with");
    let Some(Command::Discover(args)) = accepted.command else {
        panic!("expected the discover command");
    };
    assert!(args.compare);
    assert_eq!(args.project, Some(PathBuf::from("cell")));
}

/// The device that tutorial 15's search finds: PLC-B's runtime seen through PLC-A's mesh, at the
/// poll time `seen`.
fn runtime_seen_through_the_mesh(seen: u64) -> Value {
    json!({
        "id": "dev:runtime:d60c801fe910e62c6877349b814c4a25",
        "addresses": [],
        "runtime_ids": ["d60c801fe910e62c6877349b814c4a25"],
        "last_seen_ms": seen,
        "finds": [{
            "protocol": "trust_runtime",
            "source": "mesh",
            "confidence": "confirmed",
            "simulated": false,
            "identity": { "kind": "trust_runtime", "name": "PlcB" },
            "last_seen_ms": seen,
            "warnings": [MESH_WARNING],
        }],
    })
}

const MESH_WARNING: &str = "Runtime found through the mesh without a control endpoint; adopt may require manual endpoint entry.";

#[test]
fn a_runtime_found_through_the_mesh_is_named_once_with_its_warning() {
    // Specification 11, "Command line": a device's line names it by its addresses, or without one by
    // the name its find's identity reports followed by its ID, and each find's warnings follow on lines
    // of their own; a device listed again with the same line is not printed again. The mesh lists the
    // runtime again at every poll with only its `last_seen_ms` changed. Catches the device printed at
    // every poll (ten times in a 10 s search until 27 November 2026), a runtime named only by its ID,
    // and a find's warning left out.
    let mut printer = Printer::new(false, None, BTreeSet::new(), false);
    let expected = format!(
        "Found PlcB (dev:runtime:d60c801fe910e62c6877349b814c4a25): truST runtimes (confirmed)\n{}{MESH_WARNING}",
        " ".repeat(16)
    );
    assert_eq!(
        printer.device_line(&runtime_seen_through_the_mesh(1)),
        Some(expected)
    );
    assert_eq!(
        printer.device_line(&runtime_seen_through_the_mesh(2)),
        None,
        "a later poll that changed only last_seen_ms prints nothing"
    );
    let mut changed = runtime_seen_through_the_mesh(3);
    changed["finds"][0]["confidence"] = json!("likely");
    assert!(
        printer
            .device_line(&changed)
            .is_some_and(|line| line.contains("truST runtimes (likely)")),
        "a device whose line changes is printed again"
    );
}

#[test]
fn a_device_with_an_address_is_named_by_it() {
    // Specification 11, "Command line": the addresses name a device that has one, whatever name its
    // find reports. Catches the identity's name taking the place of the address.
    let device = json!({
        "id": "dev:ip:10.0.0.5",
        "addresses": ["10.0.0.5"],
        "finds": [{
            "protocol": "modbus_tcp",
            "confidence": "confirmed",
            "identity": { "kind": "modbus_tcp", "name": "Pump" },
        }],
    });
    assert_eq!(
        device_text(&device),
        "Found 10.0.0.5: Modbus TCP (confirmed)"
    );
}
