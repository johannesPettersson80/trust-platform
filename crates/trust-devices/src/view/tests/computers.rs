//! The computers: their headings, the configured label of a target that did not answer, several
//! runtimes in one frame, and the networks (specification 25 §5.5, §5.11 and §5.12).

use serde_json::{json, Value};
use trust_runtime_client::{AuthFailure, TargetProbe};

use super::fixtures::{
    answered, computer_headed, gathered, host, no_project, probed, report, request, runtime,
    status, NOW_MS,
};
use crate::request::RunMode;
use crate::targets::TargetOrigin;
use crate::topology::tests::fixtures::fleet_topology;
use crate::view::project;
use crate::view::types::{Computer, DevicesView, ItemId, RuntimeIcon};

/// The view of one runtime that answers at `endpoint` with `topology`, with this computer's
/// host ID `this_computer`.
fn view_of(endpoint: &str, topology: Value, this_computer: Option<&str>) -> DevicesView {
    project(
        &gathered(
            no_project(),
            vec![answered(
                TargetOrigin::Configured,
                endpoint,
                None,
                status("running", "admin"),
                topology,
            )],
        ),
        &request(host(RunMode::Simulate, "")),
        this_computer,
        NOW_MS,
    )
}

fn host_name(computer: &Computer) -> Option<&str> {
    computer
        .inspector
        .sections
        .iter()
        .flat_map(|section| &section.rows)
        .find(|row| row.label == "Host name")
        .map(|row| row.value.as_str())
}

#[test]
fn this_computer_and_computer_ip_headings() {
    // The computer whose runtimes report this computer's host ID is `This computer`, and its
    // host name stays as a fact.
    let mut local = fleet_topology();
    local["hosts"][0]["ips"] = json!(["127.0.0.1"]);
    local["hosts"][0]["runtimes"][0]["control_endpoint"] = json!("tcp://127.0.0.1:39855");
    let view = view_of("tcp://127.0.0.1:39855", local, Some("host:trust-pi"));
    let computer = computer_headed(&view, "This computer");
    assert_eq!(
        host_name(computer),
        Some("trust-pi"),
        "the raw host name stays a fact"
    );

    // Another computer is headed by its address, and its host name stays a fact.
    let mut remote = fleet_topology();
    remote["hosts"][0]["host_id"] = json!("host:remote-plc-host");
    remote["hosts"][0]["hostname"] = json!("remote-plc-host");
    remote["hosts"][0]["ips"] = json!(["192.168.77.10"]);
    remote["hosts"][0]["runtimes"][0]["control_endpoint"] = json!("tcp://192.168.77.10:5680");
    let view = view_of("tcp://192.168.77.10:5680", remote, Some("host:trust-pi"));
    let computer = computer_headed(&view, "Computer 192.168.77.10");
    assert_eq!(host_name(computer), Some("remote-plc-host"));

    // Without an address of its own that is not a loopback address, by its host name.
    let mut named = fleet_topology();
    named["hosts"][0]["host_id"] = json!("host:cell-pc");
    named["hosts"][0]["hostname"] = json!("cell-pc");
    named["hosts"][0]["ips"] = json!(["127.0.0.1"]);
    let view = view_of("tcp://127.0.0.1:5681", named, Some("host:trust-pi"));
    computer_headed(&view, "Computer cell-pc");

    // Without the host ID, the host of the project's configuration stands for this computer.
    let view = project(
        &gathered(
            super::fixtures::project(super::fixtures::configured_project()),
            vec![answered(
                TargetOrigin::Configured,
                "tcp://127.0.0.1:5690",
                None,
                status("running", "admin"),
                report(
                    "host:line-pc",
                    "line-pc",
                    &["127.0.0.1"],
                    vec![runtime("cell2", "cell2", "connected", Vec::new())],
                ),
            )],
        ),
        &request(host(RunMode::Simulate, "")),
        None,
        NOW_MS,
    );
    assert_eq!(
        view.computers.len(),
        1,
        "one host, the project's and the runtime's"
    );
    assert_eq!(view.computers[0].heading, "This computer");
}

