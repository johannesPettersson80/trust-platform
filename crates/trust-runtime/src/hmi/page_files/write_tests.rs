//! Specification 39, §15.2: canonical writing.

use super::fixtures::{FILE, SPEC_EXAMPLE};
use super::*;

fn changed_lines(before: &str, after: &str) -> Vec<String> {
    let before: Vec<&str> = before.lines().collect();
    let after: Vec<&str> = after.lines().collect();
    assert_eq!(
        before.len(),
        after.len(),
        "the line count changed:\n{}",
        after.join("\n")
    );
    before
        .iter()
        .zip(&after)
        .filter(|(old, new)| old != new)
        .map(|(_, new)| new.to_string())
        .collect()
}

fn spec_page() -> HmiPage {
    parse_page(FILE, SPEC_EXAMPLE).expect("the specification example loads")
}

#[test]
fn spec_example_is_written_back_byte_for_byte() {
    assert_eq!(write_page(&spec_page()), SPEC_EXAMPLE);
}

#[test]
fn moving_one_object_changes_one_line() {
    let mut page = spec_page();
    page.objects[1].at = [660, 480];
    assert_eq!(
        changed_lines(SPEC_EXAMPLE, &write_page(&page)),
        ["at = [660, 480]"]
    );
}

#[test]
fn rebinding_one_property_changes_one_line() {
    let mut page = spec_page();
    page.objects[1]
        .bind
        .get_mut("running")
        .expect("the pump binds running")
        .path = "FeedArea.P101.Run".to_string();
    assert_eq!(
        changed_lines(SPEC_EXAMPLE, &write_page(&page)),
        ["running = \"FeedArea.P101.Run\""]
    );
}

#[test]
fn every_field_round_trips() {
    let mut bind = std::collections::BTreeMap::new();
    bind.insert(
        "value".to_string(),
        HmiBinding {
            quality: None,
            path: "Line.FT101.Flow".to_string(),
            unit: Some("m³/h".to_string()),
            range: None,
            direction: HmiBindingDirection::ReadWrite,
        },
    );
    bind.insert(
        "enable".to_string(),
        HmiBinding {
            quality: None,
            path: "Line.Enable".to_string(),
            unit: None,
            range: None,
            direction: HmiBindingDirection::Write,
        },
    );
    let page = HmiPage {
        regions: Vec::new(),
        order: None,
        icon: None,
        group: None,
        hidden: false,
        incomplete: false,
        pid: None,
        title: "All fields".to_string(),
        objects: vec![
            HmiObject {
                id: "value-1".to_string(),
                tag: Some("FT101".to_string()),
                kind: "value".to_string(),
                variant: Some("large".to_string()),
                block: None,
                faceplate: None,
                instance: None,
                at: [-20, 40],
                size: [200, 60],
                rotation: 90,
                label: Some("Flow".to_string()),
                text: Some("Feed".to_string()),
                image: Some("hmi/assets/feed.svg".to_string()),
                unit: Some("m³/h".to_string()),
                decimals: Some(1),
                min: Some(0.5),
                max: Some(120.0),
                limits: None,
                span: Some(600),
                role: None,
                command: None,
                confirm: None,
                action: None,
                options: None,
                pulse: None,
                expect: None,
                timeout: None,
                attach: None,
                measures: None,
                acts_on: None,
                group: None,
                locked: false,
                arrange: None,
                gap: None,
                bind,
                click: Some(HmiClick::Popup("flow-details".to_string())),
            },
            HmiObject {
                id: "button-1".to_string(),
                tag: None,
                kind: "page-button".to_string(),
                variant: None,
                block: None,
                faceplate: None,
                instance: None,
                at: [0, 0],
                size: [120, 40],
                rotation: 0,
                label: None,
                text: Some("Trends".to_string()),
                image: None,
                unit: None,
                decimals: None,
                min: None,
                max: None,
                limits: None,
                span: None,
                role: None,
                command: None,
                confirm: None,
                action: None,
                options: None,
                pulse: None,
                expect: None,
                timeout: None,
                attach: None,
                measures: None,
                acts_on: None,
                group: None,
                locked: false,
                arrange: None,
                gap: None,
                bind: std::collections::BTreeMap::new(),
                click: Some(HmiClick::Page("trends".to_string())),
            },
        ],
        pipes: vec![],
    };
    let written = write_page(&page);
    assert_eq!(parse_page(FILE, &written).expect(&written), page);
}

#[test]
fn quotes_and_backslashes_are_escaped() {
    let mut page = spec_page();
    page.objects[0].label = Some("Tank \"A\" \\ main".to_string());
    let written = write_page(&page);
    assert!(
        written.contains("label = \"Tank \\\"A\\\" \\\\ main\""),
        "{written}"
    );
    assert_eq!(parse_page(FILE, &written).expect(&written), page);
}

#[test]
fn whole_numbers_in_ranges_are_written_without_decimals() {
    let mut page = spec_page();
    page.objects[0].min = Some(0.0);
    page.objects[0].max = Some(100.0);
    page.objects[1].min = Some(0.5);
    let written = write_page(&page);
    for expected in ["min = 0\n", "max = 100\n", "min = 0.5\n"] {
        assert!(
            written.contains(expected),
            "missing {expected:?} in\n{written}"
        );
    }
}

#[test]
fn incomplete_design_round_trips_without_becoming_a_normal_page() {
    let mut page = spec_page();
    page.incomplete = true;
    let text = write_page(&page);
    assert!(text.contains("incomplete = true"));
    assert!(parse_page(FILE, &text).expect("editable draft").incomplete);
    page.incomplete = false;
    assert!(!write_page(&page).contains("incomplete"));
}
