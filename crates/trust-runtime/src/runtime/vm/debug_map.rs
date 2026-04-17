use std::collections::HashMap;

use smol_str::SmolStr;

use crate::bytecode::{DebugMap, StringTable, VarMeta};

#[derive(Debug, Clone)]
pub(super) struct VmSourceLocation {
    pub(super) file: SmolStr,
    pub(super) line: u32,
    pub(super) column: u32,
}

#[derive(Debug, Clone, Default)]
pub(super) struct VmDebugMap {
    pub(super) symbol_to_ref: HashMap<SmolStr, u32>,
    pub(super) ref_to_symbol: HashMap<u32, SmolStr>,
    pub(super) source_by_pc: HashMap<(u32, u32), VmSourceLocation>,
}

impl VmDebugMap {
    pub(super) fn from_sections(
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
