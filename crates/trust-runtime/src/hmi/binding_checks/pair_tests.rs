//! Specification 39, §8.6: a pair's link check: each field of its faceplate needs a member of the
//! block, by the pair's map or by its own name, holding a kind of value the field takes; the block
//! is a function block or a program of the compiled program.

use super::*;
use crate::harness::CompileSession;
use crate::hmi::page_files::{parse_faceplate, parse_pair};

const PROGRAM: &str = r#"
TYPE Choice : (Stopped, Started); END_TYPE

FUNCTION_BLOCK Pump
VAR_INPUT
    StartCmd : BOOL;
    Selected : Choice;
END_VAR
VAR_OUTPUT
    Running : BOOL;
    ActualSpeed : REAL;
END_VAR
    Running := StartCmd;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    P101 : Pump;
    Setpoint : LREAL;
END_VAR
    P101();
END_PROGRAM
"#;

const FACEPLATE: &str = r#"format = 1
title = "Pump"
size = [400, 300]

[fields.Running]
accepts = ["boolean"]

[fields.Speed]
accepts = ["number"]

[fields.StartCmd]

[fields.Missing]
accepts = ["number"]
"#;

fn problems(pair: &str, faceplate: &str) -> Vec<(Option<String>, String)> {
    problems_in(PROGRAM, pair, faceplate)
}

fn problems_in(program: &str, pair: &str, faceplate: &str) -> Vec<(Option<String>, String)> {
    let declarations = CompileSession::from_source(program)
        .declarations()
        .expect("the program compiles");
    let pair = parse_pair("hmi/pairs/pump.toml", pair).expect("the pair loads");
    let faceplate =
        parse_faceplate("hmi/panels/pump.toml", faceplate).expect("the faceplate loads");
    check_pair(&pair, &faceplate, &declarations.metadata().clone())
        .into_iter()
        .map(|problem| (problem.field, problem.message))
        .collect()
}

const PAIR: &str = "format = 1\nblock = \"Pump\"\nfaceplate = \"pump\"\nsymbol = \"pump\"\n\n[fields]\nSpeed = \"ActualSpeed\"\n";

#[test]
fn a_field_may_name_an_element_of_an_array_member_within_its_bounds() {
    let program = PROGRAM.replace(
        "    ActualSpeed : REAL;\n",
        "    ActualSpeed : REAL;\n    Selected2 : ARRAY[1..4] OF BOOL;\n    Grid : ARRAY[1..2, 0..3] OF REAL;\n",
    );
    let faceplate = "format = 1\ntitle = \"Pump\"\nsize = [400, 300]\n\n[fields.First]\naccepts = [\"boolean\"]\n\n[fields.Cell]\naccepts = [\"number\"]\n";
    let pair = |first: &str, cell: &str| {
        format!("format = 1\nblock = \"Pump\"\nfaceplate = \"pump\"\nsymbol = \"pump\"\n\n[fields]\nFirst = \"{first}\"\nCell = \"{cell}\"\n")
    };
    assert_eq!(
        problems_in(&program, &pair("Selected2[1]", "Grid[2, 3]"), faceplate),
        vec![]
    );
    assert_eq!(
        problems_in(&program, &pair("Selected2[5]", "Grid[2]"), faceplate),
        vec![
            (
                Some("Cell".to_string()),
                "`Pump` has no member `Grid[2]`, which the faceplate's field `Cell` needs"
                    .to_string()
            ),
            (
                Some("First".to_string()),
                "`Pump` has no member `Selected2[5]`, which the faceplate's field `First` needs"
                    .to_string()
            ),
        ],
        "an index outside the bounds, or too few indices, names no element"
    );
}

#[test]
fn a_field_whose_member_the_block_does_not_have_is_a_problem_of_that_field() {
    assert_eq!(
        problems(PAIR, FACEPLATE),
        vec![(
            Some("Missing".to_string()),
            "`Pump` has no member `Missing`, which the faceplate's field `Missing` needs"
                .to_string()
        )]
    );
    let fine = FACEPLATE.replace("[fields.Missing]\naccepts = [\"number\"]\n", "");
    assert_eq!(
        problems(PAIR, &fine),
        vec![],
        "by the map and by their own names"
    );
}

