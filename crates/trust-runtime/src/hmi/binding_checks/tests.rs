//! Specification 39, §7.6: binding checks against a compiled program.

use super::*;
use crate::harness::CompileSession;
use crate::hmi::page_files::parse_page;

const PROGRAM: &str = r#"
TYPE Modes :
(
    Auto := 0,
    Manual := 1
);
END_TYPE

TYPE Speeds :
(
    Off,
    Low,
    High
);
END_TYPE

FUNCTION_BLOCK Pump
VAR_INPUT
    StartCmd : BOOL;
END_VAR
VAR_OUTPUT
    Running : BOOL;
    Speed : REAL;
END_VAR
    Running := StartCmd;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    P101 : Pump;
    Level : REAL;
    Run : BOOL;
    Name : STRING;
    Mode : Modes;
    FanSpeed : Speeds;
    Count : INT;
END_VAR
VAR CONSTANT
    MaxLevel : REAL := 100.0;
END_VAR
    P101(StartCmd := Run);
END_PROGRAM
"#;

fn problems(objects: &str) -> Vec<BindingProblem> {
    let declarations = CompileSession::from_source(PROGRAM)
        .declarations()
        .expect("the program compiles");
    let text = format!("format = 1\ntitle = \"Checks\"\n{objects}");
    let page = parse_page("hmi/pages/checks.toml", &text).expect("the page loads");
    let metadata = declarations.metadata().clone();
    check_page(
        &page,
        Catalogue::shipped(),
        &metadata,
        declarations.storage(),
    )
}

fn object(id: &str, kind: &str, bind: &str) -> String {
    format!(
        "\n[[object]]\nid = \"{id}\"\ntype = \"{kind}\"\nat = [0, 0]\nsize = [80, 40]\n\n[object.bind]\n{bind}\n"
    )
}

fn single(objects: &str) -> BindingProblem {
    let found = problems(objects);
    assert_eq!(found.len(), 1, "{found:#?}");
    found.into_iter().next().expect("one problem")
}

fn assert_mentions(problem: &BindingProblem, words: &[&str]) {
    for word in words {
        assert!(
            problem.message.contains(word),
            "\"{}\" does not mention {word}",
            problem.message
        );
    }
}

#[test]
fn a_correct_page_has_no_problems() {
    let page = [
        object("level", "value", "value = \"Main.Level\""),
        object("speed", "bar", "value = \"Main.P101.Speed\""),
        object("run", "status", "state = \"Main.Run\""),
        object(
            "trend",
            "trend",
            "pv = \"Main.Level\"\nsp = \"Main.P101.Speed\"",
        ),
        object(
            "start",
            "switch",
            "value = { path = \"Main.P101.StartCmd\", direction = \"read-write\" }",
        ),
    ]
    .concat();
    assert_eq!(problems(&page), vec![]);
}

#[test]
fn an_unknown_component_type_is_reported() {
    let problem = single(&object("dial-1", "dial", "value = \"Main.Level\""));
    assert_eq!(
        (problem.object.as_str(), problem.property.as_deref()),
        ("dial-1", None)
    );
    assert_mentions(&problem, &["dial"]);
}

#[test]
fn a_property_the_component_does_not_have_is_reported() {
    let problem = single(&object("bar-1", "bar", "colour = \"Main.Level\""));
    assert_eq!(problem.property.as_deref(), Some("colour"));
    assert_mentions(&problem, &["colour", "bar"]);
}

#[test]
fn a_path_that_does_not_resolve_is_reported() {
    let problem = single(&object("level", "value", "value = \"Main.Missing\""));
    assert_eq!(problem.property.as_deref(), Some("value"));
    assert_mentions(&problem, &["Main.Missing"]);

    let problem = single(&object("level", "value", "value = \"Main..Level\""));
    assert_mentions(&problem, &["Main..Level"]);
}

#[test]
fn a_value_of_the_wrong_kind_is_reported() {
    let problem = single(&object("bar-1", "bar", "value = \"Main.Run\""));
    assert_eq!(problem.property.as_deref(), Some("value"));
    assert_mentions(&problem, &["Main.Run", "BOOL", "number"]);

    let problem = single(&object("trend", "trend", "pv = \"Main.Name\""));
    assert_eq!(problem.property.as_deref(), Some("pv"));
    assert_mentions(&problem, &["STRING"]);
}

#[test]
fn a_direction_the_property_does_not_allow_is_reported() {
    let problem = single(&object(
        "bar-1",
        "bar",
        "value = { path = \"Main.Level\", direction = \"write\" }",
    ));
    assert_eq!(problem.property.as_deref(), Some("value"));
    assert_mentions(&problem, &["write"]);
}

