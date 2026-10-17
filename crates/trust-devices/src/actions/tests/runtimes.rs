//! The runtime actions of `act` (design §4.3 B7), one test per requirement. Every expected effect
//! is written out, taken from the ported TypeScript (`localRuntime.ts`, `fleetActions.ts`,
//! `useDiscoverPane.ts`) and the specification 25 lines named in each test. The managed lifecycle
//! runs behind a fake [`Fleet`] seam, so no process is started.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{json, Map, Value};

use crate::actions::{act_with, Fleet};
use crate::request::{ActOp, ActRequest, HostFacts};
use crate::session::Sessions;
use crate::test_project::{runtime_toml, TempFolder};
use crate::view::types::{
    FieldError, HostEffect, ItemId, OperationDone, Refresh, Secret, SettingTarget, Tone,
};

/// The `trust-runtime` executable that the requests name.
const TRUST_RUNTIME: &str = "/opt/trust-runtime/trust-runtime";

/// A [`Fleet`] that answers with canned JSON instead of running a process.
struct FakeFleet {
    start: Result<Value, String>,
    stop: Result<Value, String>,
}

impl Default for FakeFleet {
    fn default() -> Self {
        Self {
            start: Ok(Value::Null),
            stop: Ok(Value::Null),
        }
    }
}

impl FakeFleet {
    /// A fake whose Start answers `answer`.
    fn starting(answer: Value) -> Self {
        Self {
            start: Ok(answer),
            ..Self::default()
        }
    }
}

impl Fleet for FakeFleet {
    fn start(&self, _executable: &Path, _fleet_root: &Path, _name: &str) -> Result<Value, String> {
        self.start.clone()
    }

    fn stop(&self, _executable: &Path, _fleet_root: &Path, _name: &str) -> Result<Value, String> {
        self.stop.clone()
    }

    fn add(
        &self,
        _executable: &Path,
        _fleet_root: &Path,
        _name: &str,
        _template: &str,
    ) -> Result<Value, String> {
        Err("the fake does not scaffold projects".to_owned())
    }
}

/// A request that performs `action` with `payload`, no form values and no host facts.
fn request(action: &str, payload: Value) -> ActRequest {
    ActRequest {
        session: "dc-test".to_owned(),
        op: ActOp {
            action: action.to_owned(),
            payload,
        },
        values: Map::new(),
        project: None,
        host: HostFacts::default(),
    }
}

#[test]
fn connect_also_selects_the_run_target() {
    // Specification 25:304 "Successful Connect also selects the runtime"; design §3.3 I9.
    let mut panel = Sessions::default();
    let session = panel.session("dc-test", Instant::now());
    let reply = act_with(
        session,
        &request(
            "runtime.connect",
            json!({
                "runtime": "trust-fleet-v1:cell",
                "endpoint": "tcp://192.168.10.31:5680",
                "label": "cell1 (this computer)",
            }),
        ),
        &FakeFleet::default(),
    );

    assert!(reply.ok);
    assert_eq!(
        reply.effects,
        vec![
            HostEffect::Attach {
                endpoint: "tcp://192.168.10.31:5680".to_owned(),
                label: "cell1 (this computer)".to_owned(),
            },
            HostEffect::SetRunTarget {
                id: "tcp://192.168.10.31:5680".to_owned(),
            },
        ]
    );
}

#[test]
fn managed_start_imports_the_token_before_attach() {
    // Design §4.3 B7: "On success it returns the effects StoreSecret (the token from
    // read_control_auth_token), then Attach, then SetRunTarget"; specification 25:858-860.
    let folder = TempFolder::new("managed-start");
    let project = folder.join("cell1");
    fs::create_dir_all(&project).expect("create the runtime's folder");
    fs::write(
        project.join("runtime.toml"),
        runtime_toml("cell1", "tcp://127.0.0.1:9901", Some("token-1")),
    )
    .expect("write runtime.toml");
    let fleet = FakeFleet::starting(json!({
        "status": "running",
        "control_endpoint": "tcp://127.0.0.1:9901",
        "path": project.display().to_string(),
        "message": "Runtime started.",
    }));
    let mut panel = Sessions::default();
    let session = panel.session("dc-test", Instant::now());
    let mut act_request = request(
        "runtime.managed_start",
        json!({ "runtime": "trust-fleet-v1:cell1", "name": "cell1" }),
    );
    act_request.project = Some(folder.path.clone());
    act_request.host.runtime_executable = Some(PathBuf::from(TRUST_RUNTIME));

    let reply = act_with(session, &act_request, &fleet);
    assert!(reply.pending.is_some());
    let done = session.take_operations_done();

    assert_eq!(done.len(), 1);
    assert_eq!(
        done[0].effects,
        vec![
            HostEffect::StoreSecret {
                endpoint: "tcp://127.0.0.1:9901".to_owned(),
                token: Secret("token-1".to_owned()),
            },
            HostEffect::Attach {
                endpoint: "tcp://127.0.0.1:9901".to_owned(),
                label: "cell1 (this computer)".to_owned(),
            },
            HostEffect::SetRunTarget {
                id: "cell1".to_owned(),
            },
        ]
    );
}

