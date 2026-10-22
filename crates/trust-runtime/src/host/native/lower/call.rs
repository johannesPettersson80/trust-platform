//! Calls (IEC §6.6.1.4, §6.6.2, §6.6.3.4; specification 44 §6.4, §6.8): functions and methods as
//! direct Rust calls with their inputs by value, function blocks by writing the inputs, running
//! the body and reading the outputs, the standard functions inline or through the library.

use smol_str::SmolStr;
use trust_hir::symbols::ParamDirection;
use trust_hir::SymbolKind;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::expr::Text;
use super::{Lowerer, Place};
use crate::native::types::{mangle, rust_field, Elem, Kind, Ty};
use crate::native::NativeError;
use crate::program_model::{FunctionBlockDef, FunctionDef, MethodDef, Param};

/// One argument of a call.
struct Arg {
    /// The formal parameter's name, or `None` for a positional argument.
    name: Option<String>,
    /// Whether the argument receives an output (`=>`).
    output: bool,
    value: SyntaxNode,
}

/// What a call calls.
enum Callee<'a> {
    Function(&'a FunctionDef),
    FunctionBlock(Place),
    Method {
        instance: Place,
        owner: &'a FunctionBlockDef,
        method: &'a MethodDef,
    },
    Standard(String),
}

/// What a call's value is wanted as.
enum Want {
    Nothing,
    Scalar(Ty),
    Aggregate,
}

/// A call's value.
enum Called {
    Nothing,
    Scalar(String),
    Place(Place),
    Text(Vec<Text>),
}

