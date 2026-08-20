use super::*;

use crate::harness::{CompileSession, TestHarness, WildcardRequirement};
use crate::memory::{IoArea, MemoryLocation};
use crate::value::{Value, ValueRef};

fn compile_runtime(source: &str) -> Runtime {
    CompileSession::from_source(source)
        .build_runtime()
        .unwrap_or_else(|error| panic!("source must compile: {error}"))
}

fn compile_error(source: &str) -> String {
    match CompileSession::from_source(source).build_runtime() {
        Ok(_) => panic!("source must fail compilation"),
        Err(error) => error.to_string(),
    }
}

fn assert_error_contains(source: &str, expected: &str) {
    let error = compile_error(source);
    assert!(
        error.contains(expected),
        "expected error containing {expected:?}, got {error:?}"
    );
}

#[test]
fn configuration_contract_symbolic_var_access_reads_and_writes_nested_array() {
    let mut harness = TestHarness::from_source(
        r#"
TYPE Row :
STRUCT
    cells : ARRAY[1..2] OF INT;
END_STRUCT
END_TYPE
PROGRAM Main
VAR
    row : Row;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    Selected : P.row.cells[2] : INT READ_WRITE;
END_VAR
END_CONFIGURATION
"#,
    )
    .expect("nested access must compile");
    harness
        .set_access("Selected", Value::Int(37))
        .expect("access write");
    assert_eq!(harness.get_access("Selected"), Some(Value::Int(37)));
}

#[test]
fn configuration_contract_var_access_global_root_precedes_program_field() {
    let mut harness = TestHarness::from_source(
        r#"
VAR_GLOBAL
    Shared : INT := INT#5;
END_VAR
PROGRAM Main
VAR
    Shared : INT := INT#99;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    Alias : Shared : INT READ_WRITE;
END_VAR
END_CONFIGURATION
"#,
    )
    .expect("global-root access must compile");
    assert_eq!(harness.get_access("Alias"), Some(Value::Int(5)));
    harness
        .set_access("Alias", Value::Int(8))
        .expect("write alias");
    assert_eq!(
        harness.runtime().storage().get_global("Shared"),
        Some(&Value::Int(8))
    );
}

#[test]
fn configuration_contract_ambiguous_unqualified_program_field_is_rejected() {
    assert_error_contains(
        r#"
PROGRAM First
VAR
    Shared : INT;
END_VAR
END_PROGRAM
PROGRAM Second
VAR
    Shared : INT;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P1 : First;
PROGRAM P2 : Second;
VAR_ACCESS
    Alias : Shared : INT READ_WRITE;
END_VAR
END_CONFIGURATION
"#,
        "unresolved access path",
    );
}

#[test]
fn configuration_contract_invalid_access_array_index_is_rejected() {
    assert_error_contains(
        r#"
PROGRAM Main
VAR
    values : ARRAY[1..2] OF INT;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    Alias : P.values[3] : INT READ_WRITE;
END_VAR
END_CONFIGURATION
"#,
        "invalid access path index",
    );
}

#[test]
fn configuration_contract_direct_var_access_creates_bound_global_not_alias() {
    let runtime = compile_runtime(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    DirectWord : %MW4 : WORD READ_WRITE;
END_VAR
END_CONFIGURATION
"#,
    );
    assert!(runtime.storage().get_global("DirectWord").is_some());
    assert!(runtime.access_map().get("DirectWord").is_none());
    assert!(runtime
        .io()
        .bindings()
        .iter()
        .any(|binding| binding.display_name.as_deref() == Some("DirectWord")));
}

#[test]
fn configuration_contract_runtime_wildcard_guard_sorts_and_deduplicates_names() {
    let reference = ValueRef {
        location: MemoryLocation::Global,
        offset: 0,
        path: Vec::new(),
    };
    let requirements = vec![
        WildcardRequirement {
            name: "Zed".into(),
            reference: reference.clone(),
            area: IoArea::Output,
        },
        WildcardRequirement {
            name: "Alpha".into(),
            reference: reference.clone(),
            area: IoArea::Input,
        },
        WildcardRequirement {
            name: "Zed".into(),
            reference,
            area: IoArea::Output,
        },
    ];
    let error = ensure_wildcards_resolved(&requirements)
        .expect_err("outstanding wildcards must fail")
        .to_string();
    assert_eq!(
        error,
        "missing VAR_CONFIG address for wildcard variables: Alpha, Zed"
    );
}

