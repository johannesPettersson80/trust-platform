//! A `#[trust_sequence]`'s body (specification 48 §15, RU-71, RU-72), as the attribute and the
//! declarations' writer read it: its wait points in the body's order, each with what it waits for and
//! the step that the `s.step` before it names, and its locals, each a variable of the sequence's
//! state; and RU-72's refusals, each at its place in the body. The members of an `s.all` or an
//! `s.race` are `s.next_cycle()`, `s.until(..)` and `s.wait(..)`, two to sixteen of them, and only an
//! `s.race` gives a value, the number of its member that completed, to a `u16` local.
//!
//! A `#[trust_sequence]` block holds the POU's sequence, the one `async fn` no other awaits, and the
//! child sequences it awaits, each with a state of its own in a field of `_seq` (RU-71, RU-72).

use std::fmt::Write as _;

use proc_macro2::Span;
use syn::ext::IdentExt as _;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{
    Block as Body, Error, Expr, ExprAwait, FnArg, ImplItemFn, Lit, Local as LetStatement, Pat,
    ReceiverKind, Stmt,
};

use crate::types::{DataType, Elementary};
use crate::{Field, Section, Structure, Variable};

mod blocks;
mod leaks;

pub use blocks::block;

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

/// The Rust name of the type of the state of the child sequence `child` of the POU whose Rust item
/// is `item`, `<Item>Sequence<Child>` with the child's name in upper camel case (RU-71):
/// `PalletizerSequenceFill` for `fill` of `Palletizer`.
#[must_use]
pub fn child_state_item(item: &str, child: &str) -> String {
    let camel: String = child
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut letters = part.chars();
            letters.next().map_or_else(String::new, |first| {
                first.to_ascii_uppercase().to_string() + letters.as_str()
            })
        })
        .collect();
    format!("{item}Sequence{camel}")
}

/// The Structured Text name of that type, `<POU>_SEQ_<CHILD>` with the child's name in capitals, in
/// the namespace of the POU named `pou` (RU-71): `PALLETIZER_SEQ_FILL`.
#[must_use]
pub fn child_state_structure(pou: &str, child: &str) -> String {
    format!("{pou}_SEQ_{}", child.to_ascii_uppercase())
}

/// The structure of the state of `sequence`, the sequence of the POU named `pou` whose Rust item is
/// `item` (RU-71): `point : UINT`, the wait point it waits at, `began : TIME`, when its wait began,
/// `done : WORD` when the body holds an `s.all` (RU-72), and its locals with their types, each
/// starting at its type's default, the sequence's top.
#[must_use]
pub fn state(pou: &str, item: &str, sequence: &Sequence) -> Structure {
    Structure {
        name: state_structure(pou),
        item: state_item(item),
        fields: fields(sequence, Some(shape(sequence))),
    }
}

/// The structures of the states of `block`'s sequences, the POU's first: its state (see
/// [`state`]), whose shape is the block's ([`block_shape`]), with a field for each child sequence
/// after its locals, named as the child and of the child's state, then each child's state, laid out
/// as the POU's sequence's without a shape, in the block's order (RU-71, RU-73).
#[must_use]
pub fn states(pou: &str, item: &str, block: &Block) -> Vec<Structure> {
    let mut first = Structure {
        name: state_structure(pou),
        item: state_item(item),
        fields: fields(&block.sequence, Some(block_shape(block))),
    };
    first
        .fields
        .extend(block.children.iter().map(|child| Field {
            name: child.name.clone(),
            ty: DataType::Named(child_state_item(item, &child.name)),
            init: None,
        }));
    let children = block.children.iter().map(|child| Structure {
        name: child_state_structure(pou, &child.name),
        item: child_state_item(item, &child.name),
        fields: fields(child, None),
    });
    std::iter::once(first).chain(children).collect()
}

/// The name of the member of the state of `sequence`, a sequence without children, that tells an
/// online change whether the changed code has the same wait points and locals (RU-73): `shape_` and
/// eight hexadecimal digits of the 32-bit FNV-1a hash of its lines: a line per wait point, `wait
/// <step>` or `wait #<number>`, a line per parameter, `parameter <name>: <type>`, and a line per local,
/// `local <name>: <type>`, each name in small letters, in the body's order.
#[must_use]
pub fn shape(sequence: &Sequence) -> String {
    let mut text = String::new();
    lines(sequence, &mut text);
    signature(&text)
}

/// The shape of the POU's state of `block` (RU-73): the hash of the lines of the POU's sequence and
/// then of each child in the block's order, each child's after a line `child <name>`, so that a
/// change of any of them starts the whole sequence at its top.
#[must_use]
pub fn block_shape(block: &Block) -> String {
    let mut text = String::new();
    lines(&block.sequence, &mut text);
    for child in &block.children {
        // Writing to a `String` cannot fail.
        let _ = writeln!(text, "child {}", child.name.to_ascii_lowercase());
        lines(child, &mut text);
    }
    signature(&text)
}

