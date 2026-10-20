use alloc::vec::Vec;

use smol_str::SmolStr;

use crate::stdlib::conversions;

use super::module::{VmNativeArgSpec, VmNativeSymbolSpec};
use crate::error::RuntimeError;

/// The opcode of `CALL_NATIVE` (specification 12 §7), whose second operand names its symbol.
const CALL_NATIVE: u8 = 0x09;

/// The parsed native-call symbols of a module, by string index (specification 34 MEM-14): only the
/// strings that a `CALL_NATIVE` instruction names are parsed, not every string of the table, and the
/// entries are sorted by string index for a binary search.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeSymbols {
    entries: Vec<(u32, VmNativeSymbolSpec)>,
}

impl NativeSymbols {
    /// Parse the strings of `strings` that `used` names, a sorted list without repeats.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::PreparationLimit`] when the table cannot be reserved, and an invalid-bytecode
    /// error when `used` names a string the table does not hold.
    pub(crate) fn parse(strings: &[SmolStr], used: &[u32]) -> Result<Self, RuntimeError> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(used.len())
            .map_err(|_| RuntimeError::PreparationLimit)?;
        for &index in used {
            let symbol = strings.get(index as usize).ok_or_else(|| {
                RuntimeError::InvalidBytecode(SmolStr::new_static(
                    "CALL_NATIVE names a string the table does not hold",
                ))
            })?;
            entries.push((index, preparse_native_symbol_spec(symbol)));
        }
        Ok(Self { entries })
    }

    /// The table of every string of `strings`, each parsed, as the legacy adapter keeps it.
    #[cfg(feature = "hir")]
    pub(crate) fn every(strings: &[SmolStr]) -> Self {
        Self {
            entries: strings
                .iter()
                .zip(0u32..)
                .map(|(symbol, index)| (index, preparse_native_symbol_spec(symbol)))
                .collect(),
        }
    }

    /// The symbol of string `symbol_idx`, when an instruction names it.
    #[must_use]
    pub fn get(&self, symbol_idx: u32) -> Option<&VmNativeSymbolSpec> {
        self.entries
            .binary_search_by_key(&symbol_idx, |(index, _)| *index)
            .ok()
            .map(|at| &self.entries[at].1)
    }

    /// Every parsed symbol with its string index, in string order.
    #[cfg(feature = "register-ir")]
    pub(crate) fn indexed(&self) -> impl Iterator<Item = (u32, &VmNativeSymbolSpec)> {
        self.entries.iter().map(|(index, spec)| (*index, spec))
    }

    /// Every parsed symbol, in string order.
    pub fn iter(&self) -> impl Iterator<Item = &VmNativeSymbolSpec> {
        self.entries.iter().map(|(_, spec)| spec)
    }

    /// Every parsed symbol, in string order, to resolve.
    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut VmNativeSymbolSpec> {
        self.entries.iter_mut().map(|(_, spec)| spec)
    }

    /// Add the symbol of string `symbol_idx`, which follows every string the table holds.
    #[cfg(feature = "hir")]
    pub(crate) fn push(&mut self, symbol_idx: u32, spec: VmNativeSymbolSpec) {
        debug_assert!(self
            .entries
            .last()
            .is_none_or(|(last, _)| *last < symbol_idx));
        self.entries.push((symbol_idx, spec));
    }

    /// The table whose entry `k` is the symbol of string `k`, for a test that builds a module by
    /// hand.
    #[cfg(any(test, feature = "test-support"))]
    pub fn from_dense(specs: Vec<VmNativeSymbolSpec>) -> Self {
        Self {
            entries: (0u32..).zip(specs).collect(),
        }
    }

    /// Replace the symbol of string `symbol_idx`, for a test that edits a module by hand.
    #[cfg(any(test, feature = "test-support"))]
    pub fn replace(&mut self, symbol_idx: u32, spec: VmNativeSymbolSpec) {
        match self
            .entries
            .binary_search_by_key(&symbol_idx, |(index, _)| *index)
        {
            Ok(at) => self.entries[at].1 = spec,
            Err(at) => self.entries.insert(at, (symbol_idx, spec)),
        }
    }
}