#[test]
fn configuration_contract_var_config_address_must_not_be_wildcard() {
    assert_error_contains(
        r#"
PROGRAM Main
VAR
    Output AT %Q* : BOOL;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_CONFIG
    P.Output AT %Q* : BOOL;
END_VAR
END_CONFIGURATION
"#,
        "VAR_CONFIG must provide a fully specified direct address",
    );
}

#[test]
fn configuration_contract_var_config_address_area_must_match_wildcard() {
    assert_error_contains(
        r#"
PROGRAM Main
VAR
    Output AT %Q* : BOOL;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_CONFIG
    P.Output AT %IX0.0 : BOOL;
END_VAR
END_CONFIGURATION
"#,
        "VAR_CONFIG address area mismatch",
    );
}

#[test]
fn configuration_contract_var_config_at_rejects_partial_target() {
    assert_error_contains(
        r#"
PROGRAM Main
VAR
    WordValue : WORD;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_CONFIG
    P.WordValue.%X0 AT %QX0.0 : BOOL;
END_VAR
END_CONFIGURATION
"#,
        "VAR_CONFIG target must be a variable access path",
    );
}

#[test]
fn configuration_contract_var_config_at_rejects_direct_target() {
    assert_error_contains(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_CONFIG
    %MW0 AT %MW2 : WORD;
END_VAR
END_CONFIGURATION
"#,
        "VAR_CONFIG target must be a variable access path",
    );
}

#[test]
fn configuration_contract_var_config_partial_initializer_is_rejected() {
    assert_error_contains(
        r#"
PROGRAM Main
VAR
    WordValue : WORD := WORD#16#0034;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_CONFIG
    P.WordValue.%B1 : BYTE := BYTE#16#12;
END_VAR
END_CONFIGURATION
"#,
        "VAR_CONFIG target must be a variable access path",
    );
}

#[test]
fn configuration_contract_var_config_initializer_overrides_program_initializer() {
    let harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR
    Value : INT := INT#3;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_CONFIG
    P.Value : INT := INT#17;
END_VAR
END_CONFIGURATION
"#,
    )
    .expect("configuration initializer must compile");
    assert_eq!(harness.get_output("Value"), Some(Value::Int(17)));
}

#[test]
fn configuration_contract_process_images_cover_highest_complete_binding_span() {
    let runtime = compile_runtime(
        r#"
VAR_GLOBAL
    InputLong AT %IL5 : LINT;
    OutputDword AT %QD7 : DWORD;
    MemoryWord AT %MW9 : WORD;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    );

    assert_eq!(runtime.io().inputs().len(), 13);
    assert_eq!(runtime.io().outputs().len(), 11);
    assert_eq!(runtime.io().memory().len(), 11);
}

#[test]
fn configuration_contract_wildcard_binding_crosses_runtime_cycle() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR
    Output AT %Q* : WORD;
END_VAR
Output := WORD#1234;
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_CONFIG
    P.Output AT %QW6 : WORD;
END_VAR
END_CONFIGURATION
"#,
    )
    .expect("resolved wildcard must compile");
    harness.cycle();
    assert_eq!(
        harness.get_direct_output("%QW6").expect("direct output"),
        Value::Word(1234)
    );
}

#[test]
fn configuration_contract_explicit_read_only_access_rejects_atomic_write() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR
    Value : INT := INT#5;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    Observed : P.Value : INT READ_ONLY;
