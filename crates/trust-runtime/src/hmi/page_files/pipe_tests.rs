//! Specification 39, §6 and §15.2: pipes in page files: their ends, style, manual bends and flow
//! binding, written back canonically, and their problems found at their lines.

use std::collections::BTreeMap;

use super::fixtures::FILE;
use super::*;

const PIPES: &str = r#"format = 1
title = "Pipes"

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

[[pipe]]
from = "p-101.outlet"
to = "v-101.inlet"
style = "signal"
waypoints = [[300, 60], [300, 120]]

[pipe.bind]
flow = "Plant.FT101"

[[pipe]]
from = "v-101.outlet"
to = "p-101.inlet"
"#;

fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .expect("the needle is in the text")
        + 1
}

#[test]
fn a_pipe_keeps_its_style_bends_and_flow_binding_and_is_written_back_byte_for_byte() {
    let page = parse_page(FILE, PIPES).expect("the page loads");
    assert_eq!(
        page.pipes[0],
        HmiPipe {
            from: "p-101.outlet".to_string(),
            to: "v-101.inlet".to_string(),
            style: HmiPipeStyle::Signal,
            waypoints: vec![[300, 60], [300, 120]],
            bind: BTreeMap::from([(
                "flow".to_string(),
                HmiBinding {
                    path: "Plant.FT101".to_string(),
                    quality: None,
                    unit: None,
                    range: None,
                    direction: HmiBindingDirection::Read,
                },
            )]),
        }
    );
    assert_eq!(
        page.pipes[1].style,
        HmiPipeStyle::Process,
        "a process line by default"
    );
    assert!(page.pipes[1].waypoints.is_empty() && page.pipes[1].bind.is_empty());
    assert_eq!(write_page(&page), PIPES);
}

#[test]
fn a_pipe_end_that_names_no_port_is_reported_on_its_line() {
    let text = PIPES.replace("from = \"v-101.outlet\"", "from = \"v-101\"");
    let errors = parse_page(FILE, &text).expect_err("the page is rejected");
    assert_eq!(
        errors[0].line,
        line_of(&text, "from = \"v-101\""),
        "{errors:?}"
    );
    assert!(errors[0].message.contains("object.port"), "{errors:?}");
}

#[test]
fn a_pipes_problem_is_found_at_its_end_or_its_binding() {
    assert_eq!(
        problem_line(PIPES, "pipe:0", Some("flow")),
        line_of(PIPES, "flow = ")
    );
    assert_eq!(
        problem_line(PIPES, "pipe:1", None),
        line_of(PIPES, "from = \"v-101.outlet\"")
    );
    assert_eq!(
        problem_line(PIPES, "pipe:1", Some("to")),
        line_of(PIPES, "to = \"p-101.inlet\"")
    );
}
