use crate::error::RuntimeError;
use crate::harness::TestHarness;
use crate::value::Value;

fn reference_output(source: &str, name: &str) -> Value {
    let mut harness = TestHarness::from_source(source)
        .unwrap_or_else(|error| panic!("reference fixture must compile: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness.try_get_output(name).unwrap()
}

#[test]
fn reference_runtime_default_ref_to_compares_equal_to_null() {
    assert_eq!(reference_output("PROGRAM Main\nVAR r : REF_TO INT; result : BOOL; END_VAR\nresult := r = NULL;\nEND_PROGRAM", "result"), Value::Bool(true));
}

#[test]
fn reference_runtime_default_pointer_compares_equal_to_null() {
    assert_eq!(reference_output("PROGRAM Main\nVAR p : POINTER TO INT; result : BOOL; END_VAR\nresult := p = NULL;\nEND_PROGRAM", "result"), Value::Bool(true));
}

#[test]
fn reference_runtime_ref_dereference_reads_target() {
    assert_eq!(reference_output("PROGRAM Main\nVAR x : INT := 7; r : REF_TO INT; result : INT; END_VAR\nr := REF(x); result := r^;\nEND_PROGRAM", "result"), Value::Int(7));
}

#[test]
fn reference_runtime_ref_dereference_writes_target() {
    assert_eq!(reference_output("PROGRAM Main\nVAR x : INT := 7; r : REF_TO INT; END_VAR\nr := REF(x); r^ := 9;\nEND_PROGRAM", "x"), Value::Int(9));
}

#[test]
fn reference_runtime_pointer_dereference_reads_target() {
    assert_eq!(reference_output("PROGRAM Main\nVAR x : INT := 7; p : POINTER TO INT; result : INT; END_VAR\np := ADR(x); result := p^;\nEND_PROGRAM", "result"), Value::Int(7));
}

#[test]
fn reference_runtime_pointer_dereference_writes_target() {
    assert_eq!(reference_output("PROGRAM Main\nVAR x : INT := 7; p : POINTER TO INT; END_VAR\np := ADR(x); p^ := 9;\nEND_PROGRAM", "x"), Value::Int(9));
}

#[test]
fn reference_runtime_ref_of_array_element_retains_exact_index() {
    assert_eq!(reference_output("PROGRAM Main\nVAR a : ARRAY[0..2] OF INT := [1, 2, 3]; r : REF_TO INT; END_VAR\nr := REF(a[1]); r^ := 9;\nEND_PROGRAM", "a[1]"), Value::Int(9));
}

#[test]
fn reference_runtime_pointer_to_array_writes_selected_index() {
    assert_eq!(reference_output("PROGRAM Main\nVAR a : ARRAY[0..2] OF INT := [1, 2, 3]; p : POINTER TO ARRAY[0..2] OF INT; END_VAR\np := ADR(a); p^[2] := 9;\nEND_PROGRAM", "a[2]"), Value::Int(9));
}

#[test]
fn reference_runtime_pointer_to_struct_writes_selected_field() {
    assert_eq!(reference_output("TYPE S : STRUCT x : INT; y : INT; END_STRUCT END_TYPE\nPROGRAM Main\nVAR s : S := (x := 1, y := 2); p : POINTER TO S; result : INT; END_VAR\np := ADR(s); p^.y := 9; result := s.y;\nEND_PROGRAM", "result"), Value::Int(9));
}

#[test]
fn reference_runtime_nested_pointer_dereference_reaches_target() {
    assert_eq!(reference_output("PROGRAM Main\nVAR x : INT := 7; p : POINTER TO INT; pp : POINTER TO POINTER TO INT; result : INT; END_VAR\np := ADR(x); pp := ADR(p); result := pp^^;\nEND_PROGRAM", "result"), Value::Int(7));
}

#[test]
fn reference_runtime_reference_copy_shares_storage_identity() {
    assert_eq!(reference_output("PROGRAM Main\nVAR x : INT := 7; a : REF_TO INT; b : REF_TO INT; END_VAR\na := REF(x); b := a; b^ := 9;\nEND_PROGRAM", "x"), Value::Int(9));
}

#[test]
fn reference_runtime_pointer_copy_shares_storage_identity() {
    assert_eq!(reference_output("PROGRAM Main\nVAR x : INT := 7; a : POINTER TO INT; b : POINTER TO INT; END_VAR\na := ADR(x); b := a; b^ := 9;\nEND_PROGRAM", "x"), Value::Int(9));
}

#[test]
fn reference_runtime_reference_observes_later_target_write() {
    assert_eq!(reference_output("PROGRAM Main\nVAR x : INT := 7; r : REF_TO INT; result : INT; END_VAR\nr := REF(x); x := 9; result := r^;\nEND_PROGRAM", "result"), Value::Int(9));
}

#[test]
fn reference_runtime_pointer_observes_later_target_write() {
    assert_eq!(reference_output("PROGRAM Main\nVAR x : INT := 7; p : POINTER TO INT; result : INT; END_VAR\np := ADR(x); x := 9; result := p^;\nEND_PROGRAM", "result"), Value::Int(9));
}

#[test]
fn reference_runtime_same_type_attempt_overwrites_existing_reference() {
    assert_eq!(reference_output("PROGRAM Main\nVAR x : INT := 1; y : INT := 2; source : REF_TO INT; result : REF_TO INT; END_VAR\nsource := REF(x); result := REF(y); result ?= source; result^ := 9;\nEND_PROGRAM", "x"), Value::Int(9));
}

#[test]
fn reference_runtime_same_type_pointer_attempt_overwrites_existing_pointer() {
    assert_eq!(reference_output("PROGRAM Main\nVAR x : INT := 1; y : INT := 2; source : POINTER TO INT; result : POINTER TO INT; END_VAR\nsource := ADR(x); result := ADR(y); result ?= source; result^ := 9;\nEND_PROGRAM", "x"), Value::Int(9));
}

#[test]
fn reference_runtime_reference_attempt_from_null_clears_existing_value() {
    assert_eq!(reference_output("PROGRAM Main\nVAR x : INT; r : REF_TO INT; result : BOOL; END_VAR\nr := REF(x); r ?= NULL; result := r = NULL;\nEND_PROGRAM", "result"), Value::Bool(true));
}

#[test]
fn reference_runtime_pointer_attempt_from_null_clears_existing_value() {
    assert_eq!(reference_output("PROGRAM Main\nVAR x : INT; p : POINTER TO INT; result : BOOL; END_VAR\np := ADR(x); p ?= NULL; result := p = NULL;\nEND_PROGRAM", "result"), Value::Bool(true));
}

#[test]
fn reference_runtime_dynamic_downcast_succeeds_for_derived_instance() {
    assert_eq!(reference_output("CLASS Base END_CLASS\nCLASS Derived EXTENDS Base END_CLASS\nPROGRAM Main\nVAR value : Derived; source : REF_TO Base; result : REF_TO Derived; ok : BOOL; END_VAR\nsource := REF(value); result ?= source; ok := result <> NULL;\nEND_PROGRAM", "ok"), Value::Bool(true));
}

#[test]
fn reference_runtime_dynamic_downcast_failure_writes_null() {
    assert_eq!(reference_output("CLASS Base END_CLASS\nCLASS Derived EXTENDS Base END_CLASS\nPROGRAM Main\nVAR baseValue : Base; derivedValue : Derived; source : REF_TO Base; result : REF_TO Derived; failed : BOOL; END_VAR\nsource := REF(baseValue); result := REF(derivedValue); result ?= source; failed := result = NULL;\nEND_PROGRAM", "failed"), Value::Bool(true));
}

#[test]
fn reference_runtime_interface_attempt_succeeds_for_implemented_interface() {
    assert_eq!(reference_output("INTERFACE I END_INTERFACE\nCLASS C IMPLEMENTS I END_CLASS\nPROGRAM Main\nVAR value : C; source : I; result : I; ok : BOOL; END_VAR\nsource := value; result ?= source; ok := result <> NULL;\nEND_PROGRAM", "ok"), Value::Bool(true));
}

#[test]
fn reference_runtime_distinct_references_compare_not_equal() {
    assert_eq!(reference_output("PROGRAM Main\nVAR x : INT; y : INT; a : REF_TO INT; b : REF_TO INT; result : BOOL; END_VAR\na := REF(x); b := REF(y); result := a <> b;\nEND_PROGRAM", "result"), Value::Bool(true));
}

#[test]
fn reference_runtime_copied_references_compare_equal() {
    assert_eq!(reference_output("PROGRAM Main\nVAR x : INT; a : REF_TO INT; b : REF_TO INT; result : BOOL; END_VAR\na := REF(x); b := a; result := a = b;\nEND_PROGRAM", "result"), Value::Bool(true));
}

#[test]
fn reference_runtime_null_read_fault_preserves_assignment_destination() {
    let mut harness = TestHarness::from_source(
        "PROGRAM Main\nVAR r : REF_TO INT; result : INT := 7; END_VAR\nresult := r^;\nEND_PROGRAM",
    )
    .expect("null read fixture must compile");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors, [RuntimeError::NullReference]);
    assert_eq!(harness.try_get_output("result").unwrap(), Value::Int(7));
}

#[test]
fn reference_runtime_null_write_fault_preserves_unrelated_storage() {
    let mut harness = TestHarness::from_source(
        "PROGRAM Main\nVAR p : POINTER TO INT; result : INT := 7; END_VAR\np^ := 9;\nEND_PROGRAM",
    )
    .expect("null write fixture must compile");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors, [RuntimeError::NullReference]);
    assert_eq!(harness.try_get_output("result").unwrap(), Value::Int(7));
}
