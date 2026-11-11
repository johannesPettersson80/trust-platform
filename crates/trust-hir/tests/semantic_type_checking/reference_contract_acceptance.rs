use crate::common::*;

#[test]
fn reference_contract_accepts_ref_of_program_variable() {
    check_no_errors(
        "PROGRAM P\nVAR x : INT; r : REF_TO INT; END_VAR\nr := REF(x); r^ := 1;\nEND_PROGRAM",
    );
}

#[test]
fn reference_contract_accepts_ref_of_function_block_state() {
    check_no_errors("FUNCTION_BLOCK F\nVAR x : INT; r : REF_TO INT; END_VAR\nr := REF(x); r^ := 1;\nEND_FUNCTION_BLOCK");
}

#[test]
fn reference_contract_accepts_ref_of_array_element() {
    check_no_errors("PROGRAM P\nVAR a : ARRAY[0..2] OF INT; r : REF_TO INT; END_VAR\nr := REF(a[1]); r^ := 1;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_ref_of_structure_field() {
    check_no_errors("TYPE S : STRUCT x : INT; END_STRUCT END_TYPE\nPROGRAM P\nVAR s : S; r : REF_TO INT; END_VAR\nr := REF(s.x); r^ := 1;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_adr_of_program_variable() {
    check_no_errors(
        "PROGRAM P\nVAR x : INT; p : POINTER TO INT; END_VAR\np := ADR(x); p^ := 1;\nEND_PROGRAM",
    );
}

#[test]
fn reference_contract_accepts_adr_of_array_element() {
    check_no_errors("PROGRAM P\nVAR a : ARRAY[0..2] OF INT; p : POINTER TO INT; END_VAR\np := ADR(a[1]); p^ := 1;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_adr_of_structure_field() {
    check_no_errors("TYPE S : STRUCT x : INT; END_STRUCT END_TYPE\nPROGRAM P\nVAR s : S; p : POINTER TO INT; END_VAR\np := ADR(s.x); p^ := 1;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_reference_dereference_read() {
    check_no_errors("PROGRAM P\nVAR x : INT; r : REF_TO INT; y : INT; END_VAR\nr := REF(x); y := r^;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_pointer_dereference_read() {
    check_no_errors("PROGRAM P\nVAR x : INT; p : POINTER TO INT; y : INT; END_VAR\np := ADR(x); y := p^;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_pointer_to_array_index_write() {
    check_no_errors("PROGRAM P\nVAR a : ARRAY[0..2] OF INT; p : POINTER TO ARRAY[0..2] OF INT; END_VAR\np := ADR(a); p^[1] := 7;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_pointer_to_structure_field_write() {
    check_no_errors("TYPE S : STRUCT x : INT; END_STRUCT END_TYPE\nPROGRAM P\nVAR s : S; p : POINTER TO S; END_VAR\np := ADR(s); p^.x := 7;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_nested_pointer_dereference() {
    check_no_errors("PROGRAM P\nVAR x : INT; p : POINTER TO INT; pp : POINTER TO POINTER TO INT; y : INT; END_VAR\np := ADR(x); pp := ADR(p); y := pp^^;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_null_reference_default() {
    check_no_errors(
        "PROGRAM P\nVAR r : REF_TO INT; END_VAR\nIF r = NULL THEN ; END_IF;\nEND_PROGRAM",
    );
}

#[test]
fn reference_contract_accepts_null_pointer_assignment() {
    check_no_errors("PROGRAM P\nVAR p : POINTER TO INT; END_VAR\np := NULL;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_reference_null_inequality() {
    check_no_errors(
        "PROGRAM P\nVAR r : REF_TO INT; b : BOOL; END_VAR\nb := r <> NULL;\nEND_PROGRAM",
    );
}

#[test]
fn reference_contract_accepts_same_type_reference_assignment() {
    check_no_errors("PROGRAM P\nVAR x : INT; a : REF_TO INT; b : REF_TO INT; END_VAR\na := REF(x); b := a;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_same_type_pointer_assignment() {
    check_no_errors("PROGRAM P\nVAR x : INT; a : POINTER TO INT; b : POINTER TO INT; END_VAR\na := ADR(x); b := a;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_direct_alias_reference_assignment() {
    check_no_errors("TYPE Meter : INT; MeterRef : REF_TO Meter; END_TYPE\nPROGRAM P\nVAR x : Meter; a : MeterRef; b : REF_TO INT; END_VAR\na := REF(x); b := a;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_derived_to_base_reference_assignment() {
    check_no_errors("CLASS Base END_CLASS\nCLASS Derived EXTENDS Base END_CLASS\nPROGRAM P\nVAR d : Derived; b : REF_TO Base; END_VAR\nb := REF(d);\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_same_type_reference_attempt() {
    check_no_errors("PROGRAM P\nVAR x : INT; a : REF_TO INT; b : REF_TO INT; END_VAR\na := REF(x); b ?= a;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_same_type_pointer_attempt() {
    check_no_errors("PROGRAM P\nVAR x : INT; a : POINTER TO INT; b : POINTER TO INT; END_VAR\na := ADR(x); b ?= a;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_reference_attempt_from_null() {
    check_no_errors("PROGRAM P\nVAR r : REF_TO INT; END_VAR\nr ?= NULL;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_pointer_attempt_from_null() {
    check_no_errors("PROGRAM P\nVAR p : POINTER TO INT; END_VAR\np ?= NULL;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_dynamic_reference_downcast_attempt() {
    check_no_errors("CLASS Base END_CLASS\nCLASS Derived EXTENDS Base END_CLASS\nPROGRAM P\nVAR d : Derived; b : REF_TO Base; result : REF_TO Derived; END_VAR\nb := REF(d); result ?= b;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_interface_assignment_attempt() {
    check_no_errors("INTERFACE I END_INTERFACE\nCLASS C IMPLEMENTS I END_CLASS\nPROGRAM P\nVAR c : C; source : I; result : I; END_VAR\nsource := c; result ?= source;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_interface_to_class_reference_downcast_attempt() {
    check_no_errors("INTERFACE I END_INTERFACE\nCLASS C IMPLEMENTS I END_CLASS\nPROGRAM P\nVAR c : C; source : I; result : REF_TO C; END_VAR\nsource := c; result ?= source;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_write_through_input_pointer() {
    check_no_errors(
        "FUNCTION_BLOCK F\nVAR_INPUT p : POINTER TO INT; END_VAR\np^ := 7;\nEND_FUNCTION_BLOCK",
    );
}

#[test]
fn reference_contract_accepts_write_through_constant_pointer_slot() {
    check_no_errors("FUNCTION F : INT\nVAR_IN_OUT CONSTANT p : POINTER TO INT; END_VAR\np^ := 7; F := p^;\nEND_FUNCTION");
}

#[test]
fn reference_contract_accepts_reference_equality() {
    check_no_errors("PROGRAM P\nVAR x : INT; a : REF_TO INT; b : REF_TO INT; same : BOOL; END_VAR\na := REF(x); b := a; same := a = b;\nEND_PROGRAM");
}

#[test]
fn reference_contract_accepts_pointer_equality() {
    check_no_errors("PROGRAM P\nVAR x : INT; a : POINTER TO INT; b : POINTER TO INT; same : BOOL; END_VAR\na := ADR(x); b := a; same := a = b;\nEND_PROGRAM");
}

/// Specification 06, "Assignment Attempt": `?=` between two interfaces needs no relation between
/// them, as IEC 61131-3 Ed.3 §6.6.6.7.3 EXAMPLE 2 shows with `interf2 ?= interf1` and `interf3 ?=
/// interf1`. Catches the E201 that refused it.
#[test]
fn reference_contract_accepts_attempt_between_unrelated_interfaces() {
    check_no_errors("INTERFACE ITF1 END_INTERFACE\nINTERFACE ITF2 END_INTERFACE\nINTERFACE ITF3 END_INTERFACE\nCLASS C IMPLEMENTS ITF1, ITF2 END_CLASS\nPROGRAM A\nVAR inst : C; interf1 : ITF1; interf2 : ITF2; interf3 : ITF3; END_VAR\ninterf1 := inst;\ninterf2 ?= interf1;\ninterf3 ?= interf1;\nEND_PROGRAM");
}

/// Specification 06: `?=` from an interface to a reference of a class that does not implement it
/// takes an instance of a derived class that does (IEC 61131-3 Ed.3 §6.6.6.7.1, a safe cast from an
/// interface reference to a class reference). Catches the E201 that refused it.
#[test]
fn reference_contract_accepts_attempt_from_interface_to_unrelated_class_reference() {
    check_no_errors("INTERFACE ITF END_INTERFACE\nCLASS Base END_CLASS\nCLASS Derived EXTENDS Base IMPLEMENTS ITF END_CLASS\nPROGRAM A\nVAR inst : Derived; itf : ITF; base : REF_TO Base; END_VAR\nitf := inst;\nbase ?= itf;\nEND_PROGRAM");
}