/// `shape_` and eight hexadecimal digits of the 32-bit FNV-1a hash of `text`.
fn signature(text: &str) -> String {
    let hash = text.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    format!("shape_{hash:08x}")
}

/// The lines of `sequence` that its shape hashes (RU-73): a line per wait point, `wait <step>` or
/// `wait #<number>`, a line per parameter, `parameter <name>: <type>`, and a line per local, `local
/// <name>: <type>`, each name in small letters, in the body's order, each line ending in a line feed.
fn lines(sequence: &Sequence, lines: &mut String) {
    for point in &sequence.wait_points {
        // Writing to a `String` cannot fail.
        let _ = match &point.step {
            Some(step) => writeln!(lines, "wait {step}"),
            None => writeln!(lines, "wait #{}", point.number),
        };
    }
    for parameter in &sequence.parameters {
        let _ = writeln!(
            lines,
            "parameter {}: {}",
            parameter.name.to_ascii_lowercase(),
            parameter.ty.iec()
        );
    }
    for local in &sequence.locals {
        let _ = writeln!(
            lines,
            "local {}: {}",
            local.name.to_ascii_lowercase(),
            local.ty.iec()
        );
    }
}

/// The fields of the state of `sequence`: `point`, `began`, `done` when its body holds an `s.all`,
/// the member `shape` names when it is the POU's state (RU-73), its parameters and its locals.
fn fields(sequence: &Sequence, shape: Option<String>) -> Vec<Field> {
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
    if let Some(shape) = shape {
        fields.push(Field {
            name: shape,
            ty: DataType::Elementary(Elementary::Bool),
            init: None,
        });
    }
    fields.extend(sequence.parameters.iter().map(|parameter| Field {
        name: parameter.name.clone(),
        ty: parameter.ty.clone(),
        init: None,
    }));
    fields.extend(sequence.locals.iter().map(|local| Field {
        name: local.name.clone(),
        ty: local.ty.clone(),
        init: None,
    }));
    fields
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
    /// `self.<sequence>(s)`: a child sequence of the same block, to its end.
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
    /// The `async fn`'s name, without a raw identifier's `r#`.
    pub name: String,
    /// Its wait points in the body's order.
    pub wait_points: Vec<WaitPoint>,
    /// A child's parameters after its context, in their order: variables of its state before its
    /// locals, set from the arguments when it starts (RU-72).
    pub parameters: Vec<Local>,
    /// Its locals in the body's order.
    pub locals: Vec<Local>,
    /// The child sequences its wait points await, each once, in the body's order.
    pub awaits: Vec<String>,
}

/// A child sequence a wait point awaits: its name, where the name stands, and the number of the
/// arguments the call passes after the context.
pub(super) struct Awaited {
    pub(super) child: String,
    pub(super) place: Span,
    pub(super) arguments: usize,
}

/// A `#[trust_sequence]` block (RU-71, RU-72): the POU's sequence, the one `async fn` of the block
/// that no other awaits, and the child sequences, every other, in the block's order.
#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// The name of the struct the block is the `impl` block of, the POU's Rust item.
    pub owner: String,
    /// The POU's sequence.
    pub sequence: Sequence,
    /// The child sequences, in the block's order.
    pub children: Vec<Sequence>,
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
    read(item).map(|(sequence, _)| sequence)
}

