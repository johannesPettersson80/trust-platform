//! Specification 39, §7.1–7.2: the variable tree of a compiled program.

use super::*;
use crate::harness::TestHarness;
use crate::hmi::member_paths::{resolve_member, MemberPath};

const PROGRAM: &str = r#"
TYPE Reading :
STRUCT
    Value : REAL;
    Valid : BOOL;
END_STRUCT
END_TYPE

FUNCTION_BLOCK Pump
VAR_INPUT
    StartCmd : BOOL;
    Reset : BOOL R_EDGE;
END_VAR
VAR_OUTPUT
    Running : BOOL;
END_VAR
VAR_IN_OUT
    Demand : REAL;
END_VAR
VAR
    Starts : DINT;
END_VAR
    Running := StartCmd;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    P101 : Pump;
    Pumps : ARRAY[1..3] OF Pump;
    Level : Reading;
    Grid : ARRAY[1..2, 0..3] OF INT;
    Count : DINT;
    Need : REAL;
END_VAR
    P101(StartCmd := TRUE, Demand := Need);
END_PROGRAM
"#;

fn tree_of(sources: &[&str]) -> (TestHarness, Vec<VariableNode>) {
    let mut harness = TestHarness::from_sources(sources).expect("the program compiles");
    harness.cycle();
    let metadata = harness.runtime().metadata_snapshot();
    let tree = variable_tree(&metadata, harness.runtime().storage()).expect("tree");
    (harness, tree)
}

fn child<'a>(node: &'a VariableNode, name: &str) -> &'a VariableNode {
    node.children
        .iter()
        .find(|child| child.name == name)
        .unwrap_or_else(|| panic!("{} has no child {name}", node.path))
}

fn names(node: &VariableNode) -> Vec<&str> {
    node.children
        .iter()
        .map(|child| child.name.as_str())
        .collect()
}

#[test]
fn program_instances_are_roots_with_members_in_declaration_order() {
    let (_harness, tree) = tree_of(&[PROGRAM]);
    let main = tree.iter().find(|node| node.name == "Main").expect("Main");
    assert_eq!(main.kind, VariableKind::Program);
    assert_eq!(main.path, "Main");
    assert_eq!(
        names(main),
        ["P101", "Pumps", "Level", "Grid", "Count", "Need"]
    );
    let count = child(main, "Count");
    assert_eq!(
        (count.kind, count.type_name.as_str()),
        (VariableKind::Value, "DINT")
    );
    assert_eq!(count.path, "Main.Count");
    assert_eq!(count.access, None);
}

#[test]
fn function_block_members_carry_their_declaration() {
    let (_harness, tree) = tree_of(&[PROGRAM]);
    let p101 = child(&tree[0], "P101");
    assert_eq!(
        (p101.kind, p101.type_name.as_str()),
        (VariableKind::FunctionBlock, "Pump")
    );
    let access = |name: &str| child(p101, name).access;
    assert_eq!(access("StartCmd"), Some(VariableAccess::Input));
    assert_eq!(access("Running"), Some(VariableAccess::Output));
    assert_eq!(access("Demand"), Some(VariableAccess::InOut));
    assert_eq!(access("Starts"), Some(VariableAccess::Internal));
    assert_eq!(child(p101, "Running").type_name, "BOOL");
    assert_eq!(child(p101, "Running").path, "Main.P101.Running");
}

#[test]
fn internal_runtime_storage_is_not_listed() {
    let (_harness, tree) = tree_of(&[PROGRAM]);
    let p101 = child(&tree[0], "P101");
    assert_eq!(
        names(p101),
        ["StartCmd", "Reset", "Running", "Demand", "Starts"]
    );
}

#[test]
fn structures_list_their_fields() {
    let (_harness, tree) = tree_of(&[PROGRAM]);
    let level = child(&tree[0], "Level");
    assert_eq!(
        (level.kind, level.type_name.as_str()),
        (VariableKind::Structure, "Reading")
    );
    assert_eq!(names(level), ["Value", "Valid"]);
    assert_eq!(child(level, "Value").type_name, "REAL");
    assert_eq!(child(level, "Value").path, "Main.Level.Value");
}

#[test]
fn arrays_appear_once_with_bounds_and_an_element_template() {
    let (_harness, tree) = tree_of(&[PROGRAM]);
    let pumps = child(&tree[0], "Pumps");
    assert_eq!(pumps.kind, VariableKind::Array);
    assert_eq!(pumps.type_name, "ARRAY[1..3] OF Pump");
    assert!(pumps.children.is_empty());
    let shape = pumps.array.as_ref().expect("array shape");
    assert_eq!(shape.dimensions, vec![[1, 3]]);
    assert_eq!(shape.element.path, "Main.Pumps[1]");
    assert_eq!(shape.element.kind, VariableKind::FunctionBlock);
    assert_eq!(
        child(&shape.element, "Running").path,
        "Main.Pumps[1].Running"
    );

    let grid = child(&tree[0], "Grid");
    assert_eq!(grid.type_name, "ARRAY[1..2, 0..3] OF INT");
    let shape = grid.array.as_ref().expect("array shape");
    assert_eq!(shape.dimensions, vec![[1, 2], [0, 3]]);
    assert_eq!(shape.element.path, "Main.Grid[1,0]");
    assert_eq!(shape.element.type_name, "INT");
}

