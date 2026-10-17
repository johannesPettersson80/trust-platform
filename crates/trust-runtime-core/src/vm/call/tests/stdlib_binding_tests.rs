//! Spec 34 MEM-09, §6.3 (census C9, C10): standard-function arguments and split outputs bind into
//! the call depth's reserved values, a named argument matches its formal without building an
//! uppercase name, and every result, count and name refusal is unchanged.
use alloc::{vec, vec::Vec};
use smol_str::SmolStr;

use super::super::bindings::{VmNativeArg, VmNativeArgValue};
use super::super::stdlib::{
    bind_stdlib_named_values, bind_stdlib_positional_values, dispatch_native_stdlib_call,
};
use super::TestContext;
use crate::error::RuntimeError;
use crate::memory::{FrameId, Generation, MemoryLocation};
use crate::stdlib::StdParams;
use crate::value::{TimeOfDayValue, Value, ValueRef};
use crate::vm::{VmFrame, VmTrap};

/// An unnamed expression argument.
fn expression(value: Value) -> VmNativeArg {
    VmNativeArg {
        name: None,
        value: VmNativeArgValue::Expr(value),
    }
}

/// A named expression argument.
fn named(name: &str, value: Value) -> VmNativeArg {
    VmNativeArg {
        name: Some(SmolStr::new(name)),
        value: VmNativeArgValue::Expr(value),
    }
}

/// A named or positional output target at a caller-local slot. The legacy sentinel frame makes a
/// path-free reference address the caller's own locals.
fn target(name: Option<&str>, offset: u32) -> VmNativeArg {
    VmNativeArg {
        name: name.map(SmolStr::new),
        value: VmNativeArgValue::Target(ValueRef {
            location: MemoryLocation::Local(FrameId(
                crate::vm::context::VM_LOCAL_SENTINEL_FRAME_ID,
            )),
            offset,
            generation: Generation::FIRST,
            path: Vec::new(),
        }),
    }
}

/// A caller frame with four DINT local slots, the `SPLIT_TOD` output width.
fn split_frame() -> VmFrame {
    VmFrame {
        activation: None,
        pou_id: Some(1),
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 4,
        locals: vec![Value::DInt(0); 4],
        parameter_values_present: Vec::new(),
        runtime_instance: None,
        instance_owner: None,
    }
}

/// Dispatch a standard function with no conversion.
fn dispatch_call(
    context: &mut TestContext,
    frame: &mut VmFrame,
    name: &str,
    args: &[VmNativeArg],
) -> Result<Value, VmTrap> {
    let name = SmolStr::new(name);
    dispatch_native_stdlib_call(context, frame, &name, &name, None, args)
}

/// Spec 34 MEM-09, §6.3 (census C9): a fixed standard function's named arguments bind to their
/// declared formals in declared order, and an actual name matches its formal with its ASCII
/// letters made uppercase without building an uppercase name. Catches a binder that keeps
/// argument order, or that matches names case-sensitively.
#[test]
fn named_standard_arguments_bind_in_declared_order() {
    let mut context = TestContext::new();
    let frame = context.frame(0);
    let params =
        StdParams::Fixed(vec![SmolStr::new("G"), SmolStr::new("IN0"), SmolStr::new("IN1")].into());
    let args = [
        named("in1", Value::Int(7)),
        named("G", Value::Bool(true)),
        named("In0", Value::Int(4)),
    ];
    let values = bind_stdlib_named_values(&mut context, &frame, &params, &args)
        .expect("the SEL-shaped named binding");
    assert_eq!(
        values,
        vec![Value::Bool(true), Value::Int(4), Value::Int(7)],
        "G, IN0 and IN1 in declared order"
    );
}

/// Spec 34 MEM-09, §6.3 (census C9): a formal matches only the actual name with its ASCII letters
/// made uppercase, so a custom registration holding a lowercase formal never matches it. Catches
/// a matcher that compares case-insensitively, which would newly accept `in` for the formal `in`.
#[test]
fn a_lowercase_formal_never_matches_as_before() {
    let mut context = TestContext::new();
    let frame = context.frame(0);
    let params = StdParams::Fixed(vec![SmolStr::new("in")].into());
    let error = bind_stdlib_named_values(
        &mut context,
        &frame,
        &params,
        &[named("in", Value::DInt(1))],
    )
    .unwrap_err();
    assert_eq!(
        error.into_runtime_error(),
        RuntimeError::InvalidArgumentName(SmolStr::new("in"))
    );
}

