use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn access_path_mapping() {
    let source = r#"
TYPE S :
STRUCT
    x : INT;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    out AT %Q* : BOOL;
    arr : ARRAY[0..1] OF INT;
    st : S;
    out_arr : INT;
    out_st : INT;
END_VAR
out := TRUE;
A1 := INT#10;
out_arr := arr[1];
out_st := st.x;
END_PROGRAM

CONFIGURATION Conf
PROGRAM P1 : Main;
VAR_ACCESS
    A1 : P1.arr[1] : INT READ_WRITE;
END_VAR
VAR_CONFIG
    out AT %QX0.1 : BOOL;
    P1.st.x : INT := INT#42;
END_VAR
END_CONFIGURATION
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();

    assert_eq!(harness.get_output("out_arr"), Some(Value::Int(10)));
    assert_eq!(harness.get_output("out_st"), Some(Value::Int(42)));
    assert_eq!(
        harness.get_direct_output("%QX0.1").unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn var_access_global_name_collision_fails_before_bytecode() {
    let source = r#"
VAR_GLOBAL
    A1 : INT := INT#111;
END_VAR

PROGRAM Main
VAR
    source : INT := INT#7;
    observed : INT := INT#0;
END_VAR
A1 := INT#55;
observed := A1;
END_PROGRAM

CONFIGURATION Conf
PROGRAM P1 : Main;
VAR_ACCESS
    A1 : P1.source : INT READ_WRITE;
END_VAR
END_CONFIGURATION
"#;

    let Err(err) = TestHarness::from_source(source) else {
        panic!("global/VAR_ACCESS name collision must fail before bytecode lowering");
    };
    let message = format!("{err:?}");
    assert!(
        message.contains("E104") && message.contains("duplicate declaration of 'A1'"),
        "expected duplicate declaration diagnostic, got {message}"
    );
    assert!(
        message.contains("E105") && message.contains("VAR_ACCESS declaration 'A1' is ambiguous"),
        "expected ambiguous VAR_ACCESS diagnostic, got {message}"
    );
}

#[test]
fn encoder_rejects_forced_access_map_binding_that_shadows_global_name() {
    let source = r#"
VAR_GLOBAL
    A1 : INT := INT#111;
END_VAR

PROGRAM Main
VAR
    source : INT := INT#7;
    observed : INT := INT#0;
END_VAR
A1 := INT#55;
observed := A1;
END_PROGRAM

CONFIGURATION Conf
PROGRAM P1 : Main;
END_CONFIGURATION
"#;

    let mut runtime = TestHarness::from_source(source)
        .expect("source-level global without VAR_ACCESS collision should compile")
        .into_runtime();
    let p1_instance = match runtime.storage().get_global("P1") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected P1 global to hold program instance id, got {other:?}"),
    };
    let source_ref = runtime
        .storage()
        .ref_for_instance_recursive(p1_instance, "source")
        .expect("P1.source ref");
    runtime.access_map_mut().bind("A1".into(), source_ref, None);

    let err = trust_runtime::bytecode::BytecodeModule::from_runtime(&runtime)
        .expect_err("forced VAR_ACCESS/global name shadowing must fail during bytecode encoding");
    let message = err.to_string();
    assert!(
        message.contains("VAR_ACCESS") && message.contains("A1") && message.contains("global"),
        "expected VAR_ACCESS/global collision diagnostic, got {message}"
    );
}

#[test]
fn var_config_memory_binding_syncs_with_program_storage() {
    let source = r#"
PROGRAM Main
VAR
    counter : INT;
    observed : INT;
END_VAR
observed := counter;
counter := counter + 1;
END_PROGRAM

CONFIGURATION Conf
PROGRAM P1 : Main;
VAR_CONFIG
    P1.counter AT %MW0 : INT;
END_VAR
END_CONFIGURATION
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness
        .set_direct_input("%MW0", Value::Word(41))
        .expect("seed marker memory");

    harness.cycle();

    assert_eq!(harness.get_output("observed"), Some(Value::Int(41)));
    assert_eq!(harness.get_output("counter"), Some(Value::Int(42)));
    assert_eq!(harness.get_direct_output("%MW0").unwrap(), Value::Word(42));
}

#[test]
fn memory_variants_sync_via_var_config_wildcards() {
    let source = r#"
PROGRAM Main
VAR
    mem_bit AT %M* : BOOL;
    mem_byte AT %M* : USINT;
    mem_word AT %M* : INT;
    mem_dword AT %M* : DINT;
    mem_lword AT %M* : LINT;
    obs_bit : BOOL;
    obs_byte : USINT;
    obs_word : INT;
    obs_dword : DINT;
    obs_lword : LINT;
END_VAR

obs_bit := mem_bit;
obs_byte := mem_byte;
obs_word := mem_word;
obs_dword := mem_dword;
obs_lword := mem_lword;

mem_bit := FALSE;
mem_byte := USINT#7;
mem_word := INT#300;
mem_dword := DINT#70000;
mem_lword := LINT#5000000000;
END_PROGRAM

CONFIGURATION Conf
PROGRAM P1 : Main;
VAR_CONFIG
    P1.mem_bit AT %MX0.0 : BOOL;
    P1.mem_byte AT %MB1 : USINT;
    P1.mem_word AT %MW2 : INT;
    P1.mem_dword AT %MD4 : DINT;
    P1.mem_lword AT %ML8 : LINT;
END_VAR
END_CONFIGURATION
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness
        .set_direct_input("%MX0.0", Value::Bool(true))
        .expect("seed %MX");
    harness
        .set_direct_input("%MB1", Value::Byte(9))
        .expect("seed %MB");
    harness
        .set_direct_input("%MW2", Value::Word(41))
        .expect("seed %MW");
    harness
        .set_direct_input("%MD4", Value::DWord(123_456))
        .expect("seed %MD");
    harness
        .set_direct_input("%ML8", Value::LWord(6_000_000_000))
        .expect("seed %ML");

    harness.cycle();

    assert_eq!(harness.get_output("obs_bit"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("obs_byte"), Some(Value::USInt(9)));
    assert_eq!(harness.get_output("obs_word"), Some(Value::Int(41)));
    assert_eq!(harness.get_output("obs_dword"), Some(Value::DInt(123_456)));
    assert_eq!(
        harness.get_output("obs_lword"),
        Some(Value::LInt(6_000_000_000))
    );

    assert_eq!(
        harness.get_direct_output("%MX0.0").unwrap(),
        Value::Bool(false)
    );
    assert_eq!(harness.get_direct_output("%MB1").unwrap(), Value::Byte(7));
    assert_eq!(harness.get_direct_output("%MW2").unwrap(), Value::Word(300));
    assert_eq!(
        harness.get_direct_output("%MD4").unwrap(),
        Value::DWord(70_000)
    );
    assert_eq!(
        harness.get_direct_output("%ML8").unwrap(),
        Value::LWord(5_000_000_000)
    );
}

#[test]
fn file_scope_globals_are_shared_across_program_and_function_blocks() {
    let gvl = r#"
VAR_GLOBAL
    shared : INT := 0;
END_VAR
"#;
    let support = r#"
FUNCTION_BLOCK Bump
VAR_EXTERNAL
    shared : INT;
END_VAR
shared := shared + 1;
END_FUNCTION_BLOCK
"#;
    let main = r#"
PROGRAM Main
VAR
    bump1 : Bump;
    bump2 : Bump;
    observed : INT;
END_VAR
VAR_EXTERNAL
    shared : INT;
END_VAR
bump1();
bump2();
observed := shared;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_sources(&[gvl, support, main]).unwrap();
    harness.cycle();

    match harness.get_output("observed") {
        Some(Value::Int(value)) => assert_eq!(i64::from(value), 2),
        Some(Value::DInt(value)) => assert_eq!(i64::from(value), 2),
        Some(Value::LInt(value)) => assert_eq!(value, 2),
        other => panic!("unexpected observed value {other:?}"),
    }
}

#[test]
fn namespaced_globals_support_qualified_access() {
    let gvl = r#"
NAMESPACE GVL
VAR_GLOBAL
    shared : INT := 3;
END_VAR
END_NAMESPACE
"#;
    let main = r#"
PROGRAM Main
VAR
    observed : INT;
END_VAR
observed := GVL.shared;
GVL.shared := GVL.shared + 1;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_sources(&[gvl, main]).unwrap();
    harness.cycle();

    assert_eq!(harness.get_output("observed"), Some(Value::Int(3)));
    match harness.runtime().storage().get_global("GVL.shared") {
        Some(Value::Int(value)) => assert_eq!(i64::from(*value), 4),
        Some(Value::DInt(value)) => assert_eq!(i64::from(*value), 4),
        Some(Value::LInt(value)) => assert_eq!(*value, 4),
        other => panic!("unexpected namespaced global value {other:?}"),
    }
}

#[test]
fn globals_are_accessible_without_var_external_across_vendor_parity_scopes() {
    let file_gvl = r#"
VAR_GLOBAL
    gFile : INT := 1;
END_VAR
"#;
    let namespaced_gvl = r#"
NAMESPACE GVL
VAR_GLOBAL
    shared : INT := 2;
END_VAR
END_NAMESPACE
"#;
    let main = r#"
PROGRAM Main
VAR_GLOBAL
    gProgram : INT := 3;
END_VAR
VAR
    observedFile : INT;
    observedProgram : INT;
    observedNamespace : INT;
    observedConfig : INT;
END_VAR
observedFile := gFile;
observedProgram := gProgram;
observedNamespace := GVL.shared;
observedConfig := gConfig;
gFile := gFile + 1;
gProgram := gProgram + 1;
GVL.shared := GVL.shared + 1;
gConfig := gConfig + 1;
END_PROGRAM
"#;
    let configuration = r#"
CONFIGURATION Conf
VAR_GLOBAL
    gConfig : INT := 4;
END_VAR
PROGRAM P1 : Main;
END_CONFIGURATION
"#;

    let mut harness =
        TestHarness::from_sources(&[file_gvl, namespaced_gvl, main, configuration]).unwrap();
    harness.cycle();

    assert_eq!(harness.get_output("observedFile"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("observedProgram"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("observedNamespace"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("observedConfig"), Some(Value::Int(4)));

    match harness.runtime().storage().get_global("gFile") {
        Some(Value::Int(value)) => assert_eq!(i64::from(*value), 2),
        Some(Value::DInt(value)) => assert_eq!(i64::from(*value), 2),
        Some(Value::LInt(value)) => assert_eq!(*value, 2),
        other => panic!("unexpected file-scope global value {other:?}"),
    }
    match harness.runtime().storage().get_global("gConfig") {
        Some(Value::Int(value)) => assert_eq!(i64::from(*value), 5),
        Some(Value::DInt(value)) => assert_eq!(i64::from(*value), 5),
        Some(Value::LInt(value)) => assert_eq!(*value, 5),
        other => panic!("unexpected configuration global value {other:?}"),
    }
    match harness.runtime().storage().get_global("GVL.shared") {
        Some(Value::Int(value)) => assert_eq!(i64::from(*value), 3),
        Some(Value::DInt(value)) => assert_eq!(i64::from(*value), 3),
        Some(Value::LInt(value)) => assert_eq!(*value, 3),
        other => panic!("unexpected namespaced global value {other:?}"),
    }
}
