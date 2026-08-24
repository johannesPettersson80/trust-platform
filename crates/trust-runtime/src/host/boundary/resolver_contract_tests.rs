use super::*;

use crate::harness::TestHarness;
use crate::value::Value;

fn boundary_source() -> &'static str {
    r#"
TYPE Packet :
STRUCT
    flag : BOOL;
    count : INT;
END_STRUCT
END_TYPE

VAR_GLOBAL
    GlobalValue : INT := INT#11;
    Masked : INT := INT#44;
    GlobalWord : WORD := WORD#16#1234;
    GlobalSensor : BOOL;
END_VAR

PROGRAM Alpha
VAR
    unique : INT := INT#7;
    shared : INT := INT#1;
    masked : INT := INT#2;
    values : ARRAY[1..3] OF INT;
    grid : ARRAY[1..2, 3..4] OF INT;
    packet : Packet;
    wordValue : WORD := WORD#16#1234;
    refValue : REF_TO INT;
    nullPointer : REF_TO INT;
    sensor : BOOL;
    seen : BOOL;
END_VAR
refValue := REF(unique);
seen := sensor;
END_PROGRAM

PROGRAM Beta
VAR
    shared : INT := INT#2;
    masked : INT := INT#3;
END_VAR
END_PROGRAM
"#
}

fn harness() -> TestHarness {
    TestHarness::from_source(boundary_source()).expect("boundary fixture must compile")
}

fn cycled_harness() -> TestHarness {
    let mut harness = harness();
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
}

fn alpha_instance(harness: &TestHarness) -> crate::memory::InstanceId {
    match harness.runtime().storage().get_global("Alpha") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected Alpha instance, got {other:?}"),
    }
}

#[test]
fn boundary_resolver_contract_global_read_precedes_same_named_program_vars() {
    let harness = harness();
    assert_eq!(harness.try_get_output("Masked"), Ok(Value::Int(44)));
}

#[test]
fn boundary_resolver_contract_unique_simple_program_read_returns_exact_value() {
    let harness = harness();
    assert_eq!(harness.try_get_output("unique"), Ok(Value::Int(7)));
}

#[test]
fn boundary_resolver_contract_unique_simple_program_write_updates_instance() {
    let mut harness = harness();
    harness
        .try_set_input("unique", Value::Int(19))
        .expect("unique write");
    assert_eq!(harness.try_get_output("unique"), Ok(Value::Int(19)));
}

#[test]
fn boundary_resolver_contract_simple_write_preserves_runtime_value_identity() {
    let mut harness = harness();
    harness
        .try_set_input("unique", Value::WString("not coerced".into()))
        .expect("boundary write is untyped");
    assert_eq!(
        harness.try_get_output("unique"),
        Ok(Value::WString("not coerced".into()))
    );
}

#[test]
fn boundary_resolver_contract_missing_read_preserves_complete_path() {
    let harness = harness();
    let error = harness
        .try_get_output("missing")
        .expect_err("missing read must fail");
    assert_eq!(
        error,
        BoundaryError::UnresolvedName {
            path: "missing".into()
        }
    );
}

#[test]
fn boundary_resolver_contract_missing_write_creates_no_fallback_global() {
    let mut harness = harness();
    let globals_before = harness.runtime().storage().globals().len();
    let error = harness
        .try_set_input("missing", Value::Int(9))
        .expect_err("missing write must fail");
    assert_eq!(error.code(), "unresolved_name");
    assert_eq!(harness.runtime().storage().globals().len(), globals_before);
    assert_eq!(harness.runtime().storage().get_global("missing"), None);
}

#[test]
fn boundary_resolver_contract_ambiguous_read_preserves_registration_order() {
    let harness = harness();
    let error = harness
        .try_get_output("shared")
        .expect_err("ambiguous read must fail");
    assert_eq!(
        error,
        BoundaryError::AmbiguousName {
            path: "shared".into(),
            candidates: vec!["Alpha.shared".into(), "Beta.shared".into()]
        }
    );
}

#[test]
fn boundary_resolver_contract_ambiguous_write_mutates_no_candidate() {
    let mut harness = harness();
    let alpha = alpha_instance(&harness);
    let beta = match harness.runtime().storage().get_global("Beta") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected Beta instance, got {other:?}"),
    };
    let before_alpha = harness
        .runtime()
        .storage()
        .get_instance_var(alpha, "shared")
        .cloned();
    let before_beta = harness
        .runtime()
        .storage()
        .get_instance_var(beta, "shared")
        .cloned();

    let error = harness
        .try_set_input("shared", Value::Int(99))
        .expect_err("ambiguous write must fail");
    assert_eq!(error.code(), "ambiguous_name");
    assert_eq!(
        harness
            .runtime()
            .storage()
            .get_instance_var(alpha, "shared")
            .cloned(),
        before_alpha
    );
    assert_eq!(
        harness
            .runtime()
            .storage()
            .get_instance_var(beta, "shared")
            .cloned(),
        before_beta
    );
}