/// A loop whose setpoint is an output, set through a method (Plant §3.6).
const METHOD_PROGRAM: &str = r#"
INTERFACE ISetpoint
METHOD SetSetpoint : BOOL
VAR_INPUT
    Value : REAL;
END_VAR
END_METHOD
END_INTERFACE

FUNCTION_BLOCK Loop IMPLEMENTS ISetpoint
VAR_OUTPUT
    Setpoint : REAL;
END_VAR
METHOD PUBLIC SetSetpoint : BOOL
VAR_INPUT
    Value : REAL;
END_VAR
    Setpoint := Value;
    SetSetpoint := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    L1 : Loop;
END_VAR
    L1();
END_PROGRAM
"#;

#[test]
fn a_control_that_calls_a_method_may_only_read_its_value() {
    let check = |command: &str| {
        let declarations = CompileSession::from_source(METHOD_PROGRAM)
            .declarations()
            .expect("the program compiles");
        let text = format!(
            "format = 1\ntitle = \"Checks\"\n\n[[object]]\nid = \"sp\"\ntype = \"setpoint\"\nat = [0, 0]\nsize = [240, 96]\n\n[object.bind]\nvalue = \"Main.L1.Setpoint\"\n{command}"
        );
        let page = parse_page("hmi/pages/checks.toml", &text).expect("the page loads");
        let metadata = declarations.metadata().clone();
        check_page(
            &page,
            Catalogue::shipped(),
            &metadata,
            declarations.storage(),
        )
    };
    let calls = "\n[object.command]\ninterface = \"ISetpoint\"\nmethod = \"SetSetpoint\"\ninstance = \"Main.L1\"\n\n[object.command.arguments]\nValue = { input = true }\n";
    assert_eq!(
        check(calls),
        vec![],
        "the method sets the value; the binding shows it"
    );
    let without = check("");
    assert_eq!(without.len(), 1, "{without:#?}");
    assert_eq!(without[0].property.as_deref(), Some("value"));
    assert_mentions(&without[0], &["setpoint", "reads"]);
}

#[test]
fn writing_a_function_block_output_is_reported() {
    let problem = single(&object(
        "running",
        "switch",
        "value = { path = \"Main.P101.Running\", direction = \"read-write\" }",
    ));
    assert_eq!(problem.property.as_deref(), Some("value"));
    assert_mentions(&problem, &["Main.P101.Running", "output"]);
}

#[test]
fn writing_a_constant_is_reported() {
    let problem = single(&object(
        "limit",
        "numeric-input",
        "value = { path = \"Main.MaxLevel\", direction = \"read-write\" }",
    ));
    assert_mentions(&problem, &["Main.MaxLevel", "constant"]);
}

const GLOBALS: &str = r#"
VAR_GLOBAL CONSTANT
    Limit : REAL := 5.0;
END_VAR
VAR_GLOBAL
    Speed : REAL;
END_VAR

PROGRAM Main
VAR
    X : REAL;
END_VAR
    X := Limit + Speed;
END_PROGRAM
"#;

/// Specification 39, §7.6: a constant cannot be written, a global constant included.
#[test]
fn writes_to_global_constants_are_refused() {
    let declarations = CompileSession::from_source(GLOBALS)
        .declarations()
        .expect("the program compiles");
    let metadata = declarations.metadata().clone();
    let check = |path: &str| {
        let bind = format!("value = {{ path = \"{path}\", direction = \"read-write\" }}");
        let text = format!(
            "format = 1\ntitle = \"Checks\"\n{}",
            object("entry", "numeric-input", &bind)
        );
        let page = parse_page("hmi/pages/checks.toml", &text).expect("the page loads");
        check_page(
            &page,
            Catalogue::shipped(),
            &metadata,
            declarations.storage(),
        )
    };
    let refused = check("Limit");
    assert_eq!(refused.len(), 1, "{refused:#?}");
    assert_mentions(
        &refused[0],
        &["Limit", "is a constant", "cannot be written"],
    );
    assert!(
        check("Speed").is_empty(),
        "a global variable can be written"
    );
}

fn chooser(kind: &str, options: Option<&str>, property: &str, path: &str) -> String {
    let options = options.map_or(String::new(), |options| format!("options = {options}\n"));
    format!(
        "\n[[object]]\nid = \"choice\"\ntype = \"{kind}\"\nat = [0, 0]\nsize = [240, 40]\n{options}\n[object.bind]\n{property} = {{ path = \"{path}\", direction = \"read-write\" }}\n"
    )
}

