//! The ten scenarios of the golden views (design §4.3, B4): each a gather and the extension's
//! facts, as runtimes answer them, projected at `NOW_MS`.

use serde_json::{json, Value};
use trust_runtime_client::{AuthFailure, TargetProbe};

use super::fixtures::{
    answered, configured_project, endpoint, gathered, host, managed, no_project, probed, project,
    report, request, runtime, status, stopped_browsable_project, unanswered, with_simulator,
    CONFIGURED_CONTROL, NOW_MS, SIMULATOR,
};
use crate::gather::{ManagedState, TargetReport};
use crate::request::{HostFacts, IoValue, RunMode};
use crate::targets::TargetOrigin;
use crate::topology::tests::fixtures::fleet_topology;
use crate::view::project as project_view;
use crate::view::types::DevicesView;

/// Every scenario by its file name.
pub(super) fn all() -> Vec<(&'static str, DevicesView)> {
    vec![
        ("empty_project", empty_project()),
        ("simulator_stopped", simulator_stopped()),
        ("simulator_running_with_connections", simulator_running()),
        (
            "three_runtimes_on_two_computers_with_a_chip",
            runtimes_with_a_chip(),
        ),
        ("auth_failed_peer", auth_failed_peer()),
        ("managed_runtimes_in_every_state", managed_runtimes()),
        ("mesh", mesh()),
        ("ads_server", ads_server()),
        ("ethercat_with_subdevices", ethercat()),
        ("unreachable_peer", unreachable_peer()),
        ("stopped_browses", stopped_browses()),
        (
            "connections_and_connector_status",
            super::graph_scenarios::connections_and_connector_status(),
        ),
        (
            "search_and_filter",
            super::graph_scenarios::search_and_filter(),
        ),
        (
            "faults_and_warnings",
            super::graph_scenarios::faults_and_warnings(),
        ),
        (
            "servers_and_ads_client",
            super::graph_scenarios::servers_and_ads_client(),
        ),
    ]
}

fn view(
    project: crate::gather::ProjectInputs,
    targets: Vec<TargetReport>,
    host: HostFacts,
) -> DevicesView {
    project_view(&gathered(project, targets), &request(host), None, NOW_MS)
}

/// A workspace without a truST project: one stopped simulator on this computer.
fn empty_project() -> DevicesView {
    view(no_project(), Vec::new(), host(RunMode::Simulate, ""))
}

/// The project's runtime, configured and not running, in the simulator that is stopped.
fn simulator_stopped() -> DevicesView {
    view(
        project(configured_project()),
        Vec::new(),
        host(RunMode::Simulate, ""),
    )
}

/// The simulator runs the project and answers with its connections: simulated I/O with live
/// values and a degraded Modbus meter.
fn simulator_running() -> DevicesView {
    let mut simulator = runtime(
        "RESOURCE",
        "RESOURCE",
        "simulate",
        vec![
            endpoint("RESOURCE", "simulated", "simulated", "connected"),
            endpoint("RESOURCE", "modbus_tcp", "Modbus meter", "degraded"),
        ],
    );
    simulator["mode"] = json!("simulate");
    simulator["control_endpoint"] = json!(SIMULATOR);
    let mut host = with_simulator(host(RunMode::Simulate, ""), false);
    host.io_state = vec![IoValue {
        name: "start_button".to_owned(),
        address: "%IX0.0".to_owned(),
        value: "TRUE".to_owned(),
    }];
    view(
        project(configured_project()),
        vec![answered(
            TargetOrigin::Simulator,
            SIMULATOR,
            None,
            status("running", "admin"),
            report("host:line-pc", "line-pc", &["127.0.0.1"], vec![simulator]),
        )],
        host,
    )
}

/// A computer with two runtimes, the project's at its control endpoint and another one, and a
/// chip with the truST firmware of the STM32U5A9.
fn runtimes_with_a_chip() -> DevicesView {
    let mut configuration = configured_project();
    let host_facts = &mut configuration["hosts"][0];
    host_facts["ips"] = json!(["192.168.10.20"]);
    host_facts["board"] = json!("Raspberry Pi 5");
    host_facts["arch"] = json!("aarch64");
    let configured = &mut host_facts["runtimes"][0];
    configured["runtime_id"] = json!("line");
    configured["name"] = json!("Line controller");
    configured["control_endpoint"] = json!("tcp://192.168.10.20:5680");

    let line = runtime(
        "line",
        "Line controller",
        "connected",
        vec![endpoint("line", "simulated", "simulated", "connected")],
    );
    let packaging = runtime("packaging", "Packaging", "connected", Vec::new());
    let mut pi = report("host:line-pc", "line-pc", &["192.168.10.20"], vec![line]);
    pi["hosts"][0]["board"] = json!("Raspberry Pi 5");
    let mut second = report(
        "host:line-pc",
        "line-pc",
        &["192.168.10.20"],
        vec![packaging],
    );
    second["hosts"][0]["board"] = json!("Raspberry Pi 5");
    let conveyor = runtime(
        "conveyor",
        "Conveyor node",
        "connected",
        vec![endpoint("conveyor", "modbus_tcp", "Drive A", "connected")],
    );
    let mut chip = report(
        "host:conveyor-node",
        "conveyor-node",
        &["192.168.10.31"],
        vec![conveyor],
    );
    chip["hosts"][0]["arch"] = json!("arm");
    chip["hosts"][0]["os"] = json!("none");
    let mut chip_status = status("running", "admin");
    chip_status["platform"] = json!("stm32u5a9");
    chip_status["program_control"] = json!("keeps_control");
    chip_status["version"] = json!("0.24.77");
    chip_status["metrics"]["cycle_ms"] = json!({ "last": 1.02, "max": 1.31 });
    view(
        project(configuration),
        vec![
            answered(
                TargetOrigin::Project,
                "tcp://192.168.10.20:5680",
                None,
                status("running", "admin"),
                pi,
            ),
            answered(
                TargetOrigin::Configured,
                "tcp://192.168.10.20:5681",
                Some("Packaging"),
                status("running", "engineer"),
                second,
            ),
            answered(
                TargetOrigin::Configured,
                "tcp://192.168.10.31:5680",
                Some("Conveyor node"),
                chip_status,
                chip,
            ),
        ],
        host(RunMode::Online, "tcp://192.168.10.20:5680"),
    )
}

