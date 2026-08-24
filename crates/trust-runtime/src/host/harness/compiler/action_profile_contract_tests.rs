use crate::harness::{CompileSession, SourceFile};

fn runtime_error(source: &str) -> String {
    match CompileSession::from_source(source).build_runtime() {
        Ok(_) => panic!("textual ACTION source must not produce a runnable runtime"),
        Err(error) => error.to_string(),
    }
}

fn bytecode_error(source: &str) -> String {
    match CompileSession::from_source(source).build_bytecode_module() {
        Ok(_) => panic!("textual ACTION source must not produce bytecode"),
        Err(error) => error.to_string(),
    }
}

fn assert_action_profile_error(error: &str) {
    let normalized = error.to_ascii_lowercase();
    assert!(
        normalized.contains("action"),
        "diagnostic must identify ACTION: {error}"
    );
    assert!(
        normalized.contains("sfc"),
        "diagnostic must identify the textual-SFC boundary: {error}"
    );
    assert!(
        normalized.contains("unsupported") || normalized.contains("not supported"),
        "diagnostic must identify unsupported execution: {error}"
    );
}

#[test]
fn action_profile_rejects_empty_program_action_at_runtime_boundary() {
    let error = runtime_error(
        r#"
PROGRAM Main
ACTION Reset:
END_ACTION
END_PROGRAM
"#,
    );
    assert_action_profile_error(&error);
}

#[test]
fn action_profile_rejects_observable_program_action_instead_of_omitting_it() {
    let error = runtime_error(
        r#"
PROGRAM Main
VAR
    output : INT;
END_VAR
ACTION SetOutput:
    output := INT#99;
END_ACTION
output := INT#1;
END_PROGRAM
"#,
    );
    assert_action_profile_error(&error);
}

#[test]
fn action_profile_rejects_function_block_action_even_when_type_is_unused() {
    let error = runtime_error(
        r#"
FUNCTION_BLOCK Controller
VAR
    state : INT;
END_VAR
ACTION Reset:
    state := INT#0;
END_ACTION
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );
    assert_action_profile_error(&error);
}

#[test]
fn action_profile_rejects_multiple_actions_as_one_unsupported_source() {
    let error = runtime_error(
        r#"
PROGRAM Main
ACTION Start:
END_ACTION
ACTION Stop:
END_ACTION
END_PROGRAM
"#,
    );
    assert_action_profile_error(&error);
}

#[test]
fn action_profile_rejects_action_found_in_any_multifile_input() {
    let sources = vec![
        SourceFile::with_path(
            "controller.st",
            r#"
FUNCTION_BLOCK Controller
ACTION Reset:
END_ACTION
END_FUNCTION_BLOCK
"#,
        ),
        SourceFile::with_path("main.st", "PROGRAM Main\nEND_PROGRAM"),
    ];
    let error = match CompileSession::from_sources(sources).build_runtime() {
        Ok(_) => panic!("project containing textual ACTION must fail"),
        Err(error) => error.to_string(),
    };
    assert_action_profile_error(&error);
    assert!(
        error.contains("controller.st"),
        "multi-file diagnostic must retain source ownership: {error}"
    );
}

#[test]
fn action_profile_rejects_bytecode_module_emission() {
    let error = bytecode_error(
        r#"
PROGRAM Main
ACTION Reset:
END_ACTION
END_PROGRAM
"#,
    );
    assert_action_profile_error(&error);
}

#[test]
fn action_profile_rejects_encoded_bytecode_emission() {
    let error = match CompileSession::from_source(
        r#"
PROGRAM Main
ACTION Reset:
END_ACTION
END_PROGRAM
"#,
    )
    .build_bytecode_bytes()
    {
        Ok(_) => panic!("textual ACTION source must not produce encoded bytecode"),
        Err(error) => error.to_string(),
    };
    assert_action_profile_error(&error);
}

#[test]
fn action_profile_does_not_reject_ordinary_generated_state_machine_st() {
    let result = CompileSession::from_source(
        r#"
FUNCTION_BLOCK GeneratedSequence
VAR
    state : INT;
END_VAR
CASE state OF
    0: state := INT#1;
    1: state := INT#2;
    ELSE state := INT#0;
END_CASE;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    sequence : GeneratedSequence;
END_VAR
sequence();
END_PROGRAM
"#,
    )
    .build_runtime();
    assert!(
        result.is_ok(),
        "ordinary companion ST must remain executable: {result:?}"
    );
}