#[test]
fn boundary_resolver_contract_struct_field_read_uses_program_root() {
    let mut harness = harness();
    harness
        .try_set_input("packet.count", Value::Int(23))
        .expect("field write");
    assert_eq!(harness.try_get_output("packet.count"), Ok(Value::Int(23)));
}

#[test]
fn boundary_resolver_contract_struct_field_write_changes_only_selected_field() {
    let mut harness = harness();
    harness
        .try_set_input("packet.flag", Value::Bool(true))
        .expect("flag write");
    harness
        .try_set_input("packet.count", Value::Int(31))
        .expect("count write");
    assert_eq!(harness.try_get_output("packet.flag"), Ok(Value::Bool(true)));
    assert_eq!(harness.try_get_output("packet.count"), Ok(Value::Int(31)));
}

#[test]
fn boundary_resolver_contract_array_element_read_and_write_use_declared_bounds() {
    let mut harness = harness();
    harness
        .try_set_input("values[2]", Value::Int(55))
        .expect("array write");
    assert_eq!(harness.try_get_output("values[2]"), Ok(Value::Int(55)));
    assert_eq!(harness.try_get_output("values[1]"), Ok(Value::Int(0)));
}

#[test]
fn boundary_resolver_contract_multidimensional_index_preserves_dimension_order() {
    let mut harness = harness();
    harness
        .try_set_input("grid[2, 3]", Value::Int(73))
        .expect("grid write");
    harness
        .try_set_input("grid[1, 4]", Value::Int(14))
        .expect("grid write");
    assert_eq!(harness.try_get_output("grid[2, 3]"), Ok(Value::Int(73)));
    assert_eq!(harness.try_get_output("grid[1, 4]"), Ok(Value::Int(14)));
}

#[test]
fn boundary_resolver_contract_out_of_bounds_read_is_wrong_kind() {
    let harness = harness();
    let error = harness
        .try_get_output("values[4]")
        .expect_err("out-of-bounds read must fail");
    assert_eq!(error.code(), "wrong_kind");
    assert_eq!(error.path(), Some("values[4]"));
}

#[test]
fn boundary_resolver_contract_out_of_bounds_write_preserves_array() {
    let mut harness = harness();
    let before = harness.try_get_output("values").expect("array value");
    let error = harness
        .try_set_input("values[0]", Value::Int(99))
        .expect_err("out-of-bounds write must fail");
    assert_eq!(error.code(), "wrong_kind");
    assert_eq!(harness.try_get_output("values"), Ok(before));
}

#[test]
fn boundary_resolver_contract_missing_field_is_unresolved_name() {
    let harness = harness();
    let error = harness
        .try_get_output("packet.missing")
        .expect_err("missing field must fail");
    assert_eq!(error.code(), "unresolved_name");
    assert_eq!(error.path(), Some("packet.missing"));
}

#[test]
fn boundary_resolver_contract_binary_read_expression_is_unsupported_path() {
    let harness = harness();
    let error = harness
        .try_get_output("unique + INT#1")
        .expect_err("binary expression is not a path");
    assert_eq!(error.code(), "unsupported_path_syntax");
    assert_eq!(error.path(), Some("unique + INT#1"));
}

#[test]
fn boundary_resolver_contract_literal_read_expression_is_unsupported_path() {
    let harness = harness();
    let error = harness
        .try_get_output("INT#1")
        .expect_err("literal is not a path");
    assert_eq!(error.code(), "unsupported_path_syntax");
}

#[test]
fn boundary_resolver_contract_call_read_expression_is_unsupported_path() {
    let harness = harness();
    let error = harness
        .try_get_output("ABS(unique)")
        .expect_err("call is not a path");
    assert_eq!(error.code(), "unsupported_path_syntax");
}

#[test]
fn boundary_resolver_contract_call_write_is_unsupported_without_mutation() {
    let mut harness = harness();
    let before = harness.try_get_output("unique").expect("unique");
    let error = harness
        .try_set_input("ABS(unique)", Value::Int(99))
        .expect_err("call is not an assignment path");
    assert_eq!(error.code(), "unsupported_path_syntax");
    assert_eq!(harness.try_get_output("unique"), Ok(before));
}

#[test]
fn boundary_resolver_contract_null_reference_read_is_wrong_kind() {
    let harness = harness();
    let error = harness
        .try_get_output("nullPointer^")
        .expect_err("null dereference must fail");
    assert_eq!(error.code(), "wrong_kind");
    assert!(error.to_string().contains("null reference"));
}

#[test]
fn boundary_resolver_contract_null_reference_write_is_wrong_kind() {
    let mut harness = harness();
    let error = harness
        .try_set_input("nullPointer^", Value::Int(8))
        .expect_err("null dereference must fail");
    assert_eq!(error.code(), "wrong_kind");
}

#[test]
fn boundary_resolver_contract_initialized_reference_can_be_read_and_written() {
    let mut harness = cycled_harness();
    assert_eq!(harness.try_get_output("refValue^"), Ok(Value::Int(7)));
    harness
        .try_set_input("refValue^", Value::Int(88))
        .expect("reference write");
    assert_eq!(harness.try_get_output("refValue^"), Ok(Value::Int(88)));
    assert_eq!(harness.try_get_output("unique"), Ok(Value::Int(88)));
}

