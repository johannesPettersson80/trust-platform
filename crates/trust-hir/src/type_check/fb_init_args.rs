//! CODESYS's `FB_init` arguments in an instance's declaration (specification 10 §4.8):
//! `com1 : serialdevice(iCOMnum := 1);` gives `iCOMnum` to the instance's `FB_init`, beside
//! `bInitRetains` and `bInCopyCode`, which the runtime passes. They bind and check as the
//! arguments of a call of `FB_init` whose parameters are its inputs after those two.

use super::*;

/// The method the arguments go to.
const FB_INIT: &str = "FB_init";

/// Checks every declaration of `root` that gives arguments, under the CODESYS profiles; refuses
/// them under the others, which call no `FB_init`.
pub(crate) fn check_fb_init_arguments(
    symbols: &mut SymbolTable,
    root: &SyntaxNode,
    diagnostics: &mut DiagnosticBuilder,
) {
    for declaration in root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
    {
        let Some(arguments) = declaration
            .children()
            .find(|child| child.kind() == SyntaxKind::ArgList)
        else {
            continue;
        };
        if !symbols.dialect().is_codesys() {
            diagnostics.error(
                DiagnosticCode::InvalidOperation,
                arguments.text_range(),
                "arguments in a declaration go to the instance's FB_init, which only the CODESYS \
                 profiles call (specification 10 §4.8)",
            );
            continue;
        }
        let Some(name) = declaration
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)
            .and_then(|name| {
                name.descendants_with_tokens()
                    .filter_map(|element| element.into_token())
                    .find(|token| !token.kind().is_trivia())
            })
        else {
            continue;
        };
        let Some(symbol) = symbols
            .lookup_by_name_range(name.text(), name.text_range())
            .and_then(|id| symbols.get(id))
        else {
            continue;
        };
        let (type_id, owner) = (symbol.type_id, symbol.parent);
        let fb_init = symbols
            .resolve_member_symbol_in_type(type_id, FB_INIT)
            .and_then(|id| symbols.get(id))
            .filter(|method| matches!(method.kind, SymbolKind::Method { .. }))
            .map(|method| (method.id, method.kind.clone()));
        let Some((fb_init, kind)) = fb_init else {
            diagnostics.error(
                DiagnosticCode::InvalidOperation,
                arguments.text_range(),
                "only the instance of a function block or a class that declares FB_init takes \
                 arguments in its declaration",
            );
            continue;
        };
        let scope = owner
            .and_then(|owner| symbols.scope_for_owner(owner))
            .unwrap_or(ScopeId::GLOBAL);
        let mut checker = TypeChecker::new(symbols, diagnostics, scope);
        let mut calls = checker.calls();
        // The runtime passes the first two inputs, bInitRetains and bInCopyCode.
        let inputs: Vec<_> = calls
            .callable_parameters(fb_init, &kind)
            .into_iter()
            .skip(2)
            .collect();
        let bound = calls.bind_call_arguments(&inputs, &declaration, true);
        calls.check_bound_call_argument_types(&inputs, &bound);
    }
}
