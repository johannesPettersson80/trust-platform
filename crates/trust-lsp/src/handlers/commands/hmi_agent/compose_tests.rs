//! Ports of `editors/vscode/src/test/suite/hmi-agent-compose.test.ts` (suite "HMI agent
//! composition"): the same catalogue, inputs and expected values. Specification 43 §6 and AT-04.

use serde_json::{json, Value};
use trust_runtime::hmi::catalogue::Catalogue;
use trust_runtime::hmi::page_files::HmiPage;

use super::compose_agent_page;

/// The catalogue of the TypeScript suite: two equipment types with ports, two displays.
fn catalogue() -> Catalogue {
    serde_json::from_value(json!({
        "format": 1,
        "components": [
            {"type": "tank", "category": "equipment", "phase": 2, "size": [200, 260], "ports": ["inlet", "outlet"]},
            {"type": "pump", "category": "equipment", "phase": 2, "size": [160, 120], "ports": ["inlet", "outlet"]},
            {"type": "value", "category": "display", "phase": 1, "size": [240, 64]},
            {"type": "text", "category": "display", "phase": 1, "size": [240, 40]},
        ],
    }))
    .expect("the test catalogue is valid")
}

/// The composition request of the TypeScript suite's `input()`.
fn input() -> Value {
    json!({
        "title": "Tank process",
        "pattern": "process-overview",
        "size": [1280, 800],
        "items": [
            {
                "id": "tank",
                "type": "tank",
                "tag": "TK101",
                "region": "process",
                "bind": {"level": {"path": "Main.Level", "direction": "read"}},
            },
            {"id": "pump", "type": "pump", "tag": "P101", "region": "process"},
            {
                "id": "level",
                "type": "value",
                "label": "Tank level",
                "region": "measurements",
                "unit": "m",
                "bind": {"value": {"path": "Main.Level", "direction": "read"}},
            },
        ],
        "connections": [{"from": "tank.outlet", "to": "pump.inlet"}],
    })
}

/// The object with the given ID.
fn object<'a>(page: &'a HmiPage, id: &str) -> &'a trust_runtime::hmi::page_files::HmiObject {
    page.objects
        .iter()
        .find(|object| object.id == id)
        .expect("the composed page holds the object")
}

/// The page as its JSON text, for checks that a field is absent from the stored model.
fn text_of(page: &HmiPage) -> String {
    serde_json::to_string(page).expect("a page serializes")
}

/// TypeScript: "places real catalogue components on grid and keeps provided bindings and connections".
#[test]
fn places_real_components_on_grid_and_keeps_bindings_and_connections() {
    let page = compose_agent_page(&input(), &catalogue()).expect("composes");
    assert_eq!(page.objects.len(), 4, "heading and three objects");
    assert_eq!(page.pipes.len(), 1, "the one connection is kept");
    assert_eq!(page.pipes[0].from, "tank.outlet");
    assert_eq!(page.pipes[0].to, "pump.inlet");
    assert_eq!(
        object(&page, "tank").bind["level"].path,
        "Main.Level",
        "the provided binding is kept"
    );
    for object in &page.objects {
        assert!(object.at.iter().all(|n| n % 8 == 0), "{}", object.id);
        assert!(object.at[0] >= 0 && object.at[1] >= 0, "{}", object.id);
        assert!(
            object.at[0] + i64::from(object.size[0]) <= 1280,
            "{}",
            object.id
        );
        assert!(
            object.at[1] + i64::from(object.size[1]) <= 800,
            "{}",
            object.id
        );
    }
    let again = compose_agent_page(&input(), &catalogue()).expect("composes");
    assert_eq!(again, page, "repeatable layout");
}

/// TypeScript: "readings are separated from the dominant process region".
#[test]
fn readings_are_separated_from_the_dominant_process_region() {
    let page = compose_agent_page(&input(), &catalogue()).expect("composes");
    let tank = object(&page, "tank");
    let reading = object(&page, "level");
    assert!(reading.at[0] > tank.at[0] + i64::from(tank.size[0]));
}

/// TypeScript: "does not invent connections or bindings".
#[test]
fn does_not_invent_connections_or_bindings() {
    let mut proposal = input();
    proposal["connections"] = json!([]);
    for item in proposal["items"].as_array_mut().expect("items") {
        item.as_object_mut().expect("item").remove("bind");
    }
    let page = compose_agent_page(&proposal, &catalogue()).expect("composes");
    let objects = page
        .objects
        .iter()
        .filter(|o| o.id != "page-heading")
        .count();
    assert_eq!(objects, 3);
    assert!(page.pipes.is_empty());
    assert!(page.objects.iter().all(|o| o.bind.is_empty()));
}

/// TypeScript: "overcrowded screens fail instead of shrinking text or equipment".
#[test]
fn overcrowded_screens_fail_instead_of_shrinking() {
    let mut proposal = input();
    proposal["size"] = json!([320, 240]);
    let error = compose_agent_page(&proposal, &catalogue()).expect_err("must not fit");
    let text = error.to_string();
    assert!(
        text.contains("fit") || text.contains("constraint"),
        "{text}"
    );
}

