//! Tests of the tool input schemas (specification 43, §4 and §5): the bounds each tool states, checked on
//! both sides of every limit, the unknown-field rules and the shapes of a request.
//!
//! Port of two assertions of `editors/vscode/src/test/suite/hmi-agent-completion.test.ts` ("public schemas
//! accept explicit multipage proposals and refuse incomplete draft references") and
//! `editors/vscode/src/test/suite/hmi-agent-tools.test.ts` ("published mutation schemas reject unknown fields
//! before creating or changing a draft"), and of the bounds the TypeScript `schema.ts` states. Each limit is
//! checked with the value inside it and the value outside it.

use serde_json::{json, Value};

use super::super::error::{INVALID_ARGUMENT, LIMIT_EXCEEDED};
use super::*;

/// The refusal code of a tool input; the input must be refused.
fn refusal(tool: &str, input: Value) -> &'static str {
    check_tool_input(tool, input)
        .expect_err("the input must be refused")
        .code
}

/// Whether a tool input is accepted.
fn accepted(tool: &str, input: Value) -> bool {
    check_tool_input(tool, input).is_ok()
}

/// A composition request for one destination page, with the given items.
fn composition(items: Value) -> Value {
    json!({
        "key": "composition-1",
        "file": "hmi/pages/tank.toml",
        "title": "Tank",
        "pattern": "process-overview",
        "items": items,
    })
}

/// An edit request for one draft, with the given operations.
fn edit(operations: Value) -> Value {
    json!({
        "draftId": "draft-1",
        "revision": "1",
        "key": "edit-1",
        "operations": operations,
    })
}

/// Port of the context limit of `hmi-agent-queries.test.ts` ("library queries paginate instead of silently
/// dropping components"): an answer holds one to one hundred pages (specification 43, §4).
#[test]
fn the_context_limit_admits_one_to_one_hundred_pages() {
    for limit in [1, 100] {
        assert!(
            accepted("hmi_get_context", json!({ "limit": limit })),
            "limit {limit}"
        );
    }
    for limit in [0, 101] {
        assert_eq!(
            refusal("hmi_get_context", json!({ "limit": limit })),
            INVALID_ARGUMENT,
            "limit {limit}"
        );
    }
}

/// Port of the library bounds of `hmi-agent-queries.test.ts` ("library queries paginate ..."): one to one
/// hundred components per page, a category of at most 100 characters and a state of 1 to 64 (AT-02).
#[test]
fn the_library_limit_and_text_bounds_are_closed_at_their_edges() {
    for limit in [1, 100] {
        assert!(
            accepted("hmi_browse_library", json!({ "limit": limit })),
            "limit {limit}"
        );
    }
    for limit in [0, 101] {
        assert_eq!(
            refusal("hmi_browse_library", json!({ "limit": limit })),
            INVALID_ARGUMENT,
            "limit {limit}"
        );
    }
    assert!(accepted(
        "hmi_browse_library",
        json!({ "category": "a".repeat(100) })
    ));
    assert_eq!(
        refusal("hmi_browse_library", json!({ "category": "a".repeat(101) })),
        INVALID_ARGUMENT
    );
    assert!(accepted(
        "hmi_browse_library",
        json!({ "state": "s".repeat(64) })
    ));
    assert_eq!(
        refusal("hmi_browse_library", json!({ "state": "" })),
        INVALID_ARGUMENT
    );
    assert_eq!(
        refusal("hmi_browse_library", json!({ "state": "s".repeat(65) })),
        INVALID_ARGUMENT
    );
}

