use super::super::*;

pub(in crate::db) fn check_nondeterminism(
    symbols: &SymbolTable,
    diagnostics: &mut DiagnosticBuilder,
) {
    for symbol in symbols.iter() {
        if symbol.origin.is_some() || symbol.range.is_empty() {
            continue;
        }

        if should_warn_time_date(symbols, symbol) {
            diagnostics.warning(
                DiagnosticCode::NondeterministicTimeDate,
                symbol.range,
                format!(
                    "time/date value '{}' may introduce nondeterminism",
                    symbol.name
                ),
            );
        }

        if let Some(address) = symbol.direct_address.as_deref() {
            if is_io_address(address) {
                diagnostics.warning(
                    DiagnosticCode::NondeterministicIo,
                    symbol.range,
                    format!(
                        "direct I/O address '{}' on '{}' may introduce nondeterministic timing",
                        address, symbol.name
                    ),
                );
            }
        }
    }
}

fn should_warn_time_date(symbols: &SymbolTable, symbol: &Symbol) -> bool {
    if !matches!(
        symbol.kind,
        SymbolKind::Variable { .. }
            | SymbolKind::Parameter { .. }
            | SymbolKind::Function { .. }
            | SymbolKind::Method { .. }
            | SymbolKind::Property { .. }
    ) {
        return false;
    }
    matches!(
        symbols.resolve_alias_type(symbol.type_id),
        TypeId::TIME
            | TypeId::LTIME
            | TypeId::DATE
            | TypeId::LDATE
            | TypeId::TOD
            | TypeId::LTOD
            | TypeId::DT
            | TypeId::LDT
    )
}

fn is_io_address(address: &str) -> bool {
    let trimmed = address.trim();
    let mut chars = trimmed.chars();
    if let Some('%') = chars.next() {
        if let Some(prefix) = chars.next() {
            return matches!(prefix.to_ascii_uppercase(), 'I' | 'Q');
        }
    }
    false
}

#[cfg(test)]
#[path = "nondeterminism/contract_tests.rs"]
mod contract_tests;
