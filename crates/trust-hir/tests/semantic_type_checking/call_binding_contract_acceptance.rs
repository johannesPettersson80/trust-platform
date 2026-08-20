use crate::common::*;

const FUNCTION_INTERFACE: &str = r#"
FUNCTION Transfer : DINT
VAR_INPUT
    first : DINT := DINT#10;
    second : DINT := DINT#20;
END_VAR
VAR_OUTPUT
    copied : DINT;
END_VAR
VAR_IN_OUT
    target : DINT;
END_VAR
copied := first;
target := target + second;
Transfer := target;
END_FUNCTION
"#;

fn accepted(body: &str) {
    check_no_errors(&format!(
        "{FUNCTION_INTERFACE}\nPROGRAM Main\nVAR\nx : DINT; y : DINT; z : DINT;\nEND_VAR\n{body}\nEND_PROGRAM"
    ));
}

#[test]
fn call_binding_contract_accepts_formal_arguments_in_declaration_order() {
    accepted("z := Transfer(first := x, second := y, copied => z, target := x);");
}

#[test]
fn call_binding_contract_accepts_formal_arguments_in_arbitrary_order() {
    accepted("z := Transfer(target := x, copied => z, second := y, first := x);");
}

#[test]
fn call_binding_contract_accepts_omitted_defaulted_function_input() {
    accepted("z := Transfer(second := y, copied => z, target := x);");
}

#[test]
fn call_binding_contract_accepts_omitted_function_output_connection() {
    accepted("z := Transfer(first := x, second := y, target := x);");
}

#[test]
fn call_binding_contract_accepts_exact_type_in_out_variable() {
    accepted("z := Transfer(target := x);");
}

#[test]
fn call_binding_contract_accepts_complete_positional_function_call() {
    accepted("z := Transfer(x, y, z, x);");
}

#[test]
fn call_binding_contract_accepts_positional_prefix_and_formal_suffix() {
    accepted("z := Transfer(x, second := y, copied => z, target := x);");
}

#[test]
fn call_binding_contract_accepts_reordered_formal_suffix_after_positional_prefix() {
    accepted("z := Transfer(x, target := x, copied => z, second := y);");
}

#[test]
fn call_binding_contract_accepts_input_read_and_single_output_write_to_same_variable() {
    accepted("z := Transfer(first := x, copied => x, target := y);");
}

#[test]
fn call_binding_contract_accepts_discarded_function_result_statement() {
    accepted("Transfer(first := x, second := y, copied => z, target := x);");
}

#[test]
fn call_binding_contract_accepts_accuracy_preserving_input_widening() {
    check_no_errors(
        r#"
FUNCTION Widen : LINT
VAR_INPUT value : DINT; END_VAR
Widen := value;
END_FUNCTION
PROGRAM Main
VAR source : INT; result : LINT; END_VAR
result := Widen(value := source);
END_PROGRAM
"#,
    );
}

#[test]
fn call_binding_contract_accepts_accuracy_preserving_output_transfer() {
    check_no_errors(
        r#"
FUNCTION Produce : DINT
VAR_OUTPUT value : DINT; END_VAR
value := DINT#1;
Produce := value;
END_FUNCTION
PROGRAM Main
VAR result : LINT; END_VAR
Produce(value => result);
END_PROGRAM
"#,
    );
}

#[test]
fn call_binding_contract_accepts_distinct_array_element_output_targets() {
    check_no_errors(
        r#"
FUNCTION Pair : INT
VAR_OUTPUT left : INT; right : INT; END_VAR
left := 1; right := 2; Pair := 0;
END_FUNCTION
PROGRAM Main
VAR values : ARRAY[0..1] OF INT; END_VAR
Pair(left => values[0], right => values[1]);
END_PROGRAM
"#,
    );
}

#[test]
fn call_binding_contract_accepts_named_en_and_eno_connections() {
    check_no_errors(
        r#"
FUNCTION Controlled : INT
VAR_INPUT EN : BOOL; value : INT; END_VAR
VAR_OUTPUT ENO : BOOL; END_VAR
Controlled := value;
END_FUNCTION
PROGRAM Main
VAR enabled : BOOL; ok : BOOL; result : INT; END_VAR
result := Controlled(value := 1, EN := enabled, ENO => ok);
END_PROGRAM
"#,
    );
}

#[test]
fn call_binding_contract_accepts_positional_call_excluding_en_and_eno() {
    check_no_errors(
        r#"
FUNCTION Controlled : INT
VAR_INPUT EN : BOOL; value : INT; END_VAR
VAR_OUTPUT ENO : BOOL; copied : INT; END_VAR
VAR_IN_OUT target : INT; END_VAR
copied := value; Controlled := target;
END_FUNCTION
PROGRAM Main
VAR value : INT; copied : INT; target : INT; result : INT; END_VAR
result := Controlled(value, copied, target);
END_PROGRAM
"#,
    );
}

#[test]
fn call_binding_contract_accepts_function_block_omitted_stored_input() {
    check_no_errors(
        r#"
FUNCTION_BLOCK Accumulator
VAR_INPUT amount : INT := 1; END_VAR
VAR_OUTPUT total : INT; END_VAR
total := total + amount;
END_FUNCTION_BLOCK
PROGRAM Main
VAR fb : Accumulator; result : INT; END_VAR
fb(amount := 2, total => result);
fb(total => result);
END_PROGRAM
"#,
    );
}

#[test]
fn call_binding_contract_accepts_function_block_in_out_and_output() {
    check_no_errors(
        r#"
FUNCTION_BLOCK TransferFb
VAR_INPUT value : INT; END_VAR
VAR_OUTPUT copied : INT; END_VAR
VAR_IN_OUT target : INT; END_VAR
copied := value; target := target + value;
END_FUNCTION_BLOCK
PROGRAM Main
VAR fb : TransferFb; source : INT; copied : INT; target : INT; END_VAR
fb(value := source, copied => copied, target := target);
END_PROGRAM
"#,
    );
}

#[test]
fn call_binding_contract_accepts_value_returning_method_expression() {
    check_no_errors(
        r#"
CLASS Calculator
METHOD PUBLIC Add : INT
VAR_INPUT left : INT; right : INT; END_VAR
Add := left + right;
END_METHOD
END_CLASS
PROGRAM Main
VAR calculator : Calculator; result : INT; END_VAR
result := calculator.Add(right := 2, left := 1);
END_PROGRAM
"#,
    );
}

#[test]
fn call_binding_contract_accepts_void_method_statement() {
    check_no_errors(
        r#"
CLASS Sink
METHOD PUBLIC Store
VAR_INPUT value : INT; END_VAR
END_METHOD
END_CLASS
PROGRAM Main
VAR sink : Sink; END_VAR
sink.Store(value := 1);
END_PROGRAM
"#,
    );
}

#[test]
fn call_binding_contract_accepts_method_output_and_in_out_connections() {
    check_no_errors(
        r#"
CLASS Worker
METHOD PUBLIC Transfer : INT
VAR_INPUT source : INT; END_VAR
VAR_OUTPUT copied : INT; END_VAR
VAR_IN_OUT target : INT; END_VAR
copied := source; target := target + source; Transfer := target;
END_METHOD
END_CLASS
PROGRAM Main
VAR worker : Worker; source : INT; copied : INT; target : INT; result : INT; END_VAR
result := worker.Transfer(source := source, copied => copied, target := target);
END_PROGRAM
"#,
    );
}