/// A validation names at most three viewports, with unique identifiers (specification 43, §7).
#[test]
fn a_validation_names_at_most_three_viewports_with_unique_identifiers() {
    let viewport = |id: &str| json!({ "id": id, "width": 800, "height": 600 });
    let validate = |viewports: Value| {
        check_tool_input(
            "hmi_validate_draft",
            json!({ "draftId": "draft-1", "revision": "1", "viewports": viewports }),
        )
    };
    assert!(validate(json!([viewport("a"), viewport("b"), viewport("c")])).is_ok());
    assert_eq!(
        validate(json!([
            viewport("a"),
            viewport("b"),
            viewport("c"),
            viewport("d")
        ]))
        .expect_err("four viewports")
        .code,
        LIMIT_EXCEEDED
    );
    assert_eq!(
        validate(json!([viewport("a"), viewport("a")]))
            .expect_err("a repeated identifier")
            .code,
        INVALID_ARGUMENT
    );
}

/// A viewport size is whole pixels from one to 4096 on each side (specification 43, §7).
#[test]
fn viewport_sizes_are_whole_pixels_from_one_to_4096() {
    let size = |width: i64, height: i64| {
        check_tool_input(
            "hmi_validate_draft",
            json!({
                "draftId": "draft-1",
                "revision": "1",
                "viewports": [{ "id": "panel", "width": width, "height": height }],
            }),
        )
    };
    for edge in [1, 4096] {
        assert!(size(edge, edge).is_ok(), "{edge} pixels");
    }
    for edge in [0, 4097] {
        assert_eq!(
            size(edge, 600).expect_err("a width outside the bound").code,
            INVALID_ARGUMENT,
            "width {edge}"
        );
        assert_eq!(
            size(800, edge)
                .expect_err("a height outside the bound")
                .code,
            INVALID_ARGUMENT,
            "height {edge}"
        );
    }
}

/// A viewport identifier starts with a letter and holds at most 64 letters, digits, `_` or `-`
/// (specification 43, §7; the viewport profile pattern).
#[test]
fn viewport_identifiers_start_with_a_letter_and_hold_at_most_64_characters() {
    let with_id = |id: String| {
        check_tool_input(
            "hmi_validate_draft",
            json!({
                "draftId": "draft-1",
                "revision": "1",
                "viewports": [{ "id": id, "width": 800, "height": 600 }],
            }),
        )
    };
    assert!(
        with_id(format!("a{}", "1".repeat(63))).is_ok(),
        "64 characters"
    );
    assert!(with_id("Panel_1-a".to_owned()).is_ok());
    assert_eq!(
        with_id(format!("a{}", "1".repeat(64)))
            .expect_err("65 characters")
            .code,
        INVALID_ARGUMENT
    );
    assert_eq!(
        with_id("1panel".to_owned())
            .expect_err("a digit first")
            .code,
        INVALID_ARGUMENT
    );
    assert_eq!(
        with_id(String::new())
            .expect_err("an empty identifier")
            .code,
        INVALID_ARGUMENT
    );
    assert_eq!(
        with_id("panel.one".to_owned()).expect_err("a dot").code,
        INVALID_ARGUMENT
    );
}

/// Port of the batch bound of `hmi-agent-edits.test.ts` ("batch failure leaves the source unchanged"),
/// stated by the schema: one to 500 operations per edit (specification 43, §4 and AT-05).
#[test]
fn an_edit_batch_admits_500_operations_and_refuses_501() {
    let moves = |count: usize| vec![json!({ "op": "move", "ids": ["a"], "delta": [8, 0] }); count];
    assert!(accepted("hmi_edit_draft", edit(json!(moves(500)))));
    assert_eq!(
        refusal("hmi_edit_draft", edit(json!(moves(501)))),
        LIMIT_EXCEEDED
    );
    assert_eq!(refusal("hmi_edit_draft", edit(json!([]))), LIMIT_EXCEEDED);
}

/// Preview states name one to twelve scenarios (specification 43, AT-08).
#[test]
fn preview_states_name_one_to_twelve_scenarios() {
    let states = |count: usize| json!({ "draftId": "draft-1", "revision": "1", "states": vec!["normal"; count] });
    assert!(accepted("hmi_preview_states", states(1)));
    assert!(accepted("hmi_preview_states", states(12)));
    assert_eq!(refusal("hmi_preview_states", states(0)), LIMIT_EXCEEDED);
    assert_eq!(refusal("hmi_preview_states", states(13)), LIMIT_EXCEEDED);
}

