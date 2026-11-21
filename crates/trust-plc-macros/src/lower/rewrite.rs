//! The plain code of the lowered sequence, rewritten for its dispatch (specification 48 RU-71).

use proc_macro2::Literal;
use syn::spanned::Spanned;
use syn::visit_mut::{self, VisitMut};
use syn::{Error, Expr};

use super::{ending, Flattened, Lowering};

/// Rewrites plain code for the dispatch: a `break` or `continue` of a flattened loop becomes a
/// jump, a `return` ends the sequence as the body's result does, and `s.step(..)` names the step.
/// Closures and nested items are left as they are, as their `break`, `continue` and `return` are
/// their own.
pub(super) struct Rewrite<'a> {
    lowering: &'a Lowering,
    /// The labels of the loops of the plain code around the place being rewritten.
    plain_loops: Vec<Option<String>>,
    error: Option<Error>,
}

impl<'a> Rewrite<'a> {
    pub(super) fn new(lowering: &'a Lowering) -> Self {
        Self {
            lowering,
            plain_loops: Vec::new(),
            error: None,
        }
    }

    pub(super) fn finish(self) -> Result<(), Error> {
        self.error.map_or(Ok(()), Err)
    }

    /// The flattened loop that a `break` or `continue` with `label` leaves; `None` for one of the
    /// plain code's own loops.
    fn flattened(&self, label: Option<&syn::Lifetime>) -> Option<&'a Flattened> {
        match label {
            Some(label) => {
                let name = label.ident.to_string();
                if self
                    .plain_loops
                    .iter()
                    .any(|plain| plain.as_deref() == Some(name.as_str()))
                {
                    return None;
                }
                self.lowering
                    .loops
                    .iter()
                    .rev()
                    .find(|looped| looped.label.as_deref() == Some(name.as_str()))
            }
            None if self.plain_loops.is_empty() => self.lowering.loops.last(),
            None => None,
        }
    }
}

impl VisitMut for Rewrite<'_> {
    fn visit_expr_mut(&mut self, expression: &mut Expr) {
        let context = &self.lowering.context;
        match expression {
            Expr::Closure(_) | Expr::Async(_) => return,
            Expr::Loop(looped) => {
                let label = looped
                    .label
                    .as_ref()
                    .map(|label| label.name.ident.to_string());
                self.plain_loops.push(label);
                visit_mut::visit_expr_mut(self, expression);
                self.plain_loops.pop();
                return;
            }
            Expr::While(looped) => {
                let label = looped
                    .label
                    .as_ref()
                    .map(|label| label.name.ident.to_string());
                self.plain_loops.push(label);
                visit_mut::visit_expr_mut(self, expression);
                self.plain_loops.pop();
                return;
            }
            Expr::ForLoop(looped) => {
                let label = looped
                    .label
                    .as_ref()
                    .map(|label| label.name.ident.to_string());
                self.plain_loops.push(label);
                visit_mut::visit_expr_mut(self, expression);
                self.plain_loops.pop();
                return;
            }
            Expr::Break(left) => {
                if let Some(looped) = self.flattened(left.label.as_ref()) {
                    if left.expr.is_some() {
                        self.error.get_or_insert_with(|| {
                            Error::new(
                                left.span(),
                                "a loop that holds a wait point is a statement: its `break` takes \
                                 no value",
                            )
                        });
                        return;
                    }
                    let end = Literal::usize_unsuffixed(looped.end);
                    *expression = syn::parse_quote!({
                        __trust_at = #end;
                        continue '__trust_dispatch;
                    });
                    return;
                }
            }
            Expr::Continue(next) => {
                if let Some(looped) = self.flattened(next.label.as_ref()) {
                    let top = Literal::usize_unsuffixed(looped.top);
                    *expression = syn::parse_quote!({
                        __trust_at = #top;
                        continue '__trust_dispatch;
                    });
                    return;
                }
            }
            Expr::Return(returned) => {
                let mut result: Expr = returned
                    .expr
                    .as_deref()
                    .cloned()
                    .unwrap_or_else(|| syn::parse_quote!(::core::result::Result::Ok(())));
                self.visit_expr_mut(&mut result);
                *expression = ending(&result);
                return;
            }
            Expr::MethodCall(call)
                if call.method == "step"
                    && matches!(&*call.receiver, Expr::Path(path) if path.path.is_ident(context)) =>
            {
                for argument in &mut call.args {
                    self.visit_expr_mut(argument);
                }
                let arguments = &call.args;
                *expression = syn::parse_quote!(::trust_plc::seq::step(#arguments));
                return;
            }
            _ => {}
        }
        visit_mut::visit_expr_mut(self, expression);
    }

    fn visit_item_mut(&mut self, _: &mut syn::Item) {}
}
