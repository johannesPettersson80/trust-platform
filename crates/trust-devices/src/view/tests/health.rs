//! The health rules: the product words of every state, the dashed look of what is not proven,
//! the rollup order, and a computer's reachability (specification 25 §5.2, §5.3 and §5.5).

use serde_json::json;
use trust_runtime::connectors::{ConnectorHealth, ConnectorState, DiscoveryConfidence};

use super::fixtures::{
    answered, gathered, host, managed, no_project, probed, request, runtime_named, status, NOW_MS,
};
use crate::gather::ManagedState;
use crate::request::RunMode;
use crate::targets::TargetOrigin;
use crate::texts::graph as texts;
use crate::topology::tests::fixtures::fleet_topology;
use crate::view::health::{
    confidence_word, connector_health_word, connector_state_word, look_of, runtime_rollup, tone_of,
    Health,
};
use crate::view::project;
use crate::view::types::{Look, Tone};

/// A word that still is a backend token: lower case at its start or with an underscore.
fn raw(word: &str) -> bool {
    word.contains('_') || word.chars().next().is_some_and(char::is_lowercase)
}

#[test]
fn status_words_match_the_spec_labels() {
    for (token, word) in [
        ("connected", "Connected"),
        ("stopped", "Stopped"),
        ("starting", "Starting"),
        ("stopping", "Stopping"),
        ("unavailable", "Status unavailable"),
        ("configured_policy", "Configured"),
        ("disabled", "Disabled"),
        ("not_configured", "Not configured"),
        ("runtime_unreachable", "Runtime unreachable"),
        ("auth_failed", "Authentication failed"),
        ("degraded", "Degraded"),
        ("error", "Error"),
        ("pending", "Pending"),
        ("simulate", "Simulator"),
        ("unknown", "Unknown"),
        ("", "Unknown"),
        ("not_in_build", "Not In Build"),
        (" CONNECTED ", "Connected"),
    ] {
        let shown = texts::health_word(token);
        assert_eq!(shown, word, "health word of {token:?}");
        assert!(!raw(&shown), "{token:?} shows the raw token {shown:?}");
    }

    let states = [
        (ConnectorState::Ready, "Ready"),
        (ConnectorState::Configured, "Configured"),
        (ConnectorState::Starting, "Starting"),
        (ConnectorState::Reconnecting, "Reconnecting"),
        (ConnectorState::Degraded, "Needs attention"),
        (ConnectorState::Stale, "Needs attention"),
        (ConnectorState::NotReady, "Needs attention"),
        (ConnectorState::Faulted, "Fault"),
        (ConnectorState::Disabled, "Disabled"),
    ];
    for (state, word) in states {
        assert_eq!(connector_state_word(state), word, "{state:?}");
    }
    let healths = [
        (ConnectorHealth::Ok, "OK"),
        (ConnectorHealth::Degraded, "Degraded"),
        (ConnectorHealth::Faulted, "Fault"),
        (ConnectorHealth::Unknown, "Unknown"),
    ];
    for (health, word) in healths {
        assert_eq!(connector_health_word(health), word, "{health:?}");
    }
    let confidences = [
        (DiscoveryConfidence::Confirmed, "Confirmed"),
        (DiscoveryConfidence::Likely, "Likely"),
        (DiscoveryConfidence::PortReachable, "Port reachable only"),
        (DiscoveryConfidence::Unavailable, "Not verified"),
    ];
    for (confidence, word) in confidences {
        assert_eq!(confidence_word(confidence), word, "{confidence:?}");
    }

    // No word of a closed vocabulary is its wire token, such as `port_reachable` or `not_ready`.
    let tokens = states
        .iter()
        .map(|(state, word)| (json!(state), *word))
        .chain(healths.iter().map(|(health, word)| (json!(health), *word)))
        .chain(
            confidences
                .iter()
                .map(|(confidence, word)| (json!(confidence), *word)),
        );
    for (token, word) in tokens {
        assert!(!raw(word), "{token} shows a raw token");
        assert_ne!(json!(word), token, "{token} is shown as its token");
    }
}

