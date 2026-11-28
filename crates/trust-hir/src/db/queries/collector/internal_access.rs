//! INTERNAL elements of a namespace (IEC 61131-3 Ed.3 §6.9.2): one outside any namespace, and an
//! INTERNAL type named outside its declaring namespace.

use super::*;
use crate::db::diagnostics::namespace_path_for_symbol;

impl SymbolCollector<'_> {
    /// IEC 61131-3 Ed.3 §6.9.2: an element of a namespace may be INTERNAL; one outside any
    /// namespace has nothing to limit its access to, and is an error at its name.
    pub(super) fn refuse_internal_outside_namespace(&mut self, symbol: &Symbol, range: TextRange) {
        if symbol.visibility == Visibility::Internal
            && !matches!(
                symbol.kind,
                SymbolKind::Method { .. } | SymbolKind::Property { .. } | SymbolKind::Namespace
            )
            && self.current_namespace_path().is_empty()
        {
            self.diagnostics.error(
                DiagnosticCode::InvalidOperation,
                range,
                format!(
                    "INTERNAL applies only to an element of a namespace, and '{}' is in none \
                     (IEC 61131-3 §6.9.2)",
                    symbol.name
                ),
            );
        }
    }

    /// IEC 61131-3 Ed.3 §6.9.2: an INTERNAL type of a namespace is named only inside its
    /// declaring namespace (as an INTERNAL member, specification 04).
    pub(super) fn refuse_internal_type_outside(
        &mut self,
        symbol_id: SymbolId,
        range: Option<TextRange>,
    ) {
        let Some(symbol) = self.table.get(symbol_id) else {
            return;
        };
        if symbol.visibility != Visibility::Internal {
            return;
        }
        let declared = namespace_path_for_symbol(&self.table, symbol_id);
        if declared.is_empty() || self.current_namespace_path() == declared {
            return;
        }
        let message = format!(
            "cannot use INTERNAL type '{}' outside namespace '{}'",
            symbol.name,
            declared.join(".")
        );
        if let Some(range) = range {
            self.diagnostics
                .error(DiagnosticCode::InvalidOperation, range, message);
        }
    }
}
