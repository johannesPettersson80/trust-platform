//! The gather against scripted runtimes on local ports (design §1.5 and §3.3): parallel targets
//! within the deadline, the late answer kept for the next view, the target without an answer,
//! the project's endpoint in simulate mode and when it is disabled, and the simulator's adoption.

use std::io::ErrorKind;
use std::net::TcpListener;
use std::thread;
use std::time::{Duration, Instant};

use trust_runtime_client::{AuthFailure, TargetProbe};

use super::{gather, Budget, Cache, Gathered, ManagedState, Reach, TargetReport};
use crate::fake_runtime::{FakeRuntime, Reply, Script};
use crate::request::{
    DebugSessionFacts, DebugSessionKind, GatherMode, HostFacts, HostSettings, RunMode,
    SimulatorFacts, Token, ViewOptions, ViewRequest,
};
use crate::targets::TargetOrigin;
use crate::test_project::{fleet_toml, managed_runtime, runtime_toml, TempFolder};

/// Scheduling slack on a busy test machine; far below the waits the tests tell apart.
const SLACK: Duration = Duration::from_millis(500);

pub(super) fn view(host: HostFacts, gather: GatherMode) -> ViewRequest {
    ViewRequest {
        session: "dc-test".to_string(),
        seq: 1,
        gather,
        project: None,
        workspace_folders: Vec::new(),
        view: ViewOptions::default(),
        host,
    }
}

pub(super) fn host(mode: RunMode, endpoint: &str, additional: &[&str]) -> HostFacts {
    HostFacts {
        settings: HostSettings {
            mode,
            control_endpoint: endpoint.to_string(),
            additional_endpoints: additional.iter().copied().map(String::from).collect(),
            ..HostSettings::default()
        },
        ..HostFacts::default()
    }
}

fn with_simulator(mut host: HostFacts, endpoint: &str) -> HostFacts {
    host.debug_session = Some(DebugSessionFacts {
        kind: DebugSessionKind::Launch,
        endpoint: Some(endpoint.to_string()),
        label: None,
        starting: false,
        failure: None,
    });
    host.simulator = Some(SimulatorFacts {
        endpoint: endpoint.to_string(),
        token: Some(Token::new("simulator-token")),
    });
    host
}

pub(super) fn report(gathered: &Gathered, origin: TargetOrigin) -> &TargetReport {
    gathered
        .targets
        .iter()
        .find(|report| report.target.origin == origin)
        .expect("a report of the target")
}

/// An endpoint on a local port that nothing listens on.
pub(super) fn closed_endpoint() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    format!("tcp://{}", listener.local_addr().expect("local address"))
}

/// Whether nothing connected to `listener`.
fn never_contacted(listener: &TcpListener) -> bool {
    listener
        .set_nonblocking(true)
        .expect("a listener that does not block");
    matches!(listener.accept(), Err(error) if error.kind() == ErrorKind::WouldBlock)
}

#[test]
fn gather_runs_targets_in_parallel_within_the_deadline() {
    // Each answer takes 250 ms, so the five requests of the project's runtime and of the
    // simulator take 1.25 s each; the silent runtime's probe gives up after 750 ms. One after the
    // other they would need 3.25 s, past the deadline of 2.5 s.
    let slow = Script::running().delayed(Duration::from_millis(250));
    let project = FakeRuntime::start(slow.clone());
    let simulator = FakeRuntime::start(slow);
    let silent = FakeRuntime::start(Script::default().answer("status", Reply::Silent));
    let host = with_simulator(
        host(
            RunMode::Online,
            &project.endpoint,
            &[silent.endpoint.as_str()],
        ),
        &simulator.endpoint,
    );
    let budget = Budget::default();

    let started = Instant::now();
    let gathered = gather(
        &view(host, GatherMode::Auto),
        None,
        &mut Cache::default(),
        &budget,
    );
    let waited = started.elapsed();

    assert!(waited < budget.deadline + SLACK, "waited {waited:?}");
    for origin in [TargetOrigin::Project, TargetOrigin::Simulator] {
        let answered = report(&gathered, origin);
        assert!(
            matches!(answered.reach, Reach::Probe(TargetProbe::Reachable(_))),
            "{origin:?}: {:?}",
            answered.reach
        );
        assert!(matches!(answered.live.topology, Some(Ok(_))));
        assert!(matches!(answered.live.connectors, Some(Ok(_))));
        assert!(matches!(answered.live.capabilities, Some(Ok(_))));
        assert!(matches!(answered.live.schema, Some(Ok(_))), "{origin:?}");
        assert!(
            answered.live.events.is_none(),
            "events only for the selected runtime"
        );
    }
    let quiet = report(&gathered, TargetOrigin::Configured);
    assert_eq!(quiet.probe(), Some(&TargetProbe::Unreachable));
    assert!(quiet.live.topology.is_none());
    assert_eq!(project.count("status"), 1);
    assert_eq!(silent.count("fleet.topology"), 0);
}

