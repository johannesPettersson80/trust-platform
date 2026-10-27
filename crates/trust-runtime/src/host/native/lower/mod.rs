//! A POU's statements as Rust (specification 44 §6.4 to §6.6): every statement begins with the
//! statement hook, every operation that can fault returns the fault up to the entry point, every
//! variable is read and written where it lies.
//!
//! The lowering reads the syntax tree of the POU's body with what trust-hir learned of it: the type
//! of every expression and the symbol every name resolved to. A value that a later part of the
//! statement needs is computed into a numbered temporary first (`t1`, `t2`), in source order, so
//! that the faults of a statement come in the order its operands are written.

mod assertions;
mod call;
mod case;
mod conversions;
mod dates;
mod effects;
mod enable;
mod expr;
mod functions;
mod jumps;
mod object;
mod open;
mod overlap;
mod partial;
mod places;
mod prelude;
mod strings;
mod text;

pub(super) use call::{ident as ident_of, parameter};
pub(super) use prelude::PRELUDE;
pub(super) use text::{byte_string, unit_string};

use std::collections::BTreeSet;

use smol_str::SmolStr;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::front::{Source, StatementIds};
use super::model::Model;
use super::types::{rust_field, Elem, Kind, Ty};
use super::NativeError;

/// The POU whose body is lowered.
#[derive(Clone, Copy)]
pub(super) enum Frame {
    /// A program type: `me` is its variables, `mr` its retained variables.
    Program(usize),
    /// A function block: `me` is its instance.
    FunctionBlock(Ty),
    /// A method: `me` is its owner's instance.
    Method(Ty),
    /// A function.
    Function,
}

/// A variable of the body's own frame: a parameter, a local, a temporary or the return value.
#[derive(Clone)]
pub(super) struct Local {
    pub(super) name: SmolStr,
    /// The Rust name.
    pub(super) rust: String,
    pub(super) ty: Ty,
    /// Whether the Rust name is a reference to the storage, as a `VAR_IN_OUT` parameter is.
    pub(super) is_ref: bool,
}

/// Where a value lies: a Rust expression naming the storage, or a reference to it.
#[derive(Clone)]
pub(super) struct Place {
    pub(super) expr: String,
    pub(super) ty: Ty,
    /// Whether `expr` is a reference (`&T`) rather than the storage itself.
    pub(super) is_ref: bool,
}

impl Place {
    /// Whether the place is a member of a `STRUCT OVERLAP`, a view of the structure's bytes that
    /// no Rust reference names (the prelude's `ov`).
    pub(super) fn is_overlap_member(&self) -> bool {
        self.expr.starts_with("ov::")
    }

    /// Whether the place is a structure or an array that lies in a `STRUCT OVERLAP`'s backing, an
    /// `ov::Region` of its bytes (IEC §6.4.4.7.1; `overlap`).
    pub(super) fn is_overlap_region(&self) -> bool {
        self.expr.starts_with("ov::Region")
    }

    /// A reference to the storage.
    pub(super) fn reference(&self) -> String {
        if self.is_ref {
            self.expr.clone()
        } else {
            format!("&{}", self.expr)
        }
    }
}

/// The labels of the loop a statement is in: `EXIT` breaks the loop, `CONTINUE` its body.
struct LoopLabels {
    exit: String,
    next: String,
}