/// [`sequence`], with each child sequence a wait point awaits.
fn read(item: &ImplItemFn) -> Result<(Sequence, Vec<Awaited>), Error> {
    if item.sig.asyncness.is_none() {
        return Err(Error::new(
            item.sig.fn_token.span(),
            "a sequence is an `async fn name(&mut self, s: &mut Seq) -> Result<(), Fault>`",
        ));
    }
    let mut inputs = item.sig.inputs.iter();
    if !matches!(
        inputs.next(),
        Some(FnArg::Receiver(receiver))
            if matches!(receiver.kind, ReceiverKind::Reference(_, _, Some(_)))
    ) {
        return Err(Error::new(
            item.sig.ident.span(),
            "a sequence is a method of its POU that changes it: `async fn name(&mut self, s: &mut \
             Seq) -> Result<(), Fault>` (specification 48 RU-70)",
        ));
    }
    let Some(FnArg::Typed(context)) = inputs.next() else {
        return Err(Error::new(
            item.sig.inputs.span(),
            "a sequence takes its wait points' context after `&mut self`: `s: &mut Seq`",
        ));
    };
    let (parameters, parameter_places) = parameters(inputs)?;
    let Pat::Ident(context) = &*context.pat else {
        return Err(Error::new(
            context.pat.span(),
            "the wait points' context is a name, `s: &mut Seq`",
        ));
    };
    let mut reader = Reader {
        context: context.ident.to_string(),
        wait_points: Vec::new(),
        parameters: parameters
            .iter()
            .map(|parameter| parameter.name.clone())
            .collect(),
        locals: Vec::new(),
        local_places: Vec::new(),
        children: Vec::new(),
        step: None,
    };
    reader.block(&item.block)?;
    let mut awaits: Vec<String> = Vec::new();
    for awaited in &reader.children {
        if !awaits.contains(&awaited.child) {
            awaits.push(awaited.child.clone());
        }
    }
    let read = Sequence {
        name: item.sig.ident.unraw().to_string(),
        wait_points: reader.wait_points,
        parameters,
        locals: reader.locals,
        awaits,
    };
    if read.holds_all() {
        let named_done = |names: &[Local], places: &[Span]| {
            names
                .iter()
                .position(|named| named.name.eq_ignore_ascii_case(DONE))
                .map(|index| {
                    places
                        .get(index)
                        .copied()
                        .unwrap_or_else(|| item.sig.ident.span())
                })
        };
        if let Some(place) = named_done(&read.parameters, &parameter_places)
            .or_else(|| named_done(&read.locals, &reader.local_places))
        {
            return Err(a_field_of_the_state(place, DONE));
        }
    }
    Ok((read, reader.children))
}

/// A child sequence's parameters, the inputs after its context: each one name with its type, of
/// the types a local takes, named as a field of its state can be (RU-72); with where each name
/// stands.
fn parameters<'a>(
    inputs: impl Iterator<Item = &'a FnArg>,
) -> Result<(Vec<Local>, Vec<Span>), Error> {
    let mut parameters: Vec<Local> = Vec::new();
    let mut places = Vec::new();
    for input in inputs {
        let (FnArg::Typed(typed), Some(ident)) = (input, parameter_ident(input)) else {
            return Err(Error::new(
                input.span(),
                "a child sequence's parameter is one name with its type, `target: f64`: it is a \
                 variable of the child's state (specification 48 RU-72)",
            ));
        };
        let name = crate::variable_name(ident)?;
        if let Some(field) = ["point", "began"]
            .into_iter()
            .find(|field| name.eq_ignore_ascii_case(field))
        {
            return Err(a_field_of_the_state(ident.span(), field));
        }
        if parameters
            .iter()
            .any(|other| other.name.eq_ignore_ascii_case(&name))
        {
            return Err(Error::new(
                ident.span(),
                format!(
                    "a second parameter `{name}`: a child's parameters are variables of its state, \
                     one of each name, as Structured Text names them without case (specification \
                     48 RU-72)"
                ),
            ));
        }
        parameters.push(Local {
            name,
            ty: DataType::of(&typed.ty)?,
        });
        places.push(ident.span());
    }
    Ok((parameters, places))
}

/// The name of the parameter `input`, when it is one name: `target` in `target: f64` or `mut
/// target: f64`.
pub(super) fn parameter_ident(input: &FnArg) -> Option<&syn::Ident> {
    let FnArg::Typed(typed) = input else {
        return None;
    };
    match &*typed.pat {
        Pat::Ident(ident) if ident.subpat.is_none() && ident.by_ref.is_none() => Some(&ident.ident),
        _ => None,
    }
}

/// What reading a body has found so far.
struct Reader {
    /// The name of the wait points' context, `s`.
    context: String,
    wait_points: Vec<WaitPoint>,
    /// The names of the sequence's parameters, which no local takes.
    parameters: Vec<String>,
    locals: Vec<Local>,
    /// Where each local's name stands, for a refusal found after the body is read.
    local_places: Vec<Span>,
    /// The child sequences the wait points await.
    children: Vec<Awaited>,
    /// The step the last `s.step` named, for the next wait point.
    step: Option<String>,
}

