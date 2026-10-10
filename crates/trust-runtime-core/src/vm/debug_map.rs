#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap as HashMap;
#[cfg(test)]
use alloc::vec;
#[cfg(feature = "std")]
use std::collections::HashMap;

use smol_str::SmolStr;

use crate::bytecode::{DebugMap, StringTable, VarMeta};

/// Source position associated with an instruction.
#[derive(Debug, Clone)]
pub struct VmSourceLocation {
    /// Source file label from the debug string table.
    pub file: SmolStr,
    /// Source line recorded in bytecode debug metadata.
    pub line: u32,
    /// Source column recorded in bytecode debug metadata.
    pub column: u32,
}

/// Immutable-by-borrow source and symbolic reference indexes.
#[derive(Debug, Clone, Default)]
pub struct VmDebugMap {
    /// Named symbols mapped to reference-table indices.
    pub symbol_to_ref: HashMap<SmolStr, u32>,
    /// First recorded symbolic name for each reference index.
    pub ref_to_symbol: HashMap<u32, SmolStr>,
    /// Source locations keyed by POU identity and instruction byte offset.
    pub source_by_pc: HashMap<(u32, u32), VmSourceLocation>,
}

impl VmDebugMap {
    /// Build optional debugger indexes from bytecode metadata sections.
    pub fn from_sections(
        strings: &StringTable,
        var_meta: Option<&VarMeta>,
        debug_strings: Option<&StringTable>,
        debug_map: Option<&DebugMap>,
    ) -> Self {
        let mut map = Self::default();

        if let Some(meta) = var_meta {
            for entry in &meta.entries {
                let Some(name) = strings.entries.get(entry.name_idx as usize) else {
                    continue;
                };
                map.symbol_to_ref.insert(name.clone(), entry.ref_idx);
                map.ref_to_symbol
                    .entry(entry.ref_idx)
                    .or_insert_with(|| name.clone());
            }
        }

        if let (Some(files), Some(debug)) = (debug_strings, debug_map) {
            for entry in &debug.entries {
                let Some(file) = files.entries.get(entry.file_idx as usize) else {
                    continue;
                };
                map.source_by_pc.insert(
                    (entry.pou_id, entry.code_offset),
                    VmSourceLocation {
                        file: file.clone(),
                        line: entry.line,
                        column: entry.column,
                    },
                );
            }
        }

        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::bytecode::{DebugEntry, VarMetaEntry};

    #[test]
    fn vm_debug_map_preserves_symbols_and_first_reverse_symbol() {
        let strings = StringTable {
            entries: vec!["PrimaryName".into(), "AliasName".into()],
        };
        let var_meta = VarMeta {
            entries: vec![variable(0, 7), variable(1, 7), variable(99, 8)],
        };

        let map = VmDebugMap::from_sections(&strings, Some(&var_meta), None, None);

        assert_eq!(map.symbol_to_ref.get("PrimaryName"), Some(&7));
        assert_eq!(map.symbol_to_ref.get("AliasName"), Some(&7));
        assert_eq!(
            map.ref_to_symbol.get(&7).map(SmolStr::as_str),
            Some("PrimaryName")
        );
        assert!(!map.ref_to_symbol.contains_key(&8));
    }

    #[test]
    fn vm_debug_map_omits_invalid_source_entries_and_missing_sections() {
        let strings = StringTable::default();
        let debug_strings = StringTable {
            entries: vec!["src/main.st".into()],
        };
        let debug_map = DebugMap {
            entries: vec![source(3, 12, 0, 8, 4), source(3, 13, 99, 9, 2)],
        };

        let map = VmDebugMap::from_sections(&strings, None, Some(&debug_strings), Some(&debug_map));
        let source = map.source_by_pc.get(&(3, 12)).expect("valid source entry");
        assert_eq!(source.file, "src/main.st");
        assert_eq!(source.line, 8);
        assert_eq!(source.column, 4);
        assert!(!map.source_by_pc.contains_key(&(3, 13)));

        let empty = VmDebugMap::from_sections(&strings, None, None, None);
        assert!(empty.symbol_to_ref.is_empty());
        assert!(empty.ref_to_symbol.is_empty());
        assert!(empty.source_by_pc.is_empty());
    }

    fn variable(name_idx: u32, ref_idx: u32) -> VarMetaEntry {
        VarMetaEntry {
            name_idx,
            type_id: 5,
            ref_idx,
            retain: 0,
            init_const_idx: None,
        }
    }

    fn source(pou_id: u32, code_offset: u32, file_idx: u32, line: u32, column: u32) -> DebugEntry {
        DebugEntry {
            pou_id,
            code_offset,
            file_idx,
            line,
            column,
            kind: 0,
        }
    }
}
