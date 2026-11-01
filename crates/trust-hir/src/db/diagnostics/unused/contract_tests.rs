use crate::db::{Database, FileId, SemanticDatabase, SourceDatabase};
use crate::diagnostics::{Diagnostic, DiagnosticCode, DiagnosticSeverity};

fn diagnostics_for(sources: &[&str], file: usize) -> Vec<Diagnostic> {
    let mut database = Database::new();
    for (index, source) in sources.iter().enumerate() {
        database.set_source_text(FileId(index as u32), (*source).to_owned());
    }
    database.diagnostics(FileId(file as u32)).as_ref().clone()
}

fn warnings(source: &str) -> Vec<Diagnostic> {
    diagnostics_for(&[source], 0)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
        .collect()
}

fn messages_for(source: &str, code: DiagnosticCode) -> Vec<String> {
    warnings(source)
        .into_iter()
        .filter(|diagnostic| diagnostic.code == code)
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn unused_local_and_temporary_variables_warn_with_exact_kind_and_name() {
    let messages = messages_for(
        r#"
PROGRAM Main
VAR
    LocalValue : INT;
END_VAR
VAR_TEMP
    Scratch : INT;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::UnusedVariable,
    );

    assert!(messages.contains(&"unused variable 'LocalValue'".to_owned()));
    assert!(messages.contains(&"unused variable 'Scratch'".to_owned()));
}

#[test]
fn used_local_and_temporary_variables_do_not_warn() {
    let messages = messages_for(
        r#"
PROGRAM Main
VAR
    LocalValue : INT;
END_VAR
VAR_TEMP
    Scratch : INT;
END_VAR
Scratch := 1;
LocalValue := Scratch;
END_PROGRAM
"#,
        DiagnosticCode::UnusedVariable,
    );

    assert!(messages
        .iter()
        .all(|message| !message.contains("LocalValue") && !message.contains("Scratch")));
}

#[test]
fn unused_input_warns_but_output_and_inout_parameters_do_not() {
    let messages = messages_for(
        r#"
FUNCTION Operation : INT
VAR_INPUT
    InputValue : INT;
END_VAR
VAR_OUTPUT
    OutputValue : INT;
END_VAR
VAR_IN_OUT
    LinkedValue : INT;
END_VAR
Operation := 0;
END_FUNCTION
"#,
        DiagnosticCode::UnusedParameter,
    );

    assert_eq!(messages, vec!["unused parameter 'InputValue'"]);
}

#[test]
fn interface_method_input_parameter_is_a_prototype_not_an_unused_value() {
    let messages = messages_for(
        r#"
INTERFACE IWorker
METHOD Run
VAR_INPUT
    Command : INT;
END_VAR
END_METHOD
END_INTERFACE
"#,
        DiagnosticCode::UnusedParameter,
    );

    assert!(messages.is_empty());
}

/// Specification 28 §1: an unused local constant warns, and so does an unused global constant
/// declared at the top level of a file. Catches the former exemption of global constants.
#[test]
fn an_unused_local_or_top_level_global_constant_warns() {
    let mut messages = messages_for(
        r#"
VAR_GLOBAL CONSTANT
    GlobalLimit : INT := 10;
END_VAR
PROGRAM Main
VAR CONSTANT
    LocalLimit : INT := 5;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::UnusedVariable,
    );
    messages.sort();

    assert_eq!(
        messages,
        vec![
            "unused constant 'GlobalLimit'",
            "unused constant 'LocalLimit'"
        ]
    );
}

/// Specification 28 §1 (decided by the owner on 1 November 2026): an unused global constant warns
/// wherever it is declared: at the top level, in a NAMESPACE (also a nested one), in a CONFIGURATION
/// and in a RESOURCE. Catches a place of declaration that is left out, as the top level was before.
#[test]
fn an_unused_global_constant_warns_wherever_it_is_declared() {
    let mut messages = messages_for(
        r#"
VAR_GLOBAL CONSTANT
    TopLevel : INT := 1;
END_VAR
NAMESPACE Outer
VAR_GLOBAL CONSTANT
    Spaced : INT := 2;
END_VAR
NAMESPACE Inner
VAR_GLOBAL CONSTANT
    Nested : INT := 3;
END_VAR
END_NAMESPACE
END_NAMESPACE
CONFIGURATION Plant
VAR_GLOBAL CONSTANT
    Configured : INT := 4;
END_VAR
RESOURCE Cpu ON PLC
VAR_GLOBAL CONSTANT
    Resourced : INT := 5;
END_VAR
TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
PROGRAM MainInstance WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
PROGRAM Main
END_PROGRAM
"#,
        DiagnosticCode::UnusedVariable,
    );
    messages.sort();

    assert_eq!(
        messages,
        vec![
            "unused constant 'Configured'",
            "unused constant 'Nested'",
            "unused constant 'Resourced'",
            "unused constant 'Spaced'",
            "unused constant 'TopLevel'",
        ]
    );
}

/// Specification 28 §1: use is project-wide, so a top-level global constant that another file uses by
/// its name, or as a declaration's size, does not warn; one that no file uses does. Catches a
/// top-level constant whose uses in other files are not counted.
#[test]
fn a_top_level_global_constant_used_from_another_file_does_not_warn() {
    let diagnostics = diagnostics_for(
        &[
            r#"
VAR_GLOBAL CONSTANT
    ByName : INT := 4;
    InSize : INT := 8;
    NeverReferenced : INT := 9;
END_VAR
"#,
            r#"
PROGRAM Main
VAR
    Count : INT;
    Text : STRING[InSize];
END_VAR
Count := Count + ByName;
Text := Text;
END_PROGRAM
"#,
        ],
        0,
    );
    let unused: Vec<String> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnusedVariable)
        .map(|diagnostic| diagnostic.message.clone())
        .collect();

    assert_eq!(unused, vec!["unused constant 'NeverReferenced'"]);
}