END_VAR
END_CONFIGURATION
"#,
    )
    .expect("read-only access must compile");
    assert_eq!(harness.get_access("Observed"), Some(Value::Int(5)));
    let error = harness
        .set_access("Observed", Value::Int(9))
        .expect_err("read-only access must reject writes");
    assert!(
        error.to_string().to_ascii_lowercase().contains("read-only"),
        "{error}"
    );
    assert_eq!(harness.get_access("Observed"), Some(Value::Int(5)));
}

#[test]
fn configuration_contract_omitted_access_direction_defaults_to_read_only() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR
    Value : INT := INT#6;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    Observed : P.Value : INT;
END_VAR
END_CONFIGURATION
"#,
    )
    .expect("default-direction access must compile");
    let error = harness
        .set_access("Observed", Value::Int(12))
        .expect_err("omitted direction must be read-only");
    assert!(
        error.to_string().to_ascii_lowercase().contains("read-only"),
        "{error}"
    );
    assert_eq!(harness.get_access("Observed"), Some(Value::Int(6)));
}

#[test]
fn configuration_contract_read_write_access_mutates_exact_target() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR
    First : INT := INT#1;
    Second : INT := INT#2;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    Writable : P.Second : INT READ_WRITE;
END_VAR
END_CONFIGURATION
"#,
    )
    .expect("read-write access must compile");
    harness
        .set_access("Writable", Value::Int(14))
        .expect("read-write access");
    assert_eq!(harness.get_access("Writable"), Some(Value::Int(14)));
    assert_eq!(harness.get_output("First"), Some(Value::Int(1)));
}

#[test]
fn configuration_contract_constant_target_remains_read_only() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR CONSTANT
    Limit : INT := INT#8;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    LimitAccess : P.Limit : INT READ_WRITE;
END_VAR
END_CONFIGURATION
"#,
    )
    .expect("constant access must compile as read-only");
    let error = harness
        .set_access("LimitAccess", Value::Int(99))
        .expect_err("constant access cannot become writable");
    assert!(
        error.to_string().to_ascii_lowercase().contains("read-only")
            || error.to_string().to_ascii_lowercase().contains("constant"),
        "{error}"
    );
    assert_eq!(harness.get_access("LimitAccess"), Some(Value::Int(8)));
}

#[test]
fn configuration_contract_var_access_type_must_match_target() {
    assert_error_contains(
        r#"
PROGRAM Main
VAR
    Value : DINT;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    WrongType : P.Value : INT READ_ONLY;
END_VAR
END_CONFIGURATION
"#,
        "does not match access path type",
    );
}

#[test]
fn configuration_contract_var_access_rejects_temp_target() {
    let error = compile_error(
        r#"
PROGRAM Main
VAR_TEMP
    Scratch : INT;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    ScratchAccess : P.Scratch : INT READ_ONLY;
END_VAR
END_CONFIGURATION
"#,
    );
    assert!(
        error.contains("VAR_TEMP")
            || error.to_ascii_lowercase().contains("temporary")
            || error.to_ascii_lowercase().contains("cannot expose"),
        "{error}"
    );
}

#[test]
fn configuration_contract_var_access_rejects_external_target() {
    let error = compile_error(
        r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
PROGRAM Main
VAR_EXTERNAL
    Shared : INT;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    SharedAccess : P.Shared : INT READ_ONLY;
END_VAR
END_CONFIGURATION
"#,
    );
    assert!(
        error.contains("VAR_EXTERNAL")
            || error.to_ascii_lowercase().contains("external")
            || error.to_ascii_lowercase().contains("cannot expose"),
        "{error}"
    );
}

#[test]
fn configuration_contract_var_access_rejects_in_out_target() {
    let error = compile_error(
        r#"
PROGRAM Main
VAR_IN_OUT
    Linked : INT;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    LinkedAccess : P.Linked : INT READ_ONLY;
END_VAR
END_CONFIGURATION
"#,
    );
    assert!(
        error.contains("VAR_IN_OUT")
            || error.to_ascii_lowercase().contains("in-out")
            || error.to_ascii_lowercase().contains("cannot expose"),
        "{error}"
    );
}