#[test]
fn selector_options_must_be_values_of_a_bound_enumeration() {
    let fine = [
        chooser(
            "selector",
            Some("[\"Off\", \"low\", \"High\"]"),
            "value",
            "Main.FanSpeed",
        ),
        chooser(
            "selector",
            Some("[\"Stop\", \"Slow\"]"),
            "value",
            "Main.Count",
        ),
    ];
    for page in fine {
        assert_eq!(problems(&page), vec![], "{page}");
    }
    let problem = single(&chooser(
        "selector",
        Some("[\"Off\", \"Fast\"]"),
        "value",
        "Main.FanSpeed",
    ));
    assert_eq!(problem.property.as_deref(), Some("value"));
    assert_mentions(&problem, &["\"Fast\"", "Speeds"]);
}

#[test]
fn a_mode_selector_needs_an_enumeration_with_the_modes_it_offers() {
    assert_mentions(
        &single(&chooser("mode-selector", None, "mode", "Main.Count")),
        &["enumeration"],
    );
    let missing = single(&chooser("mode-selector", None, "mode", "Main.Mode"));
    assert_eq!(missing.property.as_deref(), Some("mode"));
    assert_mentions(&missing, &["Modes", "Track"]);
    let narrowed = chooser(
        "mode-selector",
        Some("[\"Manual\", \"Auto\"]"),
        "mode",
        "Main.Mode",
    );
    assert_eq!(problems(&narrowed), vec![]);
    let other = single(&chooser(
        "mode-selector",
        Some("[\"Manual\", \"Cascade\"]"),
        "mode",
        "Main.Mode",
    ));
    assert_mentions(&other, &["Modes", "Cascade"]);
}

/// Specification 39, §18 phase 5 item 2: a visibility binding is a BOOL of the program.
#[test]
fn a_visibility_binding_is_a_boolean_of_the_program() {
    assert_eq!(
        problems(&object(
            "level",
            "value",
            "value = \"Main.Level\"\nvisible = \"Main.Run\""
        )),
        vec![]
    );
    let wrong = single(&object(
        "level",
        "value",
        "value = \"Main.Level\"\nvisible = \"Main.Level\"",
    ));
    assert_mentions(&wrong, &["Main.Level"]);
}

/// Specification 39, §8.9 item 2: a placed faceplate is bound to a function block instance of the
/// program; the problem is at its `instance` key.
#[test]
fn a_placed_faceplate_is_bound_to_a_function_block_instance_of_the_program() {
    let placed = |instance: &str| {
        format!(
            "\n[[object]]\nid = \"card\"\ntype = \"faceplate\"\nfaceplate = \"pump\"\ninstance = \"{instance}\"\nat = [0, 0]\nsize = [400, 344]\n"
        )
    };
    assert_eq!(problems(&placed("Main.P101")), vec![]);
    let missing = single(&placed("Main.P999"));
    assert_eq!(missing.property.as_deref(), Some("instance"));
    assert_mentions(&missing, &["Main.P999", "not an instance of the program"]);
    let scalar = single(&placed("Main.Level"));
    assert_eq!(scalar.property.as_deref(), Some("instance"));
    assert_mentions(&scalar, &["Main.Level", "not an instance of the program"]);
}