impl Reader {
    fn block(&mut self, block: &Body) -> Result<(), Error> {
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
            Expr::Await(awaited) => {
                self.wait_point(awaited)?;
                if self.wait_points.last().map(|point| point.wait) == Some(Wait::Child) {
                    return Err(Error::new(
                        awaited.await_token.span(),
                        "a child sequence's fault is the fault of the sequence that awaits it: \
                         `self.fill(s).await?;` (specification 48 RU-72)",
                    ));
                }
                Ok(())
            }
            Expr::Try(tried) => match &*tried.expr {
                Expr::Await(awaited) => self.wait_point(awaited),
                other => plain(other),
            },
            Expr::Loop(looped) => {
                // Every child sequence counts as a wait here; the block's reading checks again with
                // the children that cannot return without waiting.
                if leaks::spins(&looped.body, looped.label.as_ref(), &|_| true) {
                    return Err(leaks::spinning(looped.loop_token.span()));
                }
                self.block(&looped.body)
            }
            Expr::While(looped) => {
                plain(&looped.cond)?;
                if leaks::spins(&looped.body, looped.label.as_ref(), &|_| true) {
                    return Err(leaks::spinning(looped.while_token.span()));
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
            Expr::Block(inner) => {
                if inner.label.is_some() && awaits(expression) {
                    return Err(Error::new(
                        expression.span(),
                        "a labelled block cannot hold a wait point: its `break` would leave the \
                         block's wait behind; use a loop or `if` (specification 48 RU-72)",
                    ));
                }
                self.block(&inner.block)
            }
            other => {
                plain(other)?;
                self.step_named(other);
                Ok(())
            }
        }
    }

    fn wait_point(&mut self, awaited: &ExprAwait) -> Result<(), Error> {
        let (wait, within) = self.wait_of(&awaited.base)?;
        if let Some((child, arguments)) = child_called(&awaited.base) {
            self.children.push(Awaited {
                child: child.unraw().to_string(),
                place: child.span(),
                arguments,
            });
        }
        if matches!(wait, Wait::All | Wait::Race) {
            self.child_members(&awaited.base)?;
        }
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
                    (self.is_context(&inner.receiver)
                        && ["next_cycle", "until", "wait"]
                            .iter()
                            .any(|name| inner.method == name))
                        || self.child_member(inner).is_some()
                }
                _ => false,
            };
            if !plain_wait {
                return Err(Error::new(
                    member.span(),
                    format!(
                        "a member of `{s}.{method}` is `{s}.next_cycle()`, `{s}.until(..)`, \
                         `{s}.wait(..)` or a child sequence `self.<child>({s}, ..)` (specification 48 \
                         RU-72)"
                    ),
                ));
            }
        }
        Ok(())
    }

    /// The child sequence `call` awaits as a member of an `s.all` or `s.race`, `self.fill(s, ..)`,
    /// with the number of its arguments after the context.
    fn child_member<'a>(&self, call: &'a syn::ExprMethodCall) -> Option<(&'a syn::Ident, usize)> {
        let child = matches!(&*call.receiver, Expr::Path(path) if path.path.is_ident("self"))
            && call
                .args
                .first()
                .is_some_and(|argument| self.is_context(argument));
        child.then(|| (&call.method, call.args.len().saturating_sub(1)))
    }

    /// The child sequences of the members of `s.all(..)` or `s.race(..)`, `base`'s call, recorded as
    /// awaited, each once at most (RU-72).
    fn child_members(&mut self, base: &Expr) -> Result<(), Error> {
        let mut call = base;
        while let Expr::MethodCall(within) = call
            && within.method == "within"
        {
            call = &within.receiver;
        }
        let Expr::MethodCall(call) = call else {
            return Ok(());
        };
        let mut seen: Vec<String> = Vec::new();
        for member in &call.args {
            let Expr::MethodCall(inner) = member else {
                continue;
            };
            let Some((child, arguments)) = self.child_member(inner) else {
                continue;
            };
            let name = child.unraw().to_string();
            if seen.contains(&name) {
                return Err(Error::new(
                    child.span(),
                    format!(
                        "the child sequence `{name}` is a member of this `{}.{}` twice: a child has \
                         one state (specification 48 RU-72)",
                        self.context, call.method
                    ),
                ));
            }
            seen.push(name.clone());
            self.children.push(Awaited {
                child: name,
                place: child.span(),
                arguments,
            });
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
        if let Some(parameter) = self
            .parameters
            .iter()
            .find(|parameter| parameter.eq_ignore_ascii_case(&name))
        {
            return Err(Error::new(
                ident.ident.span(),
                format!(
                    "a local `{name}` beside the parameter `{parameter}`: both are variables of the \
                     child's state, one of each name (specification 48 RU-72)"
                ),
            ));
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

/// The name of the child sequence that `base`, the expression before `.await`, awaits, when it is
/// one, `self.fill(s)` or `self.fill(s).within(t)`.
fn child_called(base: &Expr) -> Option<(&syn::Ident, usize)> {
    let Expr::MethodCall(call) = base else {
        return None;
    };
    if call.method == "within" {
        return child_called(&call.receiver);
    }
    matches!(&*call.receiver, Expr::Path(path) if path.path.is_ident("self"))
        .then(|| (&call.method, call.args.len().saturating_sub(1)))
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
