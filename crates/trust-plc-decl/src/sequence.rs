//! A `#[trust_sequence]`'s body (specification 48 §15, RU-71, RU-72), as the attribute and the
//! declarations' writer read it: its wait points in the body's order, each with what it waits for and
//! the step that the `s.step` before it names, and its locals, each a variable of the sequence's
//! state; and RU-72's refusals, each at its place in the body. The members of an `s.all` or an
//! `s.race` are `s.next_cycle()`, `s.until(..)` and `s.wait(..)`, two to sixteen of them, and only an
//! `s.race` gives a value, the number of its member that completed, to a `u16` local.

use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{
    Block, Error, Expr, ExprAwait, FnArg, ImplItemFn, Lit, Local as LetStatement, Pat, Stmt,
};

use crate::types::{DataType, Elementary};
use crate::{Field, Section, Structure, Variable};

/// The name of a sequence's state in its POU's instance (RU-71): the one leading underscore an IEC
/// identifier may have.
pub const STATE: &str = "_seq";

/// The field of the state of a sequence whose body holds an `s.all`: the members of the `s.all` it
/// waits at that have completed, bit 0 for the first (RU-72).
pub const DONE: &str = "done";

/// The most members an `s.all` or an `s.race` takes: one bit each of `done`, a `WORD` (RU-72).
pub const MOST_MEMBERS: usize = 16;

/// The Rust name of the type of the state of the sequence of the POU whose Rust item is `item`,
/// `<Item>Sequence`: `#[trust_sequence]` declares it beside the POU's `impl` block, and the POU's
/// variable `_seq` has it (RU-71).
#[must_use]
pub fn state_item(item: &str) -> String {
    format!("{item}Sequence")
}

/// The Structured Text name of that type, `<POU>_SEQ` in the namespace of the POU named `pou`
/// (RU-71).
#[must_use]
pub fn state_structure(pou: &str) -> String {
    format!("{pou}_SEQ")
}

/// The structure of the state of `sequence`, the sequence of the POU named `pou` whose Rust item is
/// `item` (RU-71): `point : UINT`, the wait point it waits at, `began : TIME`, when its wait began,
/// `done : WORD` when the body holds an `s.all` (RU-72), and its locals with their types, each
/// starting at its type's default, the sequence's top.
#[must_use]
pub fn state(pou: &str, item: &str, sequence: &Sequence) -> Structure {
    let mut fields = vec![
        Field {
            name: "point".to_string(),
            ty: DataType::Elementary(Elementary::Uint),
            init: None,
        },
        Field {
            name: "began".to_string(),
            ty: DataType::Elementary(Elementary::Time),
            init: None,
        },
    ];
    if sequence.holds_all() {
        fields.push(Field {
            name: DONE.to_string(),
            ty: DataType::Elementary(Elementary::Word),
            init: None,
        });
    }
    fields.extend(sequence.locals.iter().map(|local| Field {
        name: local.name.clone(),
        ty: local.ty.clone(),
        init: None,
    }));
    Structure {
        name: state_structure(pou),
        item: state_item(item),
        fields,
    }
}

/// The variable `_seq` of a POU whose body runs a sequence, of the type of its state, which the
/// readers of the POU's struct add as its last variable (RU-71).
pub(crate) fn state_variable(item: &str) -> Variable {
    Variable {
        name: STATE.to_string(),
        section: Section::Var,
        retain: false,
        ty: DataType::Named(state_item(item)),
        init: None,
    }
}

/// What a wait point waits for (RU-72).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wait {
    /// `s.next_cycle()`: the next activation.
    NextCycle,
    /// `s.until(pred)`: a later activation at whose start `pred` holds.
    Until,
    /// `s.wait(t)`: `t` of the task's logical time.
    Wait,
    /// `s.all(...)`: every one of its waits.
    All,
    /// `s.race(...)`: the first of its waits.
    Race,
    /// `self.<sequence>(s)`: a child sequence of the same instance, to its end.
    Child,
}

/// One wait point of a sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitPoint {
    /// Its number, its place among the body's wait points from 1 (RU-72).
    pub number: u16,
    /// What it waits for.
    pub wait: Wait,
    /// Whether `.within(t)` limits it.
    pub within: bool,
    /// The step the `s.step(.., "NAME")` before it names, which names the wait point for an online
    /// change (RU-73).
    pub step: Option<String>,
}

/// A local of a sequence: a variable of its state (RU-71).
#[derive(Clone, Debug, PartialEq)]
pub struct Local {
    /// Its name, as the body declares it.
    pub name: String,
    /// Its IEC type, from the type the `let` names.
    pub ty: DataType,
}