#[test]
fn every_path_in_the_tree_resolves() {
    let control = include_str!("../../../../../libraries/control/src/control.st");
    let loop_program = r#"
PROGRAM Loop
VAR
    Controller : Control.PID;
END_VAR
END_PROGRAM
"#;
    let (harness, tree) = tree_of(&[PROGRAM, control, loop_program]);
    let mut paths = Vec::new();
    fn collect(node: &VariableNode, paths: &mut Vec<String>) {
        paths.push(node.path.clone());
        for child in &node.children {
            collect(child, paths);
        }
        if let Some(shape) = &node.array {
            collect(&shape.element, paths);
        }
    }
    for root in &tree {
        collect(root, &mut paths);
    }
    assert!(paths
        .iter()
        .any(|path| path == "Loop.Controller.ParametersValue.ProportionalGain"));
    for path in &paths {
        let parsed = MemberPath::parse(path).unwrap_or_else(|error| panic!("{path}: {error}"));
        assert!(
            resolve_member(&parsed, harness.runtime().storage()).is_some(),
            "{path} does not resolve"
        );
    }
}

#[test]
fn the_tree_serializes_with_the_documented_field_names() {
    let (_harness, tree) = tree_of(&[PROGRAM]);
    let json = serde_json::to_value(child(&tree[0], "Pumps")).expect("json");
    assert_eq!(json["type"], "ARRAY[1..3] OF Pump");
    assert_eq!(json["kind"], "array");
    assert_eq!(json["array"]["dimensions"], serde_json::json!([[1, 3]]));
    assert_eq!(json["array"]["element"]["kind"], "function-block");
    assert_eq!(
        json["array"]["element"]["children"][0]["access"],
        serde_json::json!("input")
    );
}

const WRITES: &str = r#"
TYPE Reading :
STRUCT
    Value : REAL;
    Valid : BOOL;
END_STRUCT
END_TYPE

FUNCTION_BLOCK Valve
VAR_INPUT
    Open : BOOL;
END_VAR
VAR_OUTPUT
    Feedback : Reading;
END_VAR
VAR CONSTANT
    Stroke : REAL := 2.0;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Plant
VAR
    V1 : Valve;
    Setpoint : REAL;
END_VAR
VAR CONSTANT
    MaxFlow : REAL := 40.0;
END_VAR
    V1(Open := TRUE);
END_PROGRAM
"#;

fn root<'a>(tree: &'a [VariableNode], name: &str) -> &'a VariableNode {
    tree.iter()
        .find(|node| node.name == name)
        .unwrap_or_else(|| panic!("no root {name}"))
}

fn every_node(tree: &[VariableNode]) -> Vec<&VariableNode> {
    fn walk<'a>(node: &'a VariableNode, nodes: &mut Vec<&'a VariableNode>) {
        nodes.push(node);
        node.children.iter().for_each(|child| walk(child, nodes));
        if let Some(shape) = &node.array {
            walk(&shape.element, nodes);
        }
    }
    let mut nodes = Vec::new();
    tree.iter().for_each(|node| walk(node, &mut nodes));
    nodes
}

#[test]
fn each_variable_says_which_kinds_of_value_it_holds() {
    use crate::hmi::catalogue::ValueKind as Kind;
    let (_harness, tree) = tree_of(&[PROGRAM]);
    let main = root(&tree, "Main");
    let p101 = child(main, "P101");
    assert_eq!(child(main, "Count").kinds, [Kind::Number, Kind::Value]);
    assert_eq!(child(p101, "Running").kinds, [Kind::Boolean, Kind::Value]);
    assert_eq!(child(main, "Level").kinds, [Kind::Structure]);
    assert_eq!(child(main, "Grid").kinds, [Kind::Array]);
    assert!(p101.kinds.is_empty(), "{:?}", p101.kinds);
    assert!(main.kinds.is_empty(), "{:?}", main.kinds);
    let json = serde_json::to_value(child(main, "Count")).expect("json");
    assert_eq!(json["kinds"], serde_json::json!(["number", "value"]));
    assert_eq!(json["writable"], serde_json::json!(true));
}

