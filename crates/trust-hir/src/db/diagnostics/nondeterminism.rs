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

        if let Some(address) = symbol.direct_address.as_deref()
            && is_io_address(address)
            && in_reusable_code(symbols, symbol)
        {
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

/// Whether a declaration lies in reusable code: a function block, function, method or class,
/// where a direct I/O address ties every use to one channel and to the I/O image's timing. A
/// program's or configuration's I/O list is the normal place for addresses.
fn in_reusable_code(symbols: &SymbolTable, symbol: &Symbol) -> bool {
    let mut parent = symbol.parent;
    for _ in 0..16 {
        let Some(owner) = parent.and_then(|id| symbols.get(id)) else {
            return false;
        };
        match owner.kind {
            SymbolKind::FunctionBlock
            | SymbolKind::Function { .. }
            | SymbolKind::Method { .. }
            | SymbolKind::Class => return true,
            SymbolKind::Program
            | SymbolKind::Configuration
            | SymbolKind::Resource
            | SymbolKind::Namespace => return false,
            _ => parent = owner.parent,
        }
    }
    false
}

fn is_io_address(address: &str) -> bool {
    let trimmed = address.trim();
    let mut chars = trimmed.chars();
    if let Some('%') = chars.next()
        && let Some(prefix) = chars.next()
    {
        return matches!(prefix.to_ascii_uppercase(), 'I' | 'Q');
    }
    false
}

#[cfg(test)]
#[path = "nondeterminism/contract_tests.rs"]
mod contract_tests;
