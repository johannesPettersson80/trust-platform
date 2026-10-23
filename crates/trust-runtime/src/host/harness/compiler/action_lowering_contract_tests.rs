use crate::harness::{CompileSession, SourceFile};

// Textual ACTION lowering contract: an `ACTION x: ... END_ACTION` declared
// in a FUNCTION_BLOCK is a same-instance sub-body — it must compile through
// every emission boundary (runtime, bytecode module, encoded bytes) and be
// invokable by its bare name from the owner body (CODESYS semantics; spec
// 04 §8).

fn fb_action_source() -> &'static str {
    r#"
FUNCTION_BLOCK Controller
VAR
    state : INT;
END_VAR
ACTION Reset:
    state := INT#0;
END_ACTION
ACTION StepUp:
    state := state + INT#1;
END_ACTION
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    controller : Controller;
END_VAR
controller.Reset();
controller.StepUp();
END_PROGRAM
"#
}

#[test]
fn action_profile_lowers_function_block_action_to_runtime() {
    let result = CompileSession::from_source(fb_action_source()).build_runtime();
    assert!(
        result.is_ok(),
        "FB textual ACTION must lower to a runnable runtime: {result:?}"
    );
}

#[test]
fn action_profile_lowers_function_block_action_to_bytecode_module() {
    let result = CompileSession::from_source(fb_action_source()).build_bytecode_module();
    assert!(
        result.is_ok(),
        "FB textual ACTION must emit a bytecode module: {result:?}"
    );
}

#[test]
fn action_profile_lowers_function_block_action_to_encoded_bytecode() {
    let result = CompileSession::from_source(fb_action_source()).build_bytecode_bytes();
    assert!(
        result.is_ok(),
        "FB textual ACTION must emit encoded bytecode: {result:?}"
    );
}

#[test]
fn action_profile_lowers_action_owned_by_another_source_file() {
    let sources = vec![
        SourceFile::with_path(
            "controller.st",
            r#"
FUNCTION_BLOCK Controller
VAR state : INT; END_VAR
ACTION Reset:
    state := INT#0;
END_ACTION
END_FUNCTION_BLOCK
"#,
        ),
        SourceFile::with_path("main.st", "PROGRAM Main\nEND_PROGRAM"),
    ];
    let result = CompileSession::from_sources(sources).build_runtime();
    assert!(
        result.is_ok(),
        "multi-file projects carry ACTION lowering per source: {result:?}"
    );
}

#[test]
fn action_profile_keeps_unknown_bare_call_a_compile_error() {
    let error = match CompileSession::from_source(
        r#"
FUNCTION_BLOCK Controller
VAR state : INT; END_VAR
ACTION Reset:
    state := INT#0;
END_ACTION
Missing();
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    )
    .build_runtime()
    {
        Ok(_) => panic!("unknown bare call must stay a compile error, not a VM trap"),
        Err(error) => error.to_string(),
    };
    assert!(
        error.to_ascii_lowercase().contains("undefined function"),
        "diagnostic must identify the undefined callee: {error}"
    );
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
