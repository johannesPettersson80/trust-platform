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
    insta::assert_snapshot!(snapshot_parse(
        r#"PROGRAM Test
    ACTION Reset
        x := 0;
        y := 0;
    END_ACTION
END_PROGRAM"#
    ));
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
    insta::assert_snapshot!(snapshot_parse(
        r#"CONFIGURATION Conf
VAR_ACCESS
    Acc1 : Station_1.P1.arr[1] : INT READ_ONLY;
    Acc2 : Station_1.P1.word.0 : BOOL READ_WRITE;
END_VAR
END_CONFIGURATION"#
    ));
}