/// The Rust of one body.
pub(super) struct Lowerer<'m, 'a> {
    pub(super) model: &'m mut Model<'a>,
    pub(super) src: &'m Source,
    file: usize,
    ids: &'m StatementIds,
    pub(super) frame: Frame,
    pub(super) locals: Vec<Local>,
    out: String,
    indent: usize,
    temps: u32,
    loops: Vec<LoopLabels>,
    labels: u32,
    /// Whether statements call the statement hook (specification 44 §13.2).
    hooks: bool,
    /// The ids of the statements of this body.
    pub(super) statements: Vec<u32>,
    /// What a `RETURN` and the end of the body do: copy a function's result and return it.
    pub(super) exit: String,
    /// The result variable of a function or a method that returns a value, which `RETURN expr;`
    /// assigns (specification 06, DEV-022).
    pub(super) result: Option<Place>,
    /// The labels of the body's segments, in capitals, while the body runs as their loop
    /// (`jumps`).
    jumps: Option<Vec<(SmolStr, usize)>>,
    /// The open-array inputs of the body, views of the caller's arrays that it only reads
    /// (specification 03 §6).
    pub(super) read_only: Vec<SmolStr>,
    /// The functions and methods the body calls, which the crate must hold too.
    pub(super) needs: Vec<Need>,
    /// The generated functions the body calls, by their Rust names (specification 44 §6.4).
    pub(super) called: BTreeSet<String>,
    /// The callees whose calls take one from the call depth: those of the body's own cycle
    /// (specification 44 §6.4).
    pub(super) recursive: BTreeSet<String>,
    /// The control variables of the enclosing `FOR` loops that a release image holds in Rust
    /// locals: each one's place and the local's name (specification 44 §6.6).
    held: Vec<(String, String)>,
    /// The POU's id, which its statements and its frame carry (specification 44 §5.7, §5.9).
    pub(super) pou: u16,
    /// A debug image's frame variables, which lie in one structure `fr` at the offsets their
    /// symbols name (specification 44 §5.7, §13.2); `None` in a release image, whose frame
    /// variables are Rust locals.
    pub(super) frame_vars: Option<Vec<FrameVar>>,
    /// The bytes of an address on the target, which `SIZEOF` gives a reference or a pointer
    /// (specification 29 §1).
    pub(super) pointer_bytes: u64,
}

/// A variable of a debug image's frame structure: its declared name, its field and its type.
#[derive(Clone, Debug)]
pub(super) struct FrameVar {
    pub(super) name: SmolStr,
    pub(super) field: String,
    pub(super) ty: Ty,
}

/// A POU a body calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Need {
    /// A function, by its name.
    Function(SmolStr),
    /// A method, by its owner's and its own name.
    Method(SmolStr, SmolStr),
    /// A method, by its name, of an instance of the type or of a type derived from it: a call bound
    /// by the instance's class (IEC §6.6.8; specification 44 §6.4).
    Virtual(Ty, SmolStr),
    /// A method, by its name, through a reference of the interface, by the interface's name.
    Interface(SmolStr, SmolStr),
    /// The body of an instance of the type or of a type derived from it.
    Body(Ty),
    /// The assignment attempt `?=` to an interface or to a reference of a class's type: the
    /// reference when the instance it names takes the type, else `NULL` (IEC §6.6.6.7, Table 52).
    Cast(Ty),
}

impl Need {
    /// Adds `self` to `needs` once.
    pub(super) fn add_to(self, needs: &mut Vec<Need>) {
        if !needs.contains(&self) {
            needs.push(self);
        }
    }
}

impl<'m, 'a> Lowerer<'m, 'a> {
    pub(super) fn new(
        model: &'m mut Model<'a>,
        src: &'m Source,
        file: usize,
        ids: &'m StatementIds,
        frame: Frame,
        hooks: bool,
        pointer_bytes: u64,
    ) -> Self {
        Self {
            model,
            src,
            file,
            ids,
            frame,
            locals: Vec::new(),
            out: String::new(),
            indent: 1,
            temps: 0,
            loops: Vec::new(),
            labels: 0,
            hooks,
            statements: Vec::new(),
            exit: "return Ok(());".into(),
            result: None,
            jumps: None,
            read_only: Vec::new(),
            needs: Vec::new(),
            called: BTreeSet::new(),
            recursive: BTreeSet::new(),
            held: Vec::new(),
            pou: u16::MAX,
            frame_vars: hooks.then(Vec::new),
            pointer_bytes,
        }
    }

    /// The Rust written so far, the body's statements and a debug image's frame variables.
    pub(super) fn finish(self) -> (String, Vec<u32>, Option<Vec<FrameVar>>) {
        (self.out, self.statements, self.frame_vars)
    }

    /// Writes one line at the current indentation.
    pub(super) fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    /// A fresh temporary's name.
    pub(super) fn temp(&mut self) -> String {
        self.temps += 1;
        format!("t{}", self.temps)
    }