/// A sequence, as its body declares it.
#[derive(Clone, Debug, PartialEq)]
pub struct Sequence {
    /// The `async fn`'s name.
    pub name: String,
    /// Its wait points in the body's order.
    pub wait_points: Vec<WaitPoint>,
    /// Its locals in the body's order.
    pub locals: Vec<Local>,
}

impl Sequence {
    /// Whether its body holds an `s.all`, whose state then has `done` (RU-72).
    #[must_use]
    pub fn holds_all(&self) -> bool {
        self.wait_points.iter().any(|point| point.wait == Wait::All)
    }
}

/// Reads the sequence `item`, an `async fn name(&mut self, s: &mut Seq) -> Result<(), Fault>`.
///
/// # Errors
///
/// A refusal of RU-72 at its place: a function that is not `async`, has no context of the
/// sequence's wait points, awaits anything but one of them, awaits inside another expression, has
/// an `async` block or closure, a loop that can run without reaching a wait point, a `for` loop with
/// a wait point, an `s.all` or `s.race` with too few or too many members or a member that is not a
/// plain wait, a value from a wait point other than `s.race` or into a local that is not `u16`, a
/// local without its type, two locals of one name, a local named as a field of the state, or a local
/// of no IEC type.
pub fn sequence(item: &ImplItemFn) -> Result<Sequence, Error> {
    if item.sig.asyncness.is_none() {
        return Err(Error::new(
            item.sig.fn_token.span(),
            "a sequence is an `async fn name(&mut self, s: &mut Seq) -> Result<(), Fault>`",
        ));
    }
    let Some(FnArg::Typed(context)) = item.sig.inputs.iter().nth(1) else {
        return Err(Error::new(
            item.sig.inputs.span(),
            "a sequence takes its wait points' context after `&mut self`: `s: &mut Seq`",
        ));
    };
    let Pat::Ident(context) = &*context.pat else {
        return Err(Error::new(
            context.pat.span(),
            "the wait points' context is a name, `s: &mut Seq`",
        ));
    };
    let mut reader = Reader {
        context: context.ident.to_string(),
        wait_points: Vec::new(),
        locals: Vec::new(),
        local_places: Vec::new(),
        step: None,
    };
    reader.block(&item.block)?;
    let read = Sequence {
        name: item.sig.ident.to_string(),
        wait_points: reader.wait_points,
        locals: reader.locals,
    };
    if read.holds_all()
        && let Some(index) = read
            .locals
            .iter()
            .position(|local| local.name.eq_ignore_ascii_case(DONE))
    {
        let place = reader
            .local_places
            .get(index)
            .copied()
            .unwrap_or_else(|| item.sig.ident.span());
        return Err(a_field_of_the_state(place, DONE));
    }
    Ok(read)
}

/// What reading a body has found so far.
struct Reader {
    /// The name of the wait points' context, `s`.
    context: String,
    wait_points: Vec<WaitPoint>,
    locals: Vec<Local>,
    /// Where each local's name stands, for a refusal found after the body is read.
    local_places: Vec<Span>,
    /// The step the last `s.step` named, for the next wait point.
    step: Option<String>,
}

impl Reader {
    fn block(&mut self, block: &Block) -> Result<(), Error> {
        block
            .stmts
            .iter()
            .try_for_each(|statement| self.statement(statement))
    }