#[test]
fn a_member_holding_a_kind_the_field_does_not_take_is_a_problem() {
    let faceplate = FACEPLATE.replace(
        "[fields.Missing]\naccepts = [\"number\"]\n",
        "[fields.Selected]\naccepts = [\"number\"]\n",
    );
    assert_eq!(
        problems(PAIR, &faceplate),
        vec![(Some("Selected".to_string()), "the member `Selected` of `Pump` holds an enumeration; the field `Selected` takes a number".to_string())]
    );
}

#[test]
fn a_block_the_program_does_not_have_is_a_problem_of_the_pair_and_a_program_can_be_the_block() {
    let missing = PAIR.replace("block = \"Pump\"", "block = \"Equipment.Pump\"");
    assert_eq!(
        problems(&missing, FACEPLATE),
        vec![(
            None,
            "the program has no function block or program `Equipment.Pump`".to_string()
        )]
    );
    let program = "format = 1\nblock = \"Main\"\nfaceplate = \"loop\"\nsymbol = \"value\"\n";
    let faceplate = "format = 1\ntitle = \"Loop\"\nsize = [400, 300]\n\n[fields.Setpoint]\naccepts = [\"number\"]\n";
    assert_eq!(
        problems(program, faceplate),
        vec![],
        "the program's own variables"
    );
}

#[test]
fn a_block_in_a_namespace_is_named_with_it_or_by_its_own_name() {
    let program = "NAMESPACE Lib\nFUNCTION_BLOCK Valve\nVAR_OUTPUT\n    Open : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\nEND_NAMESPACE\n\nPROGRAM Main\nVAR\n    V : Lib.Valve;\nEND_VAR\n    V();\nEND_PROGRAM\n";
    let faceplate = "format = 1\ntitle = \"Valve\"\nsize = [400, 300]\n\n[fields.Open]\naccepts = [\"boolean\"]\n";
    for block in ["Lib.Valve", "Valve"] {
        let pair =
            format!("format = 1\nblock = \"{block}\"\nfaceplate = \"valve\"\nsymbol = \"valve\"\n");
        assert_eq!(problems_in(program, &pair, faceplate), vec![], "{block}");
    }
}

#[test]
fn a_member_of_a_named_type_holds_the_kind_of_the_type_it_names() {
    let program = "TYPE Speed : REAL; END_TYPE\nTYPE Rpm : Speed; END_TYPE\n\nFUNCTION_BLOCK Drive\nVAR_OUTPUT\n    Actual : Rpm;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nPROGRAM Main\nVAR\n    D : Drive;\nEND_VAR\n    D();\nEND_PROGRAM\n";
    let faceplate = "format = 1\ntitle = \"Drive\"\nsize = [400, 300]\n\n[fields.Actual]\naccepts = [\"number\"]\n";
    let pair = "format = 1\nblock = \"Drive\"\nfaceplate = \"drive\"\nsymbol = \"motor\"\n";
    assert_eq!(problems_in(program, pair, faceplate), vec![]);
}

#[test]
fn a_field_may_map_to_a_member_of_an_instance_the_block_holds() {
    let program = "FUNCTION_BLOCK Watch\nVAR_OUTPUT\n    Active : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nFUNCTION_BLOCK Feeder\nVAR\n    FailedToStart : Watch;\nEND_VAR\n    FailedToStart();\nEND_FUNCTION_BLOCK\n\nPROGRAM Main\nVAR\n    F : Feeder;\nEND_VAR\n    F();\nEND_PROGRAM\n";
    let faceplate = "format = 1\ntitle = \"Feeder\"\nsize = [400, 300]\n\n[fields.Failed]\naccepts = [\"boolean\"]\n";
    let pair = |member: &str| {
        format!("format = 1\nblock = \"Feeder\"\nfaceplate = \"feeder\"\nsymbol = \"motor\"\n\n[fields]\nFailed = \"{member}\"\n")
    };
    assert_eq!(
        problems_in(program, &pair("FailedToStart.Active"), faceplate),
        vec![]
    );
    assert_eq!(
        problems_in(program, &pair("FailedToStart.Gone"), faceplate),
        vec![(Some("Failed".to_string()), "`Feeder` has no member `FailedToStart.Gone`, which the faceplate's field `Failed` needs".to_string())]
    );
}

