//! The controls of a runtime node (specification 25 §5.5). Each expected list is worked out from
//! the rule: who owns the runtime's process, its health, whether a session is attached, and whether
//! its last Connect needs a token.

use super::{runtime_controls, NodeFacts, NodeOwner};

fn node(health: &str) -> NodeFacts<'_> {
    NodeFacts {
        owner: NodeOwner::Session,
        health,
        attached: false,
        control_endpoint: Some("tcp://10.0.0.2:5680"),
        auth_token_required: false,
    }
}

fn actions(node: &NodeFacts<'_>) -> Vec<(&'static str, &'static str, bool)> {
    runtime_controls(node)
        .into_iter()
        .map(|control| (control.action, control.kind, control.enabled))
        .collect()
}

/// A runtime whose process the extension does not own is connected and disconnected, never
/// started or stopped; without an endpoint Connect cannot run; it has no logs. Catches a Stop
/// offered for a remote runtime, a Connect enabled without an endpoint, and Logs for a remote.
#[test]
fn a_remote_runtime_is_connected_and_disconnected_never_stopped() {
    assert_eq!(
        actions(&node("connected")),
        [
            ("runtimeConnect", "primary", true),
            ("setAsRunTarget", "secondary", true),
            ("openRuntimeSettings", "secondary", true),
        ]
    );
    let attached = NodeFacts {
        attached: true,
        ..node("connected")
    };
    assert_eq!(
        actions(&attached)[0],
        ("runtimeDisconnect", "primary", true)
    );
    let nowhere = NodeFacts {
        control_endpoint: None,
        ..node("unknown")
    };
    assert_eq!(actions(&nowhere)[0], ("runtimeConnect", "primary", false));
}

/// A Connect that needs a token makes Set auth token the primary control with Connect kept as the
/// retry; an attached session keeps Disconnect first and offers Set auth token beside it. Catches
/// a recovery hidden behind a Connect that cannot succeed, and the retry dropped.
#[test]
fn a_connect_that_needs_a_token_offers_set_auth_token_first() {
    let refused = NodeFacts {
        auth_token_required: true,
        ..node("error")
    };
    assert_eq!(
        actions(&refused)[..2],
        [
            ("setAuthToken", "primary", true),
            ("runtimeConnect", "secondary", true)
        ]
    );
    let attached = NodeFacts {
        attached: true,
        ..refused
    };
    assert_eq!(
        actions(&attached)[..2],
        [
            ("runtimeDisconnect", "primary", true),
            ("setAuthToken", "secondary", true)
        ]
    );
}

/// The simulator and a managed runtime are started and stopped by their health, show progress
/// while pending and have logs; a managed runtime whose state is not known offers a disabled
/// Start. Catches a Connect for a process the extension owns, a Stop offered while it starts, and
/// an enabled Start for a runtime in an unknown state.
#[test]
fn a_runtime_the_extension_owns_is_started_and_stopped() {
    let simulator = |health| NodeFacts {
        owner: NodeOwner::Simulator,
        control_endpoint: None,
        ..node(health)
    };
    assert_eq!(
        actions(&simulator("stopped")),
        [
            ("startLocalSimulator", "primary", true),
            ("setAsRunTarget", "secondary", true),
            ("openRuntimeLogs", "secondary", true),
            ("openRuntimeSettings", "secondary", true),
        ]
    );
    assert_eq!(
        actions(&simulator("connected"))[0],
        ("stopLocalSimulator", "primary", true)
    );
    assert_eq!(
        actions(&simulator("pending"))[0],
        ("none", "primary", false)
    );
    let managed = |health| NodeFacts {
        owner: NodeOwner::Managed,
        ..node(health)
    };
    assert_eq!(
        actions(&managed("connected"))[0],
        ("managedStop", "primary", true)
    );
    assert_eq!(
        actions(&managed("stopped"))[0],
        ("managedStart", "primary", true)
    );
    assert_eq!(actions(&managed("pending"))[0], ("none", "primary", false));
    assert_eq!(
        actions(&managed("unknown"))[0],
        ("managedStart", "primary", false)
    );
    assert!(actions(&managed("stopped")).contains(&("openRuntimeLogs", "secondary", true)));
}
