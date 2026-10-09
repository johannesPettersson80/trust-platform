//! Specification 39, §6: a pipe's ends must be ports of objects of the page, each port taking
//! one pipe, and its flow binding a number or a boolean of the program.

use super::*;
use crate::harness::TestHarness;
use crate::hmi::page_files::parse_page;

const PROGRAM: &str = r#"
PROGRAM Plant
VAR
    FT101 : REAL;
    Flowing : BOOL;
    Name : STRING;
END_VAR
END_PROGRAM
"#;

const OBJECTS: &str = r#"
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
"#;

fn problems(pipes: &str) -> Vec<BindingProblem> {
    let mut harness = TestHarness::from_source(PROGRAM).expect("the program compiles");
    harness.cycle();
    let text = format!("format = 1\ntitle = \"Pipes\"\n{OBJECTS}{pipes}");
    let page = parse_page("hmi/pages/pipes.toml", &text).expect("the page loads");
    let metadata = harness.runtime().metadata_snapshot();
    check_page(
        &page,
        Catalogue::shipped(),
        &metadata,
        harness.runtime().storage(),
    )
}

fn found(problems: &[BindingProblem]) -> Vec<(String, Option<String>, String)> {
    problems
        .iter()
        .map(|problem| {
            (
                problem.object.clone(),
                problem.property.clone(),
                problem.message.clone(),
            )
        })
        .collect()
}

#[test]
fn a_pipe_between_ports_of_the_page_with_a_flow_binding_has_no_problem() {
    let pipes = "\n[[pipe]]\nfrom = \"p-101.outlet\"\nto = \"v-101.inlet\"\n\n[pipe.bind]\nflow = \"Plant.FT101\"\n\n[[pipe]]\nfrom = \"v-101.outlet\"\nto = \"p-101.inlet\"\n\n[pipe.bind]\nflow = \"Plant.Flowing\"\n";
    assert_eq!(problems(pipes), vec![]);
}

#[test]
fn a_pipe_to_an_object_or_port_the_page_does_not_have_or_to_a_taken_port_is_a_problem() {
    let pipes = "\n[[pipe]]\nfrom = \"p-101.outlet\"\nto = \"v-101.inlet\"\n\n[[pipe]]\nfrom = \"p-101.outlet\"\nto = \"x-9.inlet\"\n\n[[pipe]]\nfrom = \"v-101.shaft\"\nto = \"p-101.inlet\"\n";
    assert_eq!(
        found(&problems(pipes)),
        vec![
            (
                "pipe:1".to_string(),
                Some("from".to_string()),
                "the port p-101.outlet already has a pipe".to_string()
            ),
            (
                "pipe:1".to_string(),
                Some("to".to_string()),
                "the page has no object \"x-9\"".to_string()
            ),
            (
                "pipe:2".to_string(),
                Some("from".to_string()),
                "a valve has no port \"shaft\"; it has inlet and outlet".to_string()
            ),
        ]
    );
}

#[test]
fn a_pipes_flow_binding_is_a_number_or_a_boolean_of_the_program() {
    let pipes = "\n[[pipe]]\nfrom = \"p-101.outlet\"\nto = \"v-101.inlet\"\n\n[pipe.bind]\nflow = \"Plant.Name\"\n\n[[pipe]]\nfrom = \"v-101.outlet\"\nto = \"p-101.inlet\"\n\n[pipe.bind]\nflow = \"Plant.Gone\"\n";
    let found = found(&problems(pipes));
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(
        (found[0].0.as_str(), found[0].1.as_deref()),
        ("pipe:0", Some("flow"))
    );
    assert!(
        found[0]
            .2
            .contains("a pipe's flow needs a number or a boolean"),
        "{found:?}"
    );
    assert_eq!(
        (found[1].0.as_str(), found[1].1.as_deref()),
        ("pipe:1", Some("flow"))
    );
    assert!(
        found[1].2.contains("is not a variable of the program"),
        "{found:?}"
    );
}
