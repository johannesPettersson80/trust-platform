use std::collections::HashMap;

use smol_str::SmolStr;

use crate::stdlib::conversions;

use super::super::{VmNativeArgSpec, VmNativeSymbolSpec};

pub(in crate::runtime::vm) fn preparse_native_symbol_spec(symbol: &SmolStr) -> VmNativeSymbolSpec {
    match parse_native_symbol(symbol) {
        Ok((target_name, arg_specs)) => {
            let normalized_target_name = SmolStr::new(target_name.to_ascii_uppercase());
            let conversion_spec = conversions::conversion_spec(normalized_target_name.as_str());
            VmNativeSymbolSpec::Parsed {
                normalized_target_name,
                resolved_function_pou_id: None,
                conversion_spec,
                target_name,
                arg_specs,
            }
        }
        Err(err) => VmNativeSymbolSpec::ParseError(err),
    }
}

pub(in crate::runtime::vm) fn resolve_native_symbol_specs(
    specs: &mut [VmNativeSymbolSpec],
    function_ids: &HashMap<SmolStr, u32>,
) {
    for spec in specs {
        if let VmNativeSymbolSpec::Parsed {
            normalized_target_name,
            resolved_function_pou_id,
            ..
        } = spec
        {
            *resolved_function_pou_id = function_ids.get(normalized_target_name).copied();
        }
    }
}

fn parse_native_symbol(symbol: &SmolStr) -> Result<(SmolStr, Vec<VmNativeArgSpec>), SmolStr> {
    let mut parts = symbol.split('|');
    let target = SmolStr::new(parts.next().unwrap_or_default());
    let mut args = Vec::new();
    for raw in parts {
        if raw.is_empty() {
            return Err("empty CALL_NATIVE arg token".into());
        }
        let (is_target, suffix) = if let Some(rest) = raw.strip_prefix('E') {
            (false, rest)
        } else if let Some(rest) = raw.strip_prefix('T') {
            (true, rest)
        } else {
            return Err("CALL_NATIVE arg token must start with E/T".into());
        };
        let name = if suffix.is_empty() {
            None
        } else if let Some(named) = suffix.strip_prefix(':') {
            if named.is_empty() {
                return Err("CALL_NATIVE named token missing argument name".into());
            }
            Some(SmolStr::new(named))
        } else {
            return Err("CALL_NATIVE arg token suffix must be ':NAME'".into());
        };
        args.push(VmNativeArgSpec { name, is_target });
    }
    Ok((target, args))
}
