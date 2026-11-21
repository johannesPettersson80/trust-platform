//! The loop rule of a sequence (specification 48 RU-72): a loop that can go round without reaching a
//! wait point would keep its activation from ending, and a child sequence that can return without
//! waiting is no wait point. For each statement, the ways control can leave it before it has reached
//! a wait point: on to the next statement, out of the sequence by `return`, or out of a loop by
//! `break` or `continue`.

use proc_macro2::Span;
use syn::ext::IdentExt as _;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Block, Error, Expr, Label, Stmt};

use super::{awaited, child_called};

/// Whether the child sequence of this name waits before every way it returns, so that awaiting it
/// is a wait point.
pub(super) type Waits<'a> = &'a dyn Fn(&str) -> bool;

/// The ways out of a statement that pass no wait point.
#[derive(Default)]
struct Leaks {
    /// On to the next statement.
    through: bool,
    /// Out of the sequence by `return`.
    returns: bool,
    /// Out of the loops of these labels by `break`, `None` for the innermost loop around it.
    breaks: Vec<Option<String>>,
    /// Round the loops of these labels by `continue`.
    continues: Vec<Option<String>>,
}

impl Leaks {
    fn through() -> Self {
        Self {
            through: true,
            ..Self::default()
        }
    }

    fn join(&mut self, other: Self) {
        self.through |= other.through;
        self.returns |= other.returns;
        self.breaks.extend(other.breaks);
        self.continues.extend(other.continues);
    }
}

/// Whether the loop whose body is `body` and whose label is `label` can go round without reaching a
/// wait point: through its body's end or by a `continue` of it.
pub(super) fn spins(body: &Block, label: Option<&Label>, waits: Waits<'_>) -> bool {
    let leaks = block(body, waits);
    let name = label.map(|label| label.name.ident.to_string());
    leaks.through
        || leaks
            .continues
            .iter()
            .any(|target| target.is_none() || *target == name)
}

/// The refusal of a loop that can go round without reaching a wait point.
pub(super) fn spinning(place: Span) -> Error {
    Error::new(
        place,
        "this loop can run without reaching a wait point: every way through it awaits one, or a \
         scan would never end (specification 48 RU-72)",
    )
}

/// Whether every way through `body`, a sequence's, reaches a wait point before it returns.
pub(super) fn waits_before_returning(body: &Block, waits: Waits<'_>) -> bool {
    let leaks = block(body, waits);
    !leaks.through && !leaks.returns
}

/// The first loop in `body` that can go round without reaching a wait point, where it stands.
pub(super) fn spinning_loop(body: &Block, waits: Waits<'_>) -> Option<Span> {
    body.stmts.iter().find_map(|statement| match statement {
        Stmt::Expr(expression, _) => spinning_in(expression, waits),
        _ => None,
    })
}

fn spinning_in(expression: &Expr, waits: Waits<'_>) -> Option<Span> {
    match expression {
        Expr::Loop(looped) => {
            if spins(&looped.body, looped.label.as_ref(), waits) {
                return Some(looped.loop_token.span());
            }
            spinning_loop(&looped.body, waits)
        }
        Expr::While(looped) => {
            if spins(&looped.body, looped.label.as_ref(), waits) {
                return Some(looped.while_token.span());
            }
            spinning_loop(&looped.body, waits)
        }
        Expr::If(branch) => spinning_loop(&branch.then_branch, waits).or_else(|| {
            branch
                .else_branch
                .as_ref()
                .and_then(|(_, otherwise)| spinning_in(otherwise, waits))
        }),
        Expr::Block(inner) => spinning_loop(&inner.block, waits),
        _ => None,
    }
}

/// The ways out of `block` that pass no wait point: those of each statement in turn, until one after
/// which every way on has passed one.
fn block(block: &Block, waits: Waits<'_>) -> Leaks {
    let mut leaks = Leaks::default();
    for statement in &block.stmts {
        let mut found = statement_leaks(statement, waits);
        let through = std::mem::take(&mut found.through);
        leaks.join(found);
        if !through {
            return leaks;
        }
    }
    leaks.through = true;
    leaks
}

fn statement_leaks(statement: &Stmt, waits: Waits<'_>) -> Leaks {
    match statement {
        Stmt::Local(local) => match &local.init {
            Some(init) => {
                // `let won: u16 = s.race(..).await;`, the only wait point that gives a value.
                if let Some(waited) = awaited(&init.expr) {
                    wait_point(waited, waits)
                } else {
                    let mut leaks = plain(&init.expr);
                    if let Some((_, otherwise)) = &init.diverge {
                        leaks.join(plain(otherwise));
                    }
                    leaks
                }
            }
            None => Leaks::through(),
        },
        Stmt::Expr(expression, _) => expression_leaks(expression, waits),
        Stmt::Item(_) => Leaks::through(),
        // A macro's tokens are not read as code: it may return.
        Stmt::Macro(_) => Leaks {
            through: true,
            returns: true,
            ..Leaks::default()
        },
    }
}

fn expression_leaks(expression: &Expr, waits: Waits<'_>) -> Leaks {
    if let Some(waited) = awaited(expression) {
        return wait_point(waited, waits);
    }
    match expression {
        Expr::Block(inner) if inner.label.is_none() => block(&inner.block, waits),
        Expr::If(branch) => {
            let mut leaks = escapes(&branch.cond);
            leaks.join(block(&branch.then_branch, waits));
            leaks.join(match &branch.else_branch {
                Some((_, otherwise)) => expression_leaks(otherwise, waits),
                None => Leaks::through(),
            });
            leaks
        }
        Expr::Loop(looped) => around(block(&looped.body, waits), looped.label.as_ref(), false),
        Expr::While(looped) => {
            let mut leaks = around(block(&looped.body, waits), looped.label.as_ref(), true);
            leaks.join(escapes(&looped.cond));
            leaks
        }
        other => plain(other),
    }
}

