pub(super) fn eval_ref_call(
    ctx: &mut EvalContext<'_>,
    args: &[CallArg],
) -> Result<Value, RuntimeError> {
    if args.len() != 1 {
        return Err(RuntimeError::InvalidArgumentCount {
            expected: 1,
            got: args.len(),
        });
    }
    let arg = &args[0];
    let ArgValue::Target(target) = &arg.value else {
        return Err(RuntimeError::TypeMismatch);
    };
    let reference = resolve_reference_for_lvalue(ctx, target)?;
    Ok(Value::Reference(Some(reference)))
}