/// TypeScript: "unknown components and duplicate ids are refused".
#[test]
fn unknown_components_and_duplicate_ids_are_refused() {
    let mut proposal = input();
    proposal["items"][1]["id"] = json!("tank");
    let error = compose_agent_page(&proposal, &catalogue()).expect_err("duplicate id");
    assert!(error.to_string().contains("id"), "{error}");
    proposal["items"][1]["id"] = json!("pump");
    proposal["items"][1]["type"] = json!("made-up");
    let error = compose_agent_page(&proposal, &catalogue()).expect_err("unknown type");
    assert!(error.to_string().contains("component"), "{error}");
}

/// TypeScript: "a sparse overview gives equipment visual prominence and readings a consistent side
/// column".
#[test]
fn a_sparse_overview_gives_equipment_prominence_and_readings_a_column() {
    let mut proposal = input();
    proposal["items"]
        .as_array_mut()
        .expect("items")
        .push(json!({"id": "temperature", "type": "value", "region": "measurements", "label": "Temperature"}));
    let page = compose_agent_page(&proposal, &catalogue()).expect("composes");
    let tank = object(&page, "tank");
    let level = object(&page, "level");
    let temperature = object(&page, "temperature");
    assert!(
        tank.size[0] >= 300 && tank.size[1] >= 360,
        "primary equipment should occupy the available process region"
    );
    assert_eq!(level.at[0], temperature.at[0]);
    assert!(level.size[0] >= 300 && temperature.size[0] >= 300);
    assert!(temperature.at[1] >= level.at[1] + i64::from(level.size[1]) + 24);
}

/// TypeScript: "explicit regions and vertical flow control deterministic placement".
#[test]
fn explicit_regions_and_vertical_flow_control_placement() {
    let mut proposal = input();
    proposal["connections"] = json!([]);
    proposal["layout"] = json!({
        "flow": "vertical",
        "gap": 16,
        "regions": {
            "process": {"x": 32, "y": 88, "width": 704, "height": 672},
            "measurements": {"x": 768, "y": 88, "width": 480, "height": 672},
        },
    });
    proposal["items"] = json!([
        {"id": "first", "type": "pump", "size": [160, 120]},
        {"id": "second", "type": "pump", "size": [160, 120]},
        {"id": "level", "type": "value", "region": "measurements"},
    ]);
    let page = compose_agent_page(&proposal, &catalogue()).expect("composes");
    assert_eq!(object(&page, "first").at, [32, 88]);
    assert_eq!(object(&page, "second").at, [32, 224]);
    assert_eq!(object(&page, "level").at[0], 768);
    assert_eq!(object(&page, "level").size[0], 480);
    let again = compose_agent_page(&proposal, &catalogue()).expect("composes");
    assert_eq!(again, page);
}

/// TypeScript: "pinned objects preserve their geometry and auto placement leaves their boxes free".
#[test]
fn pinned_objects_keep_their_geometry_and_auto_placement_leaves_them_free() {
    let mut proposal = input();
    proposal["connections"] = json!([]);
    proposal["items"] = json!([
        {"id": "auto", "type": "pump", "size": [160, 120]},
        {"id": "fixed", "type": "pump", "size": [160, 120], "pinnedAt": [32, 88]},
    ]);
    let page = compose_agent_page(&proposal, &catalogue()).expect("composes");
    let fixed = object(&page, "fixed");
    let auto = object(&page, "auto");
    assert_eq!(fixed.at, [32, 88]);
    assert_eq!(fixed.size, [160, 120]);
    assert_eq!(auto.at, [216, 88]);
    assert!(
        !text_of(&page).contains("pinnedAt"),
        "layout-only fields must not enter the stored page model"
    );
}

/// TypeScript: "conflicting regions and pinned boxes fail with actionable object or region identity".
#[test]
fn conflicting_regions_and_pinned_boxes_name_the_objects_or_regions() {
    let mut base = input();
    base["connections"] = json!([]);

    let mut overlapping = base.clone();
    overlapping["layout"] =
        json!({"regions": {"process": {"x": 32, "y": 88, "width": 1000, "height": 672}}});
    let text = compose_agent_page(&overlapping, &catalogue())
        .expect_err("regions overlap")
        .to_string();
    assert!(
        text.contains("process") && text.contains("measurements"),
        "{text}"
    );

    let mut off_grid = base.clone();
    off_grid["layout"] =
        json!({"regions": {"process": {"x": 31, "y": 88, "width": 704, "height": 672}}});
    let text = compose_agent_page(&off_grid, &catalogue())
        .expect_err("off grid")
        .to_string();
    assert!(text.contains("grid"), "{text}");

    let mut outside = base.clone();
    outside["items"] = json!([{"id": "fixed", "type": "pump", "pinnedAt": [1200, 88]}]);
    let text = compose_agent_page(&outside, &catalogue())
        .expect_err("pinned outside")
        .to_string();
    assert!(text.contains("fixed") && text.contains("fit"), "{text}");

    let mut colliding = base;
    colliding["items"] = json!([
        {"id": "a", "type": "pump", "pinnedAt": [32, 88]},
        {"id": "b", "type": "pump", "pinnedAt": [32, 88]},
    ]);
    let text = compose_agent_page(&colliding, &catalogue())
        .expect_err("pinned boxes collide")
        .to_string();
    assert!(text.contains('a') && text.contains('b'), "{text}");
}

/// The composition refuses a request that is not an object, with the TypeScript's text.
#[test]
fn a_request_that_is_not_an_object_is_refused() {
    let error = compose_agent_page(&json!("nope"), &catalogue()).expect_err("refused");
    assert_eq!(error.code, "INVALID_ARGUMENT");
    assert_eq!(error.message, "invalid composition constraints.");
}
