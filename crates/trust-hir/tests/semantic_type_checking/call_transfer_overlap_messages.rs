use crate::common::*;

const OVERLAP_MESSAGE: &str =
    "writable call connections cannot target the same or overlapping storage";

fn check_has_error_message(source: &str, expected: DiagnosticCode, message_fragment: &str) {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    let diagnostics = db
        .diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .cloned()
        .collect::<Vec<_>>();

    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code == expected && diagnostic.message.contains(message_fragment)
        }),
        "expected {expected:?} containing {message_fragment:?}, got {diagnostics:?}"
    );
}

/// Spec 10 §7.5: two outputs bound to one variable are rejected, and the message says why.
#[test]
fn call_transfer_same_target_for_two_outputs_names_the_overlap() {
    check_has_error_message(
        r#"
FUNCTION Transfer : DINT
VAR_OUTPUT left : DINT; right : DINT; END_VAR
Transfer := 0;
END_FUNCTION
PROGRAM Main
VAR x : DINT; z : DINT; END_VAR
z := Transfer(left => x, right => x);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
        OVERLAP_MESSAGE,
    );
}

/// Spec 10 §7.5: a whole aggregate and one of its members as two outputs is rejected, and the
/// message says why.
#[test]
fn call_transfer_aggregate_and_member_outputs_name_the_overlap() {
    check_has_error_message(
        r#"
TYPE Pair : STRUCT left : INT; right : INT; END_STRUCT END_TYPE
FUNCTION Produce : INT
VAR_OUTPUT whole : Pair; member : INT; END_VAR
Produce := 0;
END_FUNCTION
PROGRAM Main
VAR value : Pair; result : INT; END_VAR
result := Produce(whole => value, member => value.left);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
        OVERLAP_MESSAGE,
    );
}

/// Spec 10 §7.5: a function block output and in-out bound to one variable is rejected, and the
/// message says why.
#[test]
fn call_transfer_function_block_output_and_in_out_alias_names_the_overlap() {
    check_has_error_message(
        r#"
FUNCTION_BLOCK TransferFb
VAR_OUTPUT copied : INT; END_VAR
VAR_IN_OUT target : INT; END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR fb : TransferFb; shared : INT; END_VAR
fb(copied => shared, target := shared);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
        OVERLAP_MESSAGE,
    );
}

/// Spec 10 §7.5: elements whose indices are only equal at run time are not a static overlap.
#[test]
fn call_transfer_run_time_coinciding_elements_are_not_a_static_error() {
    check_no_errors(
        r#"
FUNCTION Pair : INT
VAR_OUTPUT first : INT; second : INT; END_VAR
Pair := INT#0;
END_FUNCTION
PROGRAM Main
VAR values : ARRAY[0..1] OF INT; i : INT; j : INT; r : INT; END_VAR
r := Pair(first => values[i], second => values[j]);
END_PROGRAM
"#,
    );
}

/// Spec 10 §7.5: function block outputs on elements whose indices are only equal at run time are
/// not a static overlap.
#[test]
fn call_transfer_run_time_coinciding_function_block_outputs_are_not_a_static_error() {
    check_no_errors(
        r#"
FUNCTION_BLOCK Split
VAR_OUTPUT first : INT; second : INT; END_VAR
first := INT#1;
second := INT#2;
END_FUNCTION_BLOCK
PROGRAM Main
VAR values : ARRAY[0..1] OF INT; i : INT; j : INT; splitter : Split; END_VAR
splitter(first => values[i], second => values[j]);
END_PROGRAM
"#,
    );
}

/// Spec 10 §7.5: a dereferenced reference and the variable it designates are not a static overlap.
#[test]
fn call_transfer_dereferenced_reference_and_its_variable_are_not_a_static_error() {
    check_no_errors(
        r#"
FUNCTION Pair : INT
VAR_OUTPUT first : INT; second : INT; END_VAR
Pair := INT#0;
END_FUNCTION
PROGRAM Main
VAR x : INT; p : REF_TO INT; r : INT; END_VAR
p := REF(x);
r := Pair(first => x, second => p^);
END_PROGRAM
"#,
    );
}
