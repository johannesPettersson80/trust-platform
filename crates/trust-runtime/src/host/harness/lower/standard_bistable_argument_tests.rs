use crate::harness::{lower_application, SourceFile};
use crate::program_model::{Expr, Stmt};

/// Compiles `source` and returns the argument names of the call that is the first statement of
/// `Main`. An unnamed argument is reported as `<positional>`.
fn call_argument_names(source: &str) -> Vec<String> {
    let (runtime, _) = lower_application(&[SourceFile::new(source)], false)
        .and_then(|lowered| lowered.construct(&[]))
        .unwrap_or_else(|error| panic!("bistable fixture must compile: {error}"));
    let program = runtime.programs().values().next().expect("Main program");
    let Stmt::Expr {
        expr: Expr::Call { args, .. },
        ..
    } = &program.body[0]
    else {
        panic!("the first statement of Main must be a call");
    };
    args.iter()
        .map(|arg| arg.name.as_deref().unwrap_or("<positional>").to_owned())
        .collect()
}

#[test]
fn sr_long_input_names_lower_to_canonical_argument_names() {
    // IEC 61131-3 Ed.3 Table 43 row 1b: SET1 and RESET are the long names of S1 and R.
    let names = call_argument_names(
        r#"
PROGRAM Main
VAR
    latch : SR;
    set_sig : BOOL;
    reset_sig : BOOL;
    q_out : BOOL;
END_VAR
latch(SET1 := set_sig, RESET := reset_sig, Q1 => q_out);
END_PROGRAM
"#,
    );
    assert_eq!(names, ["S1", "R", "Q1"]);
}

#[test]
fn rs_long_input_names_lower_to_canonical_argument_names() {
    // IEC 61131-3 Ed.3 Table 43 row 2b: SET and RESET1 are the long names of S and R1.
    let names = call_argument_names(
        r#"
PROGRAM Main
VAR
    latch : RS;
    set_sig : BOOL;
    reset_sig : BOOL;
    q_out : BOOL;
END_VAR
latch(SET := set_sig, RESET1 := reset_sig, Q1 => q_out);
END_PROGRAM
"#,
    );
    assert_eq!(names, ["S", "R1", "Q1"]);
}

#[test]
fn empty_named_arguments_drop_from_the_lowered_call() {
    // IEC_DEVIATIONS DEV-029: an empty right-hand side on a named binding
    // omits the argument — `RESET := ` leaves the input unbound and an
    // untargeted `Q1 =>` drops the output connection (the s4 project
    // writes TOF calls exactly this way).
    let dropped = call_argument_names(
        r#"
PROGRAM Main
VAR
    latch : SR;
    set_sig : BOOL;
    q_out : BOOL;
END_VAR
latch(SET1 := set_sig, RESET := , Q1 => );
END_PROGRAM
"#,
    );
    assert_eq!(dropped, ["S1"]);

    let mixed = call_argument_names(
        r#"
PROGRAM Main
VAR
    latch : SR;
    set_sig : BOOL;
    q_out : BOOL;
END_VAR
latch(SET1 := set_sig, RESET := , Q1 => q_out);
END_PROGRAM
"#,
    );
    assert_eq!(mixed, ["S1", "Q1"]);
}

#[test]
fn user_block_inputs_named_like_long_bistable_names_keep_their_names() {
    // Only the standard bistables are normalized: a user block's own SET1 input keeps its name.
    let names = call_argument_names(
        r#"
FUNCTION_BLOCK UserLatch
VAR_INPUT
    SET1 : BOOL;
    RESET : BOOL;
END_VAR
VAR_OUTPUT
    Q1 : BOOL;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    latch : UserLatch;
    set_sig : BOOL;
    reset_sig : BOOL;
    q_out : BOOL;
END_VAR
latch(SET1 := set_sig, RESET := reset_sig, Q1 => q_out);
END_PROGRAM
"#,
    );
    assert_eq!(names, ["SET1", "RESET", "Q1"]);
}
