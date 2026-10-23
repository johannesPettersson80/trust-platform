//! Calls (IEC §6.6.1.4, §6.6.2, §6.6.3.4; specification 44 §6.4, §6.8): functions and methods as
//! direct Rust calls with their inputs by value, function blocks by writing the inputs, running
//! the body and reading the outputs, the standard functions inline or through the library.

use smol_str::SmolStr;
use trust_hir::symbols::ParamDirection;
use trust_hir::SymbolKind;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::expr::Text;
use super::object::Receiver;
use super::{Lowerer, Place};
use crate::native::types::{mangle, rust_field, Kind, Ty};
use crate::native::NativeError;
use crate::program_model::{FunctionDef, Param};

/// One argument of a call.
pub(super) struct Arg {
    /// The formal parameter's name, or `None` for a positional argument.
    pub(super) name: Option<String>,
    /// Whether the argument receives an output (`=>`).
    pub(super) output: bool,
    pub(super) value: SyntaxNode,
}

/// What a call calls.
enum Callee<'a> {
    Function(&'a FunctionDef),
    FunctionBlock(Place),
    /// `SUPER()`: the base's own body on the instance's part of the base (IEC §6.6.7.2.9, Table 48
    /// 10c), never an override.
    BaseBody(Place),
    /// A method, by its name, of an instance or through an interface reference.
    Method(Receiver, String),
    Standard(String),
}

/// The outputs of a call: each output's place in the caller and the variable it is copied to after
/// the call.
pub(super) type Outputs = Vec<(Place, SyntaxNode)>;

/// What a call's value is wanted as.
pub(super) enum Want {
    Nothing,
    Scalar(Ty),
    Aggregate,
}

/// A call's value.
pub(super) enum Called {
    Nothing,
    Scalar(String),
    Place(Place),
    Text(Vec<Text>),
}

/// The name `node` holds: its first token that is not whitespace or a comment, an identifier or
/// a keyword used as a name (`TIME()`); a local's `#` prefix of the Siemens profile is no part of
/// it (`#count`, specification 02).
pub(in crate::native) fn ident(node: &SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia() && token.kind() != SyntaxKind::Hash)
        .map(|token| token.text().to_string())
        .unwrap_or_default()
}

fn arguments(node: &SyntaxNode) -> Vec<Arg> {
    let Some(list) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::ArgList)
    else {
        return Vec::new();
    };
    list.children()
        .filter(|child| child.kind() == SyntaxKind::Arg)
        .filter_map(|arg| {
            let name = arg
                .children()
                .find(|child| child.kind() == SyntaxKind::Name)
                .map(|name| ident(&name));
            let output = arg
                .children_with_tokens()
                .any(|element| element.kind() == SyntaxKind::Arrow);
            let value = arg
                .children()
                .find(|child| child.kind() != SyntaxKind::Name)?;
            Some(Arg {
                name,
                output,
                value,
            })
        })
        .collect()
}