/// A composed object is at least 8 pixels on each side, and its rotation is a quarter turn: 0, 90, 180 or
/// 270 degrees (specification 43, §5; the object schema of `schema.ts`).
#[test]
fn composed_objects_are_at_least_eight_pixels_and_quarter_turns() {
    let item = |extra: Value| {
        let mut item = json!({ "id": "tank", "type": "tank" });
        item["size"] = extra["size"].clone();
        composition(json!([item]))
    };
    assert!(accepted(
        "hmi_compose_page",
        item(json!({ "size": [8, 8] }))
    ));
    assert_eq!(
        refusal("hmi_compose_page", item(json!({ "size": [7, 8] }))),
        INVALID_ARGUMENT
    );
    assert_eq!(
        refusal("hmi_compose_page", item(json!({ "size": [8, 7] }))),
        INVALID_ARGUMENT
    );
    for rotation in [0, 90, 180, 270] {
        let mut object = json!({ "id": "tank", "type": "tank" });
        object["rotation"] = json!(rotation);
        assert!(
            accepted("hmi_compose_page", composition(json!([object]))),
            "rotation {rotation}"
        );
    }
    let mut turned = json!({ "id": "tank", "type": "tank" });
    turned["rotation"] = json!(45);
    assert_eq!(
        refusal("hmi_compose_page", composition(json!([turned]))),
        INVALID_ARGUMENT
    );
}

/// An object identifier starts with a letter, then holds letters, digits, `_` or `-` (specification 43, §5).
#[test]
fn object_identifiers_follow_the_letter_then_word_pattern() {
    let with_id = |id: &str| {
        refusal_or_ok(
            "hmi_compose_page",
            composition(json!([{ "id": id, "type": "tank" }])),
        )
    };
    assert_eq!(with_id("tank-1_a"), None);
    assert_eq!(with_id("1tank"), Some(INVALID_ARGUMENT));
    assert_eq!(with_id(""), Some(INVALID_ARGUMENT));
    assert_eq!(with_id("tank.1"), Some(INVALID_ARGUMENT));
    assert_eq!(with_id("tank 1"), Some(INVALID_ARGUMENT));
}

/// The refusal code of a tool input, or `None` when it is accepted.
fn refusal_or_ok(tool: &str, input: Value) -> Option<&'static str> {
    check_tool_input(tool, input).err().map(|error| error.code)
}

/// Port of `hmi-agent-tools.test.ts` ("published mutation schemas reject unknown fields before creating or
/// changing a draft"): a key an operation or a composition does not own is refused, and an empty
/// composition of items is still a valid request (specification 43, §4 and §5).
#[test]
fn a_request_refuses_keys_it_does_not_own() {
    assert_eq!(
        refusal(
            "hmi_edit_draft",
            edit(json!([{ "op": "move", "ids": ["a"], "delta": [8, 0], "typo": true }]))
        ),
        INVALID_ARGUMENT
    );
    let mut composed = composition(json!([]));
    assert!(accepted("hmi_compose_page", composed.clone()));
    composed["unsupportedOption"] = json!(true);
    assert_eq!(refusal("hmi_compose_page", composed), INVALID_ARGUMENT);
}

