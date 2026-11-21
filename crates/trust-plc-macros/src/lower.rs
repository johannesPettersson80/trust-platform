//! The state machine a `#[trust_sequence]` lowers its `async fn` to (specification 48 §15, RU-71,
//! RU-72): no future and no executor. The body becomes labelled chunks of plain code that a
//! `loop` over a `match` dispatches, a statement-level `loop`, `while` or `if` that holds a wait
//! point becomes jumps between them, and a wait point stores where the sequence waits and returns.
//! The next activation resumes at the wait point: it checks `.within(t)` first, then whether the
//! wait is over, and either stays or runs on to the next wait point. An `s.all` keeps the members
//! that have completed in the state's `done`, one bit each, and an `s.race` gives the number of the
//! first member in order that completed to the local of `let won: u16 = s.race(..).await;`.
//!
//! The body's locals are the sequence's state: the function reads each from `_seq` when it begins,
//! keeps it as an ordinary variable, and writes it back when it reaches a wait point. A local's
//! `let` becomes an assignment where it stands, as the reader of `trust-plc-decl` finds the locals
//! (the statements of the body's blocks, of `if` and of loops), so that both agree on them.

use proc_macro2::{Literal, TokenStream};
use quote::quote;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::visit_mut::VisitMut;
use syn::{Block, Error, Expr, ExprAwait, Ident, ImplItemFn, Pat, Stmt, Type};

mod rewrite;
mod waits;

use rewrite::Rewrite;
use waits::{Check, WaitPoint};

/// A local of the sequence, a field of its state: its name and its Rust type as its `let` writes
/// them.
pub(crate) struct Hoisted {
    pub(crate) name: Ident,
    pub(crate) ty: Type,
}

/// The locals of `body` in the order of `trust-plc-decl`'s reader: the `let`s of the body's blocks
/// and of the blocks of its `if`s and loops, each with an identifier and a type (the reader refuses
/// any other).
pub(crate) fn locals(body: &Block) -> Vec<Hoisted> {
    let mut found = Vec::new();
    collect_locals(body, &mut found);
    found
}

fn collect_locals(block: &Block, found: &mut Vec<Hoisted>) {
    for statement in &block.stmts {
        match statement {
            Stmt::Local(local) => {
                if let Some((name, ty)) = typed_name(&local.pat) {
                    found.push(Hoisted {
                        name: name.clone(),
                        ty: ty.clone(),
                    });
                }
            }
            Stmt::Expr(expression, _) => collect_in_expression(expression, found),
            Stmt::Item(_) | Stmt::Macro(_) => {}
        }
    }
}

fn collect_in_expression(expression: &Expr, found: &mut Vec<Hoisted>) {
    match expression {
        Expr::Loop(looped) => collect_locals(&looped.body, found),
        Expr::While(looped) => collect_locals(&looped.body, found),
        Expr::If(branch) => {
            collect_locals(&branch.then_branch, found);
            if let Some((_, otherwise)) = &branch.else_branch {
                collect_in_expression(otherwise, found);
            }
        }
        Expr::Block(inner) => collect_locals(&inner.block, found),
        _ => {}
    }
}

/// The identifier and the type of a `let name: Type` pattern.
fn typed_name(pattern: &Pat) -> Option<(&Ident, &Type)> {
    let Pat::Type(typed) = pattern else {
        return None;
    };
    let Pat::Ident(ident) = &*typed.pat else {
        return None;
    };
    Some((&ident.ident, &typed.ty))
}

/// One step of the flattened body.
enum Op {
    /// Plain code.
    Code(Stmt),
    /// The wait point of this number begins: store the state and return.
    Begin(u16),
    /// The check of the wait point of this number, where its resumption begins.
    Resume(u16),
    /// Go on at the label.
    Goto(usize),
    /// Go on at the label unless the condition holds.
    GotoUnless(Expr, usize),
    /// A place the dispatch can go to.
    Label(usize),
}

/// A loop that holds wait points, flattened: where `continue` and `break` go.
struct Flattened {
    label: Option<String>,
    top: usize,
    end: usize,
}

/// The lowering of one sequence.
struct Lowering {
    /// The name of the wait points' context, `s`.
    context: Ident,
    ops: Vec<Op>,
    labels: usize,
    waits: Vec<WaitPoint>,
    resumes: Vec<usize>,
    loops: Vec<Flattened>,
}