#[test]
fn late_answer_is_cached_for_the_next_view() {
    // The status arrives after 200 ms and the last of the four requests after 1 s, long after
    // the deadline of 300 ms.
    let late = FakeRuntime::start(Script::running().delayed(Duration::from_millis(200)));
    let host = host(RunMode::Online, &late.endpoint, &[]);
    let budget = Budget {
        deadline: Duration::from_millis(300),
        period: Duration::from_secs(60),
        ..Budget::default()
    };
    let mut cache = Cache::default();

    let first = gather(
        &view(host.clone(), GatherMode::Auto),
        None,
        &mut cache,
        &budget,
    );
    let unanswered = report(&first, TargetOrigin::Project);
    assert!(
        matches!(unanswered.reach, Reach::Unanswered),
        "{:?}",
        unanswered.reach
    );

    let until = Instant::now() + Duration::from_secs(10);
    while cache.asking() {
        assert!(Instant::now() < until, "the late answer arrives");
        thread::sleep(Duration::from_millis(20));
    }

    let started = Instant::now();
    let second = gather(&view(host, GatherMode::Auto), None, &mut cache, &budget);
    assert!(
        started.elapsed() < budget.deadline,
        "the second view waits for nothing"
    );
    let answered = report(&second, TargetOrigin::Project);
    assert!(matches!(
        answered.reach,
        Reach::Probe(TargetProbe::Reachable(_))
    ));
    assert!(matches!(answered.live.schema, Some(Ok(_))));
    assert_eq!(
        late.count("status"),
        1,
        "the late answer is used, not asked again"
    );
}

#[test]
fn unanswered_target_is_status_unavailable_never_stopped() {
    let slow = Script::running().delayed(Duration::from_millis(600));
    let peer = FakeRuntime::start(slow.clone());
    let managed = FakeRuntime::start(slow);
    let plant = TempFolder::new("unanswered");
    plant.write(
        "runtime.toml",
        &runtime_toml("line", "unix:///tmp/trust-devices-unanswered.sock", None),
    );
    plant.write(
        "fleet.toml",
        &fleet_toml(&[("cell", managed.endpoint.as_str(), 10_001)]),
    );
    managed_runtime(&plant.path, "cell", &managed.endpoint);
    let mut request = view(
        host(RunMode::Simulate, "", &[peer.endpoint.as_str()]),
        GatherMode::Auto,
    );
    request.workspace_folders = vec![plant.path.clone()];
    let budget = Budget {
        deadline: Duration::from_millis(300),
        ..Budget::default()
    };

    let gathered = gather(&request, None, &mut Cache::default(), &budget);

    let configured = report(&gathered, TargetOrigin::Configured);
    assert!(matches!(configured.reach, Reach::Unanswered));
    assert!(
        configured.probe().is_none(),
        "neither reachable nor unreachable"
    );
    assert_eq!(configured.managed_state(), None);
    let cell = report(&gathered, TargetOrigin::Managed);
    assert!(matches!(cell.reach, Reach::Unanswered));
    assert_eq!(cell.managed_state(), Some(ManagedState::Unavailable));
}