    /// Computes `value` into a fresh temporary and returns its name.
    pub(super) fn bind(&mut self, value: &str) -> String {
        let name = self.temp();
        self.line(&format!("let {name} = {value};"));
        name
    }

    pub(super) fn info(&self, ty: Ty) -> &super::types::TypeInfo {
        self.model.types.info(ty)
    }

    /// An error naming the construct and where it is (specification 44 NX-09).
    pub(super) fn unsupported(&self, node: &SyntaxNode, what: impl Into<String>) -> NativeError {
        NativeError::Unsupported {
            construct: what.into(),
            at: self.src.at(node),
        }
    }

    /// The type trust-hir gave `node`; a sign or a parenthesis it gave none has its operand's, and
    /// an arithmetic operation it gave none the type of its operands when they have one.
    pub(super) fn type_of(&mut self, node: &SyntaxNode) -> Result<Ty, NativeError> {
        let Some(hir) = self.src.type_of(node) else {
            let signed = node.kind() == SyntaxKind::UnaryExpr
                && matches!(
                    effects::operator(node),
                    Some(SyntaxKind::Minus | SyntaxKind::Plus)
                );
            if (signed || node.kind() == SyntaxKind::ParenExpr)
                && let Some(operand) = node.children().next()
            {
                return self.type_of(&operand);
            }
            let arithmetic = node.kind() == SyntaxKind::BinaryExpr
                && matches!(
                    effects::operator(node),
                    Some(
                        SyntaxKind::Plus
                            | SyntaxKind::Minus
                            | SyntaxKind::Star
                            | SyntaxKind::Slash
                            | SyntaxKind::KwMod
                            | SyntaxKind::Power
                    )
                );
            let mut operands = node.children();
            if arithmetic
                && let (Some(left), Some(right)) = (operands.next(), operands.next())
                && let (Ok(left), Ok(right)) = (self.type_of(&left), self.type_of(&right))
                && left == right
            {
                return Ok(left);
            }
            return Err(self.unsupported(node, "an expression without a type"));
        };
        let symbols = self.src.analysis.symbols.clone();
        self.model.types.of_hir(&symbols, hir)
    }

    /// Declares a local of the frame at its type's zero value; `frame_init` gives it its starting
    /// value.
    pub(super) fn declare(&mut self, name: &SmolStr, ty: Ty) -> String {
        let field = rust_field(name);
        let rust = if let Some(vars) = self.frame_vars.as_mut() {
            // A debug image lays its frame variables out in one structure, which the kernel reads
            // at a stop (specification 44 §13.2).
            vars.push(FrameVar {
                name: name.clone(),
                field: field.clone(),
                ty,
            });
            format!("fr.{field}")
        } else {
            let storage = self.info(ty).rust.clone();
            self.line(&format!(
                "let {field}: {storage} = <{storage} as Plain>::ZEROED;"
            ));
            field
        };
        self.locals.push(Local {
            name: name.clone(),
            rust: rust.clone(),
            ty,
            is_ref: false,
        });
        rust
    }

    /// Names the parameter `argument`, a reference to the caller's variable, `name` in the frame:
    /// a function's or a method's `VAR_IN_OUT` (IEC §6.6.1.2.2).
    /// Names `rust`, a variable that lies outside the frame, `name` in the body: a `VAR_STAT`
    /// variable (specification 01 §3).
    pub(super) fn bind_static(&mut self, name: &SmolStr, rust: String, ty: Ty) {
        self.locals.push(Local {
            name: name.clone(),
            rust,
            ty,
            is_ref: false,
        });
    }

    pub(super) fn declare_reference(&mut self, name: &SmolStr, ty: Ty, argument: &str) {
        self.locals.push(Local {
            name: name.clone(),
            rust: argument.to_string(),
            ty,
            is_ref: true,
        });
    }

    /// Lowers the statements among `nodes`, the children of a statement list or a branch.
    pub(super) fn statements(
        &mut self,
        nodes: impl Iterator<Item = SyntaxNode>,
    ) -> Result<(), NativeError> {
        for node in nodes {
            if super::front::is_statement(node.kind()) {
                self.statement(&node)?;
            }
        }
        Ok(())
    }

