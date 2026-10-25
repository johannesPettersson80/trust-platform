//! Public checked engineering writes apply the finite floating-point admission policy
//! (spec 34 §4.3.1 "finite floating-point admission", spec 11 finite policy): the compiled
//! runtime's variable writes and the harness's `VAR_ACCESS` writes.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.
use std::sync::Arc;

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

const SOURCE: &str = r#"
TYPE Record :
STRUCT
    samples : ARRAY[0..1] OF REAL;
    precise : LREAL;
END_STRUCT
END_TYPE

FUNCTION_BLOCK Probe
VAR_INPUT
    x : REAL := 1.5;
END_VAR
END_FUNCTION_BLOCK

VAR_GLOBAL
    level : REAL := 1.0;
    wide : LREAL := 2.0;
    rec : Record;
    records : ARRAY[0..1] OF Record;
END_VAR

PROGRAM Main
VAR
    probe : Probe;
    local_level : REAL := 3.0;
END_VAR
END_PROGRAM

CONFIGURATION Conf
PROGRAM P1 : Main;
VAR_ACCESS
    LevelAccess : P1.local_level : REAL READ_WRITE;
END_VAR
END_CONFIGURATION
"#;

fn build() -> TestHarness {
    TestHarness::from_source(SOURCE).expect("finite-policy fixture compiles")
}

fn assert_non_finite(error: RuntimeError) {
    assert_eq!(error, RuntimeError::NonFiniteValue);
    assert_eq!(error.stable_code().as_str(), "runtime_non_finite_value");
}

fn non_finite_reals() -> [Value; 3] {
    [
        Value::Real(f32::NAN),
        Value::Real(f32::INFINITY),
        Value::Real(f32::NEG_INFINITY),
    ]
}

fn non_finite_lreals() -> [Value; 3] {
    [
        Value::LReal(f64::NAN),
        Value::LReal(f64::INFINITY),
        Value::LReal(f64::NEG_INFINITY),
    ]
}

fn real_bits(value: Result<Value, RuntimeError>) -> Option<u32> {
    match value {
        Ok(Value::Real(value)) => Some(value.to_bits()),
        _ => None,
    }
}

fn lreal_bits(value: Result<Value, RuntimeError>) -> Option<u64> {
    match value {
        Ok(Value::LReal(value)) => Some(value.to_bits()),
        _ => None,
    }
}

/// Spec 34 §4.3.1: a checked engineering write of a global rejects every non-finite REAL and
/// LREAL with `runtime_non_finite_value` and leaves the global unchanged.
#[test]
fn checked_admission_public_write_global_rejects_non_finite_values_with_stable_code() {
    let mut harness = build();
    let runtime = harness.runtime_mut();
    for value in non_finite_reals() {
        let error = runtime
            .write_variable("level", &value)
            .expect_err("non-finite REAL must be rejected");
        assert_non_finite(error);
    }
    for value in non_finite_lreals() {
        let error = runtime
            .write_variable("wide", &value)
            .expect_err("non-finite LREAL must be rejected");
        assert_non_finite(error);
    }
    assert_eq!(runtime.read_variable("level"), Ok(Value::Real(1.0)));
    assert_eq!(runtime.read_variable("wide"), Ok(Value::LReal(2.0)));
    assert!(!runtime.faulted());
}

/// Spec 34 §4.3.1: a checked engineering write of a member of an existing function block instance
/// rejects non-finite REAL values and leaves the member unchanged.
#[test]
fn checked_admission_public_write_instance_rejects_non_finite_member_with_stable_code() {
    let mut harness = build();
    let runtime = harness.runtime_mut();
    for value in non_finite_reals() {
        let error = runtime
            .write_variable("P1.probe.x", &value)
            .expect_err("non-finite REAL member must be rejected");
        assert_non_finite(error);
    }
    assert_eq!(runtime.read_variable("P1.probe.x"), Ok(Value::Real(1.5)));
    assert!(!runtime.faulted());
}

/// Spec 34 §4.3.1: checked engineering `write_access` rejects non-finite REAL values for a
/// writable VAR_ACCESS alias and leaves the aliased variable unchanged.
#[test]
fn checked_admission_public_write_access_rejects_non_finite_values_with_stable_code() {
    let mut harness = build();
    for value in non_finite_reals() {
        let error = harness
            .set_access("LevelAccess", value)
            .expect_err("non-finite REAL alias write must be rejected");
        assert_non_finite(error);
    }
    assert_eq!(harness.get_access("LevelAccess"), Some(Value::Real(3.0)));
    assert!(!harness.runtime().faulted());
}

