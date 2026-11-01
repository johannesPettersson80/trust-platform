//! Deleting an element, clearing a rung's wires and the text and choice fields of the properties
//! panel (specification 16 §3 to §5, specification 25 §11.4). A coil's power is worked out by
//! hand from specification 16 §4: a node's input is the OR of what its wires bring, a contact
//! passes its input AND its operand, and a node no wire feeds hangs on the left rail (`TRUE`).

use super::json;
use crate::ladder::edit;
use crate::ladder::power::incoming_power;
use crate::Json;

/// The power reaching the node `id` of the rung `rung`, the operand of a contact its term.
fn power_of(program: &Json, rung: usize, id: &str) -> String {
    let network = &program
        .get("networks")
        .and_then(Json::as_array)
        .expect("networks")[rung];
    let term = |node: &Json| node.str_at("variable").unwrap_or_default().to_owned();
    incoming_power(network, &term)
        .into_iter()
        .find(|(node, _)| node == id)
        .unwrap_or_else(|| panic!("node {id}"))
        .1
}

fn ids<'a>(program: &'a Json, member: &str, key: &str) -> Vec<&'a str> {
    let network = &program
        .get("networks")
        .and_then(Json::as_array)
        .expect("networks")[0];
    network
        .get(member)
        .and_then(Json::as_array)
        .expect("items")
        .iter()
        .map(|item| item.str_at(key).unwrap_or_default())
        .collect()
}

fn wires(program: &Json) -> Vec<(&str, &str)> {
    let network = &program
        .get("networks")
        .and_then(Json::as_array)
        .expect("networks")[0];
    network
        .get("edges")
        .and_then(Json::as_array)
        .expect("edges")
        .iter()
        .map(|wire| {
            (
                wire.str_at("fromNodeId").unwrap_or_default(),
                wire.str_at("toNodeId").unwrap_or_default(),
            )
        })
        .collect()
}

fn variable_names(program: &Json) -> Vec<&str> {
    program
        .get("variables")
        .and_then(Json::as_array)
        .expect("variables")
        .iter()
        .map(|variable| variable.str_at("name").unwrap_or_default())
        .collect()
}

/// One rung: contacts `A` and `X` in series feeding the coil `Q`.
fn series() -> Json {
    json(
        r#"{"schemaVersion":2,"metadata":{"name":"s","description":"s"},
          "variables":[{"name":"A","type":"BOOL"},{"name":"X","type":"BOOL"},{"name":"Q","type":"BOOL"}],
          "networks":[{"id":"rung_1","order":0,"layout":{"y":100},"nodes":[
            {"id":"a","type":"contact","contactType":"NO","variable":"A","position":{"x":100,"y":100}},
            {"id":"x","type":"contact","contactType":"NO","variable":"X","position":{"x":200,"y":100}},
            {"id":"q","type":"coil","coilType":"NORMAL","variable":"Q","position":{"x":300,"y":100}}],
          "edges":[{"id":"edge_rung_1_0","fromNodeId":"a","toNodeId":"x"},{"id":"edge_rung_1_1","fromNodeId":"x","toNodeId":"q"}]}]}"#,
    )
}

/// One rung: contacts `A` and `X` in parallel between a split and a merge, feeding the coil `Q`.
fn parallel() -> Json {
    json(
        r#"{"schemaVersion":2,"metadata":{"name":"p","description":"p"},
          "variables":[{"name":"A","type":"BOOL"},{"name":"X","type":"BOOL"},{"name":"Q","type":"BOOL"}],
          "networks":[{"id":"rung_1","order":0,"layout":{"y":100},"nodes":[
            {"id":"s","type":"branchSplit","position":{"x":100,"y":100}},
            {"id":"a","type":"contact","contactType":"NO","variable":"A","position":{"x":200,"y":100}},
            {"id":"x","type":"contact","contactType":"NO","variable":"X","position":{"x":200,"y":160}},
            {"id":"m","type":"branchMerge","position":{"x":300,"y":100}},
            {"id":"q","type":"coil","coilType":"NORMAL","variable":"Q","position":{"x":400,"y":100}}],
          "edges":[{"id":"edge_rung_1_0","fromNodeId":"s","toNodeId":"a"},{"id":"edge_rung_1_1","fromNodeId":"s","toNodeId":"x"},
                   {"id":"edge_rung_1_2","fromNodeId":"a","toNodeId":"m"},{"id":"edge_rung_1_3","fromNodeId":"x","toNodeId":"m"},
                   {"id":"edge_rung_1_4","fromNodeId":"m","toNodeId":"q"}]}]}"#,
    )
}

