//! The target list of design §3.3 I3, ported from `NC/networkCanvasPanel.ts` `resolveFleetTargets`
//! with the simulator and the managed runtimes that the Rust gather asks too.

use std::path::Path;

use trust_runtime::config::fleet_manifest::FleetManifestRuntime;

use super::{panel_targets, TargetOrigin};
use crate::request::{DebugSessionFacts, DebugSessionKind, HostFacts, HostSettings, RunMode};
use crate::request::{SimulatorFacts, Token};

fn host(mode: RunMode, additional: &[&str]) -> HostFacts {
    HostFacts {
        settings: HostSettings {
            mode,
            control_endpoint: " tcp://10.0.0.5:5680 ".to_string(),
            additional_endpoints: additional.iter().copied().map(String::from).collect(),
            ..HostSettings::default()
        },
        tokens: [
            ("tcp://10.0.0.5:5680".to_string(), Token::new("primary")),
            ("tcp://10.0.0.7:5680".to_string(), Token::new("peer")),
        ]
        .into(),
        endpoint_labels: [("tcp://10.0.0.7:5680".to_string(), "Line 2".to_string())].into(),
        debug_session: Some(DebugSessionFacts {
            kind: DebugSessionKind::Launch,
            endpoint: None,
            label: None,
            starting: false,
            failure: None,
        }),
        simulator: Some(SimulatorFacts {
            endpoint: "unix:///tmp/trust-debug-ab12cd34.sock".to_string(),
            token: Some(Token::new("simulator")),
        }),
        ..HostFacts::default()
    }
}

#[test]
fn panel_targets_trims_dedupes_and_drops_the_primary() {
    let additional = [
        " tcp://10.0.0.7:5680 ",
        "tcp://10.0.0.7:5680",
        "",
        "   ",
        "tcp://10.0.0.5:5680",
        "unix:///tmp/trust-debug-ab12cd34.sock",
        "tcp://10.0.0.8:5680",
    ];
    let managed = [FleetManifestRuntime {
        name: "cell".to_string(),
        path: "cell".to_string(),
        control_endpoint: "tcp://127.0.0.1:9910".to_string(),
        web_port: 18_080,
    }];
    let targets = panel_targets(
        &host(RunMode::Online, &additional),
        Some(Path::new("/plant/line1")),
        &managed,
    );
    let listed = targets
        .iter()
        .map(|target| (target.origin, target.endpoint.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        listed,
        [
            (TargetOrigin::Project, "tcp://10.0.0.5:5680"),
            (
                TargetOrigin::Simulator,
                "unix:///tmp/trust-debug-ab12cd34.sock"
            ),
            (TargetOrigin::Managed, "tcp://127.0.0.1:9910"),
            (TargetOrigin::Configured, "tcp://10.0.0.7:5680"),
            (TargetOrigin::Configured, "tcp://10.0.0.8:5680"),
        ]
    );
    // Each target carries the token and the name stored for its trimmed endpoint.
    let secret = |index: usize| targets[index].token.as_ref().map(Token::secret);
    assert_eq!(secret(0), Some("primary"));
    assert_eq!(secret(1), Some("simulator"));
    assert_eq!(
        secret(2),
        None,
        "a managed token comes from its runtime.toml"
    );
    assert_eq!(secret(3), Some("peer"));
    assert_eq!(secret(4), None);
    assert_eq!(targets[3].label.as_deref(), Some("Line 2"));
    let owner = targets[2].managed.as_ref().expect("the managed runtime");
    assert_eq!(owner.fleet_root, Path::new("/plant/line1"));
    assert_eq!(owner.runtime.name, "cell");
    assert!(targets
        .iter()
        .all(|target| target.enabled && !target.events));

    // In simulate mode the project's endpoint is no target, and still no peer.
    let simulate = panel_targets(&host(RunMode::Simulate, &additional), None, &managed);
    let listed = simulate
        .iter()
        .map(|target| (target.origin, target.endpoint.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        listed,
        [
            (
                TargetOrigin::Simulator,
                "unix:///tmp/trust-debug-ab12cd34.sock"
            ),
            (TargetOrigin::Configured, "tcp://10.0.0.7:5680"),
            (TargetOrigin::Configured, "tcp://10.0.0.8:5680"),
        ],
        "without a fleet root nothing is managed"
    );

    // Without a debug session the simulator's endpoint is no target, and it is a peer again.
    let mut idle = host(RunMode::Simulate, &additional);
    idle.debug_session = None;
    let origins = panel_targets(&idle, None, &[])
        .iter()
        .map(|target| target.origin)
        .collect::<Vec<_>>();
    assert_eq!(origins, [TargetOrigin::Configured; 3]);
}
