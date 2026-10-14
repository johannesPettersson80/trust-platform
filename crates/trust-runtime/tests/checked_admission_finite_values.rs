//! Public checked engineering writes apply the finite floating-point admission policy
//! (spec 34 §4.3.1 "finite floating-point admission", spec 11 finite policy).
use std::sync::Arc;

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::CompileSession;
use trust_runtime::value::Value;
use trust_runtime::Runtime;

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

fn build() -> Runtime {
    CompileSession::from_source(SOURCE)
        .build_runtime()
        .expect("finite-policy fixture compiles")
}

fn probe_instance(runtime: &Runtime) -> trust_runtime::memory::InstanceId {
    let Some(Value::Instance(program)) = runtime.storage().get_global("P1") else {
        panic!("program instance P1")
    };
    let Some(Value::Instance(probe)) = runtime.storage().get_instance_var(*program, "probe") else {
        panic!("function block instance probe")
    };
    *probe
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

fn real_bits(value: Option<&Value>) -> Option<u32> {
    match value {
        Some(Value::Real(value)) => Some(value.to_bits()),
        _ => None,
    }
}

fn lreal_bits(value: Option<&Value>) -> Option<u64> {
    match value {
        Some(Value::LReal(value)) => Some(value.to_bits()),
        _ => None,
    }
}

/// Spec 34 §4.3.1: checked engineering `write_global` rejects every non-finite REAL and
/// LREAL with `runtime_non_finite_value` and leaves the global unchanged.
#[test]
fn checked_admission_public_write_global_rejects_non_finite_values_with_stable_code() {
    let mut runtime = build();
    for value in non_finite_reals() {
        let error = runtime
            .write_global("level", value)
            .expect_err("non-finite REAL must be rejected");
        assert_non_finite(error);
    }
    for value in non_finite_lreals() {
        let error = runtime
            .write_global("wide", value)
            .expect_err("non-finite LREAL must be rejected");
        assert_non_finite(error);
    }
    assert_eq!(
        runtime.storage().get_global("level"),
        Some(&Value::Real(1.0))
    );
    assert_eq!(
        runtime.storage().get_global("wide"),
        Some(&Value::LReal(2.0))
    );
    assert!(!runtime.faulted());
}

/// Spec 34 §4.3.1: checked engineering `write_reference` rejects every non-finite REAL and
/// LREAL with `runtime_non_finite_value` and leaves the destination unchanged.
#[test]
fn checked_admission_public_write_reference_rejects_non_finite_values_with_stable_code() {
    let mut runtime = build();
    let level = runtime
        .storage()
        .ref_for_global("level")
        .expect("level reference");
    let wide = runtime
        .storage()
        .ref_for_global("wide")
        .expect("wide reference");
    for value in non_finite_reals() {
        let error = runtime
            .write_reference(&level, value)
            .expect_err("non-finite REAL must be rejected");
        assert_non_finite(error);
    }
    for value in non_finite_lreals() {
        let error = runtime
            .write_reference(&wide, value)
            .expect_err("non-finite LREAL must be rejected");
        assert_non_finite(error);
    }
    assert_eq!(
        runtime.storage().get_global("level"),
        Some(&Value::Real(1.0))
    );
    assert_eq!(
        runtime.storage().get_global("wide"),
        Some(&Value::LReal(2.0))
    );
    assert!(!runtime.faulted());
}

/// Spec 34 §4.3.1: checked engineering `write_instance` rejects non-finite REAL members of an
/// existing function block instance and leaves the member unchanged.
#[test]
fn checked_admission_public_write_instance_rejects_non_finite_member_with_stable_code() {
    let mut runtime = build();
    let probe = probe_instance(&runtime);
    for value in non_finite_reals() {
        let error = runtime
            .write_instance(probe, "x", value)
            .expect_err("non-finite REAL member must be rejected");
        assert_non_finite(error);
    }
    assert_eq!(
        runtime.storage().get_instance_var(probe, "x"),
        Some(&Value::Real(1.5))
    );
    assert!(!runtime.faulted());
}

/// Spec 34 §4.3.1: checked engineering `write_access` rejects non-finite REAL values for a
/// writable VAR_ACCESS alias and leaves the aliased variable unchanged.
#[test]
fn checked_admission_public_write_access_rejects_non_finite_values_with_stable_code() {
    let mut runtime = build();
    for value in non_finite_reals() {
        let error = runtime
            .write_access("LevelAccess", value)
            .expect_err("non-finite REAL alias write must be rejected");
        assert_non_finite(error);
    }
    assert_eq!(runtime.read_access("LevelAccess"), Some(Value::Real(3.0)));
    assert!(!runtime.faulted());
}

/// Spec 34 §4.3.1: non-finite values are rejected recursively through a structure and its
/// array member, on both `write_global` and `write_reference`.
#[test]
fn checked_admission_public_writes_reject_nan_nested_in_a_struct_array_member() {
    let mut runtime = build();
    let original = runtime.storage().get_global("rec").cloned().expect("rec");
    let mut invalid = original.clone();
    let Value::Struct(record) = &mut invalid else {
        panic!("rec is a structure")
    };
    let Some(Value::Array(samples)) = Arc::make_mut(record).field_mut("samples") else {
        panic!("samples array")
    };
    samples.elements_mut()[1] = Value::Real(f32::NAN);

    let error = runtime
        .write_global("rec", invalid.clone())
        .expect_err("nested NaN must be rejected by write_global");
    assert_non_finite(error);

    let reference = runtime
        .storage()
        .ref_for_global("rec")
        .expect("rec reference");
    let error = runtime
        .write_reference(&reference, invalid)
        .expect_err("nested NaN must be rejected by write_reference");
    assert_non_finite(error);

    assert_eq!(runtime.storage().get_global("rec"), Some(&original));
    assert!(!runtime.faulted());
}

/// Spec 34 §4.3.1: a non-finite LREAL is rejected recursively through an array of structures.
#[test]
fn checked_admission_public_write_global_rejects_infinity_nested_in_an_array_of_structs() {
    let mut runtime = build();
    let original = runtime
        .storage()
        .get_global("records")
        .cloned()
        .expect("records");
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
        .write_global("records", invalid)
        .expect_err("nested +Inf must be rejected");
    assert_non_finite(error);
    assert_eq!(runtime.storage().get_global("records"), Some(&original));
    assert!(!runtime.faulted());
}

/// Spec 34 §4.3.1: the finite policy keeps -0.0, the smallest subnormal REAL and LREAL::MAX
/// bit-for-bit through every checked engineering write surface.
#[test]
fn checked_admission_public_writes_accept_finite_boundary_values_bit_for_bit() {
    let mut runtime = build();
    let negative_zero = -0.0_f32;
    let subnormal = f32::from_bits(1);

    runtime
        .write_global("level", Value::Real(negative_zero))
        .expect("-0.0 is finite");
    assert_eq!(
        real_bits(runtime.storage().get_global("level")),
        Some(negative_zero.to_bits())
    );

    let level = runtime
        .storage()
        .ref_for_global("level")
        .expect("level reference");
    runtime
        .write_reference(&level, Value::Real(subnormal))
        .expect("smallest subnormal is finite");
    assert_eq!(
        real_bits(runtime.storage().get_global("level")),
        Some(subnormal.to_bits())
    );

    runtime
        .write_access("LevelAccess", Value::Real(negative_zero))
        .expect("-0.0 alias write is finite");
    let Some(Value::Real(alias)) = runtime.read_access("LevelAccess") else {
        panic!("alias is a REAL")
    };
    assert_eq!(alias.to_bits(), negative_zero.to_bits());

    let probe = probe_instance(&runtime);
    runtime
        .write_instance(probe, "x", Value::Real(subnormal))
        .expect("smallest subnormal member is finite");
    assert_eq!(
        real_bits(runtime.storage().get_instance_var(probe, "x")),
        Some(subnormal.to_bits())
    );

    runtime
        .write_global("wide", Value::LReal(f64::MAX))
        .expect("LREAL::MAX is finite");
    assert_eq!(
        lreal_bits(runtime.storage().get_global("wide")),
        Some(f64::MAX.to_bits())
    );
    assert!(!runtime.faulted());
}
