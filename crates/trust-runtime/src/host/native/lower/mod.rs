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
mod expr;
mod functions;
mod object;
mod strings;

pub(super) use call::{ident as ident_of, parameter};
pub(super) use expr::byte_string;

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
    /// The functions and methods the body calls, which the crate must hold too.
    pub(super) needs: Vec<Need>,
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
            needs: Vec::new(),
        }
    }

    /// The Rust written so far.
    pub(super) fn finish(self) -> (String, Vec<u32>) {
        (self.out, self.statements)
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

    /// The type trust-hir gave `node`.
    pub(super) fn type_of(&mut self, node: &SyntaxNode) -> Result<Ty, NativeError> {
        let Some(hir) = self.src.type_of(node) else {
            return Err(self.unsupported(node, "an expression without a type"));
        };
        let symbols = self.src.analysis.symbols.clone();
        self.model.types.of_hir(&symbols, hir)
    }

    /// Declares a local of the frame at its type's zero value; `frame_init` gives it its starting
    /// value.
    pub(super) fn declare(&mut self, name: &SmolStr, ty: Ty) -> String {
        let rust = rust_field(name);
        let storage = self.info(ty).rust.clone();
        self.line(&format!(
            "let {rust}: {storage} = <{storage} as Plain>::ZEROED;"
        ));
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
            return Err(self.unsupported(node, "the assignment attempt ?="));
        }
        if self.string_element_write(&target, &value)? || self.property_write(&target, &value)? {
            return Ok(());
        }
        let place = self.place(&target)?;
        self.store(&place, &value)
    }

    /// Stores the value of `value` into `place`, converted to the place's type as the assignment
    /// converts it.
    pub(super) fn store(&mut self, place: &Place, value: &SyntaxNode) -> Result<(), NativeError> {
        let info = self.info(place.ty).clone();
        match &info.kind {
            Kind::Str(_) => self.store_text(place, value),
            Kind::WStr(_) => Err(self.unsupported(value, "WSTRING values")),
            Kind::Ref { target } => {
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
            Kind::Array { .. } | Kind::Struct { .. } | Kind::Fb { .. } | Kind::StdFb(_) => {
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

    /// Reads the scalar in `place` as a Rust value of its type.
    pub(super) fn load(&self, place: &Place) -> String {
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
        self.store_scalar(&place, &start);
        self.labels += 1;
        let exit = format!("'l{}", self.labels);
        let next = format!("'n{}", self.labels);
        self.line(&format!("{exit}: loop {{"));
        self.indent += 1;
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
        let advanced = self.bind(&format!("lib::add(cx, {current}, {step})?"));
        self.store_scalar(&place, &advanced);
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
        if let Some(value) = node.children().next() {
            return Err(self.unsupported(&value, "RETURN with a value"));
        }
        let exit = self.exit.clone();
        self.line(&exit);
        Ok(())
    }

    /// The place `node` names: a variable, a member, an element, `THIS`; a property's value read
    /// into a temporary.
    pub(super) fn place(&mut self, node: &SyntaxNode) -> Result<Place, NativeError> {
        if let Some(place) = self.object_place(node) {
            return place;
        }
        match node.kind() {
            SyntaxKind::ParenExpr => match node.children().next() {
                Some(inner) => self.place(&inner),
                None => Err(self.unsupported(node, "an empty parenthesis")),
            },
            SyntaxKind::NameRef => {
                if let Some(place) = self.property_read(node)? {
                    return Ok(place);
                }
                let name = call::ident(node);
                self.name_place(node, &name)
            }
            SyntaxKind::FieldExpr => {
                if let Some(place) = self.property_read(node)? {
                    return Ok(place);
                }
                self.field_place(node)
            }
            SyntaxKind::IndexExpr => self.index_place(node),
            SyntaxKind::DerefExpr => self.deref_place(node),
            SyntaxKind::CallExpr => self.result_instance(node),
            other => Err(self.unsupported(node, format!("a {other:?} as a place"))),
        }
    }

    /// The instance `value` names when a reference to an instance of type `target` takes it:
    /// `THIS`, `SUPER` or a value of a function block's type; `None` for a reference value.
    fn instance_source(
        &mut self,
        target: Ty,
        value: &SyntaxNode,
    ) -> Result<Option<Place>, NativeError> {
        if !self.info(target).kind.is_instance() {
            return Ok(None);
        }
        if let Some(place) = self.object_place(value) {
            return place.map(Some);
        }
        if self
            .type_of(value)
            .is_ok_and(|ty| self.info(ty).kind.is_instance())
        {
            return self.place(value).map(Some);
        }
        Ok(None)
    }

    /// The instance the result of call `node` names, a call of a function block's type: the
    /// receiver of a chained call `b.SetVolume(10).Build()` (§6.4).
    fn result_instance(&mut self, node: &SyntaxNode) -> Result<Place, NativeError> {
        let ty = self.type_of(node)?;
        if !self.info(ty).kind.is_instance() {
            return Err(self.unsupported(node, "a call as a place"));
        }
        let reference = self.model.types.reference(ty);
        let result = self.call_aggregate(node, reference)?;
        Ok(self.dereference(&format!("{}.get()", result.expr), ty))
    }

    /// `reference^`: the variable a reference names, checked as specification 44 §7.3 says.
    fn deref_place(&mut self, node: &SyntaxNode) -> Result<Place, NativeError> {
        let Some(reference) = node.children().next() else {
            return Err(self.unsupported(node, "a dereference without its reference"));
        };
        let ty = self.type_of(&reference)?;
        let Kind::Ref { target } = self.info(ty).kind else {
            return Err(self.unsupported(node, "a dereference of a value that is no reference"));
        };
        let value = self.reference(&reference)?;
        Ok(self.dereference(&value, target))
    }

    /// The variable of type `target` the reference value `reference` names, bound to a temporary.
    pub(super) fn dereference(&mut self, reference: &str, target: Ty) -> Place {
        let storage = self.info(target).rust.clone();
        let expr = self.bind(&format!("w.areas.deref::<{storage}>(cx, &{reference})?"));
        Place {
            expr,
            ty: target,
            is_ref: true,
        }
    }

    /// The variable `name` as `node` names it: the frame's own, the instance's, a global.
    pub(super) fn name_place(
        &mut self,
        node: &SyntaxNode,
        name: &str,
    ) -> Result<Place, NativeError> {
        if let Some(local) = self
            .locals
            .iter()
            .rev()
            .find(|local| local.name.eq_ignore_ascii_case(name))
        {
            return Ok(Place {
                expr: local.rust.clone(),
                ty: local.ty,
                is_ref: local.is_ref,
            });
        }
        match self.frame {
            Frame::Program(index) => {
                let program = &self.model.program_types[index];
                if let Some((member, retained)) = program.member(name) {
                    let owner = if retained { "mr" } else { "me" };
                    return Ok(Place {
                        expr: format!("{owner}.{}", member.field()),
                        ty: member.ty,
                        is_ref: false,
                    });
                }
            }
            Frame::FunctionBlock(ty) | Frame::Method(ty) => {
                if let Some(member) = self
                    .info(ty)
                    .members()
                    .iter()
                    .find(|member| member.name.eq_ignore_ascii_case(name))
                    .cloned()
                {
                    // A `VAR_IN_OUT` is the variable its stored reference names (§6.4).
                    if member.flags & trust_runtime_core::native::image::SYMBOL_IN_OUT != 0
                        && let Kind::Ref { target } = self.info(member.ty).kind
                    {
                        let reference = format!("me.{}.get()", member.field());
                        return Ok(self.dereference(&reference, target));
                    }
                    return Ok(Place {
                        expr: format!("me.{}", member.field()),
                        ty: member.ty,
                        is_ref: false,
                    });
                }
            }
            Frame::Function => {}
        }
        if let Some(global) = self.model.global(name) {
            let area = if global.retained { "w.r" } else { "w.g" };
            return Ok(Place {
                expr: format!("{area}.{}", rust_field(&global.name)),
                ty: global.ty,
                is_ref: false,
            });
        }
        Err(self.unsupported(node, format!("the name {name}")))
    }

    fn field_place(&mut self, node: &SyntaxNode) -> Result<Place, NativeError> {
        let mut children = node.children();
        let (Some(base), Some(member)) = (children.next(), children.next()) else {
            return Err(self.unsupported(node, "a member access without a member"));
        };
        let name = call::ident(&member);
        if self.namespace_qualified(node) {
            return self.name_place(node, &name);
        }
        let base = self.place(&base)?;
        let info = self.info(base.ty).clone();
        let members = match &info.kind {
            Kind::Struct { members } | Kind::Fb { members, .. } => members.clone(),
            Kind::StdFb(fb) => self.model.types.std_fb_members(*fb),
            _ => return Err(self.unsupported(node, "a member of a value without members")),
        };
        let Some(member) = members
            .iter()
            .find(|member| member.name.eq_ignore_ascii_case(&name))
        else {
            return Err(self.unsupported(node, format!("the member {name}")));
        };
        let expr = format!("{}.{}", base.expr, member.field());
        Ok(Place {
            expr,
            ty: member.ty,
            is_ref: false,
        })
    }

    /// Whether `node` is a name qualified by a namespace (`Control.LoopMode`), which trust-hir
    /// resolved to a symbol of its own.
    fn namespace_qualified(&self, node: &SyntaxNode) -> bool {
        let Some(symbol) = self.src.symbol_of(node) else {
            return false;
        };
        let symbols = &self.src.analysis.symbols;
        symbols
            .get(symbol)
            .and_then(|symbol| symbol.parent)
            .and_then(|parent| symbols.get(parent))
            .is_some_and(|parent| matches!(parent.kind, trust_hir::SymbolKind::Namespace))
    }

    /// `array[i, j]`: each index checked against its dimension's bounds (§6.5), one nesting of
    /// Rust arrays per dimension.
    fn index_place(&mut self, node: &SyntaxNode) -> Result<Place, NativeError> {
        let mut children = node.children();
        let Some(base) = children.next() else {
            return Err(self.unsupported(node, "an index without an array"));
        };
        let array = self.place(&base)?;
        let indices: Vec<SyntaxNode> = children.collect();
        let info = self.info(array.ty).clone();
        let Kind::Array { elem, dims } = &info.kind else {
            return Err(self.unsupported(node, "an index of a value that is no array"));
        };
        if indices.len() != dims.len() {
            return Err(self.unsupported(node, "an index list unlike the array's dimensions"));
        }
        let lint = self.model.types.elem(Elem::LInt);
        let mut current = array.reference();
        for (index, (lower, _)) in indices.iter().zip(dims) {
            let value = self.scalar_to(index, lint)?;
            current = self.bind(&format!("lib::at(cx, {current}, {value}, {lower})?"));
        }
        Ok(Place {
            expr: current,
            ty: *elem,
            is_ref: true,
        })
    }
}

/// The program library's and the interface's Rust names every generated crate uses.
pub(super) const PRELUDE: &str = "use core::cell::Cell;
use trust_plc_abi::{self as abi, plain_struct, Cx, Fault, Plain, Str, WStr};
use trust_plc_abi::module::{fault, run, stmt as st, tick, Areas};
use trust_plc_lib as lib;

type R<T> = Result<T, Fault>;

/// A value copied into a place of its type: a scalar, a string, an array, a structure.
trait Assign {
    fn assign(&self, source: &Self);
}

impl<T: Copy> Assign for Cell<T> {
    #[inline]
    fn assign(&self, source: &Self) {
        self.set(source.get());
    }
}

impl<const N: usize> Assign for Str<N> {
    #[inline]
    fn assign(&self, source: &Self) {
        lib::assign(self.view(), source.view());
    }
}

impl<T: Assign, const N: usize> Assign for [T; N] {
    #[inline]
    fn assign(&self, source: &Self) {
        for (target, value) in self.iter().zip(source.iter()) {
            target.assign(value);
        }
    }
}

/// Whether two references or interface values name the same variable or instance, whatever type
/// each was taken as (an instance of a derived type as its base spans less).
#[inline]
fn same_target(a: abi::RefValue, b: abi::RefValue) -> bool {
    a.kind == b.kind
        && a.area == b.area
        && a.char_index == b.char_index
        && a.offset == b.offset
        && a.generation == b.generation
}
";

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
