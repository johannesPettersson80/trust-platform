use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;
use trust_runtime::RestartMode;

fn struct_field<'a>(value: &'a Value, name: &str) -> &'a Value {
    let Value::Struct(struct_value) = value else {
        panic!("expected struct value, got {value:?}");
    };
    struct_value
        .fields()
        .get(name)
        .unwrap_or_else(|| panic!("missing struct field {name} in {struct_value:?}"))
}

fn instance_field<'a>(harness: &'a TestHarness, value: &Value, name: &str) -> &'a Value {
    let Value::Instance(instance_id) = value else {
        panic!("expected instance value, got {value:?}");
    };
    harness
        .runtime()
        .storage()
        .get_instance_var(*instance_id, name)
        .unwrap_or_else(|| panic!("missing instance field {name}"))
}

#[test]
fn named_struct_initializer_materializes_runtime_value() {
    let source = r#"
TYPE
    StepCfg : STRUCT
        cyl : INT;
        ext : BOOL;
    END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    cfg : StepCfg := (cyl := 5, ext := TRUE);
END_VAR
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    let cfg = harness.get_output("cfg").expect("cfg should exist");
    assert_eq!(struct_field(&cfg, "cyl"), &Value::Int(5));
    assert_eq!(struct_field(&cfg, "ext"), &Value::Bool(true));
}

#[test]
fn type_name_call_initializer_materializes_runtime_value() {
    let source = r#"
TYPE
    StepCfg : STRUCT
        cyl : INT;
        ext : BOOL;
    END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    cfg : StepCfg := StepCfg(cyl := 11, ext := TRUE);
END_VAR
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    let cfg = harness.get_output("cfg").expect("cfg should exist");
    assert_eq!(struct_field(&cfg, "cyl"), &Value::Int(11));
    assert_eq!(struct_field(&cfg, "ext"), &Value::Bool(true));
}

#[test]
fn multi_name_struct_initializer_values_are_independent() {
    let source = r#"
TYPE
    StepCfg : STRUCT
        x : INT;
        y : INT;
    END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    a, b : StepCfg := (x := 1, y := 2);
END_VAR
a.x := INT#5;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    let a = harness.get_output("a").expect("a should exist");
    let b = harness.get_output("b").expect("b should exist");
    assert_eq!(struct_field(&a, "x"), &Value::Int(5));
    assert_eq!(struct_field(&b, "x"), &Value::Int(1));
    assert_eq!(struct_field(&b, "y"), &Value::Int(2));
}

#[test]
fn struct_field_defaults_feed_default_and_partial_aggregate_values() {
    let source = r#"
TYPE
    StepCfg : STRUCT
        cyl : INT := 2;
        ext : BOOL := TRUE;
    END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    from_type : StepCfg;
    partial : StepCfg := (cyl := 7);
END_VAR
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    let from_type = harness
        .get_output("from_type")
        .expect("from_type should exist");
    let partial = harness.get_output("partial").expect("partial should exist");
    assert_eq!(struct_field(&from_type, "cyl"), &Value::Int(2));
    assert_eq!(struct_field(&from_type, "ext"), &Value::Bool(true));
    assert_eq!(struct_field(&partial, "cyl"), &Value::Int(7));
    assert_eq!(struct_field(&partial, "ext"), &Value::Bool(true));
}