/// Spec 34 §4.3.1: non-finite values are rejected recursively through a structure and its
/// array member.
#[test]
fn checked_admission_public_writes_reject_nan_nested_in_a_struct_array_member() {
    let mut harness = build();
    let runtime = harness.runtime_mut();
    let original = runtime.read_variable("rec").expect("rec");
    let mut invalid = original.clone();
    let Value::Struct(record) = &mut invalid else {
        panic!("rec is a structure")
    };
    let Some(Value::Array(samples)) = Arc::make_mut(record).field_mut("samples") else {
        panic!("samples array")
    };
    samples.elements_mut()[1] = Value::Real(f32::NAN);

    let error = runtime
        .write_variable("rec", &invalid)
        .expect_err("nested NaN must be rejected");
    assert_non_finite(error);

    assert_eq!(runtime.read_variable("rec"), Ok(original));
    assert!(!runtime.faulted());
}

/// Spec 34 §4.3.1: a non-finite LREAL is rejected recursively through an array of structures.
#[test]
fn checked_admission_public_write_global_rejects_infinity_nested_in_an_array_of_structs() {
    let mut harness = build();
    let runtime = harness.runtime_mut();
    let original = runtime.read_variable("records").expect("records");
    let mut invalid = original.clone();
    let Value::Array(elements) = &mut invalid else {
        panic!("records is an array")
    };
    let Value::Struct(record) = &mut elements.elements_mut()[1] else {
        panic!("records[1] is a structure")
    };
    let Some(precise) = Arc::make_mut(record).field_mut("precise") else {
        panic!("precise field")
    };
    *precise = Value::LReal(f64::INFINITY);

    let error = runtime
        .write_variable("records", &invalid)
        .expect_err("nested +Inf must be rejected");
    assert_non_finite(error);
    assert_eq!(runtime.read_variable("records"), Ok(original));
    assert!(!runtime.faulted());
}

/// Spec 34 §4.3.1: the finite policy keeps -0.0, the smallest subnormal REAL and LREAL::MAX
/// bit-for-bit through every checked engineering write surface.
#[test]
fn checked_admission_public_writes_accept_finite_boundary_values_bit_for_bit() {
    let mut harness = build();
    let negative_zero = -0.0_f32;
    let subnormal = f32::from_bits(1);

    let runtime = harness.runtime_mut();
    runtime
        .write_variable("level", &Value::Real(negative_zero))
        .expect("-0.0 is finite");
    assert_eq!(
        real_bits(runtime.read_variable("level")),
        Some(negative_zero.to_bits())
    );

    runtime
        .write_variable("level", &Value::Real(subnormal))
        .expect("smallest subnormal is finite");
    assert_eq!(
        real_bits(runtime.read_variable("level")),
        Some(subnormal.to_bits())
    );

    harness
        .set_access("LevelAccess", Value::Real(negative_zero))
        .expect("-0.0 alias write is finite");
    let Some(Value::Real(alias)) = harness.get_access("LevelAccess") else {
        panic!("alias is a REAL")
    };
    assert_eq!(alias.to_bits(), negative_zero.to_bits());

    let runtime = harness.runtime_mut();
    runtime
        .write_variable("P1.probe.x", &Value::Real(subnormal))
        .expect("smallest subnormal member is finite");
    assert_eq!(
        real_bits(runtime.read_variable("P1.probe.x")),
        Some(subnormal.to_bits())
    );

    runtime
        .write_variable("wide", &Value::LReal(f64::MAX))
        .expect("LREAL::MAX is finite");
    assert_eq!(
        lreal_bits(runtime.read_variable("wide")),
        Some(f64::MAX.to_bits())
    );
    assert!(!runtime.faulted());
}

const OVERLAP_SOURCE: &str = r#"
TYPE Ieee : STRUCT OVERLAP
    number AT %B0 : REAL;
    bits AT %B0 : DWORD;
END_STRUCT; END_TYPE

VAR_GLOBAL
    probe : Ieee;
END_VAR

PROGRAM Main
END_PROGRAM
"#;

/// Spec 34 §4.3.1 (an engineering write checks the IEEE aliases it reconstructs, not only the
/// incoming bits) and specification 02 §3.6 (`STRUCT OVERLAP` members share their bits): writing
/// `16#7FC00000`, the bits of a quiet NaN, into the `DWORD` view of an overlap whose `REAL` shares
/// them is refused with `runtime_non_finite_value`, and both views keep their zero bits. Wrong
/// behaviour caught: a write that checks only the `DWORD` it was given and leaves a NaN in the
/// `REAL` view, which the finite policy forbids.
#[test]
fn checked_admission_overlap_bits_write_rejects_a_non_finite_real_alias() {
    let mut harness = TestHarness::from_source(OVERLAP_SOURCE).expect("overlap fixture compiles");
    let runtime = harness.runtime_mut();
    let error = runtime
        .write_variable("probe.BiTs", &Value::DWord(0x7FC0_0000))
        .expect_err("a NaN in the REAL alias must be rejected");
    assert_non_finite(error);
    assert_eq!(runtime.read_variable("probe.bits"), Ok(Value::DWord(0)));
    assert_eq!(runtime.read_variable("probe.NuMbEr"), Ok(Value::Real(0.0)));
    assert!(!runtime.faulted());
}
