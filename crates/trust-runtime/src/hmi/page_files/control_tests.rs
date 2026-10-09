//! Specification 39, §5.1 and §12.8: what a control declares.

use super::fixtures::FILE;
use super::*;

const CONTROLS: &str = r#"format = 1
title = "Controls"

[[object]]
id = "start"
type = "pulse-button"
at = [40, 40]
size = [120, 40]
text = "Start"
role = "engineer"
confirm = "Start feed pump P101?"
pulse = 0.5
expect = true
timeout = 8

[object.bind]
feedback = "Plant.P101.Running"
interlock = "Plant.P101.Permit"
interlock-reason = "Plant.P101.PermitText"
target = { path = "Plant.P101.StartCmd", direction = "write" }

[[object]]
id = "stop"
type = "button"
at = [40, 96]
size = [120, 40]
text = "Stop"
action = "reset"

[object.bind]
target = { path = "Plant.P101.Run", direction = "write" }
"#;

fn errors(text: &str) -> Vec<PageFileError> {
    parse_page(FILE, text).expect_err("the page should be rejected")
}

fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .expect("the needle is in the text")
        + 1
}

#[test]
fn a_control_declares_its_permissions_and_confirmation() {
    let page = parse_page(FILE, CONTROLS).expect("the page loads");
    let start = &page.objects[0];
    assert_eq!(start.role, Some(HmiRole::Engineer));
    assert_eq!(start.confirm.as_deref(), Some("Start feed pump P101?"));
    assert_eq!(start.pulse, Some(0.5));
    assert_eq!(start.expect, Some(HmiScalar::Bool(true)));
    assert_eq!(start.timeout, Some(8.0));
    assert_eq!(page.objects[1].action, Some(HmiButtonAction::Reset));
}

#[test]
fn a_control_is_written_back_byte_for_byte() {
    let page = parse_page(FILE, CONTROLS).expect("the page loads");
    assert_eq!(write_page(&page), CONTROLS);
}

#[test]
fn bad_control_keys_are_errors_on_their_line() {
    for (bad, words) in [
        ("role = \"viewer\"", "role"),
        ("pulse = 0.0", "pulse"),
        ("timeout = -1.0", "timeout"),
        ("action = \"flip\"", "action"),
        ("confirm = \"  \"", "confirm"),
    ] {
        // In place of the stop button's action, so no key appears twice.
        let text = CONTROLS.replace("action = \"reset\"", bad);
        let found = errors(&text);
        assert!(
            found
                .iter()
                .any(|error| error.line == line_of(&text, bad) && error.message.contains(words)),
            "{bad}: {found:#?}"
        );
    }
}

const CHOICES: &str = r#"format = 1
title = "Choices"

[[object]]
id = "speed"
type = "selector"
at = [40, 40]
size = [160, 40]
label = "Fan speed"
options = ["Off", "Low", "High"]

[object.bind]
value = { path = "Plant.Fan.Speed", direction = "read-write" }

[[object]]
id = "mode"
type = "mode-selector"
at = [40, 96]
size = [240, 40]
options = ["Manual", "Auto"]

[object.bind]
mode = { path = "Plant.Loop.Mode", direction = "read-write" }
"#;

#[test]
fn selectors_declare_their_options_and_are_written_back_byte_for_byte() {
    let page = parse_page(FILE, CHOICES).expect("the page loads");
    assert_eq!(
        page.objects[0].options.as_deref(),
        Some(&["Off".to_string(), "Low".to_string(), "High".to_string()][..])
    );
    assert_eq!(write_page(&page), CHOICES);
}

/// §5.8 item 5: a mode selector offers the modes of the enumeration it is bound to, as the
/// equipment's Out of service or another enumeration's own; the binding checks hold them against
/// the program, so the file names no fixed list.
#[test]
fn a_mode_selector_offers_the_modes_of_its_enumeration() {
    let text = CHOICES.replace(
        "options = [\"Manual\", \"Auto\"]",
        "options = [\"Manual\", \"Auto\", \"Cascade\"]",
    );
    let page =
        parse_page(FILE, &text).unwrap_or_else(|errors| panic!("the page loads: {errors:#?}"));
    assert_eq!(
        page.objects[1].options.as_deref(),
        Some(
            &[
                "Manual".to_string(),
                "Auto".to_string(),
                "Cascade".to_string()
            ][..]
        )
    );
}

/// §5.8 item 3: a switch may name its two states' words, off first, as a PID loop's Manual
/// switch reads Auto or Manual; any other number of words is an error on its line.
#[test]
fn a_switch_names_its_two_states_or_none() {
    let switch = "format = 1\ntitle = \"Loop\"\n\n[[object]]\nid = \"mode\"\ntype = \"switch\"\nat = [0, 0]\nsize = [320, 48]\noptions = [\"Auto\", \"Manual\"]\n\n[object.bind]\nvalue = { path = \"Main.TIC101_Manual\", direction = \"read-write\" }\n";
    let page =
        parse_page(FILE, switch).unwrap_or_else(|errors| panic!("the page loads: {errors:#?}"));
    assert_eq!(
        page.objects[0].options.as_deref(),
        Some(&["Auto".to_string(), "Manual".to_string()][..])
    );
    let three = switch.replace(
        "[\"Auto\", \"Manual\"]",
        "[\"Auto\", \"Manual\", \"Track\"]",
    );
    let found = errors(&three);
    assert!(
        found
            .iter()
            .any(|error| error.line == line_of(&three, "options = ")
                && error.message.contains("two states")),
        "{found:#?}"
    );
}

#[test]
fn bad_options_are_errors_on_their_line() {
    for (from, to, words) in [
        (
            "options = [\"Off\", \"Low\", \"High\"]",
            "options = []",
            "at least one",
        ),
        (
            "options = [\"Off\", \"Low\", \"High\"]",
            "options = [\"Low\", \"low\"]",
            "twice",
        ),
    ] {
        let text = CHOICES.replace(from, to);
        let found = errors(&text);
        assert!(
            found
                .iter()
                .any(|error| error.line == line_of(&text, to) && error.message.contains(words)),
            "{to}: {found:#?}"
        );
    }
    let text = CHOICES.replace("options = [\"Off\", \"Low\", \"High\"]\n", "");
    let found = errors(&text);
    assert!(
        found
            .iter()
            .any(|error| error.line == line_of(&text, "id = \"speed\"")
                && error.message.contains("needs options")),
        "a selector without options: {found:#?}"
    );
    let text = CONTROLS.replace("action = \"reset\"", "options = [\"A\"]");
    let found = errors(&text);
    assert!(
        found
            .iter()
            .any(|error| error.line == line_of(&text, "options = [\"A\"]")
                && error.message.contains("selectors")),
        "options on a button: {found:#?}"
    );
}
