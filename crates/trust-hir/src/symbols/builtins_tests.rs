use super::super::{SymbolKind, SymbolTable};
use super::STANDARD_FUNCTION_BLOCKS;

// Specification 08: catches a standard function block that the builtin table declares and the
// list lacks, which the editor and the runtime's tests would then not check, or one the list
// names that the analyzer does not declare.
#[test]
fn the_builtin_table_declares_exactly_the_standard_function_blocks() {
    let table = SymbolTable::new();
    let mut declared: Vec<&str> = table
        .iter()
        .filter(|symbol| matches!(symbol.kind, SymbolKind::FunctionBlock))
        .map(|symbol| symbol.name.as_str())
        .collect();
    declared.sort_unstable();
    let mut listed = STANDARD_FUNCTION_BLOCKS.to_vec();
    listed.sort_unstable();
    assert_eq!(declared, listed);
}
