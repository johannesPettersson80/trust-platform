mod common;
use common::*;

// Program Organization Units (POUs)
#[test]
// IEC 61131-3 Ed.3 Table 47 (PROGRAM declaration)
fn test_empty_program() {
    insta::assert_snapshot!(snapshot_parse("PROGRAM Test END_PROGRAM"));
}

#[test]
fn test_test_program() {
    insta::assert_snapshot!(snapshot_parse(
        r#"TEST_PROGRAM TestSuite
VAR
    passed : BOOL;
END_VAR
    passed := TRUE;
END_TEST_PROGRAM"#
    ));
}

#[test]
fn test_program_with_var_block() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
VAR
    x : INT;
    y : BOOL;
END_VAR
END_PROGRAM"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 19 (FUNCTION declaration)
fn test_function_with_return_type() {
    insta::assert_snapshot!(snapshot_parse(
        r#"FUNCTION Add : INT
VAR_INPUT
    a : INT;
    b : INT;
END_VAR
    Add := a + b;
END_FUNCTION"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 40 (FUNCTION_BLOCK declaration)
fn test_function_block() {
    insta::assert_snapshot!(snapshot_parse(
        r#"FUNCTION_BLOCK FB_Timer
VAR_INPUT
    enable : BOOL;
END_VAR
VAR_OUTPUT
    done : BOOL;
END_VAR
VAR
    counter : INT;
END_VAR
END_FUNCTION_BLOCK"#
    ));
}

#[test]
fn test_test_function_block() {
    insta::assert_snapshot!(snapshot_parse(
        r#"TEST_FUNCTION_BLOCK FB_TestCase
VAR
    ok : BOOL;
END_VAR
    ok := TRUE;
END_TEST_FUNCTION_BLOCK"#
    ));
}

#[test]
fn test_function_block_top_level_statements_form_stmt_list() {
    let parsed = parse(
        r#"FUNCTION_BLOCK FB_Test
VAR
    ok : BOOL;
END_VAR
ok := TRUE;
END_FUNCTION_BLOCK"#,
    );
    assert!(
        parsed.ok(),
        "expected FUNCTION_BLOCK with top-level statements to parse, got: {:?}",
        parsed.errors()
    );
    let syntax = parsed.syntax();
    let fb = syntax
        .children()
        .find(|child| child.kind() == SyntaxKind::FunctionBlock)
        .expect("expected function block node");
    assert!(
        fb.children()
            .any(|child| child.kind() == SyntaxKind::StmtList),
        "expected FUNCTION_BLOCK statements to be wrapped in StmtList, got:\n{}",
        snapshot_parse(
            r#"FUNCTION_BLOCK FB_Test
VAR
    ok : BOOL;
END_VAR
ok := TRUE;
END_FUNCTION_BLOCK"#
        )
    );
}

#[test]
fn test_function_block_extends() {
    insta::assert_snapshot!(snapshot_parse(
        r#"FUNCTION_BLOCK FB_Child EXTENDS FB_Parent
VAR
    extra : INT;
END_VAR
END_FUNCTION_BLOCK"#
    ));
}

#[test]
fn test_function_block_implements() {
    insta::assert_snapshot!(snapshot_parse(
        r#"FUNCTION_BLOCK FB_Impl IMPLEMENTS IMotor, ISensor
END_FUNCTION_BLOCK"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 48 (CLASS declaration)
fn test_class_declaration() {
    insta::assert_snapshot!(snapshot_parse(
        r#"CLASS FINAL Motor EXTENDS Base IMPLEMENTS IDevice, IResettable
VAR
    Speed : INT;
END_VAR
METHOD PUBLIC Start
    Speed := 0;
END_METHOD
END_CLASS"#
    ));
}

#[test]
fn test_class_qualified_extends_implements() {
    insta::assert_snapshot!(snapshot_parse(
        r#"CLASS Motor EXTENDS Company.Base IMPLEMENTS Lib.IDevice, Lib.IResettable
END_CLASS"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 51 (INTERFACE declaration)
fn test_interface() {
    insta::assert_snapshot!(snapshot_parse(
        r#"INTERFACE IMotor
    METHOD Start : BOOL
    END_METHOD
    METHOD Stop
    END_METHOD
END_INTERFACE"#
    ));
}

#[test]
fn test_method_with_body() {
    insta::assert_snapshot!(snapshot_parse(
        r#"FUNCTION_BLOCK FB_Test
    METHOD PUBLIC DoWork : INT
    VAR_INPUT
        value : INT;
    END_VAR
        DoWork := value * 2;
    END_METHOD
END_FUNCTION_BLOCK"#
    ));
}

#[test]
fn test_property() {
    insta::assert_snapshot!(snapshot_parse(
        r#"FUNCTION_BLOCK FB_Test
    PROPERTY Value : INT
    GET
        Value := _value;
    END_GET
    SET
        _value := Value;
    END_SET
    END_PROPERTY
END_FUNCTION_BLOCK"#
    ));
}

#[test]
// IEC 61131-3 Ed.3 Table 56 / Table 72 (ACTION declaration)
fn test_action() {
    let source = r#"PROGRAM Test
    ACTION Reset:
        x := 0;
        y := 0;
    END_ACTION
END_PROGRAM"#;
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected ACTION declaration to parse, got: {:?}",
        parsed.errors()
    );
    insta::assert_snapshot!(snapshot_parse(source));
}

#[test]
// IEC 61131-3 Ed.3 Tables 64-66 (NAMESPACE declaration)
fn test_namespace() {
    insta::assert_snapshot!(snapshot_parse(
        r#"NAMESPACE MyLib
    FUNCTION Helper : INT
    END_FUNCTION
END_NAMESPACE"#
    ));
}

#[test]
fn test_namespace_with_var_global() {
    let parsed = parse(
        r#"NAMESPACE GVL
VAR_GLOBAL
    shared : INT := 1;
END_VAR
END_NAMESPACE"#,
    );
    assert!(
        parsed.ok(),
        "expected namespace VAR_GLOBAL to parse, got: {:?}",
        parsed.errors()
    );
    let syntax = parsed.syntax();
    let namespace = syntax
        .children()
        .find(|child| child.kind() == SyntaxKind::Namespace)
        .expect("expected namespace node");
    assert!(
        namespace
            .children()
            .any(|child| child.kind() == SyntaxKind::VarBlock),
        "expected namespace to contain VarBlock, got:\n{}",
        snapshot_parse(
            r#"NAMESPACE GVL
VAR_GLOBAL
    shared : INT := 1;
END_VAR
END_NAMESPACE"#
        )
    );
}

#[test]
// IEC 61131-3 Ed.3 Table 66 (USING directive)
fn test_using_directive() {
    insta::assert_snapshot!(snapshot_parse(
        r#"USING Standard.Timers, Counters;

PROGRAM Test
    USING Standard.Timers;
    VAR
        Ton1 : TON;
    END_VAR
END_PROGRAM"#
    ));
}

#[test]
fn test_namespace_qualified_name() {
    insta::assert_snapshot!(snapshot_parse(
        r#"NAMESPACE Company.Project
    FUNCTION Helper : INT
    END_FUNCTION
END_NAMESPACE"#
    ));
}

#[test]
fn test_configuration() {
    insta::assert_snapshot!(snapshot_parse(
        r#"CONFIGURATION Cell_1
VAR_GLOBAL
    gCounter : INT;
END_VAR
RESOURCE Station_1 ON Processor_A
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    TASK Slow (INTERVAL := T#20ms, PRIORITY := 2);
    PROGRAM P1 WITH Fast : MyProgram;
END_RESOURCE
VAR_ACCESS
    A1 : Station_1.P1.gCounter : INT READ_WRITE;
END_VAR
VAR_CONFIG
    Station_1.P1.gCounter : INT := 1;
END_VAR
END_CONFIGURATION"#
    ));
}

#[test]
fn test_var_access_with_index_and_bit() {
    let source = r#"CONFIGURATION Conf
VAR_ACCESS
    Acc1 : Station_1.P1.arr[1] : INT READ_ONLY;
    Acc2 : Station_1.P1.bits.0 : BOOL READ_WRITE;
END_VAR
END_CONFIGURATION"#;
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected indexed and bit-selected VAR_ACCESS paths to parse, got: {:?}",
        parsed.errors()
    );
    insta::assert_snapshot!(snapshot_parse(source));
}

#[test]
// e!COCKPIT dialect: PROGRAM-scoped METHOD (IEC 61131-3 scopes METHODs to
// classes/FBs; see docs/specs/04-pou-declarations.md §4 "Program methods").
// The raw imported form nests the METHOD inside the PROGRAM body.
fn test_program_with_method() {
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM MqttSubscribeMulti
VAR
    x : BOOL;
END_VAR
    x := StateToBool('1');
METHOD PUBLIC StateToBool : BOOL
VAR_INPUT
    sInput : STRING;
END_VAR
    StateToBool := sInput = '1';
END_METHOD
END_PROGRAM"#
    ));
}

#[test]
// Statements may appear before and after the program-scoped METHOD; the
// method is a declaration, not part of the statement stream.
fn test_program_method_between_statements() {
    let source = r#"PROGRAM Main
VAR
    count : INT;
END_VAR
    count := 1;
METHOD PRIVATE Bump : INT
    Bump := count + 1;
END_METHOD
    count := Bump();
END_PROGRAM"#;
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected PROGRAM with a mid-body METHOD to parse, got: {:?}",
        parsed.errors()
    );
    let syntax = parsed.syntax();
    let program = syntax
        .children()
        .find(|child| child.kind() == SyntaxKind::Program)
        .expect("expected program node");
    let method = program
        .descendants()
        .find(|node| node.kind() == SyntaxKind::Method)
        .expect("expected a Method node inside the program");
    assert!(method.text().to_string().contains("Bump"));
}

#[test]
// A METHOD line inside a PROGRAM body must reach the method parser (which
// reports its own grammar errors), not fall out of the body loop with
// "expected END_PROGRAM".
fn test_program_method_error_is_method_scoped() {
    let parsed = parse(
        r#"PROGRAM Test
METHOD 1Bad
END_PROGRAM"#,
    );
    assert!(!parsed.ok());
    assert!(
        parsed
            .errors()
            .iter()
            .any(|error| error.message == "expected method name"),
        "errors: {:?}",
        parsed.errors()
    );
    assert!(
        !parsed
            .errors()
            .iter()
            .any(|error| error.message == "expected END_PROGRAM"),
        "errors: {:?}",
        parsed.errors()
    );
}