/// The ways out of the wait point `waited` that pass no wait: on, when it can complete at the
/// activation that reaches it. A child sequence can when it can return without waiting, an
/// `s.race` when one of its child members can, and an `s.all` when all its members are children that
/// can (RU-72); a plain wait never does.
fn wait_point(waited: &syn::ExprAwait, waits: Waits<'_>) -> Leaks {
    let at_once = |child: &syn::Ident| !waits(&child.unraw().to_string());
    if let Some((child, _)) = child_called(&waited.base) {
        return if at_once(child) {
            Leaks::through()
        } else {
            Leaks::default()
        };
    }
    let mut base = &*waited.base;
    while let Expr::MethodCall(within) = base
        && within.method == "within"
    {
        base = &within.receiver;
    }
    let Expr::MethodCall(call) = base else {
        return Leaks::default();
    };
    let member_at_once = |member: &Expr| match child_called(member) {
        Some((child, _)) => at_once(child),
        None => false,
    };
    let through = if call.method == "race" {
        call.args.iter().any(member_at_once)
    } else if call.method == "all" {
        call.args.iter().all(member_at_once)
    } else {
        false
    };
    if through {
        Leaks::through()
    } else {
        Leaks::default()
    }
}

/// The ways out of a loop labelled `label` whose body leaks `body`: a `break` of it goes on after
/// it, and so does a `while` whose condition fails (`ends`); going round is the loop's own rule
/// ([`spins`]).
fn around(body: Leaks, label: Option<&Label>, ends: bool) -> Leaks {
    let name = label.map(|label| label.name.ident.to_string());
    let mine = |target: &Option<String>| target.is_none() || *target == name;
    Leaks {
        through: ends || body.breaks.iter().any(mine),
        returns: body.returns,
        breaks: body
            .breaks
            .into_iter()
            .filter(|target| !mine(target))
            .collect(),
        continues: body
            .continues
            .into_iter()
            .filter(|target| !mine(target))
            .collect(),
    }
}

/// The ways out of plain code, which holds no wait point: on, unless it is itself a `return`,
/// `break` or `continue`, and by the `return`, `break` and `continue` that leave it.
fn plain(expression: &Expr) -> Leaks {
    let mut leaks = escapes(expression);
    leaks.through = !matches!(
        expression,
        Expr::Return(_) | Expr::Break(_) | Expr::Continue(_)
    );
    leaks
}

/// The `return`, `break` and `continue` of `expression` that leave it.
fn escapes(expression: &Expr) -> Leaks {
    let mut found = Escapes::default();
    found.visit_expr(expression);
    Leaks {
        through: false,
        returns: found.returns,
        breaks: found.breaks,
        continues: found.continues,
    }
}

/// Finds the `return`, `break` and `continue` that leave the code it visits.
#[derive(Default)]
struct Escapes {
    /// The labels of the loops inside the code around the place visited.
    loops: Vec<Option<String>>,
    /// The labels of the loops and blocks inside the code around the place visited.
    labels: Vec<String>,
    returns: bool,
    breaks: Vec<Option<String>>,
    continues: Vec<Option<String>>,
}

impl Escapes {
    /// Whether a `break` or `continue` of `label` leaves the code visited.
    fn leaves(&self, label: Option<&syn::Lifetime>) -> bool {
        match label {
            None => self.loops.is_empty(),
            Some(label) => !self.labels.contains(&label.ident.to_string()),
        }
    }

    fn inside(&mut self, label: Option<&Label>, looped: bool, visit: impl FnOnce(&mut Self)) {
        let name = label.map(|label| label.name.ident.to_string());
        if looped {
            self.loops.push(name.clone());
        }
        if let Some(name) = &name {
            self.labels.push(name.clone());
        }
        visit(self);
        if name.is_some() {
            self.labels.pop();
        }
        if looped {
            self.loops.pop();
        }
    }
}

impl<'ast> Visit<'ast> for Escapes {
    fn visit_expr_return(&mut self, returned: &'ast syn::ExprReturn) {
        self.returns = true;
        visit::visit_expr_return(self, returned);
    }

    fn visit_expr_break(&mut self, left: &'ast syn::ExprBreak) {
        if self.leaves(left.label.as_ref()) {
            self.breaks
                .push(left.label.as_ref().map(|label| label.ident.to_string()));
        }
        visit::visit_expr_break(self, left);
    }

    fn visit_expr_continue(&mut self, next: &'ast syn::ExprContinue) {
        if self.leaves(next.label.as_ref()) {
            self.continues
                .push(next.label.as_ref().map(|label| label.ident.to_string()));
        }
    }

    fn visit_expr_loop(&mut self, looped: &'ast syn::ExprLoop) {
        self.inside(looped.label.as_ref(), true, |me| {
            visit::visit_expr_loop(me, looped);
        });
    }

    fn visit_expr_while(&mut self, looped: &'ast syn::ExprWhile) {
        self.inside(looped.label.as_ref(), true, |me| {
            visit::visit_expr_while(me, looped);
        });
    }

    fn visit_expr_for_loop(&mut self, looped: &'ast syn::ExprForLoop) {
        self.inside(looped.label.as_ref(), true, |me| {
            visit::visit_expr_for_loop(me, looped);
        });
    }

    fn visit_expr_block(&mut self, inner: &'ast syn::ExprBlock) {
        self.inside(inner.label.as_ref(), false, |me| {
            visit::visit_expr_block(me, inner);
        });
    }

    // A closure's `return`, `break` and `continue` are its own, and so are a nested item's.
    fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}

    fn visit_item(&mut self, _: &'ast syn::Item) {}
}
