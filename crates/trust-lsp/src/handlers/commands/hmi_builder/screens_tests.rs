//! New HMI Screen's templates and first page (specification 39, §4 items 1, 8 and 10).

use serde_json::{json, Value};
use trust_runtime::hmi::page_files::{
    parse_page, parse_screen, HmiNavigationPosition, HmiNavigationStyle, HmiScreen, HmiTheme,
};

use super::*;

fn created(template: &str, folder: &str) -> Value {
    answer(json!({ "action": "create", "template": template, "folder": folder }))
        .expect("the screen is created")
}

fn screen_of(created: &Value) -> HmiScreen {
    assert_eq!(created["screen"]["file"], "hmi/screen.toml");
    parse_screen(
        "hmi/screen.toml",
        created["screen"]["text"]
            .as_str()
            .expect("the layout's text"),
    )
    .expect("the screen layout loads")
}

#[test]
fn the_templates_are_offered_in_order_with_their_labels_and_details() {
    // §4 item 10. Catches a template missing from the quick pick or offered under another id.
    assert_eq!(
        answer(json!({ "action": "templates" })).expect("answered"),
        json!({
            "ok": true,
            "templates": [
                {
                    "id": "operator-panel",
                    "label": "Operator panel",
                    "detail": "For a 1920 × 1080 panel: top bar with title, clock, alarms and user, side bar on the left",
                },
                {
                    "id": "overview",
                    "label": "Overview",
                    "detail": "For a 1280 × 800 panel: top bar with title, clock and user, tabs",
                },
                {
                    "id": "single-page",
                    "label": "Single page",
                    "detail": "1024 × 768, without top bar or navigation",
                },
            ],
        })
    );
}

#[test]
fn the_operator_panel_has_a_full_top_bar_titled_after_the_folder_and_a_side_bar() {
    // §4 items 8 and 10: page size 1688 × 1024 for a 1920 × 1080 panel; the title is the folder's
    // name in words. Catches the folder's name used as it is, and a missing top bar option.
    let created = created("operator-panel", "room-heating");
    let screen = screen_of(&created);
    assert_eq!((screen.size, screen.theme), ([1688, 1024], HmiTheme::Dark));
    let bar = screen.top_bar.expect("a top bar");
    assert_eq!(
        (
            bar.title.as_str(),
            bar.clock,
            bar.alarms,
            bar.user,
            bar.page_title
        ),
        ("Room heating", true, true, true, true)
    );
    assert_eq!(
        (screen.navigation.style, screen.navigation.position),
        (HmiNavigationStyle::Side, HmiNavigationPosition::Left)
    );
}

#[test]
fn the_overview_and_single_page_templates_choose_their_size_and_navigation() {
    // §4 item 10. Catches the overview with alarms or a page title in its top bar, and a single
    // page with a top bar or navigation.
    let tabs = screen_of(&created("overview", "plant"));
    let bar = tabs.top_bar.expect("a top bar");
    assert_eq!(
        (tabs.size, tabs.navigation.style),
        ([1280, 696], HmiNavigationStyle::Tabs)
    );
    assert_eq!(
        (
            bar.title.as_str(),
            bar.clock,
            bar.alarms,
            bar.user,
            bar.page_title
        ),
        ("Plant", true, false, true, false)
    );
    let single = screen_of(&created("single-page", "plant"));
    assert_eq!(
        (single.size, single.navigation.style, single.top_bar),
        ([1024, 768], HmiNavigationStyle::None, None)
    );
}

#[test]
fn the_folder_name_becomes_the_title_in_words() {
    // §4 item 8. Catches `room_heating-2` titled other than "Room heating 2" and separators at
    // the ends left as spaces.
    for (folder, title) in [
        ("room_heating-2", "Room heating 2"),
        ("--tank__farm-", "Tank farm"),
        ("Mill", "Mill"),
    ] {
        let screen = screen_of(&created("overview", folder));
        assert_eq!(screen.top_bar.expect("a top bar").title, title, "{folder}");
    }
}

#[test]
fn the_first_page_is_an_overview_first_in_the_navigation_with_a_heading() {
    // Catches a first page in another file, without its order or icon, or without its heading.
    let created = created("single-page", "plant");
    assert_eq!(created["page"]["file"], "hmi/pages/overview.toml");
    let page = parse_page(
        "hmi/pages/overview.toml",
        created["page"]["text"].as_str().expect("the page's text"),
    )
    .expect("the first page loads");
    assert_eq!(
        (page.title.as_str(), page.order, page.icon.as_deref()),
        ("Overview", Some(0), Some("overview"))
    );
    let [heading] = page.objects.as_slice() else {
        panic!("one object: {:?}", page.objects);
    };
    assert_eq!(
        (
            heading.id.as_str(),
            heading.kind.as_str(),
            heading.variant.as_deref(),
            heading.text.as_deref(),
            heading.at,
            heading.size,
        ),
        (
            "heading",
            "text",
            Some("heading"),
            Some("Overview"),
            [40, 32],
            [600, 40]
        )
    );
}

#[test]
fn an_unknown_template_is_refused() {
    // Catches a screen made from a default when the template is not one of the three.
    assert_eq!(
        answer(json!({ "action": "create", "template": "dashboard", "folder": "plant" })),
        Err("There is no screen template \"dashboard\".".to_string())
    );
}

#[test]
fn the_default_screen_layout_is_1920_by_1080_dark_flat_without_navigation() {
    // §3.4 and §4 item 1, decided on 26 October 2026 for a page or panel made in a project
    // without a screen layout. Catches the `format = 1` alone Create Pages from Program wrote,
    // which does not load, and the 1280 × 720 New HMI Page wrote.
    let text = default_screen();
    assert_eq!(
        text,
        "format = 1\nsize = [1920, 1080]\ntheme = \"dark\"\nsymbols = \"flat\"\n\n[navigation]\nstyle = \"none\"\n"
    );
    let screen = parse_screen("hmi/screen.toml", &text).expect("the layout loads");
    assert_eq!(
        (
            screen.size,
            screen.theme,
            screen.navigation.style,
            screen.top_bar
        ),
        ([1920, 1080], HmiTheme::Dark, HmiNavigationStyle::None, None)
    );
}
