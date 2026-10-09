//! Specification 39, §6 item 5 and §15.2: an instrument's attachment to the equipment or the pipe
//! it measures, and a loop controller's signal lines, stored as references, written back
//! canonically, and checked at their lines.

use super::fixtures::FILE;
use super::*;

const PAGE: &str = r#"format = 1
title = "Instruments"

[[object]]
id = "p-101"
type = "pump"
at = [40, 40]
size = [128, 108]

[[object]]
id = "v-101"
type = "valve"
at = [400, 40]
size = [128, 108]

[[object]]
id = "tt-101"
type = "temperature-sensor"
at = [200, 200]
size = [176, 56]
attach = "p-101"

[[object]]
id = "ft-101"
type = "flow-sensor"
at = [200, 300]
size = [176, 56]
attach = "pipe:0"

[[pipe]]
from = "p-101.outlet"
to = "v-101.inlet"
"#;

fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .expect("the needle is in the text")
        + 1
}

#[test]
fn an_attachment_to_equipment_or_a_pipe_is_kept_and_written_back_byte_for_byte() {
    let page = parse_page(FILE, PAGE).expect("the page loads");
    assert_eq!(page.objects[2].attach.as_deref(), Some("p-101"));
    assert_eq!(page.objects[3].attach.as_deref(), Some("pipe:0"));
    assert_eq!(write_page(&page), PAGE);
}

#[test]
fn an_attachment_to_what_the_page_does_not_have_is_an_error_at_its_line() {
    for (attach, message) in [
        ("x-9", "attaches to `x-9`, which is not on this page"),
        ("tt-101", "an object cannot attach to itself"),
        ("pipe:4", "attaches to pipe 4, but the page has 1 pipe"),
        (
            "pipe:x",
            "attaches to `pipe:x`; a pipe is written `pipe:<n>`, counting from 0",
        ),
    ] {
        let text = PAGE.replace("attach = \"p-101\"", &format!("attach = \"{attach}\""));
        let errors = parse_page(FILE, &text).expect_err(attach);
        assert_eq!(errors.len(), 1, "{attach}: {errors:?}");
        assert_eq!(
            errors[0].line,
            line_of(&text, &format!("attach = \"{attach}\"")),
            "{attach}"
        );
        assert!(
            errors[0].message.contains(message),
            "{attach}: {}",
            errors[0].message
        );
    }
}

#[test]
fn a_problem_with_an_attachment_is_at_its_attach_line() {
    assert_eq!(
        problem_line(PAGE, "tt-101", Some("attach")),
        line_of(PAGE, "attach = \"p-101\"")
    );
}

#[test]
fn a_controllers_signal_lines_are_kept_written_back_and_checked_at_their_lines() {
    let text = PAGE.replace(
        "[[pipe]]",
        "[[object]]\nid = \"tic-101\"\ntype = \"controller\"\nat = [600, 200]\nsize = [256, 56]\nmeasures = \"tt-101\"\nacts_on = \"v-101\"\n\n[[pipe]]",
    );
    let page = parse_page(FILE, &text).expect("the page loads");
    let controller = page
        .objects
        .iter()
        .find(|object| object.id == "tic-101")
        .expect("the controller");
    assert_eq!(controller.measures.as_deref(), Some("tt-101"));
    assert_eq!(controller.acts_on.as_deref(), Some("v-101"));
    assert_eq!(write_page(&page), text);
    assert_eq!(
        problem_line(&text, "tic-101", Some("acts_on")),
        line_of(&text, "acts_on = \"v-101\"")
    );
    for (from, to, message) in [
        (
            "measures = \"tt-101\"",
            "measures = \"x-9\"",
            "`measures` names `x-9`, which is not on this page",
        ),
        (
            "acts_on = \"v-101\"",
            "acts_on = \"tic-101\"",
            "`acts_on` names the object itself",
        ),
    ] {
        let broken = text.replace(from, to);
        let errors = parse_page(FILE, &broken).expect_err(to);
        assert_eq!(errors.len(), 1, "{to}: {errors:?}");
        assert_eq!(errors[0].line, line_of(&broken, to), "{to}");
        assert!(
            errors[0].message.contains(message),
            "{to}: {}",
            errors[0].message
        );
    }
}