/// A series element's wires go with it and its neighbours are wired together, so the coil keeps
/// the other contact's condition: `A AND X` becomes `A`. Catches the wire left to the deleted
/// contact (the coil would get `FALSE`: it is fed by a node that is gone) and a deletion that
/// drops the coil's only input (it would hang on the left rail, `TRUE`, bypassing `A` too).
#[test]
fn deleting_a_series_contact_closes_the_gap() {
    let mut program = series();
    assert_eq!(power_of(&program, 0, "q"), "A AND X");
    edit::remove_element(&mut program, 0, 1).expect("remove x");
    assert_eq!(ids(&program, "nodes", "id"), ["a", "q"]);
    assert_eq!(wires(&program), [("a", "q")]);
    assert_eq!(ids(&program, "edges", "id"), ["edge_rung_1_0"]);
    assert_eq!(power_of(&program, 0, "q"), "A");
}

/// An element that was a parallel leg on its own leaves the other legs: `A OR X` becomes `A`.
/// Catches a wire from the split to the merge in its place, which would make the coil `TRUE`.
#[test]
fn deleting_a_contact_that_was_a_parallel_leg_keeps_the_other_legs() {
    let mut program = parallel();
    assert_eq!(power_of(&program, 0, "q"), "A OR X");
    edit::remove_element(&mut program, 0, 2).expect("remove x");
    assert_eq!(wires(&program), [("s", "a"), ("a", "m"), ("m", "q")]);
    assert_eq!(power_of(&program, 0, "q"), "A");
}

/// A rung without wires is a series chain from left to right, so a deleted element simply
/// leaves the chain; the first element hangs on the left rail. Catches wires added to a rung
/// that had none.
#[test]
fn deleting_from_a_rung_without_wires_keeps_the_chain() {
    let mut program = series();
    edit::clear_wiring(&mut program, 0).expect("clear");
    edit::remove_element(&mut program, 0, 0).expect("remove a");
    assert_eq!(wires(&program), Vec::<(&str, &str)>::new());
    assert_eq!(power_of(&program, 0, "q"), "X");
}

/// The implicit local of a deleted contact's operand goes with it (specification 16 §5), while a
/// declared variable stays. Catches the local left behind.
#[test]
fn deleting_an_element_removes_the_implicit_local_no_element_names() {
    let mut program = series();
    edit::set_field(&mut program, 0, 0, "variable", "Start").expect("operand");
    assert!(variable_names(&program).contains(&"Start"));
    edit::remove_element(&mut program, 0, 0).expect("remove a");
    assert_eq!(variable_names(&program), ["A", "X", "Q"]);
}

/// Clear wiring removes every wire; the rung runs as a series chain from left to right: the
/// parallel `A OR X` becomes `A AND X`. Catches a wire kept.
#[test]
fn clearing_a_rung_s_wiring_leaves_a_series_chain() {
    let mut program = parallel();
    edit::clear_wiring(&mut program, 0).expect("clear");
    assert_eq!(wires(&program), Vec::<(&str, &str)>::new());
    assert_eq!(power_of(&program, 0, "q"), "A AND X");
}

/// A choice field takes one of its values and refuses any other with them named, storing
/// nothing; a field another kind has is refused. Catches a coil type the schema refuses
/// written into the file, which would then no longer load.
#[test]
fn a_choice_field_takes_only_its_values() {
    let mut program = series();
    edit::set_field(&mut program, 0, 2, "coilType", "SET").expect("SET");
    assert_eq!(ids(&program, "nodes", "coilType")[2], "SET");
    assert_eq!(
        edit::set_field(&mut program, 0, 2, "coilType", "TOGGLE"),
        Err("coilType must be one of NORMAL, SET, RESET, NEGATED.".to_owned())
    );
    assert_eq!(ids(&program, "nodes", "coilType")[2], "SET");
    assert_eq!(
        edit::set_field(&mut program, 0, 0, "coilType", "SET"),
        Err("A contact has no field \"coilType\".".to_owned())
    );
    assert_eq!(
        edit::set_field(&mut program, 0, 7, "variable", "A"),
        Err("Ladder rung 0 has no element 7.".to_owned())
    );
}

/// A contact's operand typed letter by letter declares the last name only: each change
/// reconciles the implicit locals (specification 16 §5). Catches the names typed on the way
/// (`S`, `St`) left as locals.
#[test]
fn an_operand_typed_into_the_panel_declares_its_symbol() {
    let mut program = series();
    for text in ["S", "St", "Start"] {
        edit::set_field(&mut program, 0, 0, "variable", text).expect("operand");
    }
    assert_eq!(variable_names(&program), ["A", "X", "Q", "Start"]);
}

/// Toggling a contact turns NO into NC and back; a coil is refused. Catches a toggle that only
/// ever sets NC.
#[test]
fn toggling_a_contact_turns_no_into_nc_and_back() {
    let mut program = series();
    assert_eq!(edit::toggle_contact_type(&mut program, 0, 0), Ok("NC"));
    assert_eq!(edit::toggle_contact_type(&mut program, 0, 0), Ok("NO"));
    assert_eq!(
        edit::toggle_contact_type(&mut program, 0, 2),
        Err("Ladder rung 0 element 2 is not a contact.".to_owned())
    );
}