/// Lowers the sequence `item`, an `async fn name(&mut self, s: &mut Seq) -> Result<(), Fault>` that
/// `trust-plc-decl` has read without a refusal, to a plain method of the same name that runs one
/// activation. `locals` are its locals, the fields of its state `self._seq`.
///
/// # Errors
///
/// A wait point the lowering does not take yet (a child sequence), an `if let` or `while let` that
/// holds a wait point, whose binding would have to live across the wait, and a `break` with a value
/// out of a loop that holds a wait point.
pub(crate) fn lower(
    item: &ImplItemFn,
    context: &Ident,
    locals: &[Hoisted],
) -> Result<TokenStream, Error> {
    let mut lowering = Lowering {
        context: context.clone(),
        ops: Vec::new(),
        labels: 1,
        waits: Vec::new(),
        resumes: Vec::new(),
        loops: Vec::new(),
    };
    lowering.body(&item.block)?;
    let dispatch = lowering.dispatch(locals);
    let mut signature = item.sig.clone();
    signature.asyncness = None;
    let attributes = &item.attrs;
    let visibility = &item.vis;
    let names: Vec<&Ident> = locals.iter().map(|local| &local.name).collect();
    let types: Vec<&Type> = locals.iter().map(|local| &local.ty).collect();
    let numbers: Vec<Literal> = (1..=lowering.waits.len())
        .map(Literal::usize_unsuffixed)
        .collect();
    let resumes: Vec<Literal> = lowering
        .resumes
        .iter()
        .map(|label| Literal::usize_unsuffixed(*label))
        .collect();
    Ok(quote! {
        #(#attributes)*
        #[allow(unused_mut, unused_assignments, unreachable_code, clippy::never_loop)]
        #visibility #signature {
            #(let mut #names: #types = self._seq.#names;)*
            let mut __trust_at: usize = match self._seq.point {
                #(#numbers => #resumes,)*
                _ => 0,
            };
            '__trust_dispatch: loop {
                match __trust_at {
                    #dispatch
                }
            }
        }
    })
}

impl Lowering {
    fn label(&mut self) -> usize {
        let label = self.labels;
        self.labels += 1;
        label
    }