    fn statement(&mut self, statement: &Stmt) -> Result<(), Error> {
        match statement {
            Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    if let Some(awaited) = awaited(&init.expr) {
                        return self.race_into(local, awaited);
                    }
                    plain(&init.expr)?;
                    if let Some((_, otherwise)) = &init.diverge {
                        plain(otherwise)?;
                    }
                }
                self.local(local)
            }
            Stmt::Expr(expression, _) => self.expression(expression),
            Stmt::Item(item) => plain_item(item),
            Stmt::Macro(called) => no_await_in(called.mac.tokens.clone(), called.span()),
        }
    }

    /// An expression that stands as a statement: a wait point, a loop, a branch, a block, or a plain
    /// expression without a wait point.
    fn expression(&mut self, expression: &Expr) -> Result<(), Error> {
        match expression {
            Expr::Await(awaited) => self.wait_point(awaited),
            Expr::Try(tried) => match &*tried.expr {
                Expr::Await(awaited) => self.wait_point(awaited),
                other => plain(other),
            },
            Expr::Loop(looped) => {
                if !always_waits(&looped.body) {
                    return Err(Error::new(
                        looped.loop_token.span(),
                        "this loop can run without reaching a wait point: every way through it awaits \
                         one, or a scan would never end (specification 48 RU-72)",
                    ));
                }
                self.block(&looped.body)
            }
            Expr::While(looped) => {
                plain(&looped.cond)?;
                if !always_waits(&looped.body) {
                    return Err(Error::new(
                        looped.while_token.span(),
                        "this loop can run without reaching a wait point: every way through it awaits \
                         one, or a scan would never end (specification 48 RU-72)",
                    ));
                }
                self.block(&looped.body)
            }
            Expr::ForLoop(looped) => {
                if awaits(expression) {
                    return Err(Error::new(
                        looped.for_token.span(),
                        "a `for` loop cannot hold a wait point: its iterator would have to live \
                         across the wait; count with a typed local and `while` (specification 48 RU-72)",
                    ));
                }
                Ok(())
            }
            Expr::If(branch) => {
                plain(&branch.cond)?;
                self.block(&branch.then_branch)?;
                match &branch.else_branch {
                    Some((_, otherwise)) => self.expression(otherwise),
                    None => Ok(()),
                }
            }
            Expr::Block(inner) => self.block(&inner.block),
            other => {
                plain(other)?;
                self.step_named(other);
                Ok(())
            }
        }
    }

    fn wait_point(&mut self, awaited: &ExprAwait) -> Result<(), Error> {
        let (wait, within) = self.wait_of(&awaited.base)?;
        let number = u16::try_from(self.wait_points.len() + 1)
            .map_err(|_| Error::new(awaited.span(), "a sequence has at most 65535 wait points"))?;
        self.wait_points.push(WaitPoint {
            number,
            wait,
            within,
            step: self.step.take(),
        });
        Ok(())
    }

    /// `let won: u16 = s.race(..).await;`: the wait point, and the local that keeps the number of
    /// the member that completed. Any other wait point gives no value.
    fn race_into(&mut self, local: &LetStatement, awaited: &ExprAwait) -> Result<(), Error> {
        if self.wait_of(&awaited.base)?.0 != Wait::Race {
            return Err(Error::new(
                awaited.span(),
                "a wait point stands as a statement of its own, `s.wait(t).await;`: only `s.race` \
                 gives a value, the number of its member that completed, `let won: u16 = \
                 s.race(..).await;` (specification 48 RU-72)",
            ));
        }
        if let Some((_, otherwise)) = local.init.as_ref().and_then(|init| init.diverge.as_ref()) {
            return Err(Error::new(
                otherwise.span(),
                "the number of the member that completed is always there: `let won: u16 = \
                 s.race(..).await;` takes no `else` (specification 48 RU-72)",
            ));
        }
        self.wait_point(awaited)?;
        self.local(local)?;
        let number = self.locals.last().map(|won| &won.ty);
        if number != Some(&DataType::Elementary(Elementary::Uint)) {
            return Err(Error::new(
                local.pat.span(),
                "the number of the `s.race`'s member that completed is a `u16`: `let won: u16 = \
                 s.race(..).await;` (specification 48 RU-72)",
            ));
        }
        Ok(())
    }

    /// What `base`, the expression before `.await`, waits for, and whether `.within` limits it.
    fn wait_of(&self, base: &Expr) -> Result<(Wait, bool), Error> {
        let Expr::MethodCall(call) = base else {
            return Err(not_a_wait_point(base.span()));
        };
        if call.method == "within" {
            call.args.iter().try_for_each(plain)?;
            let (wait, _) = self.wait_of(&call.receiver)?;
            return Ok((wait, true));
        }
        call.args.iter().try_for_each(plain)?;
        if self.is_context(&call.receiver) {
            let wait = match call.method.to_string().as_str() {
                "next_cycle" => Wait::NextCycle,
                "until" => Wait::Until,
                "wait" => Wait::Wait,
                "all" => {
                    self.members(call)?;
                    Wait::All
                }
                "race" => {
                    self.members(call)?;
                    Wait::Race
                }
                other => {
                    return Err(Error::new(
                        call.method.span(),
                        format!(
                            "`{}.{other}` is no wait point: they are `next_cycle`, `until`, `wait`, \
                             `all` and `race` (specification 48 RU-72)",
                            self.context
                        ),
                    ));
                }
            };
            return Ok((wait, false));
        }
        let child = matches!(&*call.receiver, Expr::Path(path) if path.path.is_ident("self"))
            && call.args.len() == 1
            && call
                .args
                .first()
                .is_some_and(|argument| self.is_context(argument));
        if child {
            Ok((Wait::Child, false))
        } else {
            Err(not_a_wait_point(base.span()))
        }
    }

    /// The members of `s.all(..)` or `s.race(..)`: two to sixteen, each `s.next_cycle()`,
    /// `s.until(..)` or `s.wait(..)` without a `.within` of its own.
    fn members(&self, call: &syn::ExprMethodCall) -> Result<(), Error> {
        let (s, method) = (&self.context, &call.method);
        if !(2..=MOST_MEMBERS).contains(&call.args.len()) {
            return Err(Error::new(
                method.span(),
                format!(
                    "`{s}.{method}` takes two to sixteen waits, one bit each of the state's `done`: \
                     `{s}.{method}({s}.until(..), {s}.wait(..))` (specification 48 RU-72)"
                ),
            ));
        }
        for member in &call.args {
            let plain_wait = match member {
                Expr::MethodCall(inner) if inner.method == "within" => {
                    return Err(Error::new(
                        inner.method.span(),
                        format!(
                            "a member of `{s}.{method}` takes no `.within` of its own: `.within(t)` \
                             stands last and limits the whole wait point, \
                             `{s}.{method}(..).within(t).await?` (specification 48 RU-72)"
                        ),
                    ));
                }
                Expr::MethodCall(inner) => {
                    self.is_context(&inner.receiver)
                        && ["next_cycle", "until", "wait"]
                            .iter()
                            .any(|name| inner.method == name)
                }
                _ => false,
            };
            if !plain_wait {
                return Err(Error::new(
                    member.span(),
                    format!(
                        "a member of `{s}.{method}` is `{s}.next_cycle()`, `{s}.until(..)` or \
                         `{s}.wait(..)` (specification 48 RU-72)"
                    ),
                ));
            }
        }
        Ok(())
    }

    fn is_context(&self, expression: &Expr) -> bool {
        matches!(expression, Expr::Path(path) if path.path.is_ident(self.context.as_str()))
    }

    /// Notes the step that `s.step(&mut .., "NAME")` names, for the next wait point.
    fn step_named(&mut self, expression: &Expr) {
        let Expr::MethodCall(call) = expression else {
            return;
        };
        if call.method != "step" || !self.is_context(&call.receiver) {
            return;
        }
        if let Some(Expr::Lit(literal)) = call.args.iter().nth(1)
            && let Lit::Str(name) = &literal.lit
        {
            self.step = Some(name.value());
        }
    }

    fn local(&mut self, local: &LetStatement) -> Result<(), Error> {
        let Pat::Type(typed) = &local.pat else {
            return Err(Error::new(
                local.pat.span(),
                "a local of a sequence names its type, `let count: i16 = 0;`: it is a variable of the \
                 sequence's state, whose type the attribute cannot infer (specification 48 RU-71)",
            ));
        };
        let Pat::Ident(ident) = &*typed.pat else {
            return Err(Error::new(
                typed.pat.span(),
                "a local of a sequence is one name, not a pattern (specification 48 RU-71)",
            ));
        };
        // A local is a field of the state in Structured Text, named as a variable is.
        let name = crate::variable_name(&ident.ident)?;
        if let Some(field) = ["point", "began"]
            .into_iter()
            .find(|field| name.eq_ignore_ascii_case(field))
        {
            return Err(a_field_of_the_state(ident.ident.span(), field));
        }
        if self
            .locals
            .iter()
            .any(|other| other.name.eq_ignore_ascii_case(&name))
        {
            return Err(Error::new(
                ident.ident.span(),
                format!(
                    "a second local `{name}`: a sequence's locals are variables of its state, one of \
                     each name, as Structured Text names them without case (specification 48 RU-72)"
                ),
            ));
        }
        let ty = DataType::of(&typed.ty)?;
        self.locals.push(Local { name, ty });
        self.local_places.push(ident.ident.span());
        Ok(())
    }
}