#[test]
fn array_of_structs_and_repetition_materialize_defaults() {
    let source = r#"
TYPE
    StepCfg : STRUCT
        x : INT := 2;
        y : INT := 3;
    END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    arr : ARRAY[1..3] OF StepCfg := [3((x := 7))];
    firstX : INT;
    firstY : INT;
    thirdX : INT;
END_VAR
firstX := arr[1].x;
firstY := arr[1].y;
thirdX := arr[3].x;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    assert_eq!(harness.get_output("firstX"), Some(Value::Int(7)));
    assert_eq!(harness.get_output("firstY"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("thirdX"), Some(Value::Int(7)));
}

#[test]
fn var_global_and_direct_address_aggregate_initializers_materialize() {
    let source = r#"
TYPE
    StepCfg : STRUCT
        x : INT := 2;
    END_STRUCT;
END_TYPE

VAR_GLOBAL
    gCfg AT %QW10 : StepCfg := (x := 9);
END_VAR

PROGRAM Main
VAR
    observed : INT;
END_VAR
observed := gCfg.x;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    assert_eq!(harness.get_output("observed"), Some(Value::Int(9)));
}

#[test]
fn type_level_aggregate_default_materializes_alias_value() {
    let source = r#"
TYPE
    StepCfg : STRUCT
        cyl : INT;
        ext : BOOL;
    END_STRUCT;
    DefaultStep : StepCfg := (cyl := 9, ext := TRUE);
END_TYPE

PROGRAM Main
VAR
    cfg : DefaultStep;
END_VAR
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    let cfg = harness.get_output("cfg").expect("cfg should exist");
    assert_eq!(struct_field(&cfg, "cyl"), &Value::Int(9));
    assert_eq!(struct_field(&cfg, "ext"), &Value::Bool(true));
}

#[test]
fn type_level_array_of_struct_default_materializes() {
    let source = r#"
TYPE
    StepCfg : STRUCT
        x : INT := 1;
    END_STRUCT;
    StepArray : ARRAY[1..2] OF StepCfg := [2((x := 8))];
END_TYPE

PROGRAM Main
VAR
    arr : StepArray;
    observed : INT;
END_VAR
observed := arr[2].x;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    assert_eq!(harness.get_output("observed"), Some(Value::Int(8)));
}

#[test]
fn union_variant_default_materializes() {
    let source = r#"
TYPE
    U : UNION
        a : INT := 6;
    END_UNION;
END_TYPE

PROGRAM Main
VAR
    u : U;
    observed : INT;
END_VAR
observed := u.a;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    assert_eq!(harness.get_output("observed"), Some(Value::Int(6)));
}

#[test]
fn variable_level_ref_initializer_materializes_reference() {
    let source = r#"
PROGRAM Main
VAR
    target : INT := 5;
    ref_value : REF_TO INT := REF(target);
    observed : INT;
END_VAR
observed := ref_value^;
ref_value^ := 8;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    assert_eq!(harness.get_output("observed"), Some(Value::Int(5)));
    assert_eq!(harness.get_output("target"), Some(Value::Int(8)));
    assert!(matches!(
        harness.get_output("ref_value"),
        Some(Value::Reference(Some(_)))
    ));
}

#[test]
fn self_referential_ref_to_field_defaults_to_null_without_recursive_expansion() {
    let source = r#"
TYPE
    Node : STRUCT
        value : INT := 9;
        next : REF_TO Node;
    END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    node : Node;
END_VAR
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    let node = harness.get_output("node").expect("node should exist");
    assert_eq!(struct_field(&node, "value"), &Value::Int(9));
    assert!(matches!(
        struct_field(&node, "next"),
        Value::Reference(None) | Value::Null
    ));
}

#[test]
fn case_insensitive_field_matching_materializes_same_value() {
    let source = r#"
TYPE
    StepCfg : STRUCT
        x : INT;
    END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    cfg : StepCfg := (X := 10);
END_VAR
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();
    let cfg = harness.get_output("cfg").expect("cfg should exist");
    assert_eq!(struct_field(&cfg, "x"), &Value::Int(10));
}

#[test]
fn retained_struct_value_wins_over_defaults_on_warm_restart() {
    let source = r#"
TYPE
    StepCfg : STRUCT
        x : INT := 2;
    END_STRUCT;
END_TYPE

PROGRAM Main
VAR RETAIN
    cfg : StepCfg := (x := 5);
END_VAR
cfg.x := cfg.x + 1;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();
    assert_eq!(
        struct_field(&harness.get_output("cfg").expect("cfg"), "x"),
        &Value::Int(6)
    );

    harness.restart(RestartMode::Warm).expect("warm restart");
    assert_eq!(
        struct_field(&harness.get_output("cfg").expect("cfg"), "x"),
        &Value::Int(6)
    );

    harness.restart(RestartMode::Cold).expect("cold restart");
    assert_eq!(
        struct_field(&harness.get_output("cfg").expect("cfg"), "x"),
        &Value::Int(5)
    );
}

#[test]
fn var_config_aggregate_override_wins_over_defaults() {
    let source = r#"
TYPE
    StepCfg : STRUCT
        x : INT := 2;
        y : INT := 3;
    END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    cfg : StepCfg := (x := 4);
    observedX : INT;
    observedY : INT;
END_VAR
observedX := cfg.x;
observedY := cfg.y;
END_PROGRAM

CONFIGURATION Conf
PROGRAM P1 : Main;
VAR_CONFIG
    P1.cfg : StepCfg := (x := 9);
END_VAR
END_CONFIGURATION
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    assert_eq!(harness.get_output("observedX"), Some(Value::Int(9)));
    assert_eq!(harness.get_output("observedY"), Some(Value::Int(3)));
}

#[test]
fn global_fb_initializer_applies_allowed_member_overrides() {
    let source = r#"
FUNCTION_BLOCK InitFb
VAR_INPUT
    preset : INT;
END_VAR
VAR_OUTPUT
    observed : INT;
END_VAR
observed := preset;
END_FUNCTION_BLOCK

VAR_GLOBAL
    gFb : InitFb := (preset := 12);
END_VAR

PROGRAM Main
VAR
    observed : INT;
END_VAR
gFb();
observed := gFb.observed;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    assert_eq!(harness.get_output("observed"), Some(Value::Int(12)));
}

#[test]
fn function_block_instance_initializer_applies_allowed_member_overrides() {
    let source = r#"
FUNCTION_BLOCK InitFb
VAR_INPUT
    enable : BOOL;
END_VAR
VAR_OUTPUT
    count : INT;
END_VAR
VAR PUBLIC
    local : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    step_fb : InitFb := (enable := TRUE, count := 3, local := 4);
END_VAR
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    let step_fb = harness.get_output("step_fb").expect("step_fb should exist");
    assert_eq!(
        instance_field(&harness, &step_fb, "enable"),
        &Value::Bool(true)
    );
    assert_eq!(instance_field(&harness, &step_fb, "count"), &Value::Int(3));
    assert_eq!(instance_field(&harness, &step_fb, "local"), &Value::Int(4));
}

#[test]
fn function_block_instance_initializer_rejects_var_in_out_member() {
    let source = r#"
FUNCTION_BLOCK InitFb
VAR_IN_OUT
    shared : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    step_fb : InitFb := (shared := 1);
END_VAR
END_PROGRAM
"#;

    let err = match TestHarness::from_source(source) {
        Ok(_) => panic!("VAR_IN_OUT aggregate target should be rejected"),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("VAR_IN_OUT"),
        "unexpected error: {err}"
    );
}

#[test]
fn function_local_fb_initializer_applies_in_vm_local_init() {
    let source = r#"
FUNCTION_BLOCK InitFb
VAR_INPUT
    enable : BOOL;
END_VAR
VAR_OUTPUT
    count : INT;
END_VAR
VAR PUBLIC
    local : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION Probe : INT
VAR
    fb : InitFb := (enable := TRUE, count := 3, local := 4);
END_VAR
Probe := fb.count + fb.local;
END_FUNCTION

PROGRAM Main
VAR
    observed : INT;
END_VAR
observed := Probe();
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    assert_eq!(harness.get_output("observed"), Some(Value::Int(7)));
}

#[test]
fn function_local_initializers_can_read_vm_frame_params_and_prior_locals() {
    let source = r#"
FUNCTION Probe : INT
VAR_INPUT
    seed : INT;
END_VAR
VAR
    first : INT := seed + INT#2;
    second : INT := first + INT#3;
END_VAR
Probe := second;
END_FUNCTION

PROGRAM Main
VAR
    observed : INT;
END_VAR
observed := Probe(INT#5);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    assert_eq!(harness.get_output("observed"), Some(Value::Int(10)));
}

#[test]
fn multi_name_fb_initializer_instances_are_independent() {
    let source = r#"
FUNCTION_BLOCK InitFb
VAR PUBLIC
    local : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    a, b : InitFb := (local := 1);
END_VAR
a.local := 5;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("program should compile");
    harness.cycle();

    let a = harness.get_output("a").expect("a should exist");
    let b = harness.get_output("b").expect("b should exist");
    assert_eq!(instance_field(&harness, &a, "local"), &Value::Int(5));
    assert_eq!(instance_field(&harness, &b, "local"), &Value::Int(1));
}