/// The string indexes that the `CALL_NATIVE` instructions within `ranges` of `bodies` name, sorted
/// and without repeats (specification 34 MEM-14). Each range is an offset and a length in `bodies`,
/// as POU_INDEX and INITIALIZERS give them.
///
/// # Errors
///
/// An invalid-bytecode error for a range outside `bodies`, an unknown opcode or a truncated
/// instruction, which validation has already refused.
pub(crate) fn used_native_symbols(
    bodies: &[u8],
    ranges: impl IntoIterator<Item = (u32, u32)>,
) -> Result<Vec<u32>, RuntimeError> {
    let invalid = |reason: &'static str| RuntimeError::InvalidBytecode(SmolStr::new_static(reason));
    let mut used = Vec::new();
    for (offset, length) in ranges {
        let start = offset as usize;
        let code = start
            .checked_add(length as usize)
            .and_then(|end| bodies.get(start..end))
            .ok_or_else(|| invalid("invalid executable range"))?;
        let mut pc = 0usize;
        while pc < code.len() {
            let opcode = code[pc];
            let width = super::opcode_operand_len(opcode)
                .ok_or_else(|| invalid("unsupported executable opcode"))?;
            let next = pc
                .checked_add(1 + width)
                .filter(|end| *end <= code.len())
                .ok_or_else(|| invalid("truncated executable instruction"))?;
            if opcode == CALL_NATIVE {
                let operand = code
                    .get(pc + 5..pc + 9)
                    .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
                    .ok_or_else(|| invalid("truncated executable instruction"))?;
                used.push(u32::from_le_bytes(operand));
            }
            pc = next;
        }
    }
    used.sort_unstable();
    used.dedup();
    Ok(used)
}

/// Parse and normalize a native-call symbol once during materialization.
pub fn preparse_native_symbol_spec(symbol: &SmolStr) -> VmNativeSymbolSpec {
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

/// Associate parsed call descriptors with known function POU identities.
pub fn resolve_native_symbol_specs(
    specs: &mut NativeSymbols,
    names: &super::Names,
    function_ids: &super::names::NameIndex,
) {
    for spec in specs.iter_mut() {
        if let VmNativeSymbolSpec::Parsed {
            normalized_target_name,
            resolved_function_pou_id,
            ..
        } = spec
        {
            *resolved_function_pou_id = function_ids.get(names, normalized_target_name);
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

impl crate::memory::figure::HeapBytes for NativeSymbols {
    fn add_heap(&self, count: &mut crate::memory::figure::HeapCount) {
        count.of(&self.entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::names::NameIndex;
    use crate::vm::Names;
    use alloc::vec;

    /// A native call names its function in any case, and resolution caches that function's POU
    /// (specification 34 MEM-14); a symbol the module has no function for resolves to none. Catches
    /// a lookup by exact bytes, which misses "Add" for the function "ADD", and one that resolves
    /// every symbol.
    #[test]
    fn resolve_native_symbol_specs_caches_resolved_function_id() {
        let mut specs = NativeSymbols::from_dense(vec![
            preparse_native_symbol_spec(&SmolStr::new("Add|E:a")),
            preparse_native_symbol_spec(&SmolStr::new("Len|E:in")),
        ]);
        let names: Names = ["ADD"].into_iter().collect();
        let mut function_ids = NameIndex::with_capacity(1).unwrap();
        assert!(function_ids.insert(&names, 0, 7));

        resolve_native_symbol_specs(&mut specs, &names, &function_ids);

        let resolved = |index| match specs.get(index) {
            Some(VmNativeSymbolSpec::Parsed {
                resolved_function_pou_id,
                ..
            }) => *resolved_function_pou_id,
            other => panic!("symbol {index} did not parse: {other:?}"),
        };
        assert_eq!(resolved(0), Some(7));
        assert_eq!(resolved(1), None);
    }
}