/// Specification 28 §1 and IEC 61131-3 §6.5.2.1, Figure 8: a POU uses a global through its
/// `VAR_EXTERNAL` declaration, so a use of the external reaches the global; the declaration alone is
/// no use. Catches a global constant reported unused although a program uses it through its
/// external (as before), and an external that counts as a use without being used.
#[test]
fn a_global_constant_used_through_var_external_does_not_warn() {
    let diagnostics = diagnostics_for(
        &[
            r#"
CONFIGURATION Plant
VAR_GLOBAL CONSTANT
    ViaExternal : INT := 3;
    DeclaredOnly : INT := 4;
END_VAR
VAR_GLOBAL
    Level : INT;
END_VAR
RESOURCE Cpu ON PLC
TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
PROGRAM MainInstance WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#,
            r#"
PROGRAM Main
VAR_EXTERNAL CONSTANT
    ViaExternal : INT;
    DeclaredOnly : INT;
END_VAR
VAR_EXTERNAL
    Level : INT;
END_VAR
Level := ViaExternal;
END_PROGRAM
"#,
        ],
        0,
    );
    let unused: Vec<String> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnusedVariable)
        .map(|diagnostic| diagnostic.message.clone())
        .collect();

    assert_eq!(unused, vec!["unused constant 'DeclaredOnly'"]);
}

#[test]
fn unused_program_function_and_function_block_each_warn() {
    let messages = messages_for(
        r#"
PROGRAM Idle
END_PROGRAM
FUNCTION Calculate : INT
Calculate := 1;
END_FUNCTION
FUNCTION_BLOCK Controller
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::UnusedPou,
    );

    assert!(messages.contains(&"unused program 'Idle'".to_owned()));
    assert!(messages.contains(&"unused function 'Calculate'".to_owned()));
    assert!(messages.contains(&"unused function block 'Controller'".to_owned()));
}

