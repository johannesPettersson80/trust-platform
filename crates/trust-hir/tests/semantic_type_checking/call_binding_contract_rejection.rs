use crate::common::*;

const FUNCTION_INTERFACE: &str = r#"
FUNCTION Transfer : DINT
VAR_INPUT
    first : DINT;
    second : DINT;
END_VAR
VAR_OUTPUT
    left : DINT;
    right : DINT;
END_VAR
VAR_IN_OUT
    target : DINT;
END_VAR
Transfer := target;
END_FUNCTION
"#;

fn rejected(body: &str, code: DiagnosticCode) {
    check_has_error(
        &format!(
            "{FUNCTION_INTERFACE}\nPROGRAM Main\nVAR\nx : DINT; y : DINT; z : DINT; values : ARRAY[0..1] OF DINT;\nEND_VAR\n{body}\nEND_PROGRAM"
        ),
        code,
    );
}

#[test]
fn call_binding_contract_rejects_omitted_in_out() {
    rejected(
        "z := Transfer(first := x, second := y, left => x, right => y);",
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_expression_for_in_out() {
    rejected(
        "z := Transfer(first := x, second := y, target := x + y);",
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_literal_for_in_out() {
    rejected(
        "z := Transfer(first := x, second := y, target := DINT#1);",
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_constant_for_in_out() {
    check_has_error(
        &format!(
            "{FUNCTION_INTERFACE}\nPROGRAM Main\nVAR CONSTANT fixed : DINT := 1; END_VAR\nVAR result : DINT; END_VAR\nresult := Transfer(target := fixed);\nEND_PROGRAM"
        ),
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_widening_for_in_out() {
    check_has_error(
        &format!(
            "{FUNCTION_INTERFACE}\nPROGRAM Main\nVAR narrow : INT; result : DINT; END_VAR\nresult := Transfer(target := narrow);\nEND_PROGRAM"
        ),
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_expression_for_output() {
    rejected(
        "z := Transfer(left => x + y, target := z);",
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_literal_for_output() {
    rejected(
        "z := Transfer(left => DINT#1, target := z);",
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_constant_for_output() {
    check_has_error(
        &format!(
            "{FUNCTION_INTERFACE}\nPROGRAM Main\nVAR CONSTANT fixed : DINT := 1; END_VAR\nVAR result : DINT; target : DINT; END_VAR\nresult := Transfer(left => fixed, target := target);\nEND_PROGRAM"
        ),
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_output_arrow_for_input() {
    rejected(
        "z := Transfer(first => x, target := z);",
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_input_assignment_for_output() {
    rejected(
        "z := Transfer(left := x, target := z);",
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_output_arrow_for_in_out() {
    rejected(
        "z := Transfer(target => x);",
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_duplicate_named_parameter() {
    rejected(
        "z := Transfer(first := x, first := y, target := z);",
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_unknown_named_parameter() {
    rejected(
        "z := Transfer(missing := x, target := z);",
        DiagnosticCode::CannotResolve,
    );
}

#[test]
fn call_binding_contract_rejects_incomplete_positional_call() {
    rejected(
        "z := Transfer(x, y, z);",
        DiagnosticCode::WrongArgumentCount,
    );
}

#[test]
fn call_binding_contract_rejects_excess_positional_call() {
    rejected(
        "z := Transfer(x, y, x, y, z, x);",
        DiagnosticCode::WrongArgumentCount,
    );
}

#[test]
fn call_binding_contract_rejects_positional_after_formal() {
    rejected(
        "z := Transfer(first := x, y, left => z, right => y, target := x);",
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_formal_rebinding_of_positional_prefix() {
    rejected(
        "z := Transfer(x, first := y, target := z);",
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_same_target_for_two_outputs() {
    rejected(
        "z := Transfer(left => x, right => x, target := z);",
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_same_target_for_output_and_in_out() {
    rejected(
        "z := Transfer(left => x, target := x);",
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_same_target_for_two_in_outs() {
    check_has_error(
        r#"
FUNCTION Mutate : INT
VAR_IN_OUT first : INT; second : INT; END_VAR
Mutate := first + second;
END_FUNCTION
PROGRAM Main
VAR value : INT; result : INT; END_VAR
result := Mutate(first := value, second := value);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_same_target_for_output_and_eno() {
    check_has_error(
        r#"
FUNCTION Controlled : INT
VAR_OUTPUT value : BOOL; ENO : BOOL; END_VAR
Controlled := 0;
END_FUNCTION
PROGRAM Main
VAR shared : BOOL; result : INT; END_VAR
result := Controlled(value => shared, ENO => shared);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_overlapping_array_element_outputs() {
    rejected(
        "z := Transfer(left => values[0], right => values[0], target := z);",
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_overlapping_aggregate_output_targets() {
    check_has_error(
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
    );
}

#[test]
fn call_binding_contract_rejects_function_block_output_in_out_alias() {
    check_has_error(
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
    );
}

#[test]
fn call_binding_contract_rejects_method_output_alias() {
    check_has_error(
        r#"
CLASS Worker
METHOD PUBLIC Pair : INT
VAR_OUTPUT first : INT; second : INT; END_VAR
Pair := 0;
END_METHOD
END_CLASS
PROGRAM Main
VAR worker : Worker; shared : INT; result : INT; END_VAR
result := worker.Pair(first => shared, second => shared);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_narrowing_output_transfer() {
    check_has_error(
        r#"
FUNCTION Produce : DINT
VAR_OUTPUT value : DINT; END_VAR
value := DINT#1; Produce := value;
END_FUNCTION
PROGRAM Main
VAR target : INT; END_VAR
Produce(value => target);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_en_connected_with_output_arrow() {
    check_has_error(
        r#"
FUNCTION Controlled : INT
VAR_INPUT EN : BOOL; END_VAR
Controlled := 0;
END_FUNCTION
PROGRAM Main
VAR enabled : BOOL; END_VAR
Controlled(EN => enabled);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_eno_connected_with_input_assignment() {
    check_has_error(
        r#"
FUNCTION Controlled : INT
VAR_OUTPUT ENO : BOOL; END_VAR
Controlled := 0;
END_FUNCTION
PROGRAM Main
VAR ok : BOOL; END_VAR
Controlled(ENO := ok);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn call_binding_contract_rejects_en_in_positional_list() {
    check_has_error(
        r#"
FUNCTION Controlled : INT
VAR_INPUT EN : BOOL; value : INT; END_VAR
Controlled := value;
END_FUNCTION
PROGRAM Main
VAR result : INT; END_VAR
result := Controlled(TRUE, 1);
END_PROGRAM
"#,
        DiagnosticCode::WrongArgumentCount,
    );
}

#[test]
fn call_binding_contract_rejects_eno_in_positional_list() {
    check_has_error(
        r#"
FUNCTION Controlled : INT
VAR_INPUT value : INT; END_VAR
VAR_OUTPUT ENO : BOOL; END_VAR
Controlled := value;
END_FUNCTION
PROGRAM Main
VAR ok : BOOL; result : INT; END_VAR
result := Controlled(1, ok);
END_PROGRAM
"#,
        DiagnosticCode::WrongArgumentCount,
    );
}

#[test]
fn call_binding_contract_rejects_ref_of_en() {
    check_has_error(
        r#"
FUNCTION Controlled : INT
VAR_INPUT EN : BOOL; END_VAR
VAR reference : REF_TO BOOL; END_VAR
reference := REF(EN);
Controlled := 0;
END_FUNCTION
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn call_binding_contract_rejects_ref_of_eno() {
    check_has_error(
        r#"
FUNCTION Controlled : INT
VAR_OUTPUT ENO : BOOL; END_VAR
VAR reference : REF_TO BOOL; END_VAR
reference := REF(ENO);
Controlled := 0;
END_FUNCTION
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn call_binding_contract_rejects_void_method_in_expression() {
    check_has_error(
        r#"
CLASS Sink
METHOD PUBLIC Store
VAR_INPUT value : INT; END_VAR
END_METHOD
END_CLASS
PROGRAM Main
VAR sink : Sink; result : INT; END_VAR
result := sink.Store(value := 1);
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn call_binding_contract_rejects_function_block_in_expression() {
    check_has_error(
        r#"
FUNCTION_BLOCK Counter
VAR_INPUT value : INT; END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR counter : Counter; result : INT; END_VAR
result := counter(value := 1);
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}
