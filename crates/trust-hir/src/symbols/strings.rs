//! `STRING[n]` and `WSTRING[n]` types in the symbol table (`docs/IEC_DECISIONS.md` 2026-10-17).

use crate::types::{Type, TypeId, DEFAULT_STRING_LENGTH};

use super::table::SymbolTable;

impl SymbolTable {
    /// The type of `STRING[length]`, or of `WSTRING[length]` when `is_wide`. The default length
    /// is the built-in `STRING` or `WSTRING` itself, so `STRING[80]` and `STRING` are one type.
    pub fn register_string_type(&mut self, is_wide: bool, length: u32) -> TypeId {
        match (is_wide, length) {
            (false, DEFAULT_STRING_LENGTH) => TypeId::STRING,
            (true, DEFAULT_STRING_LENGTH) => TypeId::WSTRING,
            (false, _) => self.register_type(
                format!("STRING[{length}]"),
                Type::String {
                    max_len: Some(length),
                },
            ),
            (true, _) => self.register_type(
                format!("WSTRING[{length}]"),
                Type::WString {
                    max_len: Some(length),
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::symbols::SymbolTable;
    use crate::types::{Type, TypeId};

    #[test]
    fn the_default_length_is_the_built_in_type() {
        let mut table = SymbolTable::new();
        assert_eq!(table.register_string_type(false, 80), TypeId::STRING);
        assert_eq!(table.register_string_type(true, 80), TypeId::WSTRING);
        assert_eq!(
            table.type_by_id(TypeId::STRING),
            Some(&Type::String { max_len: Some(80) })
        );
        let short = table.register_string_type(false, 12);
        assert_eq!(table.register_string_type(false, 12), short);
        assert_ne!(short, TypeId::STRING);
        assert_eq!(
            table.type_by_id(short),
            Some(&Type::String { max_len: Some(12) })
        );
    }
}