impl<'a> Lowerer<'_, 'a> {
    /// The call `node`'s scalar value of type `ty`.
    pub(super) fn call_scalar(&mut self, node: &SyntaxNode, ty: Ty) -> Result<String, NativeError> {
        match self.call(node, Want::Scalar(ty))? {
            Called::Scalar(value) => Ok(value),
            _ => Err(self.unsupported(node, "a call without a scalar value where one is needed")),
        }
    }

    /// The call `node`'s text.
    pub(super) fn call_text(&mut self, node: &SyntaxNode) -> Result<Vec<Text>, NativeError> {
        match self.call(node, Want::Aggregate)? {
            Called::Text(parts) => Ok(parts),
            Called::Place(place) => self.text_of(node, &place),
            _ => Err(self.unsupported(node, "a call without a text where one is needed")),
        }
    }

    /// The call `node`'s structure or array of type `ty`.
    pub(super) fn call_aggregate(
        &mut self,
        node: &SyntaxNode,
        ty: Ty,
    ) -> Result<Place, NativeError> {
        match self.call(node, Want::Aggregate)? {
            Called::Place(place) if place.ty == ty => Ok(place),
            _ => Err(self.unsupported(node, "a call without a value of this type")),
        }
    }

    /// The call `node` for what it does.
    pub(super) fn call_statement(&mut self, node: &SyntaxNode) -> Result<(), NativeError> {
        self.call(node, Want::Nothing).map(|_| ())
    }

    fn call(&mut self, node: &SyntaxNode, want: Want) -> Result<Called, NativeError> {
        let Some(target) = node.children().next() else {
            return Err(self.unsupported(node, "a call without a callee"));
        };
        let args = arguments(node);
        match self.callee(&target)? {
            Callee::Function(def) => self.function_call(node, def, args, want),
            Callee::FunctionBlock(instance) => {
                self.function_block_call(node, &instance, args, false)?;
                Ok(Called::Nothing)
            }
            Callee::BaseBody(instance) => {
                self.function_block_call(node, &instance, args, true)?;
                Ok(Called::Nothing)
            }
            Callee::Method(receiver, name) => self.member_call(node, receiver, &name, args, want),
            Callee::Standard(name) => self.standard(node, &name, args, want),
        }
    }

    /// The qualified name of the symbol `node` resolved to: its namespaces and its own name.
    fn qualified(&self, node: &SyntaxNode) -> Option<(String, SymbolKind)> {
        let symbols = &self.src.analysis.symbols;
        let symbol = symbols.get(self.src.symbol_of(node)?)?;
        let mut name = symbol.name.to_string();
        let mut parent = symbol.parent;
        while let Some(id) = parent {
            let Some(owner) = symbols.get(id) else {
                break;
            };
            if matches!(owner.kind, SymbolKind::Namespace) {
                name = format!("{}.{name}", owner.name);
            }
            parent = owner.parent;
        }
        Some((name, symbol.kind.clone()))
    }

    fn callee(&mut self, target: &SyntaxNode) -> Result<Callee<'a>, NativeError> {
        match target.kind() {
            SyntaxKind::NameRef => {
                let name = ident(target);
                if let Ok(place) = self.name_place(target, &name)
                    && matches!(self.info(place.ty).kind, Kind::Fb { .. } | Kind::StdFb(_))
                {
                    return Ok(Callee::FunctionBlock(place));
                }
                let resolved = self.qualified(target);
                if let Some((qualified, SymbolKind::Function { .. })) = &resolved
                    && let Some(def) = self.model.function(qualified)
                {
                    return Ok(Callee::Function(def));
                }
                // A method of the instance itself, called without `THIS` (IEC §6.6.5.4).
                if let Some((_, SymbolKind::Method { .. })) = &resolved {
                    let this = self.this_place(target)?;
                    return Ok(Callee::Method(
                        Receiver::Instance {
                            place: this,
                            exact: false,
                        },
                        name,
                    ));
                }
                if let Some(def) = self.model.function(&name) {
                    return Ok(Callee::Function(def));
                }
                Ok(Callee::Standard(name.to_ascii_uppercase()))
            }
            SyntaxKind::FieldExpr => {
                if let Some((qualified, kind)) = self.qualified(target) {
                    match kind {
                        SymbolKind::Function { .. } => {
                            if let Some(def) = self.model.function(&qualified) {
                                return Ok(Callee::Function(def));
                            }
                        }
                        SymbolKind::Method { .. } => {
                            let Some(base) = target.children().next() else {
                                return Err(
                                    self.unsupported(target, "a method without its instance")
                                );
                            };
                            let member =
                                ident(&target.children().nth(1).unwrap_or_else(|| target.clone()));
                            let receiver = self.receiver(&base)?;
                            return Ok(Callee::Method(receiver, member));
                        }
                        _ => {}
                    }
                }
                let place = self.place(target)?;
                if matches!(self.info(place.ty).kind, Kind::Fb { .. } | Kind::StdFb(_)) {
                    return Ok(Callee::FunctionBlock(place));
                }
                Err(self.unsupported(target, "a call of a value that is not callable"))
            }
            // An instance that is an element of an array (`Pumps[i](...)`) or that a reference
            // names (`ref^(...)`).
            SyntaxKind::IndexExpr | SyntaxKind::DerefExpr | SyntaxKind::ParenExpr => {
                let place = self.place(target)?;
                if matches!(self.info(place.ty).kind, Kind::Fb { .. } | Kind::StdFb(_)) {
                    return Ok(Callee::FunctionBlock(place));
                }
                Err(self.unsupported(target, "a call of a value that is not callable"))
            }
            SyntaxKind::SuperExpr => Ok(Callee::BaseBody(self.super_place(target)?)),
            _ => Err(self.unsupported(target, "this callee")),
        }
    }

    /// Binds `args` to `params` as trust-hir binds them: positional arguments in the order of the
    /// parameters, formal ones by name (IEC §6.6.1.4).
    pub(super) fn bind_arguments<'p>(
        &self,
        node: &SyntaxNode,
        params: &'p [Param],
        args: Vec<Arg>,
    ) -> Result<Vec<(&'p Param, Option<Arg>)>, NativeError> {
        let mut bound: Vec<(&'p Param, Option<Arg>)> = params
            .iter()
            .filter(|param| !is_execution(param))
            .map(|param| (param, None))
            .collect();
        let mut next = 0;
        for arg in args {
            let slot = match &arg.name {
                Some(name)
                    if name.eq_ignore_ascii_case("EN") || name.eq_ignore_ascii_case("ENO") =>
                {
                    return Err(self.unsupported(node, "EN and ENO"));
                }
                Some(name) => bound
                    .iter()
                    .position(|(param, _)| param.name.eq_ignore_ascii_case(name)),
                None => {
                    next += 1;
                    Some(next - 1).filter(|slot| *slot < bound.len())
                }
            };
            let Some(slot) = slot else {
                return Err(self.unsupported(&arg.value, "an argument that names no parameter"));
            };
            bound[slot].1 = Some(arg);
        }
        Ok(bound)
    }

    /// The Rust arguments of a function's or a method's parameters, and the outputs to copy to
    /// their variables after the call: each output's place in the caller and its variable.
    pub(super) fn inputs(
        &mut self,
        node: &SyntaxNode,
        bound: &[(&Param, Option<Arg>)],
    ) -> Result<(Vec<String>, Outputs), NativeError> {
        let mut values = Vec::with_capacity(bound.len());
        let mut outputs = Vec::new();
        for (param, arg) in bound {
            let ty = self.model.ty(param.type_id)?;
            let info = self.info(ty).clone();
            if param.direction == ParamDirection::InOut
                || (param.direction == ParamDirection::In && info.kind.is_instance())
            {
                // A `VAR_IN_OUT` is the caller's variable itself (IEC §6.6.1.2.2), and a function
                // block instance given as an input is the caller's instance, whose outputs the
                // callee reads (§6.6.3.4.1, Table 42 feature 9a), as the runtime passes it; an
                // instance of a derived type as its base (§6.6.8.3).
                let Some(arg) = arg else {
                    return Err(self.unsupported(
                        node,
                        format!("the parameter {} without its variable", param.name),
                    ));
                };
                let place = self.place(&arg.value)?;
                values.push(self.as_type(&arg.value, &place, ty)?);
                continue;
            }
            if param.direction == ParamDirection::Out {
                // The output's place in the caller, copied to its variable after the call.
                let temp = self.temp();
                let storage = info.rust.clone();
                self.line(&format!(
                    "let {temp}: {storage} = <{storage} as Plain>::ZEROED;"
                ));
                values.push(format!("&{temp}"));
                if let Some(arg) = arg {
                    if !arg.output {
                        return Err(self.unsupported(
                            &arg.value,
                            format!("the output {} given without =>", param.name),
                        ));
                    }
                    outputs.push((
                        Place {
                            expr: temp,
                            ty,
                            is_ref: false,
                        },
                        arg.value.clone(),
                    ));
                }
                continue;
            }
            let value = match (&info.kind, arg) {
                (Kind::Str(capacity), Some(arg)) => {
                    let parts = self.text(&arg.value)?;
                    self.text_temp(&parts, *capacity)
                }
                (Kind::Interface, Some(arg)) => {
                    let reference = self.interface_value(&arg.value)?;
                    let temp = self.bind(&format!("Cell::new({reference})"));
                    format!("&{temp}")
                }
                (Kind::Ref { target }, Some(arg)) => {
                    let reference = self.reference_to(&arg.value, Some(*target))?;
                    let temp = self.bind(&format!("Cell::new({reference})"));
                    format!("&{temp}")
                }
                (kind, Some(arg)) if super::is_scalar(kind) => self.scalar_to(&arg.value, ty)?,
                (_, Some(arg)) => self.aggregate(&arg.value, ty)?.reference(),
                // An input the call leaves out takes its default (IEC §6.6.1.2.2).
                (kind, None) if super::is_scalar(kind) => super::super::frame_init::omitted_scalar(
                    self.model,
                    ty,
                    param.type_id,
                    param.default.as_ref(),
                    &param.name,
                )?,
                (_, None) => {
                    let temp = self.temp();
                    let storage = info.rust.clone();
                    self.line(&format!(
                        "let {temp}: {storage} = <{storage} as Plain>::ZEROED;"
                    ));
                    let bytes = super::super::frame_init::starting_bytes(
                        self.model,
                        ty,
                        param.type_id,
                        param.default.as_ref(),
                        &param.name,
                    )?;
                    let mut lines = Vec::new();
                    super::super::frame_init::statements(
                        self.model, &temp, ty, &bytes, 0, &mut lines,
                    )?;
                    for line in lines {
                        self.line(&line);
                    }
                    format!("&{temp}")
                }
            };
            values.push(value);
        }
        Ok((values, outputs))
    }

    /// Copies each output's place to its variable, after the call (IEC §6.6.1.4).
    pub(super) fn copy_outputs(&mut self, outputs: Outputs) -> Result<(), NativeError> {
        for (place, target) in outputs {
            let destination = self.place(&target)?;
            let kind = self.info(place.ty).kind.clone();
            match kind {
                kind if super::is_scalar(&kind) => {
                    let value = self.load(&place);
                    let value = self.convert(&target, &value, place.ty, destination.ty)?;
                    self.store_scalar(&destination, &value);
                }
                Kind::Str(_) => self.line(&format!(
                    "lib::assign(({}).view(), ({}).view());",
                    destination.reference(),
                    place.reference()
                )),
                _ if place.ty == destination.ty => self.line(&format!(
                    "Assign::assign({}, {});",
                    destination.reference(),
                    place.reference()
                )),
                _ => {
                    return Err(self.unsupported(&target, "an output of another type"));
                }
            }
        }
        Ok(())
    }

    /// The call of a function, or of a method on `receiver`, returning `ret`: its value as `want`
    /// needs it. The Rust arguments are the context and the areas, the receiver, the place of a
    /// result that is no scalar, then the parameters.
    pub(super) fn emit_call(
        &mut self,
        node: &SyntaxNode,
        function: &str,
        receiver: Option<String>,
        mut arguments: Vec<String>,
        ret: Option<trust_hir::TypeId>,
        want: &Want,
    ) -> Result<Called, NativeError> {
        let mut leading = vec!["cx".to_string(), "w".to_string()];
        leading.extend(receiver);
        let Some(ret) = crate::native::emit::return_type(self.model, ret)? else {
            arguments.splice(0..0, leading);
            self.call_line(function, &arguments.join(", "));
            return Ok(Called::Nothing);
        };
        let info = self.info(ret).clone();
        if super::is_scalar(&info.kind) {
            arguments.splice(0..0, leading);
            let value = self.call_value(function, &arguments.join(", "));
            return match want {
                Want::Scalar(target) => {
                    let converted = self.convert(node, &value, ret, *target)?;
                    Ok(Called::Scalar(converted))
                }
                _ => Ok(Called::Scalar(value)),
            };
        }
        let temp = self.temp();
        let storage = info.rust.clone();
        self.line(&format!(
            "let {temp}: {storage} = <{storage} as Plain>::ZEROED;"
        ));
        leading.push(format!("&{temp}"));
        arguments.splice(0..0, leading);
        self.call_line(function, &arguments.join(", "));
        Ok(Called::Place(Place {
            expr: temp,
            ty: ret,
            is_ref: false,
        }))
    }

    /// The call of the generated function `function` with `arguments` as a statement, its fault
    /// passed on.
    fn call_line(&mut self, function: &str, arguments: &str) {
        let call = self.generated_call(function, arguments);
        self.line(&format!("{call}?;"));
    }

    /// The value of the call of the generated function `function` with `arguments`, bound to a
    /// temporary, as [`Self::call_line`] calls it.
    fn call_value(&mut self, function: &str, arguments: &str) -> String {
        let call = self.generated_call(function, arguments);
        self.bind(&format!("{call}?"))
    }

    /// The call of `function` with `arguments`, which are values already computed: a recursive call
    /// through the library's `recurse`, which takes one from the call depth before it runs and
    /// gives it back when it returns, with a fault too (specification 44 §6.4).
    fn generated_call(&mut self, function: &str, arguments: &str) -> String {
        self.called.insert(function.to_string());
        if self.recursive.contains(function) {
            format!("recurse(cx, |cx| {function}({arguments}))")
        } else {
            format!("{function}({arguments})")
        }
    }

    fn function_call(
        &mut self,
        node: &SyntaxNode,
        def: &'a FunctionDef,
        args: Vec<Arg>,
        want: Want,
    ) -> Result<Called, NativeError> {
        let bound = self.bind_arguments(node, &def.params, args)?;
        let (arguments, outputs) = self.inputs(node, &bound)?;
        super::Need::Function(def.name.clone()).add_to(&mut self.needs);
        let function = format!("f_{}", mangle(&def.name));
        let called = self.emit_call(
            node,
            &function,
            None,
            arguments,
            Some(def.return_type),
            &want,
        )?;
        self.copy_outputs(outputs)?;
        Ok(called)
    }

    /// `instance(inputs, outputs)`: the given inputs written, the body run, the outputs read
    /// (IEC §6.6.3.4); inputs not given keep their values.
    fn function_block_call(
        &mut self,
        node: &SyntaxNode,
        instance: &Place,
        args: Vec<Arg>,
        exact: bool,
    ) -> Result<(), NativeError> {
        let info = self.info(instance.ty).clone();
        let (members, body) = match &info.kind {
            Kind::Fb { def, members, .. } => {
                let defs = self.model.types.defs;
                // The body of a derived type's instance, which a `VAR_IN_OUT` or a reference of
                // the base's type may name, is bound by its class (IEC §6.6.8.3, §6.6.8.4);
                // `SUPER()` calls the base's own body (`exact`).
                let body = match defs.owner(def) {
                    Some(owner) if !exact && defs.extended_below(owner) => {
                        super::Need::Body(instance.ty).add_to(&mut self.needs);
                        crate::native::dispatch::body_function(self.model, instance.ty)
                    }
                    _ => format!("fb_{}", mangle(def)),
                };
                (members.clone(), Body::User(body))
            }
            Kind::StdFb(fb) => (
                self.model.types.std_fb_members(*fb),
                Body::Library(fb.body()),
            ),
            _ => {
                return Err(
                    self.unsupported(node, "a call of an instance that is no function block")
                )
            }
        };
        use trust_runtime_core::native::image::{SYMBOL_INPUT, SYMBOL_IN_OUT};
        let mut outputs = Vec::new();
        for (position, arg) in args.into_iter().enumerate() {
            let member = match &arg.name {
                Some(name) => members
                    .iter()
                    .find(|member| member.name.eq_ignore_ascii_case(name)),
                None => members
                    .iter()
                    .filter(|member| member.flags & (SYMBOL_INPUT | SYMBOL_IN_OUT) != 0)
                    .nth(position),
            };
            let Some(member) = member else {
                return Err(self.unsupported(
                    &arg.value,
                    "an argument that names no variable of the function block",
                ));
            };
            let place = Place {
                expr: format!("{}.{}", instance.expr, member.field()),
                ty: member.ty,
                is_ref: false,
            };
            if member.flags & SYMBOL_IN_OUT != 0 {
                // The instance keeps a reference to the variable it was called with (§6.4), an
                // instance of a derived type as its base (IEC §6.6.8.3).
                let variable = self.place(&arg.value)?;
                let Kind::Ref { target } = self.info(member.ty).kind else {
                    return Err(self.unsupported(&arg.value, "a VAR_IN_OUT without its type"));
                };
                let variable = self.as_type(&arg.value, &variable, target)?;
                self.line(&format!(
                    "{}.set(w.areas.refer(cx, {variable}, abi::RefValue::VARIABLE)?);",
                    place.expr
                ));
            } else if arg.output {
                outputs.push((place, arg.value));
            } else {
                self.store(&place, &arg.value)?;
            }
        }
        match body {
            Body::User(function) => {
                self.call_line(&function, &format!("cx, w, {}", instance.reference()));
            }
            Body::Library((function, timed)) => {
                let time = if timed { ", cx.now_ns" } else { "" };
                self.line(&format!("{function}({}{time});", instance.reference()));
            }
        }
        for (member, target) in outputs {
            let destination = self.place(&target)?;
            let value = self.load(&member);
            let value = self.convert(&target, &value, member.ty, destination.ty)?;
            self.store_scalar(&destination, &value);
        }
        Ok(())
    }
}

/// The body of a function block: generated, or the library's with whether it takes the time.
enum Body {
    User(String),
    Library((String, bool)),
}

/// Whether `param` is `EN` or `ENO`, which positional arguments skip.
fn is_execution(param: &Param) -> bool {
    param.name.eq_ignore_ascii_case("EN") || param.name.eq_ignore_ascii_case("ENO")
}

/// The Rust name of a function's parameter as the function receives it.
pub(in crate::native) fn parameter(name: &SmolStr) -> String {
    format!("p_{}", rust_field(name))
}