/// Spec 34 MEM-09, §6.3 (census C9): named binding keeps every refusal and their order — an
/// unnamed argument among named ones, an unknown name, a duplicate and a missing formal each
/// fail exactly as before, and the duplicate's second value is not evaluated. Catches a binder
/// that drops a check, evaluates a duplicate's value, or reports the wrong counts.
#[test]
fn named_standard_argument_errors_are_unchanged() {
    let params = StdParams::Fixed(vec![SmolStr::new("G"), SmolStr::new("IN0")].into());
    let mut context = TestContext::new();
    let frame = context.frame(0);

    let error = bind_stdlib_named_values(
        &mut context,
        &frame,
        &params,
        &[expression(Value::Bool(true)), named("G", Value::Int(1))],
    )
    .unwrap_err();
    assert_eq!(
        error.into_runtime_error(),
        RuntimeError::InvalidArgumentName(SmolStr::new("<unnamed>"))
    );

    let error = bind_stdlib_named_values(
        &mut context,
        &frame,
        &StdParams::Fixed(vec![SmolStr::new("G")].into()),
        &[named("X", Value::Bool(true))],
    )
    .unwrap_err();
    assert_eq!(
        error.into_runtime_error(),
        RuntimeError::InvalidArgumentName(SmolStr::new("X"))
    );

    // A duplicate fails on its second valuation, which never runs: only the first value's copy
    // is admitted.
    let mut context = TestContext::new();
    let frame = context.frame(0);
    let error = bind_stdlib_named_values(
        &mut context,
        &frame,
        &StdParams::Fixed(vec![SmolStr::new("IN"), SmolStr::new("N")].into()),
        &[named("IN", Value::DInt(1)), named("IN", Value::DInt(2))],
    )
    .unwrap_err();
    assert_eq!(
        error.into_runtime_error(),
        RuntimeError::InvalidArgumentName(SmolStr::new("IN"))
    );
    assert_eq!(
        context.clone_admissions.get(),
        1,
        "the duplicate's value is not evaluated"
    );

    // A missing formal of a fixed function is the count refusal.
    let mut context = TestContext::new();
    let frame = context.frame(0);
    let error = bind_stdlib_named_values(
        &mut context,
        &frame,
        &StdParams::Fixed(vec![SmolStr::new("IN"), SmolStr::new("N")].into()),
        &[named("IN", Value::DInt(1))],
    )
    .unwrap_err();
    assert_eq!(
        error.into_runtime_error(),
        RuntimeError::InvalidArgumentCount {
            expected: 2,
            got: 1,
        }
    );
}

/// Spec 34 MEM-09, §6.3 (census C9): positional arguments bind in declared order into the
/// reserved values, after the fixed and variadic count checks. Catches a binder that binds named
/// slots for a positional call, or skips the fixed-plus-variadic minimum check.
#[test]
fn positional_standard_arguments_bind_in_declared_order() {
    let mut context = TestContext::new();
    let frame = context.frame(0);
    let params =
        StdParams::Fixed(vec![SmolStr::new("G"), SmolStr::new("IN0"), SmolStr::new("IN1")].into());
    let values = bind_stdlib_positional_values(
        &mut context,
        &frame,
        &params,
        &[
            expression(Value::Bool(true)),
            expression(Value::Int(4)),
            expression(Value::Int(7)),
        ],
    )
    .expect("the SEL-shaped positional binding");
    assert_eq!(
        values,
        vec![Value::Bool(true), Value::Int(4), Value::Int(7)],
        "G, IN0 and IN1 in declared order"
    );

    let error = bind_stdlib_positional_values(
        &mut context,
        &frame,
        &StdParams::Variadic {
            fixed: vec![SmolStr::new("K")].into(),
            prefix: SmolStr::new("IN"),
            start: 0,
            min: 2,
        },
        &[expression(Value::DInt(0)), expression(Value::DInt(10))],
    )
    .unwrap_err();
    assert_eq!(
        error.into_runtime_error(),
        RuntimeError::InvalidArgumentCount {
            expected: 3,
            got: 2,
        }
    );
}