#[test]
fn lifecycle_success_requires_the_terminal_state() {
    // Specification 25:292-294 "starting, stopping, missing, and unknown states are not success".
    let fleet = FakeFleet::starting(json!({
        "status": "starting",
        "control_endpoint": "tcp://127.0.0.1:9901",
        "path": "cell1",
        "message": "Runtime process started, but the control endpoint is not reachable yet.",
    }));
    let mut panel = Sessions::default();
    let session = panel.session("dc-test", Instant::now());
    let mut act_request = request(
        "runtime.managed_start",
        json!({ "runtime": "trust-fleet-v1:cell1", "name": "cell1" }),
    );
    act_request.project = Some(PathBuf::from("/fleet"));
    act_request.host.runtime_executable = Some(PathBuf::from(TRUST_RUNTIME));

    let reply = act_with(session, &act_request, &fleet);
    assert!(reply.pending.is_some());
    let done = session.take_operations_done();

    assert_eq!(done.len(), 1);
    assert_ne!(done[0].notice.tone, Tone::Ok);
    assert_eq!(done[0].notice.detail.as_deref(), Some("starting"));
    assert_eq!(done[0].effects, Vec::<HostEffect>::new());
}

#[test]
fn a_slow_action_reports_its_result_exactly_once() {
    // Design §1.4.1: a slow action answers `pending`; its result arrives once, in
    // `operations_done` of a later request (design §1.5).
    let fleet = FakeFleet::starting(json!({
        "status": "running",
        "control_endpoint": "tcp://127.0.0.1:9901",
        "path": "cell1",
        "message": "Runtime started.",
    }));
    let mut panel = Sessions::default();
    let session = panel.session("dc-test", Instant::now());
    let mut act_request = request(
        "runtime.managed_start",
        json!({ "runtime": "trust-fleet-v1:cell1", "name": "cell1" }),
    );
    act_request.project = Some(PathBuf::from("/fleet"));
    act_request.host.runtime_executable = Some(PathBuf::from(TRUST_RUNTIME));

    let reply = act_with(session, &act_request, &fleet);
    let pending = reply
        .pending
        .clone()
        .expect("a slow action answers with a pending operation");

    assert_eq!(reply.effects, Vec::<HostEffect>::new());
    assert_eq!(reply.refresh, Refresh::Soon);
    assert_eq!(pending.text, "Starting…");
    assert_eq!(pending.pane, "inspector:trust-fleet-v1:cell1");

    let done = session.take_operations_done();
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].operation, pending.operation);
    assert_eq!(session.take_operations_done(), Vec::<OperationDone>::new());
}