#[test]
fn test_pous_are_runner_entry_points_without_hiding_unused_production_pous() {
    let source = r#"
TEST_PROGRAM MeasuredGain
VAR unused_value : INT; END_VAR
END_TEST_PROGRAM
TEST_FUNCTION_BLOCK ZeroStepGuard
END_TEST_FUNCTION_BLOCK
PROGRAM Idle
END_PROGRAM
FUNCTION_BLOCK UnusedController
END_FUNCTION_BLOCK
"#;
    let mut messages = messages_for(source, DiagnosticCode::UnusedPou);
    messages.sort();
    assert_eq!(
        messages,
        vec![
            "unused function block 'UnusedController'",
            "unused program 'Idle'"
        ]
    );
    assert_eq!(
        messages_for(source, DiagnosticCode::UnusedVariable),
        vec!["unused variable 'unused_value'"]
    );
}

#[test]
fn function_result_assignment_does_not_make_function_externally_used() {
    let messages = messages_for(
        r#"
FUNCTION Calculate : INT
Calculate := 1;
END_FUNCTION
"#,
        DiagnosticCode::UnusedPou,
    );

    assert_eq!(messages, vec!["unused function 'Calculate'"]);
}

#[test]
fn configuration_program_instance_marks_program_type_used() {
    let messages = messages_for(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    PROGRAM MainInstance WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#,
        DiagnosticCode::UnusedPou,
    );

    assert!(messages
        .iter()
        .all(|message| message != "unused program 'Main'"));
}

#[test]
fn function_call_marks_function_used() {
    let messages = messages_for(
        r#"
FUNCTION Calculate : INT
Calculate := 1;
END_FUNCTION
PROGRAM Main
VAR result : INT; END_VAR
result := Calculate();
END_PROGRAM
"#,
        DiagnosticCode::UnusedPou,
    );

    assert!(messages
        .iter()
        .all(|message| message != "unused function 'Calculate'"));
}

#[test]
fn function_block_type_reference_marks_function_block_used() {
    let messages = messages_for(
        r#"
FUNCTION_BLOCK Controller
END_FUNCTION_BLOCK
PROGRAM Main
VAR instance : Controller; END_VAR
END_PROGRAM
"#,
        DiagnosticCode::UnusedPou,
    );

    assert!(messages
        .iter()
        .all(|message| message != "unused function block 'Controller'"));
}

#[test]
fn resolved_var_config_path_marks_member_used_across_files() {
    let sources = [
        r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    PROGRAM MainInstance WITH Fast : Main;
END_RESOURCE
VAR_CONFIG
    MainInstance.InputSignal : BOOL;
END_VAR
END_CONFIGURATION
"#,
        r#"
PROGRAM Main
VAR
    InputSignal : BOOL;
END_VAR
END_PROGRAM
"#,
    ];
    let messages = diagnostics_for(&sources, 1)
        .into_iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnusedVariable)
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>();

    assert!(messages
        .iter()
        .all(|message| message != "unused variable 'InputSignal'"));
}

#[test]
fn project_wide_function_call_marks_declaration_used_in_owner_file() {
    let sources = [
        r#"
FUNCTION Calculate : INT
Calculate := 1;
END_FUNCTION
"#,
        r#"
PROGRAM Main
VAR result : INT; END_VAR
result := Calculate();
END_PROGRAM
"#,
    ];
    let messages = diagnostics_for(&sources, 0)
        .into_iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnusedPou)
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>();

    assert!(messages
        .iter()
        .all(|message| message != "unused function 'Calculate'"));
}

#[test]
fn members_of_function_blocks_classes_and_interfaces_are_excluded() {
    let diagnostics = warnings(
        r#"
FUNCTION_BLOCK Controller
VAR
    State : INT;
END_VAR
METHOD Step
END_METHOD
END_FUNCTION_BLOCK
CLASS Holder
VAR
    Value : INT;
END_VAR
METHOD Read
END_METHOD
END_CLASS
INTERFACE IWorker
METHOD Run
VAR_INPUT Request : INT; END_VAR
END_METHOD
END_INTERFACE
"#,
    );

    for member in ["State", "Value", "Request"] {
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains(member)),
            "member {member} received an unused warning: {diagnostics:?}"
        );
    }
}

