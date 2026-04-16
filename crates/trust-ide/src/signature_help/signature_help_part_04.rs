fn conversion_signature(name: &str) -> Option<SignatureInfo> {
    let upper = name.to_ascii_uppercase();

    if upper == "TRUNC" {
        return Some(SignatureInfo {
            name: SmolStr::new(name),
            params: vec![param("IN", TypeId::ANY_REAL)],
            return_type: Some(TypeId::DINT),
        });
    }

    if let Some(dst_name) = upper.strip_prefix("TRUNC_") {
        let dst = TypeId::from_builtin_name(dst_name)?;
        return Some(SignatureInfo {
            name: SmolStr::new(name),
            params: vec![param("IN", TypeId::ANY_REAL)],
            return_type: Some(dst),
        });
    }

    if let Some((_, dst_name)) = upper.split_once("_TRUNC_") {
        let dst = TypeId::from_builtin_name(dst_name)?;
        return Some(SignatureInfo {
            name: SmolStr::new(name),
            params: vec![param("IN", TypeId::ANY_REAL)],
            return_type: Some(dst),
        });
    }

    if let Some(dst_name) = upper.strip_prefix("TO_BCD_") {
        let dst = TypeId::from_builtin_name(dst_name)?;
        return Some(SignatureInfo {
            name: SmolStr::new(name),
            params: vec![param("IN", TypeId::ANY_UNSIGNED)],
            return_type: Some(dst),
        });
    }

    if let Some((dst_name, _)) = upper.split_once("_TO_BCD_") {
        let dst = TypeId::from_builtin_name(dst_name)?;
        return Some(SignatureInfo {
            name: SmolStr::new(name),
            params: vec![param("IN", TypeId::ANY_UNSIGNED)],
            return_type: Some(dst),
        });
    }

    if let Some(dst_name) = upper.strip_prefix("BCD_TO_") {
        let dst = TypeId::from_builtin_name(dst_name)?;
        return Some(SignatureInfo {
            name: SmolStr::new(name),
            params: vec![param("IN", TypeId::ANY_BIT)],
            return_type: Some(dst),
        });
    }

    if let Some((_, dst_name)) = upper.split_once("_BCD_TO_") {
        let dst = TypeId::from_builtin_name(dst_name)?;
        return Some(SignatureInfo {
            name: SmolStr::new(name),
            params: vec![param("IN", TypeId::ANY_BIT)],
            return_type: Some(dst),
        });
    }

    if let Some(dst_name) = upper.strip_prefix("TO_") {
        let dst = TypeId::from_builtin_name(dst_name)?;
        return Some(SignatureInfo {
            name: SmolStr::new(name),
            params: vec![param("IN", TypeId::ANY)],
            return_type: Some(dst),
        });
    }

    if let Some((_, dst_name)) = upper.split_once("_TO_") {
        let dst = TypeId::from_builtin_name(dst_name)?;
        return Some(SignatureInfo {
            name: SmolStr::new(name),
            params: vec![param("IN", TypeId::ANY)],
            return_type: Some(dst),
        });
    }

    None
}

fn param(name: &str, type_id: TypeId) -> ParamData {
    ParamData {
        name: SmolStr::new(name),
        type_id,
        direction: ParamDirection::In,
        is_constant: false,
    }
}

fn out_param(name: &str, type_id: TypeId) -> ParamData {
    ParamData {
        name: SmolStr::new(name),
        type_id,
        direction: ParamDirection::Out,
        is_constant: false,
    }
}

fn fixed_in(prefix: &str, count: usize, type_id: TypeId) -> Vec<ParamData> {
    (1..=count)
        .map(|index| param(&format!("{}{}", prefix, index), type_id))
        .collect()
}

fn variadic_in(prefix: &str, count: usize, min: usize, type_id: TypeId) -> Vec<ParamData> {
    let total = std::cmp::max(count, min);
    fixed_in(prefix, total, type_id)
}

fn mux_params(arg_count: usize) -> Vec<ParamData> {
    let mut params = vec![param("K", TypeId::ANY_INT)];
    let inputs = std::cmp::max(arg_count.saturating_sub(1), 2);
    for index in 0..inputs {
        params.push(param(&format!("IN{}", index), TypeId::ANY));
    }
    params
}

fn time_binary(lhs: TypeId, rhs: TypeId) -> Vec<ParamData> {
    vec![param("IN1", lhs), param("IN2", rhs)]
}