/// The refusal of a local named as `field`, a field of the sequence's own state.
fn a_field_of_the_state(place: Span, field: &str) -> Error {
    Error::new(
        place,
        format!(
            "a local of a sequence cannot be named `{field}` in any case: the sequence's state, \
             whose variables the locals are, has a field `{field}` of its own (specification 48 \
             RU-71)"
        ),
    )
}

/// The wait point of `expression`, `w.await` or `w.await?`, when it is one.
fn awaited(expression: &Expr) -> Option<&ExprAwait> {
    match expression {
        Expr::Await(awaited) => Some(awaited),
        Expr::Try(tried) => match &*tried.expr {
            Expr::Await(awaited) => Some(awaited),
            _ => None,
        },
        _ => None,
    }
}

fn not_a_wait_point(span: Span) -> Error {
    Error::new(
        span,
        "a sequence awaits only its wait points: `s.next_cycle()`, `s.until(..)`, `s.wait(..)`, \
         `s.all(..)`, `s.race(..)` or a child sequence `self.<sequence>(s)`, each with `.within(..)` \
         when it may not last (specification 48 RU-72)",
    )
}

/// Refuses an `.await`, an `async` block or an `async` closure anywhere in `expression`: a wait point
/// stands as a statement of its own.
fn plain(expression: &Expr) -> Result<(), Error> {
    let mut found = Found::default();
    found.visit_expr(expression);
    found.refusal()
}

