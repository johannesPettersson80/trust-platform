/// The IEC 61131-3 Ed.3 Table 43 long input names of the standard bistable that a call targets,
/// as (long name, canonical name) pairs. Rows 1b and 2b name SET1/RESET for SR and SET/RESET1 for
/// RS; the runtime binds only the canonical names of rows 1a and 2a. The declared function-block
/// type of the called instance decides the block, as the runtime's builtin dispatch does. `None`
/// for every other call target.
fn standard_bistable_long_names(
    target: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Option<&'static [(&'static str, &'static str)]> {
    let (Some(semantic_db), Some(semantic_file_id)) = (ctx.semantic_db, ctx.semantic_file_id)
    else {
        return None;
    };
    let analysis = semantic_db.analyze(semantic_file_id);
    let symbols = analysis.symbols.as_ref();
    let target_id = call_target_symbol_id(target, ctx, symbols)?;
    let type_id = symbols.resolve_alias_type(symbols.get(target_id)?.type_id);
    let Some(Type::FunctionBlock { name }) = symbols.type_by_id(type_id) else {
        return None;
    };
    match builtin_kind(name.as_str())? {
        BuiltinFbKind::Sr => Some(&[("SET1", "S1"), ("RESET", "R")]),
        BuiltinFbKind::Rs => Some(&[("SET", "S"), ("RESET1", "R1")]),
        _ => None,
    }
}

/// Rewrites the long input names among a standard bistable call's arguments to the canonical
/// names (S1 and R for SR, S and R1 for RS), so the bytecode and the VM carry only those names.
/// Every other argument name is kept.
fn canonicalize_standard_bistable_arguments(args: &mut [CallArg], long_names: &[(&str, &str)]) {
    for arg in args {
        let canonical = arg.name.as_deref().and_then(|name| {
            long_names
                .iter()
                .find(|(long, _)| long.eq_ignore_ascii_case(name))
                .map(|(_, canonical)| *canonical)
        });
        if let Some(canonical) = canonical {
            arg.name = Some(SmolStr::new(canonical));
        }
    }
}
