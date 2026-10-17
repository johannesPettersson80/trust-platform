//! The toolbar's `Topology`/`List` switch (specification 25 §5.11): its two words come from
//! `texts`, so the webview decides no label.

use super::fixtures::{gathered, host, no_project, request, NOW_MS};
use crate::request::RunMode;
use crate::view::project as project_view;

#[test]
fn toolbar_names_the_view_switch() {
    let view = project_view(
        &gathered(no_project(), Vec::new()),
        &request(host(RunMode::Simulate, "")),
        None,
        NOW_MS,
    );
    assert_eq!(
        (
            view.toolbar.view_switch.topology.as_str(),
            view.toolbar.view_switch.list.as_str(),
        ),
        ("Topology", "List"),
        "the switch names both views from texts (specification 25 §5.11)"
    );
}