#[test]
fn add_host_normalizes_host_port_to_tcp() {
    // Specification 25:332 "Host:port normalizes to a control endpoint"; design §3.2 J21.
    let cases: [(&str, Option<(&str, &str)>); 5] = [
        (
            "10.0.0.9:502",
            Some((
                "tcp://10.0.0.9:502",
                "trust-fleet-v1:WyJydW50aW1lIiwiZmxlZXQ6dGNwOi8vMTAuMC4wLjk6NTAyIixudWxsLCJmbGVldDp0Y3A6Ly8xMC4wLjAuOTo1MDI6cnVudGltZSJd",
            )),
        ),
        (
            "[::1]:5680",
            Some((
                "tcp://[::1]:5680",
                "trust-fleet-v1:WyJydW50aW1lIiwiZmxlZXQ6dGNwOi8vWzo6MV06NTY4MCIsbnVsbCwiZmxlZXQ6dGNwOi8vWzo6MV06NTY4MDpydW50aW1lIl0",
            )),
        ),
        (
            "tcp://10.0.0.9:502",
            Some((
                "tcp://10.0.0.9:502",
                "trust-fleet-v1:WyJydW50aW1lIiwiZmxlZXQ6dGNwOi8vMTAuMC4wLjk6NTAyIixudWxsLCJmbGVldDp0Y3A6Ly8xMC4wLjAuOTo1MDI6cnVudGltZSJd",
            )),
        ),
        (
            "unix:///tmp/trust.sock",
            Some((
                "unix:///tmp/trust.sock",
                "trust-fleet-v1:WyJydW50aW1lIiwiZmxlZXQ6dW5peDovLy90bXAvdHJ1c3Quc29jayIsbnVsbCwiZmxlZXQ6dW5peDovLy90bXAvdHJ1c3Quc29jazpydW50aW1lIl0",
            )),
        ),
        ("/not/an/address", None),
    ];
    for (typed, expected) in cases {
        let mut panel = Sessions::default();
        let session = panel.session("dc-test", Instant::now());
        let mut act_request = request("runtime.add_host", json!({ "form": "runtime.connect" }));
        act_request
            .values
            .insert("endpoint".to_owned(), json!(typed));

        let reply = act_with(session, &act_request, &FakeFleet::default());

        if let Some((endpoint, runtime)) = expected {
            assert!(reply.ok, "{typed} is a runtime address");
            assert_eq!(
                reply.effects,
                vec![
                    HostEffect::Select {
                        item: ItemId(runtime.to_owned()),
                    },
                    HostEffect::WriteSetting {
                        key: "trust.runtime.fleetEndpoints".to_owned(),
                        value: json!([endpoint]),
                        target: SettingTarget::Workspace,
                    },
                ],
                "{typed}"
            );
        } else {
            assert!(!reply.ok, "{typed} is not a runtime address");
            assert_eq!(
                reply
                    .notice
                    .as_ref()
                    .map(|notice| notice.field_errors.clone()),
                Some(vec![FieldError {
                    field: "endpoint".to_owned(),
                    message: "Host name or IP plus port. Advanced: tcp://host:port or unix:///path/to/socket."
                        .to_owned(),
                }]),
                "{typed}"
            );
        }
    }
}

#[test]
fn add_host_stores_the_label_and_selects_the_runtime() {
    // Ported from `fleetActions.ts:17-52` (`addHost`): a label is stored against the normalised
    // endpoint, and with no token the new runtime is focused. Design §3.3 I73, specification
    // 25:330-334.
    let mut panel = Sessions::default();
    let session = panel.session("dc-test", Instant::now());
    let mut act_request = request(
        "runtime.add_host",
        json!({ "form": "runtime.connect", "label": "Line 2" }),
    );
    act_request
        .values
        .insert("endpoint".to_owned(), json!("10.0.0.5:9901"));

    let reply = act_with(session, &act_request, &FakeFleet::default());

    assert!(reply.ok);
    assert_eq!(
        reply.effects,
        vec![
            HostEffect::StoreLabel {
                endpoint: "tcp://10.0.0.5:9901".to_owned(),
                label: "Line 2".to_owned(),
            },
            HostEffect::Select {
                item: ItemId(
                    "trust-fleet-v1:WyJydW50aW1lIiwiZmxlZXQ6dGNwOi8vMTAuMC4wLjU6OTkwMSIsbnVsbCwiZmxlZXQ6dGNwOi8vMTAuMC4wLjU6OTkwMTpydW50aW1lIl0"
                        .to_owned()
                ),
            },
            HostEffect::WriteSetting {
                key: "trust.runtime.fleetEndpoints".to_owned(),
                value: json!(["tcp://10.0.0.5:9901"]),
                target: SettingTarget::Workspace,
            },
        ]
    );
}