#[test]
fn configuration_contract_var_access_names_are_case_insensitively_unique() {
    let error = compile_error(
        r#"
PROGRAM Main
VAR
    First : INT;
    Second : INT;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    SharedAccess : P.First : INT READ_ONLY;
    sharedaccess : P.Second : INT READ_ONLY;
END_VAR
END_CONFIGURATION
"#,
    );
    assert!(
        error.to_ascii_lowercase().contains("duplicate")
            && error.to_ascii_lowercase().contains("sharedaccess"),
        "{error}"
    );
}

#[test]
fn configuration_contract_var_access_name_cannot_collide_with_global() {
    let error = compile_error(
        r#"
VAR_GLOBAL
    Exposed : INT;
END_VAR
PROGRAM Main
VAR
    Value : INT;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    exposed : P.Value : INT READ_ONLY;
END_VAR
END_CONFIGURATION
"#,
    );
    assert!(
        error.to_ascii_lowercase().contains("duplicate")
            || error.to_ascii_lowercase().contains("ambiguous"),
        "{error}"
    );
}

#[test]
fn configuration_contract_var_config_type_must_match_target() {
    assert_error_contains(
        r#"
PROGRAM Main
VAR
    Value : DINT;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_CONFIG
    P.Value : INT := INT#4;
END_VAR
END_CONFIGURATION
"#,
        "does not match target type",
    );
}

#[test]
fn configuration_contract_var_config_rejects_constant_initializer_target() {
    assert_error_contains(
        r#"
PROGRAM Main
VAR CONSTANT
    Limit : INT := INT#3;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_CONFIG
    P.Limit : INT := INT#4;
END_VAR
END_CONFIGURATION
"#,
        "cannot initialize CONSTANT targets",
    );
}

#[test]
fn configuration_contract_var_config_rejects_temp_initializer_target() {
    assert_error_contains(
        r#"
PROGRAM Main
VAR_TEMP
    Scratch : INT;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_CONFIG
    P.Scratch : INT := INT#4;
END_VAR
END_CONFIGURATION
"#,
        "cannot initialize this variable section",
    );
}

#[test]
fn configuration_contract_var_config_rejects_external_initializer_target() {
    assert_error_contains(
        r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
PROGRAM Main
VAR_EXTERNAL
    Shared : INT;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_CONFIG
    P.Shared : INT := INT#4;
END_VAR
END_CONFIGURATION
"#,
        "cannot initialize this variable section",
    );
}

#[test]
fn configuration_contract_var_config_rejects_in_out_initializer_target() {
    assert_error_contains(
        r#"
PROGRAM Main
VAR_IN_OUT
    Linked : INT;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_CONFIG
    P.Linked : INT := INT#4;
END_VAR
END_CONFIGURATION
"#,
        "cannot initialize this variable section",
    );
}

#[test]
fn configuration_contract_var_config_initializes_nested_function_block_member() {
    let harness = TestHarness::from_source(
        r#"
FUNCTION_BLOCK Worker
VAR
    Preset : INT := INT#2;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    WorkerInstance : Worker;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    Observed : P.WorkerInstance.Preset : INT READ_ONLY;
END_VAR
VAR_CONFIG
    P.WorkerInstance.Preset : INT := INT#13;
END_VAR
END_CONFIGURATION
"#,
    )
    .expect("nested FB configuration initializer");
    assert_eq!(harness.get_access("Observed"), Some(Value::Int(13)));
}

#[test]
fn configuration_contract_var_config_initializes_structure_component() {
    let harness = TestHarness::from_source(
        r#"
TYPE Packet : STRUCT
    Code : INT := INT#1;
END_STRUCT;
END_TYPE
PROGRAM Main
VAR
    PacketValue : Packet;
END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_ACCESS
    Observed : P.PacketValue.Code : INT READ_ONLY;
END_VAR
VAR_CONFIG
    P.PacketValue.Code : INT := INT#21;
END_VAR
END_CONFIGURATION
"#,
    )
    .expect("structure component configuration initializer");
    assert_eq!(harness.get_access("Observed"), Some(Value::Int(21)));
}