    fn statement(&mut self, node: &SyntaxNode) -> Result<(), NativeError> {
        let Some(id) = self.ids.id(self.file, node) else {
            return Err(self.unsupported(node, "a statement without an id"));
        };
        self.statements.push(id);
        if self.hooks {
            self.line(&format!("st(cx, {id})?;"));
        }
        match node.kind() {
            SyntaxKind::AssignStmt => self.assignment(node),
            SyntaxKind::ExprStmt => {
                let Some(expr) = node.children().next() else {
                    return Ok(());
                };
                self.discard(&expr)
            }
            SyntaxKind::IfStmt => self.if_statement(node),
            SyntaxKind::CaseStmt => self.case_statement(node),
            SyntaxKind::ForStmt => self.for_statement(node),
            SyntaxKind::WhileStmt => self.while_statement(node),
            SyntaxKind::RepeatStmt => self.repeat_statement(node),
            SyntaxKind::ExitStmt => {
                let Some(labels) = self.loops.last() else {
                    return Err(self.unsupported(node, "EXIT outside a loop"));
                };
                let exit = labels.exit.clone();
                self.line(&format!("break {exit};"));
                Ok(())
            }
            SyntaxKind::ContinueStmt => {
                let Some(labels) = self.loops.last() else {
                    return Err(self.unsupported(node, "CONTINUE outside a loop"));
                };
                let next = labels.next.clone();
                self.line(&format!("break {next};"));
                Ok(())
            }
            SyntaxKind::ReturnStmt => self.return_statement(node),
            SyntaxKind::JmpStmt => self.jump(node),
            SyntaxKind::LabelStmt => self.labelled(node),
            other => Err(self.unsupported(node, format!("the statement {other:?}"))),
        }
    }

    /// `target := value`.
    fn assignment(&mut self, node: &SyntaxNode) -> Result<(), NativeError> {
        let mut parts = node.children();
        let (Some(target), Some(value)) = (parts.next(), parts.next()) else {
            return Err(self.unsupported(node, "an assignment without a target or a value"));
        };
        if node
            .children_with_tokens()
            .any(|element| element.kind() == SyntaxKind::RefAssign)
        {
            return self.assignment_attempt(node, &target, &value);
        }
        if let Some((base, access)) = self.partial_access(&target) {
            return self.partial_write(&target, &base, access, &value);
        }
        if self.string_element_write(&target, &value)? || self.property_write(&target, &value)? {
            return Ok(());
        }
        self.refuse_read_only(&target)?;
        let place = self.place(&target)?;
        self.store(&place, &value)
    }

    /// Refuses a write to an element of an open-array input, which is the caller's array: an input
    /// is the caller's value, which the callee does not change (specification 03 §6).
    fn refuse_read_only(&self, target: &SyntaxNode) -> Result<(), NativeError> {
        let mut root = target.clone();
        while matches!(
            root.kind(),
            SyntaxKind::IndexExpr | SyntaxKind::FieldExpr | SyntaxKind::ParenExpr
        ) {
            let Some(base) = root.children().next() else {
                break;
            };
            root = base;
        }
        if root.kind() == SyntaxKind::NameRef {
            let name = call::ident(&root);
            if self
                .read_only
                .iter()
                .any(|input| input.eq_ignore_ascii_case(&name))
            {
                return Err(self.unsupported(
                    target,
                    format!(
                        "a write to the open-array input {name}, which is the caller's array; \
                         declare it VAR_IN_OUT"
                    ),
                ));
            }
        }
        Ok(())
    }