    /// The function's body: its statements, its last expression the sequence's result.
    fn body(&mut self, body: &Block) -> Result<(), Error> {
        let count = body.stmts.len();
        for (index, statement) in body.stmts.iter().enumerate() {
            match statement {
                Stmt::Expr(expression, None) if index + 1 == count && !awaits(expression) => {
                    let mut ended = expression.clone();
                    self.rewrite(&mut ended)?;
                    let ended = ending(&ended);
                    self.ops.push(Op::Code(syn::parse_quote!(#ended;)));
                }
                other => self.statement(other)?,
            }
        }
        Ok(())
    }

    fn block(&mut self, block: &Block) -> Result<(), Error> {
        block
            .stmts
            .iter()
            .try_for_each(|statement| self.statement(statement))
    }

    fn statement(&mut self, statement: &Stmt) -> Result<(), Error> {
        match statement {
            Stmt::Expr(expression, _) => {
                if self.flattens(expression)? {
                    return Ok(());
                }
                let mut plain = expression.clone();
                self.plain(&mut plain)?;
                self.ops.push(Op::Code(statement_of(plain)));
                Ok(())
            }
            Stmt::Local(local) => {
                if let Some(awaited) = local.init.as_ref().and_then(|init| awaited_in(&init.expr)) {
                    return self.race_into(local, awaited);
                }
                if let Some(assignment) = self.assignment(local)? {
                    self.ops.push(Op::Code(assignment));
                }
                Ok(())
            }
            Stmt::Item(_) | Stmt::Macro(_) => {
                let mut plain = statement.clone();
                let mut rewrite = Rewrite::new(self);
                rewrite.visit_stmt_mut(&mut plain);
                rewrite.finish()?;
                self.ops.push(Op::Code(plain));
                Ok(())
            }
        }
    }

    /// `let won: u16 = s.race(..).await;`: the wait point, then the number of the member that
    /// completed into the local, where the resumption goes on.
    fn race_into(&mut self, local: &syn::Local, awaited: &ExprAwait) -> Result<(), Error> {
        let Some((name, _)) = typed_name(&local.pat) else {
            return Err(Error::new(local.pat.span(), "a local names its type"));
        };
        self.wait_point(awaited)?;
        if !matches!(
            self.waits.last(),
            Some(WaitPoint {
                check: Check::Race(_),
                ..
            })
        ) {
            // The reader refuses a value from any other wait point before the lowering.
            return Err(Error::new(awaited.span(), "only `s.race` gives a value"));
        }
        self.ops
            .push(Op::Code(syn::parse_quote!(#name = __trust_won;)));
        Ok(())
    }

    /// Flattens `expression` when it is a wait point or a construct that holds one; `false` for
    /// plain code.
    fn flattens(&mut self, expression: &Expr) -> Result<bool, Error> {
        match expression {
            Expr::Await(awaited) => self.wait_point(awaited)?,
            Expr::Try(tried) if matches!(&*tried.expr, Expr::Await(_)) => {
                if let Expr::Await(awaited) = &*tried.expr {
                    self.wait_point(awaited)?;
                }
            }
            Expr::Loop(looped) => {
                let top = self.label();
                let end = self.label();
                self.ops.push(Op::Label(top));
                self.loops.push(Flattened {
                    label: looped
                        .label
                        .as_ref()
                        .map(|label| label.name.ident.to_string()),
                    top,
                    end,
                });
                self.block(&looped.body)?;
                self.loops.pop();
                self.ops.push(Op::Goto(top));
                self.ops.push(Op::Label(end));
            }
            Expr::While(looped) => {
                if matches!(&*looped.cond, Expr::Let(_)) {
                    return Err(binding_across_a_wait(
                        looped.while_token.span(),
                        "a `while let`",
                    ));
                }
                let top = self.label();
                let end = self.label();
                self.ops.push(Op::Label(top));
                let mut condition = (*looped.cond).clone();
                self.rewrite(&mut condition)?;
                self.ops.push(Op::GotoUnless(condition, end));
                self.loops.push(Flattened {
                    label: looped
                        .label
                        .as_ref()
                        .map(|label| label.name.ident.to_string()),
                    top,
                    end,
                });
                self.block(&looped.body)?;
                self.loops.pop();
                self.ops.push(Op::Goto(top));
                self.ops.push(Op::Label(end));
            }
            Expr::If(branch) if awaits(expression) => {
                if matches!(&*branch.cond, Expr::Let(_)) {
                    return Err(binding_across_a_wait(branch.if_token.span(), "an `if let`"));
                }
                let otherwise = self.label();
                let end = self.label();
                let mut condition = (*branch.cond).clone();
                self.rewrite(&mut condition)?;
                self.ops.push(Op::GotoUnless(condition, otherwise));
                self.block(&branch.then_branch)?;
                self.ops.push(Op::Goto(end));
                self.ops.push(Op::Label(otherwise));
                if let Some((_, other)) = &branch.else_branch
                    && !self.flattens(other)?
                {
                    let mut plain = (**other).clone();
                    self.plain(&mut plain)?;
                    self.ops.push(Op::Code(statement_of(plain)));
                }
                self.ops.push(Op::Label(end));
            }
            Expr::Block(inner) if awaits(expression) => self.block(&inner.block)?,
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn wait_point(&mut self, awaited: &ExprAwait) -> Result<(), Error> {
        let wait = self.wait_of(&awaited.base)?;
        self.waits.push(wait);
        let number = u16::try_from(self.waits.len())
            .map_err(|_| Error::new(awaited.span(), "a sequence has at most 65535 wait points"))?;
        let resume = self.label();
        self.resumes.push(resume);
        self.ops.push(Op::Begin(number));
        self.ops.push(Op::Label(resume));
        self.ops.push(Op::Resume(number));
        Ok(())
    }

    /// A local's `let` as the assignment where it stands; `None` for a `let` without a value.
    fn assignment(&self, local: &syn::Local) -> Result<Option<Stmt>, Error> {
        let Some((name, _)) = typed_name(&local.pat) else {
            // The reader refuses a local without an identifier and a type before the lowering.
            return Err(Error::new(local.pat.span(), "a local names its type"));
        };
        let Some(init) = &local.init else {
            return Ok(None);
        };
        let mut value = (*init.expr).clone();
        self.rewrite(&mut value)?;
        Ok(Some(syn::parse_quote!(#name = #value;)))
    }

    /// Plain code: the `let`s of its blocks and `if`s become assignments, as the reader takes them
    /// for locals, and the rest is rewritten (see [`Rewrite`]).
    fn plain(&self, expression: &mut Expr) -> Result<(), Error> {
        match expression {
            Expr::If(branch) => {
                self.rewrite(&mut branch.cond)?;
                self.plain_block(&mut branch.then_branch)?;
                if let Some((_, other)) = &mut branch.else_branch {
                    self.plain(other)?;
                }
                Ok(())
            }
            Expr::Block(inner) => self.plain_block(&mut inner.block),
            other => self.rewrite(other),
        }
    }

    fn plain_block(&self, block: &mut Block) -> Result<(), Error> {
        let mut statements = Vec::with_capacity(block.stmts.len());
        for statement in block.stmts.drain(..) {
            match statement {
                Stmt::Local(local) => {
                    if let Some(assignment) = self.assignment(&local)? {
                        statements.push(assignment);
                    }
                }
                Stmt::Expr(mut expression, semicolon) => {
                    self.plain(&mut expression)?;
                    statements.push(Stmt::Expr(expression, semicolon));
                }
                mut other => {
                    let mut rewrite = Rewrite::new(self);
                    rewrite.visit_stmt_mut(&mut other);
                    rewrite.finish()?;
                    statements.push(other);
                }
            }
        }
        block.stmts = statements;
        Ok(())
    }

    fn rewrite(&self, expression: &mut Expr) -> Result<(), Error> {
        let mut rewrite = Rewrite::new(self);
        rewrite.visit_expr_mut(expression);
        rewrite.finish()
    }

    /// The `match` arms of the dispatch: one per label, each running its chunk of the body to a
    /// jump, a wait point or the body's end.
    fn dispatch(&self, locals: &[Hoisted]) -> TokenStream {
        let names: Vec<&Ident> = locals.iter().map(|local| &local.name).collect();
        let context = &self.context;
        let mut arms = TokenStream::new();
        let mut label = 0;
        let mut chunk = TokenStream::new();
        let mut ended = false;
        let goto = |label: usize| {
            let label = Literal::usize_unsuffixed(label);
            quote! { __trust_at = #label; continue '__trust_dispatch; }
        };
        for op in &self.ops {
            if let Op::Label(next) = op {
                if !ended {
                    chunk.extend(goto(*next));
                }
                let at = Literal::usize_unsuffixed(label);
                arms.extend(quote! { #at => { #chunk } });
                label = *next;
                chunk = TokenStream::new();
                ended = false;
                continue;
            }
            if ended {
                // Code after a jump or a wait point, before the next label, never runs.
                continue;
            }
            match op {
                Op::Code(statement) => chunk.extend(quote! { #statement }),
                Op::Begin(number) => {
                    if self.is_all(*number) {
                        // An `s.all` begins with none of its members complete (RU-72).
                        chunk.extend(quote! { self._seq.done = ::trust_plc::Word(0); });
                    }
                    let number = Literal::u16_unsuffixed(*number);
                    chunk.extend(quote! {
                        let mut __trust_wait = ::trust_plc::seq::Waiting {
                            point: self._seq.point,
                            began: self._seq.began,
                        };
                        __trust_wait.begin(#context, #number);
                        self._seq.point = __trust_wait.point;
                        self._seq.began = __trust_wait.began;
                        #(self._seq.#names = #names;)*
                        return ::core::result::Result::Ok(());
                    });
                    ended = true;
                }
                Op::Resume(number) => chunk.extend(self.resume(*number)),
                Op::Goto(target) => {
                    chunk.extend(goto(*target));
                    ended = true;
                }
                Op::GotoUnless(condition, target) => {
                    let jump = goto(*target);
                    chunk.extend(quote! { if !(#condition) { #jump } });
                }
                Op::Label(_) => {}
            }
        }
        if !ended {
            chunk.extend(quote! {
                self._seq.point = 0;
                return ::core::result::Result::Ok(());
            });
        }
        let at = Literal::usize_unsuffixed(label);
        arms.extend(quote! { #at => { #chunk } });
        arms.extend(quote! {
            _ => {
                self._seq.point = 0;
                return ::core::result::Result::Ok(());
            }
        });
        arms
    }
}

/// Plain code as a statement of a chunk, which other statements follow: with its semicolon, as a
/// statement of a loop's body or a branch has the type `()`.
fn statement_of(expression: Expr) -> Stmt {
    Stmt::Expr(expression, Some(syn::token::Semi::default()))
}

/// The body's last expression, its result: a sequence whose body returns starts again at its top
/// at the next activation (RU-71).
fn ending(result: &Expr) -> Expr {
    syn::parse_quote! {{
        let __trust_end: ::core::result::Result<(), ::trust_plc::Fault> = #result;
        if __trust_end.is_ok() {
            self._seq.point = 0;
        }
        return __trust_end;
    }}
}

fn binding_across_a_wait(span: proc_macro2::Span, construct: &str) -> Error {
    Error::new(
        span,
        format!(
            "{construct} cannot hold a wait point: its binding would have to live across the \
             wait; test the value into a typed local and use `if` or `while` (specification 48 \
             RU-72)"
        ),
    )
}

/// The wait point of `expression`, `w.await` or `w.await?`, when it is one.
fn awaited_in(expression: &Expr) -> Option<&ExprAwait> {
    match expression {
        Expr::Await(awaited) => Some(awaited),
        Expr::Try(tried) => match &*tried.expr {
            Expr::Await(awaited) => Some(awaited),
            _ => None,
        },
        _ => None,
    }
}

/// Whether `expression` awaits anywhere outside its closures.
fn awaits(expression: &Expr) -> bool {
    struct Finds(bool);
    impl<'ast> Visit<'ast> for Finds {
        fn visit_expr_await(&mut self, _: &'ast ExprAwait) {
            self.0 = true;
        }
        fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}
    }
    let mut finds = Finds(false);
    finds.visit_expr(expression);
    finds.0
}