/// Specification 39, §8.9 items 1 and 2: a placed faceplate names a faceplate of the project or
/// its libraries, and its instance's block has every field the faceplate needs.
#[test]
fn a_placed_faceplate_names_a_faceplate_of_the_bundle_that_its_instance_fits() {
    use crate::hmi::bundle::HmiBundle;
    let card = "format = 1\ntitle = \"Pump card\"\nsize = [320, 160]\n\n[fields.Running]\naccepts = [\"boolean\"]\n\n[fields.Speed]\naccepts = [\"number\"]\n";
    let open = "format = 1\ntitle = \"Valve card\"\nsize = [320, 160]\n\n[fields.Open]\naccepts = [\"boolean\"]\n";
    let bundle = HmiBundle {
        files: [
            ("hmi/panels/pump-card.toml", card),
            ("hmi/panels/valve-card.toml", open),
        ]
        .into_iter()
        .map(|(path, text)| (path.to_string(), text.as_bytes().to_vec()))
        .collect(),
    };
    let placed = |faceplate: &str, instance: &str| {
        format!(
            "format = 1\ntitle = \"Checks\"\n\n[[object]]\nid = \"card\"\ntype = \"faceplate\"\nfaceplate = \"{faceplate}\"\ninstance = \"{instance}\"\nat = [0, 0]\nsize = [320, 204]\n"
        )
    };
    let declarations = CompileSession::from_source(PROGRAM)
        .declarations()
        .expect("the program compiles");
    let storage = declarations.storage();
    let check = |text: String| {
        let page = parse_page("hmi/pages/checks.toml", &text).expect("the page loads");
        check_placements(&page, &bundle, storage)
    };
    assert_eq!(check(placed("pump-card", "Main.P101")), vec![]);
    let missing = check(placed("tank-card", "Main.P101"));
    assert_eq!(missing.len(), 1, "{missing:#?}");
    assert_eq!(missing[0].property.as_deref(), Some("faceplate"));
    assert_mentions(
        &missing[0],
        &["tank-card", "not in the project or its libraries"],
    );
    let misfit = check(placed("valve-card", "Main.P101"));
    assert_eq!(misfit.len(), 1, "{misfit:#?}");
    assert_eq!(misfit[0].property.as_deref(), Some("instance"));
    assert_mentions(&misfit[0], &["Main.P101", "Open"]);
    assert_eq!(
        check(placed("pump-card", "Main.Level")),
        vec![],
        "an instance that is none is reported by check_page alone"
    );
}

#[test]
fn remote_bindings_need_fresh_published_metadata_and_keep_owner_write_restrictions() {
    let declarations = CompileSession::from_source(PROGRAM).declarations().unwrap();
    let page = parse_page(
        "hmi/pages/remote.toml",
        &format!(
            "format = 1\ntitle = \"Remote\"\n{}",
            object("level", "gauge", "value = \"Owner:Main.Level\"")
        ),
    )
    .unwrap();
    let metadata = declarations.metadata().clone();
    assert!(!check_page(
        &page,
        Catalogue::shipped(),
        &metadata,
        declarations.storage()
    )
    .is_empty());
    let mut remote = std::collections::BTreeMap::from([(
        "Owner:main.level".to_string(),
        serde_json::json!({"kinds":["number","value"],"writable":false}),
    )]);
    assert!(check_page_with_remote(
        &page,
        Catalogue::shipped(),
        &metadata,
        declarations.storage(),
        &remote
    )
    .is_empty());
    remote.get_mut("Owner:main.level").unwrap()["kinds"] = serde_json::json!(["bool", "value"]);
    assert!(!check_page_with_remote(
        &page,
        Catalogue::shipped(),
        &metadata,
        declarations.storage(),
        &remote
    )
    .is_empty());
    remote.get_mut("Owner:main.level").unwrap()["kinds"] = serde_json::json!(["number", "value"]);
    let control = parse_page(
        "hmi/pages/remote-control.toml",
        &format!(
            "format = 1\ntitle = \"Remote control\"\n{}",
            object(
                "setpoint",
                "setpoint",
                r#"value = { path = "Owner:Main.Level", direction = "read-write" }"#,
            ),
        ),
    )
    .unwrap();
    assert!(
        !check_page_with_remote(
            &control,
            Catalogue::shipped(),
            &metadata,
            declarations.storage(),
            &remote
        )
        .is_empty(),
        "owner read-only metadata must refuse a writable control"
    );
    remote.get_mut("Owner:main.level").unwrap()["writable"] = serde_json::json!(true);
    assert!(
        check_page_with_remote(
            &control,
            Catalogue::shipped(),
            &metadata,
            declarations.storage(),
            &remote
        )
        .is_empty(),
        "a numeric writable owner declaration satisfies the control contract"
    );
}

#[test]
fn quality_bindings_require_local_types_or_a_fresh_remote_publish_contract() {
    let declarations = CompileSession::from_source(PROGRAM).declarations().unwrap();
    let storage = declarations.storage();
    let mut remote = std::collections::BTreeMap::new();
    assert!(check_quality("Main.Run", storage, &remote).is_ok());
    assert!(check_quality("Main.Level", storage, &remote).is_err());
    assert!(check_quality("Owner:Main.Quality", storage, &remote).is_err());
    remote.insert(
        crate::hmi::remote_values::qualified_key("Owner:Main.Quality"),
        serde_json::json!({"kinds":["enumeration"]}),
    );
    assert!(check_quality("Owner:Main.Quality", storage, &remote).is_ok());
    remote.insert(
        crate::hmi::remote_values::qualified_key("Owner:Main.Quality"),
        serde_json::json!({"kinds":["number"]}),
    );
    assert!(check_quality("Owner:Main.Quality", storage, &remote).is_err());
}