    /// `target ?= value` (IEC §6.6.6.7, Table 52; specification 02 §4.1, §4.2): an interface, or a
    /// reference of a class's type, takes the reference when the instance it names is of a class
    /// the target takes, and `NULL` otherwise, whatever the target held; any other reference or
    /// pointer takes the value, `NULL` or of the same type, which trust-hir checked.
    fn assignment_attempt(
        &mut self,
        node: &SyntaxNode,
        target: &SyntaxNode,
        value: &SyntaxNode,
    ) -> Result<(), NativeError> {
        let place = self.place(target)?;
        let source = if self.is_reference(value) {
            self.reference(value)?
        } else {
            self.interface_value(value)?
        };
        let cast = match self.info(place.ty).kind {
            Kind::Interface => Some(place.ty),
            Kind::Ref { target, .. } if self.model.types.class_field(target).is_some() => {
                Some(target)
            }
            Kind::Ref { .. } => None,
            _ => {
                return Err(self.unsupported(
                    node,
                    "the assignment attempt ?= to a value that is no reference",
                ));
            }
        };
        let value = match cast {
            Some(ty) => {
                Need::Cast(ty).add_to(&mut self.needs);
                let function = crate::native::dispatch::cast_function(ty);
                self.bind(&format!("{function}(cx, w, {source})?"))
            }
            None => source,
        };
        self.line(&format!("{}.set({value});", place.expr));
        Ok(())
    }

    /// Stores the value of `value` into `place`, converted to the place's type as the assignment
    /// converts it.
    pub(super) fn store(&mut self, place: &Place, value: &SyntaxNode) -> Result<(), NativeError> {
        // A structure or an array that lies in an overlap's backing (`overlap`).
        if place.is_overlap_region() {
            return self.store_region(place, value);
        }
        let info = self.info(place.ty).clone();
        match &info.kind {
            Kind::Str(_) | Kind::WStr(_) => self.store_text(place, value),
            Kind::Ref { target, .. } if matches!(self.info(*target).kind, Kind::Open { .. }) => {
                self.store_open(place, value, *target)
            }
            Kind::Ref { target, .. } => {
                let reference = match self.instance_source(*target, value)? {
                    // A result of a function block's type names the instance assigned to it,
                    // `THIS` or an instance (§6.4; IEC §6.6.5.7.2).
                    Some(source) => {
                        let instance = self.as_type(value, &source, *target)?;
                        self.bind(&format!(
                            "w.areas.refer(cx, {instance}, abi::RefValue::INSTANCE)?"
                        ))
                    }
                    None => self.reference_to(value, Some(*target))?,
                };
                self.line(&format!("{}.set({reference});", place.expr));
                Ok(())
            }
            Kind::Interface => {
                let reference = self.interface_value(value)?;
                self.line(&format!("{}.set({reference});", place.expr));
                Ok(())
            }
            // An instance, or an array or a structure that holds one, is no value to copy.
            _ if self.model.types.holds_instance(place.ty) => Err(self.unsupported(
                value,
                "an assignment of a value that holds a function block or class instance",
            )),
            Kind::Array { .. }
            | Kind::Struct { .. }
            | Kind::Overlap { .. }
            | Kind::Fb { .. }
            | Kind::StdFb(_) => {
                let source = self.aggregate(value, place.ty)?;
                self.line(&format!(
                    "Assign::assign({}, {});",
                    place.reference(),
                    source.reference()
                ));
                Ok(())
            }
            _ => {
                let scalar = self.scalar_to(value, place.ty)?;
                self.store_scalar(place, &scalar);
                Ok(())
            }
        }
    }

    /// Writes the scalar Rust value `value`, already of the place's type, into `place`.
    pub(super) fn store_scalar(&mut self, place: &Place, value: &str) {
        let info = self.info(place.ty);
        let stored = match info.scalar() {
            Some(Elem::Bool) => format!("u8::from({value})"),
            _ => value.to_string(),
        };
        self.line(&format!("{}.set({stored});", place.expr));
    }

    /// Reads the scalar in `place` as a Rust value of its type: from the local that holds it when
    /// it is the control variable of a loop that holds it.
    pub(super) fn load(&self, place: &Place) -> String {
        if let Some((_, local)) = self.held.iter().find(|(held, _)| *held == place.expr) {
            return local.clone();
        }
        match self.info(place.ty).scalar() {
            Some(Elem::Bool) => format!("({}.get() != 0)", place.expr),
            _ => format!("{}.get()", place.expr),
        }
    }

