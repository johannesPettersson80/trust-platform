//! Specification 39, §4.6 and §4.9: a page's place in the navigation, and popup files.

use super::fixtures::FILE;
use super::*;

const LISTED: &str = r#"format = 1
title = "Trends"
order = 2
icon = "trend"
group = "Process"
hidden = true

[[object]]
id = "flow"
type = "trend"
at = [0, 0]
size = [480, 240]
"#;

const POPUP_FILE: &str = "hmi/popups/pump.toml";

const POPUP: &str = r#"format = 1
title = "Pump P101"
size = [480, 320]
pinnable = true

[[object]]
id = "speed"
type = "value"
at = [24, 24]
size = [200, 96]

[object.bind]
value = "Main.P101.Speed"
"#;

fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .expect("the needle is in the text")
        + 1
}

fn assert_error(found: &[PageFileError], line: usize, words: &str) {
    assert!(
        found
            .iter()
            .any(|error| error.line == line && error.message.contains(words)),
        "line {line}, {words}: {found:#?}"
    );
}

#[test]
fn a_page_declares_its_place_in_the_navigation_and_is_written_back_byte_for_byte() {
    let page = parse_page(FILE, LISTED).expect("the page loads");
    assert_eq!(
        (
            page.order,
            page.icon.as_deref(),
            page.group.as_deref(),
            page.hidden
        ),
        (Some(2), Some("trend"), Some("Process"), true)
    );
    assert_eq!(write_page(&page), LISTED);
}

#[test]
fn an_unknown_icon_an_empty_group_or_a_negative_order_is_an_error_on_its_line() {
    for (from, to, words) in [
        ("icon = \"trend\"", "icon = \"rocket\"", "icon"),
        ("group = \"Process\"", "group = \" \"", "group"),
        ("order = 2", "order = -1", "order"),
    ] {
        let text = LISTED.replace(from, to);
        let found = parse_page(FILE, &text).expect_err("rejected");
        assert_error(&found, line_of(&text, to), words);
    }
}

#[test]
fn a_popup_has_its_title_size_and_pin_and_is_written_back_byte_for_byte() {
    let popup = parse_popup(POPUP_FILE, POPUP).expect("the popup loads");
    assert_eq!(
        (popup.page.title.as_str(), popup.size, popup.pinnable),
        ("Pump P101", [480, 320], true)
    );
    assert_eq!(popup.page.objects.len(), 1);
    assert_eq!(write_popup(&popup), POPUP);
}

#[test]
fn a_popup_needs_a_size_and_has_no_place_in_the_navigation() {
    let text = POPUP.replace("size = [480, 320]\n", "");
    let found = parse_popup(POPUP_FILE, &text).expect_err("rejected");
    assert!(
        found.iter().any(|error| error.message.contains("size")),
        "{found:#?}"
    );
    let text = POPUP.replace("pinnable = true", "order = 1");
    let found = parse_popup(POPUP_FILE, &text).expect_err("rejected");
    assert_error(&found, line_of(&text, "order = 1"), "order");
    let text = POPUP.replace("type = \"value\"", "type = \"value\"\nmin = nan");
    let found = parse_popup(POPUP_FILE, &text).expect_err("rejected");
    assert_error(&found, line_of(&text, "min = nan"), "finite");
}