fn plain_item(item: &syn::Item) -> Result<(), Error> {
    let mut found = Found::default();
    found.visit_item(item);
    found.refusal()
}

/// Refuses `.await` in a macro's tokens, which are not parsed as expressions.
fn no_await_in(tokens: proc_macro2::TokenStream, span: Span) -> Result<(), Error> {
    let awaits = tokens.into_iter().any(|token| match token {
        proc_macro2::TokenTree::Ident(ident) => ident == "await",
        proc_macro2::TokenTree::Group(group) => no_await_in(group.stream(), span).is_err(),
        _ => false,
    });
    if awaits {
        Err(Error::new(
            span,
            "a wait point cannot stand inside a macro: it stands as a statement of its own \
             (specification 48 RU-72)",
        ))
    } else {
        Ok(())
    }
}

/// Whether `expression` awaits anywhere in it.
fn awaits(expression: &Expr) -> bool {
    let mut found = Found::default();
    found.visit_expr(expression);
    found.first.is_some()
}

/// The first `.await` or `async` an expression holds.
#[derive(Default)]
struct Found {
    first: Option<(Span, &'static str)>,
}

impl Found {
    fn refusal(self) -> Result<(), Error> {
        match self.first {
            Some((span, message)) => Err(Error::new(span, message)),
            None => Ok(()),
        }
    }
}

impl<'ast> Visit<'ast> for Found {
    fn visit_expr_await(&mut self, awaited: &'ast ExprAwait) {
        self.first.get_or_insert((
            awaited.span(),
            "a wait point stands as a statement of its own, `s.wait(t).await;`, not inside another \
             expression (specification 48 RU-72)",
        ));
    }

    fn visit_expr_async(&mut self, block: &'ast syn::ExprAsync) {
        self.first.get_or_insert((
            block.async_token.span(),
            "an `async` block cannot stand in a sequence: no future is kept from one scan to the \
             next (specification 48 RU-71)",
        ));
    }

    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        if let Some(token) = closure.asyncness {
            self.first.get_or_insert((
                token.span(),
                "an `async` closure cannot stand in a sequence: no future is kept from one scan to \
                 the next (specification 48 RU-71)",
            ));
        }
        visit::visit_expr_closure(self, closure);
    }
}

/// Whether every way through `block` reaches a wait point before it could go round its loop again.
fn always_waits(block: &Block) -> bool {
    for statement in &block.stmts {
        let expression = match statement {
            Stmt::Expr(expression, _) => expression,
            Stmt::Local(local)
                if local
                    .init
                    .as_ref()
                    .is_some_and(|init| awaited(&init.expr).is_some()) =>
            {
                return true;
            }
            _ => continue,
        };
        if always_waits_expression(expression) {
            return true;
        }
        if continues(expression) {
            return false;
        }
    }
    false
}

fn always_waits_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Await(_) => true,
        Expr::Try(tried) => matches!(&*tried.expr, Expr::Await(_)),
        Expr::Block(inner) => always_waits(&inner.block),
        Expr::If(branch) => match &branch.else_branch {
            Some((_, otherwise)) => {
                always_waits(&branch.then_branch) && always_waits_expression(otherwise)
            }
            None => false,
        },
        Expr::Loop(looped) => always_waits(&looped.body),
        _ => false,
    }
}

/// Whether `expression` holds a `continue` of the loop it stands in, outside the loops it holds.
fn continues(expression: &Expr) -> bool {
    struct Continues(bool);
    impl<'ast> Visit<'ast> for Continues {
        fn visit_expr_continue(&mut self, _: &'ast syn::ExprContinue) {
            self.0 = true;
        }
        fn visit_expr_loop(&mut self, _: &'ast syn::ExprLoop) {}
        fn visit_expr_while(&mut self, _: &'ast syn::ExprWhile) {}
        fn visit_expr_for_loop(&mut self, _: &'ast syn::ExprForLoop) {}
        fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}
    }
    let mut found = Continues(false);
    found.visit_expr(expression);
    found.0
}