#[test]
fn pending_states_are_dashed_never_green() {
    for word in [
        "pending",
        "not_configured",
        "not_in_build",
        "configured_policy",
        "unknown",
        "disabled",
    ] {
        assert_eq!(look_of(word), Look::Pending, "{word} is dashed");
        assert_ne!(tone_of(word), Tone::Ok, "{word} is never green");
    }
    assert_eq!(look_of("connected"), Look::Normal);
    assert_eq!(tone_of("connected"), Tone::Ok);
    assert_eq!(tone_of("degraded"), Tone::Warning);
    assert_eq!(tone_of("runtime_unreachable"), Tone::Error);

    // Runtimes whose state nothing proves yet are dashed and never green, and only a running one
    // pulses.
    let view = project(
        &gathered(
            no_project(),
            vec![
                managed(
                    "cell1",
                    "tcp://127.0.0.1:9902",
                    ManagedState::Starting,
                    None,
                ),
                managed(
                    "cell2",
                    "tcp://127.0.0.1:9903",
                    ManagedState::Stopping,
                    None,
                ),
                probed(
                    TargetOrigin::Configured,
                    "10.0.0.9:5510",
                    Some("cell9"),
                    trust_runtime_client::TargetProbe::Unreachable,
                ),
            ],
        ),
        &request(host(RunMode::Simulate, "")),
        None,
        NOW_MS,
    );
    for (name, word) in [
        ("cell1", "Starting"),
        ("cell2", "Stopping"),
        ("cell9", "Unknown"),
    ] {
        let runtime = runtime_named(&view, name);
        assert_eq!(runtime.state.word, word, "{name}");
        assert_eq!(runtime.look, Look::Pending, "{name} is dashed");
        assert_ne!(runtime.state.tone, Tone::Ok, "{name} is never green");
        assert!(!runtime.state.live, "{name} does not pulse");
    }
}

#[test]
fn runtime_rollup_order() {
    // A connection problem always shows on its runtime: an error before a degraded connection.
    assert_eq!(
        runtime_rollup("connected", ["degraded", "error"]),
        Health::Error
    );
    assert_eq!(
        runtime_rollup("connected", ["connected", "degraded"]),
        Health::Degraded
    );
    assert_eq!(
        runtime_rollup("simulate", ["runtime_unreachable"]),
        Health::Degraded
    );
    assert_eq!(runtime_rollup("stopped", ["error"]), Health::Error);
    assert_eq!(runtime_rollup("error", ["connected"]), Health::Error);
    // Without one, the order of the panel before the move: connected, simulate, then a
    // configured runtime that is not running, then pending.
    assert_eq!(
        runtime_rollup("configured_policy", ["simulate", "connected"]),
        Health::Connected
    );
    assert_eq!(
        runtime_rollup("configured_policy", ["pending", "simulate"]),
        Health::Simulate
    );
    assert_eq!(
        runtime_rollup("configured_policy", ["pending", "configured_policy"]),
        Health::Stopped
    );
    assert_eq!(runtime_rollup("", ["pending", "disabled"]), Health::Pending);
    assert_eq!(runtime_rollup("", []), Health::Pending);
    // The runtime's own lifecycle state is not replaced by the quiet states of its connections.
    assert_eq!(
        runtime_rollup("connected", ["configured_policy"]),
        Health::Connected
    );
    assert_eq!(runtime_rollup("simulate", ["connected"]), Health::Simulate);
    assert_eq!(
        runtime_rollup("stopped", ["pending", "configured_policy"]),
        Health::Stopped
    );
    assert_eq!(
        runtime_rollup("pending", ["configured_policy"]),
        Health::Pending
    );
    assert_eq!(runtime_rollup("unknown", []), Health::Unknown);
    assert_eq!(runtime_rollup(" Connected ", []), Health::Connected);
}

#[test]
fn host_reachability_ignores_endpoint_degradation() {
    // The ported `fleetTopology()`: the host is reached, and its runtime has a degraded MQTT
    // connection and a degraded mesh connection.
    let view = project(
        &gathered(
            no_project(),
            vec![answered(
                TargetOrigin::Project,
                "tcp://192.168.77.10:5680",
                None,
                status("running", "admin"),
                fleet_topology(),
            )],
        ),
        &request(host(RunMode::Online, "tcp://192.168.77.10:5680")),
        None,
        NOW_MS,
    );
    let computer = &view.computers[0];
    assert_eq!(
        computer.reachability.word, "Reachable",
        "a host is reached even with a degraded connection inside"
    );
    assert_eq!(computer.reachability.tone, Tone::Ok);
    let runtime = runtime_named(&view, "Line runtime");
    assert_eq!(
        runtime.state.word, "Degraded",
        "the runtime carries the degradation"
    );
    assert_eq!(runtime.state.tone, Tone::Warning);

    // The search dims for presentation only: the rollup is made before it.
    let mut searching = request(host(RunMode::Online, "tcp://192.168.77.10:5680"));
    searching.view.search = "modbus".to_owned();
    let searched = project(
        &gathered(
            no_project(),
            vec![answered(
                TargetOrigin::Project,
                "tcp://192.168.77.10:5680",
                None,
                status("running", "admin"),
                fleet_topology(),
            )],
        ),
        &searching,
        None,
        NOW_MS,
    );
    assert_eq!(searched.computers[0].reachability.word, "Reachable");
    assert_eq!(
        runtime_named(&searched, "Line runtime").state.word,
        "Degraded"
    );
}