/// A runtime on another computer refused the stored token.
fn auth_failed_peer() -> DevicesView {
    view(
        no_project(),
        vec![probed(
            TargetOrigin::Configured,
            "tcp://192.168.10.44:5680",
            Some("Sensor hub"),
            TargetProbe::AuthFailed(AuthFailure::Rejected),
        )],
        host(RunMode::Simulate, ""),
    )
}

/// The project's `fleet.toml` runs five runtimes on this computer, one in each state.
fn managed_runtimes() -> DevicesView {
    let mut cell1 = runtime("cell1", "cell1", "connected", Vec::new());
    cell1["control_endpoint"] = json!("tcp://127.0.0.1:9902");
    view(
        project(configured_project()),
        vec![
            managed(
                "cell1",
                "tcp://127.0.0.1:9902",
                ManagedState::Running,
                Some((
                    status("running", "admin"),
                    report("host:line-pc", "line-pc", &["127.0.0.1"], vec![cell1]),
                )),
            ),
            managed("cell2", "tcp://127.0.0.1:9903", ManagedState::Stopped, None),
            managed(
                "cell3",
                "tcp://127.0.0.1:9904",
                ManagedState::Starting,
                None,
            ),
            managed(
                "cell4",
                "tcp://127.0.0.1:9905",
                ManagedState::Stopping,
                None,
            ),
            managed(
                "cell5",
                "tcp://127.0.0.1:9906",
                ManagedState::Unavailable,
                None,
            ),
        ],
        host(RunMode::Simulate, ""),
    )
}

/// The ported `fleetTopology()`: a runtime with a mesh peer and a shared MQTT broker.
fn mesh() -> DevicesView {
    view(
        no_project(),
        vec![answered(
            TargetOrigin::Configured,
            "tcp://192.168.77.10:5680",
            None,
            status("running", "admin"),
            fleet_topology(),
        )],
        host(RunMode::Simulate, ""),
    )
}

/// A runtime whose ADS server has two clients.
fn ads_server() -> DevicesView {
    let mut server = endpoint("plc", "ads_server", "ADS server", "connected");
    server["kind"] = json!("service");
    server["role"] = json!("server");
    server["live"] =
        json!({ "clients_count": 2, "clients_summary": "2 clients", "verified": "self_test" });
    server["params"] =
        json!({ "listen": "0.0.0.0:48898", "ams_net_id": "192.168.10.20.1.1", "ads_port": 851 });
    peer_with(runtime("plc", "ADS gateway", "connected", vec![server]))
}

/// A runtime with an `EtherCAT` bus of two devices.
fn ethercat() -> DevicesView {
    let mut bus = endpoint("io", "ethercat", "EtherCAT bus", "connected");
    bus["role"] = json!("master");
    bus["children"] = json!([
        { "id": "slave:1", "kind": "field_slave", "name": "EK1100", "slot": 1, "model": "EK1100",
          "health": "connected", "detail": "Operational." },
        { "id": "slave:2", "kind": "field_slave", "name": "Servo drive", "slot": 2,
          "model": "A6-EC400", "channels": 8, "health": "degraded", "detail": "Following error." },
    ]);
    peer_with(runtime("io", "Servo cell", "connected", vec![bus]))
}

/// A configured runtime where nothing answers.
fn unreachable_peer() -> DevicesView {
    view(
        no_project(),
        vec![probed(
            TargetOrigin::Configured,
            "10.0.0.9:5510",
            Some("cell1"),
            TargetProbe::Unreachable,
        )],
        host(RunMode::Simulate, ""),
    )
}

/// One runtime at `tcp://192.168.10.20:5680` on another computer.
fn peer_with(runtime: Value) -> DevicesView {
    view(
        no_project(),
        vec![answered(
            TargetOrigin::Configured,
            "tcp://192.168.10.20:5680",
            None,
            status("running", "admin"),
            report(
                "host:plc-gateway",
                "plc-gateway",
                &["192.168.10.20"],
                vec![runtime],
            ),
        )],
        host(RunMode::Simulate, ""),
    )
}

/// The project's runtime is stopped and has browsable protocols: the remote browses wait for it,
/// and the project's own globals open at once.
fn stopped_browses() -> DevicesView {
    view(
        stopped_browsable_project(),
        vec![unanswered(TargetOrigin::Configured, CONFIGURED_CONTROL)],
        host(RunMode::Simulate, ""),
    )
}