    fn if_statement(&mut self, node: &SyntaxNode) -> Result<(), NativeError> {
        let mut children = node.children();
        let Some(condition) = children.next() else {
            return Err(self.unsupported(node, "IF without a condition"));
        };
        let test = self.condition(&condition)?;
        self.line(&format!("if {test} {{"));
        self.indent += 1;
        let branches: Vec<SyntaxNode> = children.collect();
        let mut rest = branches.iter().peekable();
        let mut then = Vec::new();
        while let Some(child) = rest.peek() {
            if matches!(
                child.kind(),
                SyntaxKind::ElsifBranch | SyntaxKind::ElseBranch
            ) {
                break;
            }
            then.push((*child).clone());
            rest.next();
        }
        self.statements(then.into_iter())?;
        self.indent -= 1;
        let mut closes = 1;
        for branch in rest {
            match branch.kind() {
                SyntaxKind::ElsifBranch => {
                    self.line("} else {");
                    self.indent += 1;
                    let mut parts = branch.children();
                    let Some(condition) = parts.next() else {
                        return Err(self.unsupported(branch, "ELSIF without a condition"));
                    };
                    let test = self.condition(&condition)?;
                    self.line(&format!("if {test} {{"));
                    self.indent += 1;
                    self.statements(parts)?;
                    self.indent -= 1;
                    closes += 1;
                }
                SyntaxKind::ElseBranch => {
                    self.line("} else {");
                    self.indent += 1;
                    self.statements(branch.children())?;
                    self.indent -= 1;
                }
                _ => {}
            }
        }
        for close in (0..closes).rev() {
            self.line("}");
            if close > 0 {
                self.indent -= 1;
            }
        }
        Ok(())
    }

    /// A BOOL condition as a Rust `bool`.
    fn condition(&mut self, node: &SyntaxNode) -> Result<String, NativeError> {
        let bool_ty = self.model.types.elem(Elem::Bool);
        self.scalar_to(node, bool_ty)
    }

