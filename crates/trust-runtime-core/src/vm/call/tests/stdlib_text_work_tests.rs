//! TIME-02 and TIME-08: a standard function's STRING and WSTRING arguments are charged one unit
//! per UTF-8 byte before its body runs, and so is a conversion's text argument. The result
//! charge belongs to the engine tests, because this double keeps the default result hook.
use super::super::bindings::{VmNativeArg, VmNativeArgValue};
use super::super::stdlib::dispatch_native_stdlib_call;
use super::TestContext;
use crate::error::{ExecutionLimitKind, RuntimeError};
use crate::stdlib::conversions::{self, ConversionSpec};
use crate::value::Value;
use crate::vm::{VmFrame, VmTrap};
use smol_str::SmolStr;

fn expression(value: Value) -> VmNativeArg {
    VmNativeArg {
        name: None,
        value: VmNativeArgValue::Expr(value),
    }
}

fn call(
    context: &mut TestContext,
    frame: &mut VmFrame,
    name: &str,
    conversion_spec: Option<ConversionSpec>,
    args: &[VmNativeArg],
) -> Result<Value, VmTrap> {
    let name = SmolStr::new(name);
    dispatch_native_stdlib_call(context, frame, &name, &name, conversion_spec, args)
}

#[test]
fn concat_arguments_are_charged_per_byte_before_the_body_runs() {
    let mut context = TestContext::new();
    let mut frame = context.frame(0);
    let args = [
        expression(Value::String("abcdefgh".into())),
        expression(Value::String("ijklmnop".into())),
    ];
    // Two 8-byte STRINGs cost 16 units. One unit short, the argument charge is the first
    // charge and refuses, so nothing else was consumed before it.
    context.execution_budget.reset(15);
    let error = call(&mut context, &mut frame, "CONCAT", None, &args).unwrap_err();
    assert!(matches!(
        error,
        VmTrap::Runtime(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 16,
            limit: 15,
        })
    ));
    assert_eq!(context.execution_budget.remaining(), 15);

    context.execution_budget.reset(16);
    let value = call(&mut context, &mut frame, "CONCAT", None, &args).unwrap();
    assert_eq!(value, Value::String("abcdefghijklmnop".into()));
    assert_eq!(context.execution_budget.remaining(), 0);
}

#[test]
fn len_charges_its_text_bytes_and_abs_charges_no_text_work() {
    let mut context = TestContext::new();
    let mut frame = context.frame(0);
    let text = [expression(Value::String("abcdefgh".into()))];
    context.execution_budget.reset(7);
    assert!(matches!(
        call(&mut context, &mut frame, "LEN", None, &text),
        Err(VmTrap::Runtime(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 8,
            limit: 7,
        }))
    ));
    // The result is an INT, so the 8 argument bytes are the whole charge.
    context.execution_budget.reset(8);
    assert_eq!(
        call(&mut context, &mut frame, "LEN", None, &text).unwrap(),
        Value::Int(8)
    );
    assert_eq!(context.execution_budget.remaining(), 0);

    // ABS takes a non-text argument, so the call charges nothing beyond its instruction.
    context.execution_budget.reset(0);
    let number = [expression(Value::Int(-7))];
    assert_eq!(
        call(&mut context, &mut frame, "ABS", None, &number).unwrap(),
        Value::Int(7)
    );
    assert_eq!(context.execution_budget.remaining(), 0);
}

#[test]
fn conversion_text_argument_is_charged_before_the_conversion_runs() {
    let mut context = TestContext::new();
    let mut frame = context.frame(0);
    let spec = conversions::conversion_spec("STRING_TO_INT");
    let text = [expression(Value::String("12345".into()))];
    // Five bytes against a limit of four: refused before the conversion runs.
    context.execution_budget.reset(4);
    assert!(matches!(
        call(&mut context, &mut frame, "STRING_TO_INT", spec, &text),
        Err(VmTrap::Runtime(RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::Work,
            used: 0,
            requested: 5,
            limit: 4,
        }))
    ));
    context.execution_budget.reset(5);
    assert_eq!(
        call(&mut context, &mut frame, "STRING_TO_INT", spec, &text).unwrap(),
        Value::Int(12_345)
    );
    assert_eq!(context.execution_budget.remaining(), 0);
}