/// Port of `hmi-agent-completion.test.ts` ("public schemas accept explicit multipage proposals and refuse
/// incomplete draft references"): a multipage composition and a refinement of a source page are valid
/// shapes, and an inspection of a draft must name its revision (specification 43, AT-03 and AT-04).
#[test]
fn a_multipage_proposal_is_a_valid_shape_and_a_draft_inspection_needs_its_revision() {
    let proposal = json!({
        "key": "multi",
        "pages": [
            { "file": "hmi/pages/a.toml", "title": "A", "pattern": "settings", "items": [] },
            { "file": "hmi/pages/b.toml", "sourceFile": "hmi/pages/process.toml" },
        ],
    });
    assert!(accepted("hmi_compose_page", proposal));
    let refused = check_tool_input(
        "hmi_inspect_page",
        json!({ "file": "hmi/pages/a.toml", "draftId": "draft-1" }),
    )
    .expect_err("a draft without its revision is refused");
    assert_eq!(refused.code, INVALID_ARGUMENT);
    assert!(refused.message.contains("revision"), "{}", refused.message);
}

/// A composition names one to 32 unique page destinations (specification 43, §10.1).
#[test]
fn a_composition_names_at_most_32_pages() {
    let page = |index: usize| {
        json!({
            "file": format!("hmi/pages/page-{index}.toml"),
            "title": "Page",
            "pattern": "settings",
            "items": [],
        })
    };
    let pages = |count: usize| {
        json!({
            "key": "pages",
            "pages": (0..count).map(&page).collect::<Vec<Value>>(),
        })
    };
    assert!(accepted("hmi_compose_page", pages(32)));
    assert_eq!(refusal("hmi_compose_page", pages(33)), LIMIT_EXCEEDED);
}

/// The apply request takes the action review, apply or discard, with the fields of that action only
/// (specification 43, AT-09).
#[test]
fn an_apply_request_takes_one_of_three_actions_and_no_other_fields() {
    let mut apply = json!({ "draftId": "draft-1", "revision": "1", "action": "apply" });
    apply["allowIncomplete"] = json!(true);
    assert!(accepted("hmi_apply_draft", apply.clone()));
    apply["bogus"] = json!(1);
    assert_eq!(refusal("hmi_apply_draft", apply), INVALID_ARGUMENT);
    assert_eq!(
        refusal(
            "hmi_apply_draft",
            json!({ "draftId": "draft-1", "revision": "1", "action": "publish" })
        ),
        INVALID_ARGUMENT
    );
}

/// A layout gap is 8 to 128 pixels on the 8-pixel grid, and its padding 0 to 128 on the same grid
/// (specification 43, §6; `layoutOptions` in `layout.ts`).
#[test]
fn layout_gaps_and_padding_stay_on_the_eight_pixel_grid() {
    let with_layout = |layout: Value| {
        let mut request = composition(json!([{ "id": "tank", "type": "tank" }]));
        request["layout"] = layout;
        request
    };
    assert!(accepted(
        "hmi_compose_page",
        with_layout(json!({ "gap": 24, "padding": 24 }))
    ));
    assert_eq!(
        refusal("hmi_compose_page", with_layout(json!({ "gap": 20 }))),
        INVALID_ARGUMENT
    );
    assert_eq!(
        refusal("hmi_compose_page", with_layout(json!({ "gap": 136 }))),
        INVALID_ARGUMENT
    );
    assert_eq!(
        refusal("hmi_compose_page", with_layout(json!({ "padding": 132 }))),
        INVALID_ARGUMENT
    );
}

/// A render viewport is one to 4096 pixels per side (specification 43, AT-07).
#[test]
fn a_render_viewport_is_one_to_4096_pixels_per_side() {
    let render = |width: i64| {
        json!({
            "draftId": "draft-1",
            "revision": "1",
            "viewport": { "width": width, "height": 600 },
        })
    };
    assert!(accepted("hmi_render_draft", render(1)));
    assert_eq!(refusal("hmi_render_draft", render(0)), INVALID_ARGUMENT);
    assert_eq!(refusal("hmi_render_draft", render(4097)), INVALID_ARGUMENT);
}

/// A tool this service does not serve is refused (specification 43, §3).
#[test]
fn a_tool_this_service_does_not_serve_is_refused() {
    assert_eq!(refusal("hmi_unknown_tool", json!({})), INVALID_ARGUMENT);
}
