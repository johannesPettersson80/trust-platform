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