#[test]
fn boundary_resolver_contract_partial_byte_read_and_write_preserve_other_byte() {
    let mut harness = harness();
    assert_eq!(
        harness.try_get_output("wordValue.%B1"),
        Ok(Value::Byte(0x12))
    );
    harness
        .try_set_input("wordValue.%B1", Value::Byte(0xAB))
        .expect("partial byte write");
    assert_eq!(harness.try_get_output("wordValue"), Ok(Value::Word(0xAB34)));
}

#[test]
fn boundary_resolver_contract_partial_bit_write_preserves_other_bits() {
    let mut harness = harness();
    harness
        .try_set_input("wordValue.%X0", Value::Bool(true))
        .expect("partial bit write");
    assert_eq!(harness.try_get_output("wordValue"), Ok(Value::Word(0x1235)));
}

#[test]
fn boundary_resolver_contract_get_output_collapses_error_without_fake_value() {
    let harness = harness();
    assert_eq!(harness.get_output("missing"), None);
    assert_eq!(harness.get_output("values[99]"), None);
}

#[test]
fn boundary_resolver_contract_bind_global_input_crosses_cycle() {
    let mut harness = harness();
    harness
        .bind_direct("GlobalSensor", "%IX2.0")
        .expect("bind global");
    harness
        .set_direct_input("%IX2.0", Value::Bool(true))
        .expect("write direct input");
    harness.cycle();
    assert_eq!(
        harness.try_get_output("GlobalSensor"),
        Ok(Value::Bool(true))
    );
}

#[test]
fn boundary_resolver_contract_bind_program_input_uses_storage_reference() {
    let mut harness = harness();
    harness
        .bind_direct("sensor", "%IX3.1")
        .expect("bind program variable");
    harness
        .set_direct_input("%IX3.1", Value::Bool(true))
        .expect("write direct input");
    harness.cycle();
    assert_eq!(harness.try_get_output("sensor"), Ok(Value::Bool(true)));
    assert_eq!(harness.try_get_output("seen"), Ok(Value::Bool(true)));
}

#[test]
fn boundary_resolver_contract_bind_program_output_writes_direct_image() {
    let mut harness = harness();
    harness
        .bind_direct("wordValue", "%QW4")
        .expect("bind program output");
    harness.cycle();
    assert_eq!(
        harness.get_direct_output("%QW4").expect("direct output"),
        Value::Word(0x1234)
    );
}

#[test]
fn boundary_resolver_contract_bind_composite_path_is_unsupported_and_passive() {
    let mut harness = harness();
    let bindings_before = harness.runtime().io().bindings().len();
    let error = harness
        .bind_direct("packet.flag", "%QX0.0")
        .expect_err("composite binding must fail");
    assert_eq!(error.code(), "unsupported_path_syntax");
    assert_eq!(harness.runtime().io().bindings().len(), bindings_before);
}

#[test]
fn boundary_resolver_contract_bind_expression_is_unsupported_and_passive() {
    let mut harness = harness();
    let bindings_before = harness.runtime().io().bindings().len();
    let error = harness
        .bind_direct("unique + INT#1", "%QW0")
        .expect_err("expression binding must fail");
    assert_eq!(error.code(), "unsupported_path_syntax");
    assert_eq!(harness.runtime().io().bindings().len(), bindings_before);
}

#[test]
fn boundary_resolver_contract_bind_ambiguous_name_preserves_candidates() {
    let mut harness = harness();
    let error = harness
        .bind_direct("shared", "%QW0")
        .expect_err("ambiguous binding must fail");
    assert_eq!(error.code(), "ambiguous_name");
    assert_eq!(error.candidates(), ["Alpha.shared", "Beta.shared"]);
}

#[test]
fn boundary_resolver_contract_bind_missing_name_creates_no_binding() {
    let mut harness = harness();
    let bindings_before = harness.runtime().io().bindings().len();
    let error = harness
        .bind_direct("missing", "%QX0.0")
        .expect_err("missing binding must fail");
    assert_eq!(error.code(), "undeclared_binding");
    assert_eq!(harness.runtime().io().bindings().len(), bindings_before);
}

#[test]
fn boundary_resolver_contract_bind_global_precedes_ambiguous_program_fields() {
    let mut harness = harness();
    harness
        .bind_direct("Masked", "%QW0")
        .expect("global binding wins");
    assert_eq!(harness.runtime().io().bindings().len(), 1);
    assert_eq!(
        harness.runtime().io().bindings()[0].display_name.as_deref(),
        Some("Masked")
    );
}

#[test]
fn boundary_resolver_contract_invalid_direct_address_maps_to_internal_failure() {
    let mut harness = harness();
    let error = harness
        .bind_direct("unique", "not-an-address")
        .expect_err("invalid direct address must fail");
    assert_eq!(error.code(), "internal_failure");
    assert_eq!(error.path(), None);
}