/// The name `node` holds: its first token that is not whitespace or a comment, an identifier or
/// a keyword used as a name (`TIME()`).
pub(in crate::native) fn ident(node: &SyntaxNode) -> String {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())
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
            Called::Place(place) => Ok(vec![Text::Str(place.reference())]),
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
                self.function_block_call(node, &instance, args)?;
                Ok(Called::Nothing)
            }
            Callee::Method {
                instance,
                owner,
                method,
            } => self.method_call(node, &instance, owner, method, args, want),
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
                if let Some((qualified, SymbolKind::Function { .. })) = self.qualified(target)
                    && let Some(def) = self.model.function(&qualified)
                {
                    return Ok(Callee::Function(def));
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
                            let instance = self.place(&base)?;
                            let Kind::Fb { def, .. } = &self.info(instance.ty).kind else {
                                return Err(self.unsupported(
                                    target,
                                    "a method of a value that is no function block",
                                ));
                            };
                            let def_name = def.clone();
                            let Some(owner) = self.model.types.function_block_def(&def_name) else {
                                return Err(self.unsupported(
                                    target,
                                    format!("the function block {def_name}"),
                                ));
                            };
                            let member =
                                ident(&target.children().nth(1).unwrap_or_else(|| target.clone()));
                            let Some(method) = owner
                                .methods
                                .iter()
                                .find(|method| method.name.eq_ignore_ascii_case(&member))
                            else {
                                return Err(
                                    self.unsupported(target, format!("the method {member}"))
                                );
                            };
                            return Ok(Callee::Method {
                                instance,
                                owner,
                                method,
                            });
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
            _ => Err(self.unsupported(target, "this callee")),
        }
    }

    /// Binds `args` to `params` as trust-hir binds them: positional arguments in the order of the
    /// parameters, formal ones by name (IEC §6.6.1.4).
    fn bind_arguments<'p>(
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

    /// The Rust arguments of a function's or a method's inputs.
    fn inputs(
        &mut self,
        node: &SyntaxNode,
        bound: &[(&Param, Option<Arg>)],
    ) -> Result<Vec<String>, NativeError> {
        let mut values = Vec::with_capacity(bound.len());
        for (param, arg) in bound {
            if param.direction != ParamDirection::In {
                return Err(self.unsupported(
                    node,
                    format!("the {:?} parameter {}", param.direction, param.name),
                ));
            }
            let ty = self.model.ty(param.type_id)?;
            let info = self.info(ty).clone();
            let value = match (&info.kind, arg) {
                (Kind::Str(capacity), Some(arg)) => {
                    let parts = self.text(&arg.value)?;
                    self.text_temp(&parts, *capacity)
                }
                (Kind::Str(capacity), None) => {
                    let temp = self.temp();
                    self.line(&format!("let {temp} = Str::<{capacity}>::new();"));
                    format!("&{temp}")
                }
                (kind, Some(arg)) if super::is_scalar(kind) => self.scalar_to(&arg.value, ty)?,
                (_, Some(arg)) => self.aggregate(&arg.value, ty)?.reference(),
                (kind, None) if super::is_scalar(kind) => {
                    if param.default.is_some() {
                        return Err(
                            self.unsupported(node, format!("the default of {}", param.name))
                        );
                    }
                    let elem = info.scalar().unwrap_or(Elem::DInt);
                    if elem == Elem::Bool {
                        "false".to_string()
                    } else {
                        format!("0 as {}", elem.rust())
                    }
                }
                (_, None) => {
                    let temp = self.temp();
                    let storage = info.rust.clone();
                    self.line(&format!(
                        "let {temp}: {storage} = <{storage} as Plain>::ZEROED;"
                    ));
                    format!("&{temp}")
                }
            };
            values.push(value);
        }
        Ok(values)
    }

    /// The call of a function or a method returning `ret`: its value as `want` needs it.
    fn emit_call(
        &mut self,
        node: &SyntaxNode,
        function: &str,
        mut arguments: Vec<String>,
        ret: Option<trust_hir::TypeId>,
        want: &Want,
    ) -> Result<Called, NativeError> {
        let ret = match ret {
            Some(ret)
                if self
                    .model
                    .types
                    .registry()
                    .get(ret)
                    .is_some_and(|ty| !matches!(ty, trust_hir::Type::Void)) =>
            {
                Some(self.model.ty(ret)?)
            }
            _ => None,
        };
        let Some(ret) = ret else {
            arguments.insert(0, "w".into());
            arguments.insert(0, "cx".into());
            self.line(&format!("{function}({})?;", arguments.join(", ")));
            return Ok(Called::Nothing);
        };
        let info = self.info(ret).clone();
        if super::is_scalar(&info.kind) {
            arguments.insert(0, "w".into());
            arguments.insert(0, "cx".into());
            let value = self.bind(&format!("{function}({})?", arguments.join(", ")));
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
        arguments.insert(0, format!("&{temp}"));
        arguments.insert(0, "w".into());
        arguments.insert(0, "cx".into());
        self.line(&format!("{function}({})?;", arguments.join(", ")));
        Ok(Called::Place(Place {
            expr: temp,
            ty: ret,
            is_ref: false,
        }))
    }

    fn function_call(
        &mut self,
        node: &SyntaxNode,
        def: &'a FunctionDef,
        args: Vec<Arg>,
        want: Want,
    ) -> Result<Called, NativeError> {
        let bound = self.bind_arguments(node, &def.params, args)?;
        let arguments = self.inputs(node, &bound)?;
        let need = super::Need::Function(def.name.clone());
        if !self.needs.contains(&need) {
            self.needs.push(need);
        }
        let function = format!("f_{}", mangle(&def.name));
        self.emit_call(node, &function, arguments, Some(def.return_type), &want)
    }

    fn method_call(
        &mut self,
        node: &SyntaxNode,
        instance: &Place,
        owner: &'a FunctionBlockDef,
        method: &'a MethodDef,
        args: Vec<Arg>,
        want: Want,
    ) -> Result<Called, NativeError> {
        let bound = self.bind_arguments(node, &method.params, args)?;
        let mut arguments = self.inputs(node, &bound)?;
        arguments.insert(0, instance.reference());
        let need = super::Need::Method(owner.name.clone(), method.name.clone());
        if !self.needs.contains(&need) {
            self.needs.push(need);
        }
        let function = format!("m_{}__{}", mangle(&owner.name), mangle(&method.name));
        self.emit_call(node, &function, arguments, method.return_type, &want)
    }

    /// `instance(inputs, outputs)`: the given inputs written, the body run, the outputs read
    /// (IEC §6.6.3.4); inputs not given keep their values.
    fn function_block_call(
        &mut self,
        node: &SyntaxNode,
        instance: &Place,
        args: Vec<Arg>,
    ) -> Result<(), NativeError> {
        let info = self.info(instance.ty).clone();
        let (members, body) = match &info.kind {
            Kind::Fb { def, members } => {
                (members.clone(), Body::User(format!("fb_{}", mangle(def))))
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
        let mut outputs = Vec::new();
        for (position, arg) in args.into_iter().enumerate() {
            let member = match &arg.name {
                Some(name) => members
                    .iter()
                    .find(|member| member.name.eq_ignore_ascii_case(name)),
                None => members
                    .iter()
                    .filter(|member| {
                        member.flags & trust_runtime_core::native::image::SYMBOL_INPUT != 0
                    })
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
            if arg.output {
                outputs.push((place, arg.value));
            } else {
                self.store(&place, &arg.value)?;
            }
        }
        match body {
            Body::User(function) => {
                self.line(&format!("{function}(cx, w, {})?;", instance.reference()));
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

    /// A standard function (IEC §6.6.2.5; specification 07): selection, conversions, `CONCAT`,
    /// `TIME`.
    fn standard(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        args: Vec<Arg>,
        want: Want,
    ) -> Result<Called, NativeError> {
        let values: Vec<SyntaxNode> = args.into_iter().map(|arg| arg.value).collect();
        if name == "CONCAT" {
            let mut parts = Vec::new();
            for value in &values {
                parts.extend(self.text(value)?);
            }
            return Ok(Called::Text(parts));
        }
        if name == "TIME" && values.is_empty() {
            return Ok(Called::Scalar("cx.now_ns".into()));
        }
        if name.contains("_TO_") || name.starts_with("TO_") || name == "TRUNC" {
            let ty = self.type_of(node)?;
            let [value] = values.as_slice() else {
                return Err(self.unsupported(node, format!("{name} with other than one input")));
            };
            return self.conversion(node, name, value, ty);
        }
        // A selection's inputs and result take their common type, as the runtime selects
        // (specification 07 §6): the widest input's, the selector of SEL aside.
        let operands = if name == "SEL" {
            values.get(1..).unwrap_or(&[])
        } else {
            &values[..]
        };
        let Some((first, rest)) = operands.split_first() else {
            return Err(self.unsupported(node, format!("{name} without inputs")));
        };
        let mut ty = self.operand_type(first)?;
        for operand in rest {
            let other = self.operand_type(operand)?;
            ty = self.common(node, ty, other)?;
        }
        let mut scalars = Vec::with_capacity(values.len());
        for value in &values {
            let target = if name == "SEL" && scalars.is_empty() {
                self.model.types.elem(Elem::Bool)
            } else {
                ty
            };
            scalars.push(self.scalar_to(value, target)?);
        }
        let Some(elem) = self.info(ty).scalar() else {
            return Err(self.unsupported(node, format!("{name} of values that are not scalars")));
        };
        let value = match (name, scalars.as_slice()) {
            ("LIMIT", [mn, value, mx]) => format!("lib::limit({mn}, {value}, {mx})"),
            ("MAX" | "MIN", [first, rest @ ..]) if !rest.is_empty() => {
                let function = if name == "MAX" {
                    "lib::max"
                } else {
                    "lib::min"
                };
                rest.iter().fold(first.clone(), |acc, next| {
                    format!("{function}({acc}, {next})")
                })
            }
            ("ABS", [value]) if elem.is_signed() => self.bind(&format!("lib::abs(cx, {value})?")),
            ("ABS", [value]) if elem.is_real() => format!("lib::fabs({value})"),
            ("ABS", [value]) => value.clone(),
            ("SEL", [select, first, second]) => {
                format!("(if {select} {{ {second} }} else {{ {first} }})")
            }
            _ => return Err(self.unsupported(node, format!("the standard function {name}"))),
        };
        let value = self.bind(&value);
        match want {
            Want::Scalar(target) => Ok(Called::Scalar(self.convert(node, &value, ty, target)?)),
            _ => Ok(Called::Scalar(value)),
        }
    }

    /// `SRC_TO_DST(value)` of the result type `ty` (IEC §6.6.2.5.2 to §6.6.2.5.7; specification
    /// 07 §2), with the runtime's rounding and faults through the library.
    fn conversion(
        &mut self,
        node: &SyntaxNode,
        name: &str,
        value: &SyntaxNode,
        ty: Ty,
    ) -> Result<Called, NativeError> {
        let from = self.type_of(value)?;
        let source = self.scalar(value)?;
        let Some(src) = self.info(from).scalar() else {
            return Err(self.unsupported(node, format!("{name} of a value that is no scalar")));
        };
        if let Kind::Str(_) = self.info(ty).kind {
            let temp = self.temp();
            self.line(&format!("let {temp} = Str::<20>::new();"));
            if src.is_signed() {
                self.line(&format!(
                    "lib::write_int({temp}.view(), i64::from({source}));"
                ));
            } else if src.is_unsigned() || src.is_bits() {
                self.line(&format!(
                    "lib::write_uint({temp}.view(), u64::from({source}));"
                ));
            } else {
                return Err(self.unsupported(node, format!("{name}: {} as text", src.name())));
            }
            return Ok(Called::Text(vec![Text::Str(format!("&{temp}"))]));
        }
        let Some(dst) = self.info(ty).scalar() else {
            return Err(self.unsupported(node, format!("{name} to a type that is no scalar")));
        };
        let rust = dst.rust();
        let truncate = name == "TRUNC";
        let converted = if src == dst || (src.is_duration() && dst.is_duration()) {
            source
        } else if src == Elem::Bool && (dst.is_integer() || dst.is_bits()) {
            format!("({source} as {rust})")
        } else if src.is_integer() && dst.is_integer() {
            if src.is_signed() {
                self.bind(&format!("lib::sint_to::<{rust}>(cx, i64::from({source}))?"))
            } else {
                self.bind(&format!("lib::uint_to::<{rust}>(cx, u64::from({source}))?"))
            }
        } else if src.is_integer() && dst.is_real() {
            let (function, wide) = if src.is_signed() {
                ("sint", "i64")
            } else {
                ("uint", "u64")
            };
            let bits = if dst == Elem::Real { 32 } else { 64 };
            format!("lib::{function}_to_real{bits}({wide}::from({source}))")
        } else if src.is_real() && dst.is_integer() {
            let rounding = if truncate { "Truncate" } else { "Nearest" };
            self.bind(&format!(
                "lib::real_to_int::<{rust}>(cx, f64::from({source}), lib::Rounding::{rounding})?"
            ))
        } else if src == Elem::Real && dst == Elem::LReal {
            self.bind(&format!("lib::real32_to_real::<f64>(cx, {source})?"))
        } else if src == Elem::LReal && dst == Elem::Real {
            self.bind(&format!("lib::real64_to_real32::<f32>(cx, {source})?"))
        } else if (src.is_bits() || src.is_integer()) && (dst.is_bits() || dst.is_integer()) {
            // A bit string converts by its bits: cut to the destination's width or extended with
            // zeros, read with the destination's sign (specification 07 §2).
            format!("({source} as {rust})")
        } else if src == Elem::DWord && dst == Elem::Real {
            self.bind(&format!("lib::bits_to_real32(cx, {source})?"))
        } else if src == Elem::LWord && dst == Elem::LReal {
            self.bind(&format!("lib::bits_to_real64(cx, {source})?"))
        } else if src == Elem::Real && dst == Elem::DWord
            || src == Elem::LReal && dst == Elem::LWord
        {
            format!("{source}.to_bits()")
        } else if src.is_duration() && dst == Elem::DWord {
            self.bind(&format!("lib::time_to_dword(cx, {source})?"))
        } else if src == Elem::DWord && dst.is_duration() {
            format!("lib::dword_to_time({source})")
        } else {
            return Err(self.unsupported(node, format!("{name}: {} to {}", src.name(), dst.name())));
        };
        Ok(Called::Scalar(converted))
    }
}

/// The body of a function block: generated, or the library's with whether it takes the time.
enum Body {
    User(String),
    Library((&'static str, bool)),
}

/// Whether `param` is `EN` or `ENO`, which positional arguments skip.
fn is_execution(param: &Param) -> bool {
    param.name.eq_ignore_ascii_case("EN") || param.name.eq_ignore_ascii_case("ENO")
}

/// The Rust name of a function's parameter as the function receives it.
pub(in crate::native) fn parameter(name: &SmolStr) -> String {
    format!("p_{}", rust_field(name))
}
