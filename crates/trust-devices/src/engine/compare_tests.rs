//! Native tests of the engine's record of `Compare with project` (specification 25 §2): the request is a
//! change of a status request, it is the panel's own choice, it holds while the finished search is polled,
//! and a new search clears it. Each expected value is a literal from specification 25 §2.

use serde_json::json;

use super::tests::{answering_card, online, search_of};
use super::{DevicesEngine, DiscoverChange, SearchReply, SearchVerb};
use crate::fake_runtime::{FakeRuntime, Script};
use crate::test_project::{runtime_toml, TempFolder};
use crate::view::types::Action;

/// A panel that shows one runtime, so that the project of the comparison exists.
fn panel_with_a_runtime(folder: &TempFolder, runtime: &FakeRuntime) -> DevicesEngine {
    folder.write(
        "cell/runtime.toml",
        &runtime_toml("cell", &runtime.endpoint, Some("token-1")),
    );
    let project = folder.join("cell");
    let engine = DevicesEngine::new();
    let mut seq = 0_u64;
    answering_card(
        &engine,
        &folder.path,
        &project,
        &online(&runtime.endpoint),
        &mut seq,
    );
    engine
}

/// Starts a search on this computer that ends at once (GPIO is searched inside a runtime only), and answers
/// its ID.
fn finished_search(engine: &DevicesEngine) -> String {
    let mut start = search_of("dc-test", SearchVerb::Start);
    start.params = json!({ "only": ["gpio"] });
    engine
        .search(&start)
        .search_id
        .expect("the search on this computer has an ID")
}

/// The status of the search `search_id`, with `change` recorded first.
fn status_of(
    engine: &DevicesEngine,
    search_id: &str,
    change: Option<DiscoverChange>,
) -> SearchReply {
    let mut call = search_of("dc-test", SearchVerb::Status);
    call.search_id = Some(search_id.to_owned());
    call.change = change;
    engine.search(&call)
}

/// The `Compare with project` action of the Discover view of `reply`.
fn compare_action(reply: &SearchReply) -> &Action {
    reply
        .discover
        .as_deref()
        .expect("a status carries the Discover view")
        .actions
        .iter()
        .find(|action| action.id == "compare")
        .expect("the pane offers Compare with project")
}

#[test]
fn compare_with_project_is_the_panels_own_choice_and_a_new_search_clears_it() {
    // Specification 25 §2: a `compare` change turns the lists on, a poll of the same search keeps them (the
    // pane polls while it is open), the opposite change turns them off, and a new search starts without them,
    // because the lists belong to the search they were asked for. A wrong implementation forgets the choice at
    // the next poll, remembers it into the next search, or records it in the project.
    let runtime = FakeRuntime::start(Script::running());
    let folder = TempFolder::new("compare-choice");
    let engine = panel_with_a_runtime(&folder, &runtime);
    let first = finished_search(&engine);

    let before = status_of(&engine, &first, None);
    assert_eq!(compare_action(&before).label, "Compare with project");
    assert!(
        compare_action(&before).enabled,
        "a finished search can be compared"
    );
    assert!(before
        .discover
        .as_deref()
        .is_some_and(|view| view.compare.is_none()));

    let asked = status_of(
        &engine,
        &first,
        Some(DiscoverChange::Compare { shown: true }),
    );
    assert_eq!(compare_action(&asked).label, "Hide comparison");
    assert!(asked
        .discover
        .as_deref()
        .is_some_and(|view| view.compare.is_some()));

    let polled = status_of(&engine, &first, None);
    assert_eq!(
        compare_action(&polled).label,
        "Hide comparison",
        "the next poll of the search keeps the lists"
    );

    let hidden = status_of(
        &engine,
        &first,
        Some(DiscoverChange::Compare { shown: false }),
    );
    assert_eq!(compare_action(&hidden).label, "Compare with project");
    assert!(hidden
        .discover
        .as_deref()
        .is_some_and(|view| view.compare.is_none()));

    let _ = status_of(
        &engine,
        &first,
        Some(DiscoverChange::Compare { shown: true }),
    );
    let second = finished_search(&engine);
    let next = status_of(&engine, &second, None);
    assert_eq!(
        compare_action(&next).label,
        "Compare with project",
        "a new search starts without the lists"
    );
    assert!(next
        .discover
        .as_deref()
        .is_some_and(|view| view.compare.is_none()));
}

#[test]
fn compare_with_project_needs_a_runtime_on_the_panel() {
    // Specification 25 §2: the project of the comparison is the connections on the canvas, so a panel that
    // shows no runtime has none, and the action says so even when the lists were asked for. A wrong
    // implementation compares with an empty project and calls every find unused.
    let engine = DevicesEngine::new();
    let search_id = finished_search(&engine);

    let reply = status_of(
        &engine,
        &search_id,
        Some(DiscoverChange::Compare { shown: true }),
    );

    let compare = compare_action(&reply);
    assert!(!compare.enabled);
    assert_eq!(
        compare.reason.as_deref(),
        Some("The panel shows no runtime, so there is no project to compare with.")
    );
    assert!(reply
        .discover
        .as_deref()
        .is_some_and(|view| view.compare.is_none()));
}