#[test]
fn each_variable_says_whether_a_binding_may_write_it() {
    let (_harness, tree) = tree_of(&[WRITES]);
    let plant = root(&tree, "Plant");
    let valve = child(plant, "V1");
    let writable = |node: &VariableNode| (node.path.clone(), node.writable);
    assert_eq!(
        writable(child(plant, "Setpoint")),
        ("Plant.Setpoint".into(), true)
    );
    assert_eq!(
        writable(child(plant, "MaxFlow")),
        ("Plant.MaxFlow".into(), false)
    );
    assert_eq!(
        writable(child(valve, "Open")),
        ("Plant.V1.Open".into(), true)
    );
    assert_eq!(
        writable(child(valve, "Feedback")),
        ("Plant.V1.Feedback".into(), false)
    );
    let feedback_value = child(child(valve, "Feedback"), "Value");
    assert_eq!(
        writable(feedback_value),
        ("Plant.V1.Feedback.Value".into(), false)
    );
    assert_eq!(
        writable(child(valve, "Stroke")),
        ("Plant.V1.Stroke".into(), false)
    );

    // The reason, in the binding checks' words, for the picker to say why (§7.1).
    let block = |node: &VariableNode| node.write_block.clone();
    assert_eq!(block(child(plant, "Setpoint")), None);
    assert_eq!(block(child(plant, "MaxFlow")), Some("is a constant".into()));
    assert_eq!(
        block(child(valve, "Feedback")),
        Some("is an output of Valve".into())
    );
    assert_eq!(block(feedback_value), Some("is an output of Valve".into()));
    let json = serde_json::to_value(child(plant, "MaxFlow")).expect("json");
    assert_eq!(json["writeBlock"], serde_json::json!("is a constant"));
    let json = serde_json::to_value(child(plant, "Setpoint")).expect("json");
    assert!(json.get("writeBlock").is_none(), "{json}");
}

/// Specification 39, §7.1: the picker never offers a variable the binding checks would refuse.
#[test]
fn a_write_target_is_offered_exactly_when_the_binding_checks_accept_it() {
    use crate::hmi::binding_checks::check_page;
    use crate::hmi::catalogue::{Catalogue, ValueKind};
    use crate::hmi::page_files::parse_page;
    for sources in [&[PROGRAM][..], &[WRITES][..]] {
        let (harness, tree) = tree_of(sources);
        let metadata = harness.runtime().metadata_snapshot();
        let mut compared = 0;
        for node in every_node(&tree) {
            let component = if node.kinds.contains(&ValueKind::Number) {
                "numeric-input"
            } else if node.kinds.contains(&ValueKind::Boolean) {
                "switch"
            } else {
                continue;
            };
            let text = format!(
                "format = 1\ntitle = \"Writes\"\n\n[[object]]\nid = \"w\"\ntype = \"{component}\"\nat = [0, 0]\nsize = [80, 40]\n\n[object.bind]\nvalue = {{ path = \"{}\", direction = \"read-write\" }}\n",
                node.path
            );
            let page = parse_page("hmi/pages/writes.toml", &text).expect("the page loads");
            let problems = check_page(
                &page,
                Catalogue::shipped(),
                &metadata,
                harness.runtime().storage(),
            );
            assert_eq!(
                node.writable,
                problems.is_empty(),
                "{}: writable {} but the checks say {problems:?}",
                node.path,
                node.writable
            );
            compared += 1;
        }
        // WRITES has six number and boolean variables, PROGRAM fifteen.
        assert!(compared >= 6, "only {compared} variables compared");
    }
}

/// Specification 39, §7.1 and §7.6: the picker does not offer a global constant for a write.
#[test]
fn a_global_constant_cannot_be_written() {
    let source = "VAR_GLOBAL CONSTANT\n    Limit : REAL := 5.0;\nEND_VAR\nVAR_GLOBAL\n    Speed : REAL;\nEND_VAR\n\nPROGRAM Main\nVAR\n    X : REAL;\nEND_VAR\n    X := Limit + Speed;\nEND_PROGRAM\n";
    let (_harness, tree) = tree_of(&[source]);
    let limit = root(&tree, "Limit");
    assert_eq!(
        (limit.writable, limit.write_block.as_deref()),
        (false, Some("is a constant"))
    );
    assert!(root(&tree, "Speed").writable);
}

/// Specification 39, §7.1: an instance of a block that extends another lists the members it
/// inherits with their own declaration, so the picker offers an alarm's outputs.
#[test]
fn inherited_members_are_listed_with_their_declaration() {
    let source = r#"
FUNCTION_BLOCK Base
VAR_INPUT
    Gain : INT;
END_VAR
VAR_OUTPUT
    Result : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS Base
VAR_INPUT
    Value : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Block : Derived;
END_VAR
    Block();
END_PROGRAM
"#;
    let (_, tree) = tree_of(&[source]);
    let block = child(&tree[0], "Block");
    let listed = names(block);
    assert!(
        listed.contains(&"Value") && listed.contains(&"Gain") && listed.contains(&"Result"),
        "{listed:?}"
    );
    assert_eq!(child(block, "Result").access, Some(VariableAccess::Output));
    assert_eq!(child(block, "Gain").path, "Main.Block.Gain");
    assert!(
        !child(block, "Result").writable,
        "an output of the base cannot be written"
    );
}