#[test]
fn imported_declarations_do_not_receive_duplicate_unused_warnings() {
    let sources = [
        r#"
FUNCTION Calculate : INT
Calculate := 1;
END_FUNCTION
"#,
        "PROGRAM Main END_PROGRAM",
    ];
    let imported_messages = diagnostics_for(&sources, 1)
        .into_iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.code,
                DiagnosticCode::UnusedPou
                    | DiagnosticCode::UnusedVariable
                    | DiagnosticCode::UnusedParameter
            )
        })
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>();

    assert!(imported_messages
        .iter()
        .all(|message| !message.contains("Calculate")));
}

#[test]
fn warning_range_selects_the_unused_declaration_name() {
    let source = r#"
PROGRAM Main
VAR
    Forgotten : INT;
END_VAR
END_PROGRAM
"#;
    let warning = warnings(source)
        .into_iter()
        .find(|diagnostic| {
            diagnostic.code == DiagnosticCode::UnusedVariable
                && diagnostic.message.contains("Forgotten")
        })
        .expect("unused variable warning");
    let expected = source.find("Forgotten").expect("declaration name") as u32;

    assert_eq!(u32::from(warning.range.start()), expected);
    assert_eq!(warning.message, "unused variable 'Forgotten'");
}

#[test]
fn extends_base_is_used_by_the_inheriting_declaration() {
    // RED pin 2026-10-27: an EXTENDS/IMPLEMENTS target is an inheritance
    // reference - the base FB was flagged W009 although the child declares
    // the only (relationship) use. Measured on s4: fbLight and
    // fbMqttPayloadBase are pure base classes and were reported unused.
    let diagnostics = diagnostics_for(
        &[
            r#"
FUNCTION_BLOCK fbBase
VAR_INPUT
    iIn : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
            r#"
FUNCTION_BLOCK fbChild EXTENDS fbBase
VAR
    x : INT;
END_VAR
    x := x;
END_FUNCTION_BLOCK
"#,
        ],
        0,
    );
    let base_unused: Vec<String> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnusedPou)
        .map(|diagnostic| diagnostic.message.clone())
        .collect();

    assert!(
        !base_unused.iter().any(|message| message.contains("fbBase")),
        "EXTENDS target must count as used: {base_unused:?}"
    );

    // The child itself is never instantiated - it still warns, but in ITS
    // declaring file.
    let child_diagnostics = diagnostics_for(
        &[
            r#"
FUNCTION_BLOCK fbBase
VAR_INPUT
    iIn : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
            r#"
FUNCTION_BLOCK fbChild EXTENDS fbBase
VAR
    x : INT;
END_VAR
    x := x;
END_FUNCTION_BLOCK
"#,
        ],
        1,
    );
    let child_unused: Vec<String> = child_diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnusedPou)
        .map(|diagnostic| diagnostic.message.clone())
        .collect();
    assert!(
        child_unused
            .iter()
            .any(|message| message.contains("fbChild")),
        "the never-instantiated child still warns in its own file: {child_unused:?}"
    );
    assert!(
        !child_unused
            .iter()
            .any(|message| message.contains("fbBase")),
        "the EXTENDS target must not warn from the child's file either: {child_unused:?}"
    );
}

#[test]
fn implements_interface_is_used_by_the_implementing_class() {
    let diagnostics = diagnostics_for(
        &[
            r#"
INTERFACE itfThing
METHOD mDo : INT;
END_METHOD
END_INTERFACE
"#,
            r#"
CLASS cThing IMPLEMENTS itfThing
METHOD mDo : INT
    mDo := 1;
END_METHOD
END_CLASS
"#,
        ],
        0,
    );
    let unused: Vec<String> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnusedPou)
        .map(|diagnostic| diagnostic.message.clone())
        .collect();

    assert!(
        !unused.iter().any(|message| message.contains("itfThing")),
        "IMPLEMENTS target must count as used: {unused:?}"
    );
}

