//! Specification 39, §6 item 5: only an instrument attaches, and only to equipment or a pipe; only a
//! loop controller has signal lines, from an instrument and to equipment.

use super::*;
use crate::harness::CompileSession;
use crate::hmi::page_files::parse_page;

const OBJECTS: &str = r#"
[[object]]
id = "p-101"
type = "pump"
at = [40, 40]
size = [128, 108]

[[object]]
id = "note"
type = "text"
at = [40, 400]
size = [160, 32]
"#;

fn problems(objects: &str) -> Vec<(String, Option<String>, String)> {
    let declarations =
        CompileSession::from_source("PROGRAM Plant\nVAR\n    T : REAL;\nEND_VAR\nEND_PROGRAM\n")
            .declarations()
            .expect("the program compiles");
    let text = format!("format = 1\ntitle = \"Instruments\"\n{OBJECTS}{objects}");
    let page = parse_page("hmi/pages/instruments.toml", &text).expect("the page loads");
    let metadata = declarations.metadata().clone();
    check_page(
        &page,
        Catalogue::shipped(),
        &metadata,
        declarations.storage(),
    )
    .into_iter()
    .map(|problem| (problem.object, problem.property, problem.message))
    .collect()
}

#[test]
fn an_instrument_attached_to_equipment_has_no_problem() {
    assert_eq!(problems("\n[[object]]\nid = \"tt-101\"\ntype = \"temperature-sensor\"\nat = [200, 40]\nsize = [176, 56]\nattach = \"p-101\"\n"), vec![]);
}

#[test]
fn only_an_instrument_attaches_and_only_to_equipment() {
    let value = problems("\n[[object]]\nid = \"v\"\ntype = \"value\"\nat = [200, 40]\nsize = [160, 48]\nattach = \"p-101\"\n");
    assert_eq!(
        value,
        vec![(
            "v".to_string(),
            Some("attach".to_string()),
            "a value does not attach to equipment; instruments do".to_string()
        )]
    );
    let to_text = problems("\n[[object]]\nid = \"tt-101\"\ntype = \"temperature-sensor\"\nat = [200, 40]\nsize = [176, 56]\nattach = \"note\"\n");
    assert_eq!(
        to_text,
        vec![(
            "tt-101".to_string(),
            Some("attach".to_string()),
            "an instrument attaches to equipment or a pipe, not to a text".to_string()
        )]
    );
    let to_instrument = problems("\n[[object]]\nid = \"tt-101\"\ntype = \"temperature-sensor\"\nat = [200, 40]\nsize = [176, 56]\n\n[[object]]\nid = \"tt-102\"\ntype = \"temperature-sensor\"\nat = [200, 140]\nsize = [176, 56]\nattach = \"tt-101\"\n");
    assert_eq!(
        to_instrument,
        vec![(
            "tt-102".to_string(),
            Some("attach".to_string()),
            "an instrument attaches to equipment or a pipe, not to a temperature sensor"
                .to_string()
        )]
    );
}

const TRANSMITTER: &str =
    "\n[[object]]\nid = \"tt-101\"\ntype = \"temperature-sensor\"\nat = [200, 40]\nsize = [176, 56]\n";
const CONTROLLER: &str =
    "\n[[object]]\nid = \"tic\"\ntype = \"controller\"\nat = [400, 200]\nsize = [256, 56]\n";

#[test]
fn a_loop_controller_measures_an_instrument_and_acts_on_equipment() {
    assert_eq!(
        problems(&format!(
            "{TRANSMITTER}{CONTROLLER}measures = \"tt-101\"\nacts_on = \"p-101\"\n"
        )),
        vec![]
    );
    assert_eq!(
        problems(&format!(
            "{TRANSMITTER}{CONTROLLER}measures = \"p-101\"\nacts_on = \"tt-101\"\n"
        )),
        vec![
            (
                "tic".to_string(),
                Some("measures".to_string()),
                "a controller measures an instrument, not a pump".to_string()
            ),
            (
                "tic".to_string(),
                Some("acts_on".to_string()),
                "a controller acts on equipment, not a temperature sensor".to_string()
            ),
        ]
    );
    assert_eq!(
        problems("\n[[object]]\nid = \"v\"\ntype = \"value\"\nat = [200, 40]\nsize = [160, 48]\nacts_on = \"p-101\"\n"),
        vec![(
            "v".to_string(),
            Some("acts_on".to_string()),
            "a value has no signal lines; a loop controller has".to_string()
        )]
    );
}