/// Spec 34 MEM-09, §6.3 (census C9): a variadic function's named arguments bind in suffix index
/// order, and a suffix below the start, an empty suffix or a non-numeric suffix is refused by
/// the name as written. Catches a binder that keeps argument order, accepts a suffix below the
/// start, or reports a variadic hole with the wrong expected count.
#[test]
fn variadic_named_arguments_bind_in_index_order() {
    let params = StdParams::Variadic {
        fixed: Vec::new().into(),
        prefix: SmolStr::new("IN"),
        start: 1,
        min: 2,
    };
    let mut context = TestContext::new();
    let frame = context.frame(0);

    let values = bind_stdlib_named_values(
        &mut context,
        &frame,
        &params,
        &[named("IN2", Value::DInt(9)), named("IN1", Value::DInt(5))],
    )
    .expect("the variadic named binding");
    assert_eq!(values, vec![Value::DInt(5), Value::DInt(9)]);

    for refused in ["IN0", "IN", "INx"] {
        let error = bind_stdlib_named_values(
            &mut context,
            &frame,
            &params,
            &[named(refused, Value::DInt(1))],
        )
        .unwrap_err();
        assert_eq!(
            error.into_runtime_error(),
            RuntimeError::InvalidArgumentName(SmolStr::new(refused)),
            "{refused}"
        );
    }

    let error = bind_stdlib_named_values(
        &mut context,
        &frame,
        &params,
        &[named("IN1", Value::DInt(5)), named("IN3", Value::DInt(9))],
    )
    .unwrap_err();
    assert_eq!(
        error.into_runtime_error(),
        RuntimeError::InvalidArgumentCount {
            expected: 3,
            got: 2,
        },
        "IN1 and IN3 leave IN2 a hole"
    );
}

/// Spec 34 MEM-09, §6.3 (census C10): `SPLIT_TOD` binds its output targets into a fixed local
/// array from the call's arguments, and the positional and the named call write the same values
/// to the same targets; a wrong count and a duplicate name keep their refusals. Catches a split
/// binder that misorders the named formals, drops an output or accepts a hole.
#[test]
fn split_arguments_bind_without_vectors() {
    let input = Value::Tod(TimeOfDayValue::new(45_296_789));

    let mut context = TestContext::new();
    let mut positional = split_frame();
    let mut positional_args = vec![expression(input.clone())];
    for offset in 0..4u32 {
        positional_args.push(target(None, offset));
    }
    dispatch_call(&mut context, &mut positional, "SPLIT_TOD", &positional_args)
        .expect("the positional SPLIT_TOD");
    assert_eq!(
        positional.locals,
        vec![
            Value::DInt(12),
            Value::DInt(34),
            Value::DInt(56),
            Value::DInt(789)
        ],
        "TOD#12:34:56.789"
    );

    let mut context = TestContext::new();
    let mut named_frame = split_frame();
    let mut named_args = vec![named("IN", input.clone())];
    for (name, offset) in ["HOUR", "MINUTE", "SECOND", "MILLISECOND"]
        .into_iter()
        .zip(0..4u32)
    {
        named_args.push(target(Some(name), offset));
    }
    dispatch_call(&mut context, &mut named_frame, "SPLIT_TOD", &named_args)
        .expect("the named SPLIT_TOD");
    assert_eq!(
        named_frame.locals,
        vec![
            Value::DInt(12),
            Value::DInt(34),
            Value::DInt(56),
            Value::DInt(789)
        ],
        "TOD#12:34:56.789"
    );

    let mut context = TestContext::new();
    let mut short = split_frame();
    let error = dispatch_call(
        &mut context,
        &mut short,
        "SPLIT_TOD",
        &[
            expression(input.clone()),
            target(None, 0),
            target(None, 1),
            target(None, 2),
        ],
    )
    .unwrap_err();
    assert_eq!(
        error.into_runtime_error(),
        RuntimeError::InvalidArgumentCount {
            expected: 5,
            got: 4,
        }
    );

    let mut context = TestContext::new();
    let mut duplicate = split_frame();
    let error = dispatch_call(
        &mut context,
        &mut duplicate,
        "SPLIT_TOD",
        &[
            named("IN", input),
            target(Some("HOUR"), 0),
            target(Some("HOUR"), 1),
            target(Some("MINUTE"), 2),
            target(Some("SECOND"), 3),
        ],
    )
    .unwrap_err();
    assert_eq!(
        error.into_runtime_error(),
        RuntimeError::InvalidArgumentName(SmolStr::new("HOUR"))
    );
}