#[test]
fn simulate_mode_does_not_probe_the_primary() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let endpoint = format!("tcp://{}", listener.local_addr().expect("local address"));
    let host = host(RunMode::Simulate, &endpoint, &[endpoint.as_str()]);

    let gathered = gather(
        &view(host, GatherMode::Now),
        None,
        &mut Cache::default(),
        &Budget::default(),
    );

    assert!(gathered.targets.is_empty(), "{:?}", gathered.targets);
    assert!(gathered.live_primary().is_none());
    assert!(
        never_contacted(&listener),
        "simulate mode asks no configured runtime"
    );
}

#[test]
fn disabled_control_endpoint_counts_as_missing() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a local port");
    let endpoint = format!("tcp://{}", listener.local_addr().expect("local address"));
    let mut disabled = host(RunMode::Online, &endpoint, &[endpoint.as_str()]);
    disabled.settings.use_control_endpoint = false;

    let gathered = gather(
        &view(disabled, GatherMode::Now),
        None,
        &mut Cache::default(),
        &Budget::default(),
    );

    assert_eq!(
        gathered.targets.len(),
        1,
        "the disabled endpoint is no peer either"
    );
    let project = report(&gathered, TargetOrigin::Project);
    assert_eq!(project.probe(), Some(&TargetProbe::MissingEndpoint));
    assert!(
        never_contacted(&listener),
        "a disabled endpoint is never contacted"
    );

    let blank = gather(
        &view(host(RunMode::Online, "  ", &[]), GatherMode::Now),
        None,
        &mut Cache::default(),
        &Budget::default(),
    );
    let project = report(&blank, TargetOrigin::Project);
    assert_eq!(project.probe(), Some(&TargetProbe::MissingEndpoint));
}

#[test]
fn simulator_adopted_only_when_its_probe_answers() {
    let answering = FakeRuntime::start(Script::running());
    let refusing =
        FakeRuntime::start(Script::running().answer("status", Reply::Refuse("invalid_auth_token")));
    let project = FakeRuntime::start(Script::running());
    let unreachable = closed_endpoint();
    let gathered = |host: HostFacts| {
        gather(
            &view(host, GatherMode::Now),
            None,
            &mut Cache::default(),
            &Budget::default(),
        )
    };
    let primary = |host: HostFacts| {
        gathered(host)
            .live_primary()
            .map(|report| report.target.origin)
    };

    // The project's runtime does not answer, and the simulator answers with its token.
    let adopted = gathered(with_simulator(
        host(RunMode::Online, &unreachable, &[]),
        &answering.endpoint,
    ));
    assert_eq!(
        adopted.live_primary().map(|report| report.target.origin),
        Some(TargetOrigin::Simulator)
    );
    assert!(answering
        .requests()
        .iter()
        .all(|request| request["auth"] == "simulator-token"));
    // A simulator that refuses its token, or does not answer, is not adopted.
    let refused = gathered(with_simulator(
        host(RunMode::Online, &unreachable, &[]),
        &refusing.endpoint,
    ));
    assert_eq!(
        report(&refused, TargetOrigin::Simulator).probe(),
        Some(&TargetProbe::AuthFailed(AuthFailure::Rejected))
    );
    assert!(refused.live_primary().is_none());
    assert_eq!(
        primary(with_simulator(
            host(RunMode::Simulate, "", &[]),
            &unreachable
        )),
        None
    );
    // In simulate mode the simulator serves the live data.
    assert_eq!(
        primary(with_simulator(
            host(RunMode::Simulate, "", &[]),
            &answering.endpoint
        )),
        Some(TargetOrigin::Simulator)
    );
    // The project's runtime comes first when it answers.
    assert_eq!(
        primary(with_simulator(
            host(RunMode::Online, &project.endpoint, &[]),
            &answering.endpoint
        )),
        Some(TargetOrigin::Project)
    );
    // Without a debug session the simulator's endpoint is no target.
    let mut idle = with_simulator(host(RunMode::Simulate, "", &[]), &answering.endpoint);
    idle.debug_session = None;
    assert_eq!(primary(idle), None);
}