#[test]
fn unreachable_peer_keeps_its_configured_label() {
    let view = project(
        &gathered(
            no_project(),
            vec![
                probed(
                    TargetOrigin::Configured,
                    "tcp://127.0.0.1:5510",
                    Some("discoveredcell"),
                    TargetProbe::AuthFailed(AuthFailure::Rejected),
                ),
                probed(
                    TargetOrigin::Configured,
                    "10.0.0.9:5510",
                    Some("cell1"),
                    TargetProbe::Unreachable,
                ),
            ],
        ),
        &request(host(RunMode::Simulate, "")),
        None,
        NOW_MS,
    );
    // A loopback endpoint does not make a peer that refused the token `This computer`.
    let refused = computer_headed(&view, "discoveredcell");
    let unreachable = computer_headed(&view, "cell1");
    for computer in [refused, unreachable] {
        assert_eq!(computer.reachability.word, "Unreachable");
        assert_eq!(computer.platform, None, "nothing reports its platform");
    }
    let this_computer: Vec<&str> = view
        .computers
        .iter()
        .map(|computer| computer.heading.as_str())
        .filter(|heading| *heading == "This computer")
        .collect();
    assert_eq!(
        this_computer.len(),
        1,
        "only the simulator's computer is this computer"
    );
}

#[test]
fn several_runtimes_share_one_computer_frame() {
    // Two trust-runtime instances on one computer, both with the resource name `RESOURCE`
    // (specification 34 ARCH-15), and a third runtime of their own.
    let instance = |port: u16, name: &str| {
        let endpoint = format!("tcp://192.168.10.20:{port}");
        let mut runtime = runtime("RESOURCE", name, "connected", Vec::new());
        runtime["control_endpoint"] = json!(endpoint);
        answered(
            TargetOrigin::Configured,
            &endpoint,
            None,
            status("running", "admin"),
            report("host:line-pc", "line-pc", &["192.168.10.20"], vec![runtime]),
        )
    };
    let view = project(
        &gathered(
            no_project(),
            vec![
                instance(5680, "Line controller"),
                instance(5681, "Packaging"),
                instance(5682, "Palletizer"),
            ],
        ),
        &request(host(RunMode::Online, "")),
        None,
        NOW_MS,
    );
    let frame = computer_headed(&view, "Computer 192.168.10.20");
    let names: Vec<&str> = frame
        .runtimes
        .iter()
        .map(|runtime| runtime.name.as_str())
        .collect();
    assert_eq!(names, ["Line controller", "Packaging", "Palletizer"]);
    let ids: Vec<&ItemId> = frame.runtimes.iter().map(|runtime| &runtime.id).collect();
    assert_eq!(
        ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
        3,
        "each runtime keeps an identity of its own"
    );
    let addresses: Vec<Option<&str>> = frame
        .runtimes
        .iter()
        .map(|runtime| runtime.address.as_deref())
        .collect();
    assert_eq!(
        addresses,
        [
            Some("192.168.10.20:5680"),
            Some("192.168.10.20:5681"),
            Some("192.168.10.20:5682")
        ],
        "the port tells them apart"
    );
    assert!(frame
        .runtimes
        .iter()
        .all(|runtime| runtime.icon == RuntimeIcon::Computer));
}

#[test]
fn networks_group_computers_by_their_subnet() {
    let peer = |endpoint: &str, host_id: &str, ip: &str| {
        answered(
            TargetOrigin::Configured,
            endpoint,
            None,
            status("running", "admin"),
            report(
                host_id,
                host_id,
                &[ip],
                vec![runtime(host_id, host_id, "connected", Vec::new())],
            ),
        )
    };
    let view = project(
        &gathered(
            no_project(),
            vec![
                peer("tcp://192.168.10.20:5680", "host:a", "192.168.10.20"),
                peer("tcp://10.0.0.7:5680", "host:b", "10.0.0.7"),
                peer("tcp://192.168.10.31:5680", "host:c", "192.168.10.31"),
            ],
        ),
        &request(host(RunMode::Simulate, "")),
        None,
        NOW_MS,
    );
    let bands: Vec<(&str, usize)> = view
        .networks
        .iter()
        .map(|network| (network.label.as_str(), network.computers.len()))
        .collect();
    assert_eq!(
        bands,
        [
            ("Network · 192.168.10.0/24", 2),
            ("Network · 10.0.0.0/24", 1)
        ],
        "computers without a reported prefix group by /24 (decision O29)"
    );
    let simulator = computer_headed(&view, "This computer");
    assert!(
        view.networks
            .iter()
            .all(|network| !network.computers.contains(&simulator.id)),
        "a computer without an address belongs to no band"
    );
}