    fn for_statement(&mut self, node: &SyntaxNode) -> Result<(), NativeError> {
        let Some(control) = node
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)
        else {
            return Err(self.unsupported(node, "FOR without a control variable"));
        };
        let place = self.name_place(&control, &call::ident(&control))?;
        let Some(elem) = self
            .info(place.ty)
            .scalar()
            .filter(|elem| elem.is_integer())
        else {
            return Err(self.unsupported(node, "a FOR control variable that is not an integer"));
        };
        let mut expressions = node.children().filter(|child| is_expression(child.kind()));
        let (Some(start), Some(end)) = (expressions.next(), expressions.next()) else {
            return Err(self.unsupported(node, "FOR without its bounds"));
        };
        let step = expressions.next();
        // The bounds and the step are evaluated once, in source order (specification 10 §5.3).
        let start = self.scalar_to(&start, place.ty)?;
        let start = self.bind(&start);
        let end = self.scalar_to(&end, place.ty)?;
        let end = self.bind(&end);
        let step = match step {
            Some(step) => {
                let value = self.scalar_to(&step, place.ty)?;
                self.bind(&value)
            }
            None => self.bind(&format!("1{}", elem.rust())),
        };
        self.line(&format!(
            "if {step} == 0 {{ return Err(fault(cx, abi::FaultCode::ForStepZero, 0, 0, 0)); }}"
        ));
        // A release image holds the control variable in a local when nothing in the body but the
        // loop can write it, and writes it at the start of each pass, so the variable holds the
        // pass's value wherever the scan can end; a debug image's debugger can write it at any
        // statement (§6.6).
        let held = if !self.hooks && self.holds_control(node, &call::ident(&control)) {
            let local = self.temp();
            self.line(&format!("let mut {local} = {start};"));
            Some(local)
        } else {
            self.store_scalar(&place, &start);
            None
        };
        self.labels += 1;
        let exit = format!("'l{}", self.labels);
        let next = format!("'n{}", self.labels);
        self.line(&format!("{exit}: loop {{"));
        self.indent += 1;
        if let Some(local) = &held {
            self.store_scalar(&place, local);
            self.held.push((place.expr.clone(), local.clone()));
        }
        self.line("tick(cx)?;");
        let current = self.load(&place);
        let ended = if elem.is_signed() {
            format!("({step} > 0 && {current} > {end}) || ({step} < 0 && {current} < {end})")
        } else {
            format!("{current} > {end}")
        };
        self.line(&format!("if {ended} {{ break; }}"));
        self.line(&format!("{next}: {{"));
        self.indent += 1;
        self.loops.push(LoopLabels {
            exit: exit.clone(),
            next,
        });
        let body = node
            .children()
            .filter(|child| super::front::is_statement(child.kind()));
        let result = self.statements(body);
        self.loops.pop();
        result?;
        self.indent -= 1;
        self.line("}");
        // The increment that would leave the type is a fault, not a wrapped value (specification
        // 10 §5.3).
        let current = self.load(&place);
        match held {
            Some(local) => {
                self.line(&format!("{local} = lib::add(cx, {current}, {step})?;"));
                self.held.pop();
            }
            None => {
                let advanced = self.bind(&format!("lib::add(cx, {current}, {step})?"));
                self.store_scalar(&place, &advanced);
            }
        }
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    fn while_statement(&mut self, node: &SyntaxNode) -> Result<(), NativeError> {
        let Some(condition) = node.children().next() else {
            return Err(self.unsupported(node, "WHILE without a condition"));
        };
        self.labels += 1;
        let exit = format!("'l{}", self.labels);
        let next = format!("'n{}", self.labels);
        self.line(&format!("{exit}: loop {{"));
        self.indent += 1;
        self.line("tick(cx)?;");
        let test = self.condition(&condition)?;
        self.line(&format!("if !{test} {{ break; }}"));
        self.line(&format!("{next}: {{"));
        self.indent += 1;
        self.loops.push(LoopLabels { exit, next });
        let result = self.statements(node.children().skip(1));
        self.loops.pop();
        result?;
        self.indent -= 1;
        self.line("}");
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    fn repeat_statement(&mut self, node: &SyntaxNode) -> Result<(), NativeError> {
        let children: Vec<SyntaxNode> = node.children().collect();
        let Some((condition, body)) = children.split_last() else {
            return Err(self.unsupported(node, "REPEAT without UNTIL"));
        };
        self.labels += 1;
        let exit = format!("'l{}", self.labels);
        let next = format!("'n{}", self.labels);
        self.line(&format!("{exit}: loop {{"));
        self.indent += 1;
        self.line("tick(cx)?;");
        self.line(&format!("{next}: {{"));
        self.indent += 1;
        self.loops.push(LoopLabels { exit, next });
        let result = self.statements(body.iter().cloned());
        self.loops.pop();
        result?;
        self.indent -= 1;
        self.line("}");
        let test = self.condition(condition)?;
        self.line(&format!("if {test} {{ break; }}"));
        self.indent -= 1;
        self.line("}");
        Ok(())
    }

    fn return_statement(&mut self, node: &SyntaxNode) -> Result<(), NativeError> {
        // `RETURN expr;` assigns the result, then returns (specification 06, DEV-022).
        if let Some(value) = node.children().next() {
            let Some(result) = self.result.clone() else {
                return Err(self.unsupported(&value, "RETURN with a value without a result"));
            };
            self.store(&result, &value)?;
        }
        let exit = self.exit.clone();
        self.line(&exit);
        Ok(())
    }
}

/// Whether `kind` is an expression's node.
pub(super) fn is_expression(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::NameRef
            | SyntaxKind::Literal
            | SyntaxKind::BinaryExpr
            | SyntaxKind::UnaryExpr
            | SyntaxKind::ParenExpr
            | SyntaxKind::CallExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::FieldExpr
            | SyntaxKind::DerefExpr
            | SyntaxKind::AddrExpr
            | SyntaxKind::SizeOfExpr
            | SyntaxKind::ThisExpr
            | SyntaxKind::SuperExpr
    )
}

/// Whether `kind` is one of the elementary scalar kinds.
pub(super) fn is_scalar(kind: &Kind) -> bool {
    matches!(
        kind,
        Kind::Elem(_) | Kind::Enum { .. } | Kind::Subrange { .. }
    )
}
