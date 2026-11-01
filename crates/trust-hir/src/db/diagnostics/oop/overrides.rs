use super::super::super::queries::*;
use super::super::super::*;
use super::super::context::{method_signature_from_table, method_signatures_match_with_table};
use crate::diagnostics::Diagnostic;

pub(super) fn check_method_overrides(
    symbols: &SymbolTable,
    class_symbol: &Symbol,
    class_range: TextRange,
    declared_methods: &[SymbolId],
    inherited_methods: &FxHashMap<SmolStr, SymbolId>,
    diagnostics: &mut DiagnosticBuilder,
) {
    for method_id in declared_methods.iter().copied() {
        let Some(method_sym) = symbols.get(method_id) else {
            continue;
        };
        let key = normalize_member_name(method_sym.name.as_str());
        let base_method_id = inherited_methods.get(&key).copied();

        if method_sym.modifiers.is_override {
            let Some(base_method_id) = base_method_id else {
                diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    method_sym.range,
                    format!(
                        "method '{}' marked OVERRIDE but no base method found",
                        method_sym.name
                    ),
                );
                continue;
            };
            let Some(base_method) = symbols.get(base_method_id) else {
                continue;
            };
            if base_method.modifiers.is_final {
                diagnostics.error(
                    DiagnosticCode::InvalidOperation,
                    method_sym.range,
                    format!(
                        "method '{}' cannot override FINAL base method",
                        method_sym.name
                    ),
                );
            }
            let expected = method_signature_from_table(symbols, base_method_id);
            let actual = method_signature_from_table(symbols, method_id);
            if let (Some(expected), Some(actual)) = (expected, actual) {
                if !method_signatures_match_with_table(symbols, &expected, &actual) {
                    diagnostics.error(
                        DiagnosticCode::InvalidOperation,
                        method_sym.range,
                        format!("method '{}' does not match base signature", method_sym.name),
                    );
                }
                if method_sym.visibility != base_method.visibility {
                    let mut diagnostic = Diagnostic::error(
                        DiagnosticCode::InvalidOperation,
                        method_sym.range,
                        format!(
                            "method '{}' must use the same access specifier as base method",
                            method_sym.name
                        ),
                    );
                    let expected = super::visibility_label(base_method.visibility);
                    diagnostic = diagnostic.with_related(
                        method_sym.range,
                        format!(
                            "Hint: change access specifier to {expected} to match the base method."
                        ),
                    );
                    diagnostics.add(diagnostic);
                }
            }
        } else if base_method_id.is_some() {
            diagnostics.error(
                DiagnosticCode::InvalidOperation,
                method_sym.range,
                format!(
                    "method '{}' overrides a base method and must use OVERRIDE",
                    method_sym.name
                ),
            );
        }
    }

    if !class_symbol.modifiers.is_abstract {
        for (key, base_method_id) in inherited_methods.iter() {
            let Some(base_method) = symbols.get(*base_method_id) else {
                continue;
            };
            if !base_method.modifiers.is_abstract {
                continue;
            }
            let has_impl = declared_methods.iter().copied().find(|method_id| {
                symbols
                    .get(*method_id)
                    .map(|method_sym| {
                        normalize_member_name(method_sym.name.as_str()) == *key
                            && !method_sym.modifiers.is_abstract
                    })
                    .unwrap_or(false)
            });
            if has_impl.is_none() {
                diagnostics.error(
                    DiagnosticCode::InterfaceConformance,
                    class_range,
                    format!(
                        "class '{}' must implement abstract method '{}'",
                        class_symbol.name, base_method.name
                    ),
                );
            }
        }
    }
}