#[test]
fn add_host_of_a_known_endpoint_only_says_so() {
    // Ported from `fleetActions.ts:39-45`: an endpoint already in `trust.runtime.fleetEndpoints`
    // is not written again; the panel only says that it is there. Design §3.2 J21.
    let mut panel = Sessions::default();
    let session = panel.session("dc-test", Instant::now());
    let mut act_request = request("runtime.add_host", json!({ "form": "runtime.connect" }));
    act_request
        .values
        .insert("endpoint".to_owned(), json!("10.0.0.5:9901"));
    act_request.host.settings.additional_endpoints = vec!["tcp://10.0.0.5:9901".to_owned()];

    let reply = act_with(session, &act_request, &FakeFleet::default());

    assert!(reply.ok);
    assert_eq!(reply.refresh, Refresh::Now);
    assert_eq!(
        reply.notice.as_ref().map(|notice| notice.text.as_str()),
        Some("tcp://10.0.0.5:9901 is already in the fleet.")
    );
    assert!(
        !reply
            .effects
            .iter()
            .any(|effect| matches!(effect, HostEffect::WriteSetting { .. })),
        "an endpoint already in the fleet is not written again"
    );
}

#[test]
fn add_local_refuses_a_name_the_fleet_refuses() {
    // Design §3.2 J22: the name rule is the fleet's own `validate_runtime_name`; a space is not a
    // folder name.
    let mut panel = Sessions::default();
    let session = panel.session("dc-test", Instant::now());
    let mut act_request = request("runtime.add_local", json!({ "form": "runtime.local" }));
    act_request
        .values
        .insert("name".to_owned(), json!("cell one"));
    act_request.project = Some(PathBuf::from("/fleet"));
    act_request.host.runtime_executable = Some(PathBuf::from(TRUST_RUNTIME));

    let reply = act_with(session, &act_request, &FakeFleet::default());

    assert!(!reply.ok);
    assert_eq!(
        reply
            .notice
            .as_ref()
            .map(|notice| notice.field_errors.clone()),
        Some(vec![FieldError {
            field: "name".to_owned(),
            message: "runtime name may only contain ASCII letters, digits, '-' and '_'".to_owned(),
        }])
    );
}

#[test]
fn adopt_keeps_the_found_label_and_selects_it() {
    // Specification 25:89-90 "Adopting a discovered runtime MUST retain its discovered label,
    // refresh the fleet topology, and select and focus the adopted runtime node"; design §3.2 J36.
    let mut panel = Sessions::default();
    let session = panel.session("dc-test", Instant::now());
    let reply = act_with(
        session,
        &request(
            "runtime.adopt",
            json!({
                "runtime": "trust-fleet-v1:conveyor",
                "endpoint": "tcp://192.168.10.9:5680",
                "label": "conveyor@conv-node",
            }),
        ),
        &FakeFleet::default(),
    );

    assert!(reply.ok);
    assert_eq!(reply.refresh, Refresh::Now);
    assert_eq!(
        reply.effects,
        vec![
            HostEffect::StoreLabel {
                endpoint: "tcp://192.168.10.9:5680".to_owned(),
                label: "conveyor@conv-node".to_owned(),
            },
            HostEffect::Select {
                item: ItemId("trust-fleet-v1:conveyor".to_owned()),
            },
            HostEffect::WriteSetting {
                key: "trust.runtime.fleetEndpoints".to_owned(),
                value: json!(["tcp://192.168.10.9:5680"]),
                target: SettingTarget::Workspace,
            },
        ]
    );
}

#[test]
fn results_are_addressed_to_the_pane_that_started_them() {
    // Specification 25:890-891 "Results of an action started in the panel appear in the pane that
    // started it".
    let mut panel = Sessions::default();
    let session = panel.session("dc-test", Instant::now());

    let mut form = request("runtime.add_host", json!({ "form": "runtime.connect" }));
    form.values
        .insert("endpoint".to_owned(), json!("/not/an/address"));
    let from_form = act_with(session, &form, &FakeFleet::default());
    assert_eq!(
        from_form.notice.as_ref().map(|notice| notice.pane.as_str()),
        Some("form:runtime.connect")
    );

    let from_runtime = act_with(
        session,
        &request(
            "runtime.connect",
            json!({ "runtime": "trust-fleet-v1:cell", "endpoint": null, "label": "cell" }),
        ),
        &FakeFleet::default(),
    );
    assert_eq!(
        from_runtime
            .notice
            .as_ref()
            .map(|notice| notice.pane.as_str()),
        Some("inspector:trust-fleet-v1:cell")
    );
}