#[test]
fn qualified_cross_file_gvl_member_use_clears_the_unused_constant_warning() {
    // RED pin 2026-10-27: name_from_node sees only the FIRST segment of a
    // qualified value reference (GVals.UsedCrossFile -> "GVals"), so the
    // collector marked the namespace used and left the member terminal
    // unused - s4's GVLMqtt/GVLEntities constants fired W001 despite
    // cross-file expression and declaration-size uses.
    let diagnostics = diagnostics_for(
        &[
            r#"
NAMESPACE GVals
VAR_GLOBAL CONSTANT
    UsedCrossFile : INT := 5;
    NeverReferenced : INT := 6;
END_VAR
END_NAMESPACE
"#,
            r#"
PROGRAM Main
VAR
    Value : INT;
END_VAR
Value := GVals.UsedCrossFile;
END_PROGRAM
"#,
        ],
        0,
    );
    let unused: Vec<String> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnusedVariable)
        .map(|diagnostic| diagnostic.message.clone())
        .collect();

    assert!(
        !unused
            .iter()
            .any(|message| message.contains("UsedCrossFile")),
        "qualified cross-file use must mark the constant used: {unused:?}"
    );
    assert!(
        unused
            .iter()
            .any(|message| message.contains("NeverReferenced")),
        "genuinely unreferenced constant must still warn: {unused:?}"
    );
}

#[test]
fn qualified_declaration_size_use_clears_the_unused_constant_warning() {
    // The s4 uiLenMsgTopic shape: the only reference is a STRING size in
    // another file's declaration. (A first cut of this pin used an enum
    // member comparison as the "use" - but the pinned constant itself was
    // never referenced there, so the warning was CORRECT; kept the fixture
    // honest instead.)
    let diagnostics = diagnostics_for(
        &[
            r#"
NAMESPACE GVals
VAR_GLOBAL CONSTANT
    TopicLen : INT := 12;
    NeverReferenced : INT := 6;
END_VAR
END_NAMESPACE
"#,
            r#"
FUNCTION Build : INT
VAR
    Topic : STRING[GVals.TopicLen];
END_VAR
    Topic := Topic;
    Build := 1;
END_FUNCTION
"#,
        ],
        0,
    );
    let unused: Vec<String> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnusedVariable)
        .map(|diagnostic| diagnostic.message.clone())
        .collect();

    assert!(
        !unused.iter().any(|message| message.contains("TopicLen")),
        "qualified declaration-size use must mark the constant used: {unused:?}"
    );
    assert!(
        unused
            .iter()
            .any(|message| message.contains("NeverReferenced")),
        "genuinely unreferenced constant must still warn: {unused:?}"
    );
}

#[test]
fn implementing_method_interface_pins_do_not_warn_but_extra_methods_do() {
    // RED pin 2026-10-27: parameters of a method that IMPLEMENTS an
    // interface method are contract-mandated (the implementing FB must
    // declare the full interface signature even where the body ignores a
    // pin) - they must not fire W002. The old exclusion covered only
    // methods declared INSIDE an INTERFACE POU. A method that is NOT part
    // of any implemented interface keeps the warning.
    let diagnostics = diagnostics_for(
        &[
            r#"
INTERFACE itfSub
METHOD mHandle : BOOL
VAR_INPUT
    sTopic : STRING[10];
    sCmd : STRING[10];
END_VAR
END_METHOD
END_INTERFACE
"#,
            r#"
FUNCTION_BLOCK fbSub IMPLEMENTS itfSub
METHOD mHandle : BOOL
VAR_INPUT
    sTopic : STRING[10];
    sCmd : STRING[10];
END_VAR
    sCmd := sCmd;
    mHandle := TRUE;
END_METHOD
METHOD mExtra : BOOL
VAR_INPUT
    iDead : INT;
END_VAR
    mExtra := TRUE;
END_METHOD
END_FUNCTION_BLOCK
"#,
        ],
        1,
    );
    let unused: Vec<String> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnusedParameter)
        .map(|diagnostic| diagnostic.message.clone())
        .collect();

    assert!(
        !unused.iter().any(|message| message.contains("sTopic")),
        "interface-mandated implementing-method pin must not warn: {unused:?}"
    );
    assert!(
        unused.iter().any(|message| message.contains("iDead")),
        "a pin of a method outside every implemented interface still warns: {unused:?}"
    );
}
