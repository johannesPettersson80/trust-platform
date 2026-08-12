use crate::harness::{CompileSession, SourceFile};

fn runtime_error(source: &str) -> String {
    match CompileSession::from_source(source).build_runtime() {
        Ok(_) => panic!("source-level generic type must not produce a runnable runtime"),
        Err(error) => error.to_string(),
    }
}

fn bytecode_error(source: &str) -> String {
    match CompileSession::from_source(source).build_bytecode_module() {
        Ok(_) => panic!("source-level generic type must not produce bytecode"),
        Err(error) => error.to_string(),
    }
}

fn assert_generic_type_error(error: &str, generic: &str) {
    let normalized = error.to_ascii_lowercase();
    assert!(
        normalized.contains("generic"),
        "diagnostic must identify the generic-type boundary: {error}"
    );
    assert!(
        normalized.contains(&generic.to_ascii_lowercase()),
        "diagnostic must identify {generic}: {error}"
    );
}

#[test]
fn generic_type_profile_rejects_local_storage_at_runtime_boundary() {
    let error = runtime_error(
        r#"
PROGRAM Main
VAR
    value : ANY;
END_VAR
END_PROGRAM
"#,
    );
    assert_generic_type_error(&error, "ANY");
}

#[test]
fn generic_type_profile_rejects_function_parameter_and_result_types() {
    let input_error = runtime_error(
        r#"
FUNCTION GenericInput : INT
VAR_INPUT
    value : ANY_INT;
END_VAR
GenericInput := INT#0;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    assert_generic_type_error(&input_error, "ANY_INT");

    let result_error = runtime_error(
        r#"
FUNCTION GenericResult : ANY_REAL
GenericResult := REAL#0.0;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    assert_generic_type_error(&result_error, "ANY_REAL");
}

#[test]
fn generic_type_profile_rejects_alias_to_generic_category() {
    let error = runtime_error(
        r#"
TYPE GenericNumber : ANY_NUM;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    assert_generic_type_error(&error, "ANY_NUM");
}

#[test]
fn generic_type_profile_rejects_generic_array_element() {
    let error = runtime_error(
        r#"
TYPE GenericArray : ARRAY[0..3] OF ANY_BIT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    assert_generic_type_error(&error, "ANY_BIT");
}

#[test]
fn generic_type_profile_rejects_generic_struct_member() {
    let error = runtime_error(
        r#"
TYPE Packet : STRUCT
    value : ANY_DERIVED;
END_STRUCT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    assert_generic_type_error(&error, "ANY_DERIVED");
}

#[test]
fn generic_type_profile_rejects_generic_pointer_and_reference_targets() {
    let pointer_error = runtime_error(
        r#"
TYPE GenericPointer : POINTER TO ANY_DURATION;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    assert_generic_type_error(&pointer_error, "ANY_DURATION");

    let reference_error = runtime_error(
        r#"
TYPE GenericReference : REF_TO ANY_MAGNITUDE;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
    );
    assert_generic_type_error(&reference_error, "ANY_MAGNITUDE");
}

#[test]
fn generic_type_profile_rejects_unused_function_block_state() {
    let error = runtime_error(
        r#"
FUNCTION_BLOCK GenericState
VAR
    value : ANY_CHARS;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
    assert_generic_type_error(&error, "ANY_CHARS");
}

#[test]
fn generic_type_profile_rejects_unused_class_state() {
    let error = runtime_error(
        r#"
CLASS GenericState
VAR
    value : ANY_UNSIGNED;
END_VAR
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
    );
    assert_generic_type_error(&error, "ANY_UNSIGNED");
}

#[test]
fn generic_type_profile_rejects_generic_found_in_any_multifile_input() {
    let sources = vec![
        SourceFile::with_path(
            "generic_types.st",
            r#"
TYPE GenericText : ANY_STRING;
END_TYPE
"#,
        ),
        SourceFile::with_path("main.st", "PROGRAM Main\nEND_PROGRAM"),
    ];
    let error = match CompileSession::from_sources(sources).build_runtime() {
        Ok(_) => panic!("project containing a source-level generic type must fail"),
        Err(error) => error.to_string(),
    };
    assert_generic_type_error(&error, "ANY_STRING");
    assert!(
        error.contains("generic_types.st"),
        "multi-file diagnostic must retain source ownership: {error}"
    );
}

#[test]
fn generic_type_profile_rejects_bytecode_module_emission() {
    let error = bytecode_error(
        r#"
PROGRAM Main
VAR
    value : ANY_DATE;
END_VAR
END_PROGRAM
"#,
    );
    assert_generic_type_error(&error, "ANY_DATE");
}

#[test]
fn generic_type_profile_rejects_encoded_bytecode_emission() {
    let error = match CompileSession::from_source(
        r#"
PROGRAM Main
VAR
    value : ANY_ELEMENTARY;
END_VAR
END_PROGRAM
"#,
    )
    .build_bytecode_bytes()
    {
        Ok(_) => panic!("source-level generic type must not produce encoded bytecode"),
        Err(error) => error.to_string(),
    };
    assert_generic_type_error(&error, "ANY_ELEMENTARY");
}

#[test]
fn generic_type_profile_does_not_reject_concrete_aliases_and_subranges() {
    let result = CompileSession::from_source(
        r#"
TYPE
    Count : DINT;
    SmallCount : Count;
    Window : UINT (1..10);
    Label : STRING[16];
END_TYPE
PROGRAM Main
VAR
    count : SmallCount;
    window : Window;
    label : Label;
END_VAR
count := SmallCount#1;
window := Window#2;
label := 'ready';
END_PROGRAM
"#,
    )
    .build_runtime();
    assert!(
        result.is_ok(),
        "concrete aliases and subranges must remain executable: {result:?}"
    );
}
