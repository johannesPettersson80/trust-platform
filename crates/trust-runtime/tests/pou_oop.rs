use trust_runtime::harness::TestHarness;

#[test]
fn polymorphism() {
    let source = r#"
CLASS BaseDevice
VAR PUBLIC
    base_val : INT := INT#5;
END_VAR
METHOD PUBLIC GetBase : INT
GetBase := base_val;
END_METHOD
END_CLASS

FUNCTION_BLOCK DerivedFromClass EXTENDS BaseDevice
VAR PUBLIC
    delta : INT := INT#2;
END_VAR
METHOD PUBLIC GetBasePlus : INT
GetBasePlus := base_val + delta;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK BaseFb
VAR PUBLIC
    count : INT := INT#10;
END_VAR
METHOD PUBLIC GetCount : INT
GetCount := count;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK DerivedFb EXTENDS BaseFb
VAR PUBLIC
    extra : INT := INT#3;
END_VAR
METHOD PUBLIC GetCount : INT
GetCount := count + extra;
END_METHOD
METHOD PUBLIC GetSuper : INT
GetSuper := SUPER.GetCount();
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb_class : DerivedFromClass;
    fb_derived : DerivedFb;
    out_base : INT := INT#0;
    out_plus : INT := INT#0;
    out_count : INT := INT#0;
    out_override : INT := INT#0;
    out_super : INT := INT#0;
END_VAR
out_base := fb_class.GetBase();
out_plus := fb_class.GetBasePlus();
out_count := fb_derived.count;
out_override := fb_derived.GetCount();
out_super := fb_derived.GetSuper();
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    harness.assert_eq("out_base", 5i16);
    harness.assert_eq("out_plus", 7i16);
    harness.assert_eq("out_count", 10i16);
    harness.assert_eq("out_override", 13i16);
    harness.assert_eq("out_super", 10i16);
}

/// IEC 61131-3 Edition 3, 6.6.5.3: a derived function block inherits its base's input and output
/// variables, so a call of the derived block binds them like its own (specification 04).
#[test]
fn a_derived_function_block_call_binds_inherited_inputs_and_outputs() {
    let source = r#"
FUNCTION_BLOCK Base
VAR_INPUT
    Gain : INT := 1;
END_VAR
VAR_OUTPUT
    Result : INT;
END_VAR
METHOD PROTECTED Apply
VAR_INPUT
    Input : INT;
END_VAR
    Result := Input * Gain;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Scaled EXTENDS Base
VAR_INPUT
    Value : INT;
END_VAR
    Apply(Input := Value);
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Block : Scaled;
    Named : INT;
    Formal : INT;
END_VAR
    Block(Value := 7, Gain := 3, Result => Formal);
    Named := Block.Result;
END_PROGRAM
"#;
    let harness = TestHarness::from_source(source);
    assert!(
        harness.is_ok(),
        "inherited inputs and outputs are parameters of the derived block: {:?}",
        harness.as_ref().err()
    );
    let mut harness = harness.expect("compiled");
    harness.cycle();
    harness.assert_eq("Named", 21i16);
    harness.assert_eq("Formal", 21i16);
}

/// A function block may extend a base declared in a namespace; its body calls the base's methods
/// and its callers bind the inherited parameters (IEC 61131-3 Edition 3, 6.6.5.3, Tables 64–66).
#[test]
fn a_function_block_extends_a_base_in_a_namespace() {
    let library = r#"
NAMESPACE Lib
FUNCTION_BLOCK Base
VAR_INPUT
    Gain : INT := 1;
END_VAR
VAR_OUTPUT
    Result : INT;
END_VAR
METHOD PROTECTED Apply
VAR_INPUT
    Input : INT;
END_VAR
    Result := Input * Gain;
END_METHOD
END_FUNCTION_BLOCK
END_NAMESPACE
"#;
    let program = r#"
FUNCTION_BLOCK Scaled EXTENDS Lib.Base
VAR_INPUT
    Value : INT;
END_VAR
    Apply(Input := Value);
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Block : Scaled;
    Out : INT;
END_VAR
    Block(Value := 7, Gain := 3);
    Out := Block.Result;
END_PROGRAM
"#;
    let mut harness = TestHarness::from_sources(&[library, program]).expect("compiled");
    let result = harness.cycle();
    assert!(
        result.errors.is_empty(),
        "the scan runs: {:?}",
        result.errors
    );
    harness.assert_eq("Out", 21i16);
}
