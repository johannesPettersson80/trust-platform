//! Managed runtimes in the gather: their status comes from the chosen project's fleet root as
//! `trust-runtime fleet runtime status` reads it, and a status that cannot be read is
//! `Status unavailable`, never `Stopped` (specification 25 §5.10 and §6.5).

use super::tests::{closed_endpoint, view};
use super::{gather, Budget, Cache, ManagedState, Reach};
use crate::fake_runtime::{FakeRuntime, Reply, Script};
use crate::request::{GatherMode, HostFacts};
use crate::test_project::{fleet_toml, managed_runtime, runtime_toml, TempFolder};

#[test]
fn managed_status_unavailable_on_malformed_status() {
    let good = FakeRuntime::start(Script::running());
    let garbled = FakeRuntime::start(Script::running().answer("status", Reply::Raw("not-json")));
    let refused =
        FakeRuntime::start(Script::running().answer("status", Reply::Refuse("invalid_auth_token")));
    let down = closed_endpoint();
    let runtimes = [
        ("good", good.endpoint.as_str(), 10_001),
        ("garbled", garbled.endpoint.as_str(), 10_002),
        ("refused", refused.endpoint.as_str(), 10_003),
        ("down", down.as_str(), 10_004),
    ];
    let plant = TempFolder::new("managed-status");
    plant.write(
        "runtime.toml",
        &runtime_toml("line", "unix:///tmp/trust-devices-managed.sock", None),
    );
    plant.write("fleet.toml", &fleet_toml(&runtimes));
    for (name, endpoint, _) in runtimes {
        managed_runtime(&plant.path, name, endpoint);
    }
    let mut request = view(HostFacts::default(), GatherMode::Now);
    request.workspace_folders = vec![plant.path.clone()];

    let gathered = gather(&request, None, &mut Cache::default(), &Budget::default());

    let managed = |name: &str| {
        gathered
            .targets
            .iter()
            .find(|report| {
                report
                    .target
                    .managed
                    .as_ref()
                    .is_some_and(|managed| managed.runtime.name == name)
            })
            .expect("a report of the managed runtime")
    };
    assert_eq!(managed("good").managed_state(), Some(ManagedState::Running));
    assert_eq!(
        managed("garbled").managed_state(),
        Some(ManagedState::Unavailable),
        "a malformed status is no evidence of a stopped runtime"
    );
    assert_eq!(
        managed("refused").managed_state(),
        Some(ManagedState::Unavailable)
    );
    assert_eq!(
        managed("down").managed_state(),
        Some(ManagedState::Stopped),
        "only an endpoint that accepts no connection is stopped"
    );
    let Reach::Managed(garbled_status) = &managed("garbled").reach else {
        panic!("a managed status");
    };
    assert!(garbled_status.report.is_none());
    assert!(garbled_status
        .error
        .as_deref()
        .is_some_and(|error| error.contains("not JSON")));

    // The running runtime's live data is read with the token of its runtime.toml, and its fleet
    // root is the chosen project.
    let running = managed("good");
    assert!(matches!(running.live.topology, Some(Ok(_))));
    assert!(
        running.live.schema.is_none(),
        "the catalog comes from the primary runtimes"
    );
    assert!(good
        .requests()
        .iter()
        .all(|request| request["auth"] == "managed-token"));
    let owner = running.target.managed.as_ref().expect("a managed runtime");
    assert_eq!(owner.fleet_root, plant.path);
    assert_eq!(
        gathered.project.chosen.as_deref(),
        Some(plant.path.as_path())
    );
    assert!(managed("down").live.topology.is_none());
}