#[test]
fn a_field_may_map_to_a_member_a_block_inherits() {
    let program = "FUNCTION_BLOCK Watch\nVAR_OUTPUT\n    Active : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nFUNCTION_BLOCK TimedWatch EXTENDS Watch\nVAR_INPUT\n    Delay : TIME;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nFUNCTION_BLOCK Feeder\nVAR\n    FailedToStart : TimedWatch;\nEND_VAR\n    FailedToStart();\nEND_FUNCTION_BLOCK\n\nPROGRAM Main\nVAR\n    F : Feeder;\n    W : TimedWatch;\nEND_VAR\n    F();\n    W();\nEND_PROGRAM\n";
    let faceplate = "format = 1\ntitle = \"Watch\"\nsize = [400, 300]\n\n[fields.Active]\naccepts = [\"boolean\"]\n";
    let own = "format = 1\nblock = \"TimedWatch\"\nfaceplate = \"watch\"\nsymbol = \"status\"\n";
    assert_eq!(
        problems_in(program, own, faceplate),
        vec![],
        "a member of the block's base"
    );
    let held = "format = 1\nblock = \"Feeder\"\nfaceplate = \"watch\"\nsymbol = \"motor\"\n\n[fields]\nActive = \"FailedToStart.Active\"\n";
    assert_eq!(
        problems_in(program, held, faceplate),
        vec![],
        "a member of the base of an instance the block holds"
    );
}

#[test]
fn a_symbol_binding_needs_a_property_of_the_symbol_and_a_member_of_a_kind_it_takes() {
    // §8.5: each `[bind]` entry maps a property of the pair's symbol to a member of the block.
    let pair = "format = 1\nblock = \"Pump\"\nfaceplate = \"pump\"\nsymbol = \"pump\"\n\n[bind]\nfault = \"ActualSpeed\"\nrunning = \"Missing\"\nspeedy = \"Running\"\n";
    let faceplate = "format = 1\ntitle = \"Pump\"\nsize = [400, 300]\n";
    let declarations = CompileSession::from_source(PROGRAM)
        .declarations()
        .expect("the program compiles");
    let pair = parse_pair("hmi/pairs/pump.toml", pair).expect("the pair loads");
    let faceplate =
        parse_faceplate("hmi/panels/pump.toml", faceplate).expect("the faceplate loads");
    let found: Vec<(Option<String>, String)> =
        check_pair(&pair, &faceplate, &declarations.metadata().clone())
            .into_iter()
            .map(|problem| (problem.binding, problem.message))
            .collect();
    assert_eq!(
        found,
        vec![
            (Some("fault".to_string()), "the member `ActualSpeed` of `Pump` holds a number; the symbol's `fault` takes a boolean".to_string()),
            (Some("running".to_string()), "`Pump` has no member `Missing`, which the symbol's `running` shows".to_string()),
            (Some("speedy".to_string()), "the symbol `pump` has no property `speedy`".to_string()),
        ]
    );
}

#[test]
fn settings_terminals_and_dependencies_name_members_that_can_hold_them() {
    let fine = FACEPLATE.replace("[fields.Missing]\naccepts = [\"number\"]\n", "");
    let pair = format!(
        "{PAIR}\n[[settings]]\nmember = \"Selected\"\ntitle = \"Selection\"\n\n[[settings]]\nmember = \"Absent\"\ntitle = \"Absent\"\n\n[[io]]\nmember = \"StartCmd\"\ntitle = \"Start\"\ndirection = \"input\"\nrequired = true\n\n[[io]]\nmember = \"Running\"\ntitle = \"Running\"\ndirection = \"output\"\nrequired_when = \"ActualSpeed\"\n\n[[dependencies]]\nmember = \"Upstream\"\ntitle = \"Upstream\"\ninterface = \"IRun\"\nrequired = false\n"
    );
    let messages: Vec<String> = problems(&pair, &fine)
        .into_iter()
        .map(|(_, message)| message)
        .collect();
    assert_eq!(
        messages,
        vec![
            "`Pump` has no member `Absent`, which a setting names".to_string(),
            "`required_when` of `Running` names `ActualSpeed`, which is not a BOOL member of `Pump`"
                .to_string(),
            "`Pump` has no member `Upstream`, which a dependency names".to_string(),
        ]
    );
}
