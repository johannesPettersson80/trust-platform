//! The bodies of the generated crate (specification 44 §6.4 to §6.6): each program, function block,
//! method and function lowered into one Rust function, with its frame in a debug image (§13.2).

use std::collections::BTreeSet;

use smol_str::SmolStr;
use trust_syntax::syntax::SyntaxKind as K;

use super::super::classes::Owner;
use super::super::front::{Front, StatementIds};
use super::super::lower::{Frame, Lowerer, Need, Place};
use super::super::model::{AreaLayout, Model};
use super::super::recursion::Recursion;
use super::super::types::{mangle, rust_field, Kind, Ty};
use super::super::NativeError;
use super::{method_function, return_type, signature, structure, PouFrame, PouInstance, Work};
use crate::program_model::{FunctionDef, MethodDef, Param, VarDef};

/// What the lowering of every body takes besides its POU: the sources, the statement ids, whether
/// the image is a debug image with statement hooks, the recursive calls, the POU's id and the
/// target's pointer width.
pub(super) struct BodyContext<'c> {
    pub(super) front: &'c Front,
    pub(super) ids: &'c StatementIds,
    pub(super) hooks: bool,
    pub(super) recursion: &'c Recursion,
    pub(super) pou: u16,
    pub(super) pointer_bytes: u64,
}

impl BodyContext<'_> {
    /// A lowerer of the body of the generated function `name`, in `file`, in `frame`.
    fn lowerer<'m, 'a>(
        &'m self,
        model: &'m mut Model<'a>,
        file: usize,
        frame: Frame,
        name: &str,
    ) -> Lowerer<'m, 'a> {
        let mut lowerer = Lowerer::new(
            model,
            &self.front.sources[file],
            file,
            self.ids,
            frame,
            self.hooks,
            self.pointer_bytes,
        );
        lowerer.pou = self.pou;
        lowerer.recursive = self.recursion.callees_of(name);
        lowerer
    }
}

/// The Rust of one body, its statements, the POUs it calls, its function's name, the generated
/// functions it calls and, in a debug image, its frame; its calls that the context's `recursion`
/// names take one from the call depth.
pub(super) fn body<'a>(
    context: &BodyContext<'_>,
    model: &mut Model<'a>,
    item: &Work<'a>,
) -> Result<GeneratedBody, NativeError> {
    match item {
        Work::Program(index) => program_body(context, model, *index),
        Work::FunctionBlock(ty) => block_body(context, model, *ty),
        Work::Method(owner, method, ty) => method_pou_body(context, model, *owner, method, *ty),
        Work::ProgramMethod(index, method) => program_method_body(context, model, *index, method),
        Work::Function(def) => function_body(context, model, def),
    }
}

/// The body of the program type `index`.
fn program_body(
    context: &BodyContext<'_>,
    model: &mut Model<'_>,
    index: usize,
) -> Result<GeneratedBody, NativeError> {
    let (def, rust, rust_retain, name) = {
        let program = &model.program_types[index];
        (
            program.def,
            program.rust.clone(),
            program.rust_retain.clone(),
            program.body.clone(),
        )
    };
    let signature = format!(
        "fn {name}(cx: &mut Cx, w: &World<'_>, me: &{rust}, mr: &{rust_retain}) -> R<()> {{\n",
    );
    let Some((file, node)) = find_pou(context.front, K::Program, &def.name, None) else {
        return Err(NativeError::Compile(format!(
            "program {} has no body",
            def.name
        )));
    };
    let mut lowerer = context.lowerer(model, file, Frame::Program(index), &name);
    declare_locals(&mut lowerer, &def.temps)?;
    edge_inputs(&mut lowerer, &def.name, |model, input| {
        let (member, retained) = model.program_types[index].member(input)?;
        let owner = if retained { "mr" } else { "me" };
        Some(format!("{owner}.{}", member.field()))
    })?;
    if let Some(list) = body_of(&node) {
        lowerer.body(&list)?;
    }
    lowerer.line("Ok(())");
    let of = PouOf {
        name: def.name.to_string(),
        instance: Some(PouInstance::Program(index)),
        returns: "()".into(),
    };
    Ok(finished(lowerer, &signature, name, of))
}

/// The body of the function block type `ty`.
fn block_body(
    context: &BodyContext<'_>,
    model: &mut Model<'_>,
    ty: Ty,
) -> Result<GeneratedBody, NativeError> {
    let info = model.types.info(ty).clone();
    let Kind::Fb { def, .. } = &info.kind else {
        return Err(NativeError::Compile(
            "a function block without its definition".into(),
        ));
    };
    let Some(Owner::Fb(def)) = model.types.owner(def) else {
        return Err(NativeError::Compile(format!(
            "function block {def} is not declared"
        )));
    };
    let signature = format!(
        "fn fb_{}(cx: &mut Cx, w: &World<'_>, me: &{}) -> R<()> {{\n",
        mangle(&def.name),
        info.rust
    );
    let Some((file, node)) = find_pou(context.front, K::FunctionBlock, &def.name, None) else {
        return Err(NativeError::Compile(format!(
            "function block {} has no body",
            def.name
        )));
    };
    let name = format!("fb_{}", mangle(&def.name));
    let mut lowerer = context.lowerer(model, file, Frame::FunctionBlock(ty), &name);
    declare_locals(&mut lowerer, &def.temps)?;
    edge_inputs(&mut lowerer, &def.name, |model, input| {
        model
            .types
            .info(ty)
            .members()
            .iter()
            .find(|member| member.via.is_empty() && member.name.eq_ignore_ascii_case(input))
            .map(|member| format!("me.{}", member.field()))
    })?;
    // Its statements may stand before and after its methods and actions, each run of them a
    // statement list of its own, run in source order (specification 04 §8).
    for list in node.children().filter(|child| child.kind() == K::StmtList) {
        lowerer.body(&list)?;
    }
    lowerer.line("Ok(())");
    let of = PouOf {
        name: def.name.to_string(),
        instance: Some(PouInstance::Block(ty)),
        returns: "()".into(),
    };
    Ok(finished(lowerer, &signature, name, of))
}

/// The body of `method` of `owner`, whose instance type is `ty`.
fn method_pou_body(
    context: &BodyContext<'_>,
    model: &mut Model<'_>,
    owner: Owner<'_>,
    method: &MethodDef,
    ty: Ty,
) -> Result<GeneratedBody, NativeError> {
    let info = model.types.info(ty).clone();
    let Some((file, list)) = method_body(context.front, owner.name(), method) else {
        return Err(NativeError::Compile(format!(
            "method {}.{} has no body",
            owner.name(),
            method.name
        )));
    };
    let name = method_function(owner, method);
    let mut lowerer = context.lowerer(model, file, Frame::Method(ty), &name);
    let (parameters, returns) = frame(
        &mut lowerer,
        &method.name,
        method.return_type,
        &method.params,
        &method.locals,
    )?;
    // Its `VAR_STAT` variables are members of the instance (specification 01 §3).
    for local in &method.static_locals {
        let hidden =
            crate::native::types::method_static_name(owner.name(), &method.name, &local.name);
        let Some(member) = lowerer
            .info(ty)
            .members()
            .iter()
            .find(|member| member.via.is_empty() && member.name == hidden)
            .cloned()
        else {
            return Err(NativeError::Compile(format!(
                "VAR_STAT {} of {}.{} is not laid out",
                local.name,
                owner.name(),
                method.name
            )));
        };
        lowerer.bind_static(&local.name, format!("me.{}", member.field()), member.ty);
    }
    if let Some(list) = list {
        lowerer.body(&list)?;
    }
    let exit = lowerer.exit.clone();
    lowerer.line(&exit);
    let signature = format!(
        "fn {name}(cx: &mut Cx, w: &World<'_>, me: &{}{parameters}) -> R<{returns}> {{\n",
        info.rust
    );
    let of = PouOf {
        name: format!("{}.{}", owner.name(), method.name),
        instance: Some(PouInstance::Block(ty)),
        returns,
    };
    Ok(finished(lowerer, &signature, name, of))
}

/// The body of `method`, a method or a textual action of the program type `index`: a function on
/// the variables of the program instance it is called on, `me` and `mr` as the program's body has
/// them (specification 04 §4, §8).
fn program_method_body(
    context: &BodyContext<'_>,
    model: &mut Model<'_>,
    index: usize,
    method: &MethodDef,
) -> Result<GeneratedBody, NativeError> {
    let (def, rust, rust_retain, name) = {
        let program = &model.program_types[index];
        (
            program.def,
            program.rust.clone(),
            program.rust_retain.clone(),
            program.method_function(method),
        )
    };
    let Some((file, list)) = method_body(context.front, &def.name, method) else {
        return Err(NativeError::Compile(format!(
            "method {}.{} has no body",
            def.name, method.name
        )));
    };
    let mut lowerer = context.lowerer(model, file, Frame::Program(index), &name);
    let (parameters, returns) = frame(
        &mut lowerer,
        &method.name,
        method.return_type,
        &method.params,
        &method.locals,
    )?;
    // Its `VAR_STAT` variables are members of the program instance (specification 01 §3).
    for local in &method.static_locals {
        let hidden = crate::native::types::method_static_name(&def.name, &method.name, &local.name);
        let Some((member, retained)) = lowerer.model.program_types[index]
            .member(&hidden)
            .map(|(member, retained)| (member.clone(), retained))
        else {
            return Err(NativeError::Compile(format!(
                "VAR_STAT {} of {}.{} is not laid out",
                local.name, def.name, method.name
            )));
        };
        let owner = if retained { "mr" } else { "me" };
        lowerer.bind_static(
            &local.name,
            format!("{owner}.{}", member.field()),
            member.ty,
        );
    }
    if let Some(list) = list {
        lowerer.body(&list)?;
    }
    let exit = lowerer.exit.clone();
    lowerer.line(&exit);
    let signature = format!(
        "fn {name}(cx: &mut Cx, w: &World<'_>, me: &{rust}, mr: &{rust_retain}{parameters}) -> R<{returns}> {{\n",
    );
    let of = PouOf {
        name: format!("{}.{}", def.name, method.name),
        instance: Some(PouInstance::Program(index)),
        returns,
    };
    Ok(finished(lowerer, &signature, name, of))
}

/// The body of the function `def`.
fn function_body(
    context: &BodyContext<'_>,
    model: &mut Model<'_>,
    def: &FunctionDef,
) -> Result<GeneratedBody, NativeError> {
    let Some((file, node)) = find_pou(context.front, K::Function, &def.name, None) else {
        return Err(NativeError::Compile(format!(
            "function {} has no body",
            def.name
        )));
    };
    let name = format!("f_{}", mangle(&def.name));
    let mut lowerer = context.lowerer(model, file, Frame::Function, &name);
    let (parameters, returns) = frame(
        &mut lowerer,
        &def.name,
        Some(def.return_type),
        &def.params,
        &def.locals,
    )?;
    // Its `VAR_STAT` variables are globals of hidden names (specification 01 §3).
    for local in &def.static_locals {
        let hidden = crate::program_model::static_storage_name(&def.name, &local.name);
        let Some((retained, ty)) = lowerer
            .model
            .global(&hidden)
            .map(|global| (global.retained, global.ty))
        else {
            return Err(NativeError::Compile(format!(
                "VAR_STAT {} of {} is not laid out",
                local.name, def.name
            )));
        };
        let area = if retained { "w.r" } else { "w.g" };
        lowerer.bind_static(&local.name, format!("{area}.{}", rust_field(&hidden)), ty);
    }
    if let Some(list) = body_of(&node) {
        lowerer.body(&list)?;
    }
    let exit = lowerer.exit.clone();
    lowerer.line(&exit);
    let signature =
        format!("fn {name}(cx: &mut Cx, w: &World<'_>{parameters}) -> R<{returns}> {{\n");
    let of = PouOf {
        name: def.name.to_string(),
        instance: None,
        returns,
    };
    Ok(finished(lowerer, &signature, name, of))
}

/// The source file of a POU: the file whose syntax tree holds its body.
pub(super) fn find_pou(
    front: &Front,
    kind: trust_syntax::syntax::SyntaxKind,
    name: &str,
    owner: Option<&str>,
) -> Option<(usize, trust_syntax::syntax::SyntaxNode)> {
    let last = name.rsplit('.').next().unwrap_or(name);
    for (index, source) in front.sources.iter().enumerate() {
        for node in source.root.descendants().filter(|node| node.kind() == kind) {
            let Some(found) = node
                .children()
                .find(|child| child.kind() == trust_syntax::syntax::SyntaxKind::Name)
            else {
                continue;
            };
            if !crate::native::lower::ident_of(&found).eq_ignore_ascii_case(last) {
                continue;
            }
            if let Some(owner) = owner {
                let owner_last = owner.rsplit('.').next().unwrap_or(owner);
                let parent = node.ancestors().skip(1).find(|ancestor| {
                    matches!(
                        ancestor.kind(),
                        trust_syntax::syntax::SyntaxKind::FunctionBlock
                            | trust_syntax::syntax::SyntaxKind::Class
                            | trust_syntax::syntax::SyntaxKind::Program
                    )
                });
                let Some(parent) = parent else {
                    continue;
                };
                let Some(parent_name) = parent
                    .children()
                    .find(|child| child.kind() == trust_syntax::syntax::SyntaxKind::Name)
                else {
                    continue;
                };
                if !crate::native::lower::ident_of(&parent_name).eq_ignore_ascii_case(owner_last) {
                    continue;
                }
            }
            return Some((index, node));
        }
    }
    None
}

/// The statement list of a POU's body.
fn body_of(node: &trust_syntax::syntax::SyntaxNode) -> Option<trust_syntax::syntax::SyntaxNode> {
    node.children()
        .find(|child| child.kind() == trust_syntax::syntax::SyntaxKind::StmtList)
}

/// The file and the statement list of `method` of the POU `owner`: a method's body, a textual
/// action's (specification 04 §8), or a property's `GET` or `SET`, which the declarations hold as a
/// method named like the property and one named `__set_<property>`.
fn method_body(
    front: &Front,
    owner: &str,
    method: &MethodDef,
) -> Option<(usize, Option<trust_syntax::syntax::SyntaxNode>)> {
    let accessor = |kind: K, name: &str| {
        let (file, node) = find_pou(front, K::Property, name, Some(owner))?;
        let accessor = node.children().find(|child| child.kind() == kind)?;
        Some((file, body_of(&accessor)))
    };
    if let Some(property) = method.name.strip_prefix("__set_") {
        return accessor(K::PropertySet, property);
    }
    if let Some((file, node)) = find_pou(front, K::Method, &method.name, Some(owner))
        .or_else(|| find_pou(front, K::Action, &method.name, Some(owner)))
    {
        return Some((file, body_of(&node)));
    }
    accessor(K::PropertyGet, &method.name)
}

/// One body as [`body`] generates it: its Rust, its statements, the POUs it needs, its function's
/// name, the generated functions it calls and, in a debug image, its POU's frame with the
/// definition of its frame structure.
pub(super) type GeneratedBody = (
    String,
    Vec<u32>,
    Vec<Need>,
    String,
    BTreeSet<String>,
    Option<(PouFrame, String)>,
);

/// Runs the trigger of each edge-qualified input of `owner` on the level the input holds as the
/// body starts, and names the trigger's `Q` by the input's name for the rest of the body, so the
/// body reads the pulse and the variable keeps the level (IEC §6.5.2.2). `place` gives the Rust
/// place of a variable of the body's instance by its name.
fn edge_inputs(
    lowerer: &mut Lowerer<'_, '_>,
    owner: &str,
    place: impl Fn(&Model<'_>, &str) -> Option<String>,
) -> Result<(), NativeError> {
    let bool_ty = lowerer.model.types.elem(crate::native::types::Elem::Bool);
    for edge in lowerer.model.types.defs.edge_inputs(owner) {
        let trigger_name = crate::native::types::edge_trigger_name(&edge.name);
        let (Some(input), Some(trigger)) = (
            place(lowerer.model, &edge.name),
            place(lowerer.model, &trigger_name),
        ) else {
            return Err(NativeError::Compile(format!(
                "the edge-qualified input {} of {owner} is not laid out",
                edge.name
            )));
        };
        let function = match edge.qualifier {
            trust_hir::symbols::EdgeQualifier::Rising => "lib::r_trig",
            trust_hir::symbols::EdgeQualifier::Falling => "lib::f_trig",
        };
        lowerer.line(&format!("{trigger}.CLK.set({input}.get());"));
        lowerer.line(&format!("{function}(&{trigger});"));
        lowerer.bind_static(&edge.name, format!("{trigger}.Q"), bool_ty);
    }
    Ok(())
}

/// What the frame of a body names: the POU's declared name, the instance it runs on and its
/// function's result type.
struct PouOf {
    name: String,
    instance: Option<PouInstance>,
    returns: String,
}

/// The body `lowerer` wrote, under `signature`, as the function `name`. A body that makes a
/// recursive call is never inlined, so that no cycle collapses into a call of a function to itself,
/// which the link keeps no relocation for (specification 44 §6.4, §8.5).
///
/// In a debug image the body links its `Frame` into `cx.frame` for the length of the call, a fault's
/// return too, and its frame variables lie in one structure at the offsets their symbols name
/// (specification 44 §5.7, §13.2).
fn finished(lowerer: Lowerer<'_, '_>, signature: &str, name: String, of: PouOf) -> GeneratedBody {
    let needs = lowerer.needs.clone();
    let called = lowerer.called.clone();
    let attribute = if lowerer.recursive.is_empty() {
        ""
    } else {
        "#[inline(never)]\n"
    };
    let pou = lowerer.pou;
    let parts: Option<Vec<(SmolStr, String, u32, u32)>> = lowerer.frame_vars.as_ref().map(|vars| {
        vars.iter()
            .map(|var| {
                let info = lowerer.model.types.info(var.ty);
                (var.name.clone(), info.rust.clone(), info.size, info.align)
            })
            .collect()
    });
    let (text, statements, frame_vars) = lowerer.finish();
    let (text, frame) = match (frame_vars, parts) {
        (Some(vars), Some(parts)) => {
            let layout = AreaLayout::new(parts);
            let structure_name = format!("Fr{pou}");
            let mut definition = String::new();
            let locals = if vars.is_empty() {
                "core::ptr::null()".to_string()
            } else {
                let fields: Vec<(String, String, u32)> = vars
                    .iter()
                    .zip(&layout.fields)
                    .map(|(var, field)| (var.field.clone(), field.rust.clone(), field.offset))
                    .collect();
                structure(
                    &mut definition,
                    &structure_name,
                    &fields,
                    layout.size,
                    false,
                );
                "(&fr as *const _ as *const u8)".to_string()
            };
            let instance = if of.instance.is_some() {
                "(me as *const _ as *const u8)"
            } else {
                "core::ptr::null()"
            };
            let declaration = if vars.is_empty() {
                String::new()
            } else {
                format!("    let fr: {structure_name} = <{structure_name} as Plain>::ZEROED;\n")
            };
            let returns = &of.returns;
            let text = format!(
                "{declaration}    let link = abi::Frame {{ caller: cx.frame, pou: {pou}, call_stmt: cx.stmt, locals: {locals}, instance: {instance} }};\n    cx.frame = &link;\n    let result = (|| -> R<{returns}> {{\n{text}    }})();\n    cx.frame = link.caller;\n    result\n"
            );
            let offsets = vars
                .into_iter()
                .zip(layout.fields.iter().map(|field| field.offset))
                .collect();
            let frame = PouFrame {
                id: pou,
                name: of.name,
                instance: of.instance,
                vars: offsets,
                size: layout.size,
            };
            (text, Some((frame, definition)))
        }
        _ => (text, None),
    };
    (
        format!("{attribute}{signature}{text}}}\n\n"),
        statements,
        needs,
        name,
        called,
        frame,
    )
}

/// Declares the `VAR_TEMP` of a program or a function block, each at its starting value at each
/// call (IEC §6.5.2.1).
fn declare_locals(lowerer: &mut Lowerer<'_, '_>, temps: &[VarDef]) -> Result<(), NativeError> {
    for temp in temps {
        let ty = local_type(lowerer, &temp.name, temp.type_id)?;
        let variable = lowerer.declare(&temp.name, ty);
        let place = Place {
            expr: variable,
            ty,
            is_ref: false,
        };
        crate::native::frame_init::start(
            lowerer,
            &place,
            temp.type_id,
            temp.initializer.as_ref(),
            &temp.name,
        )?;
    }
    Ok(())
}

/// The type of the frame variable `name`: never one that holds an instance with a class id, which
/// a dynamic call reaches only where it lies in an area (specification 44 §6.4, §7.3).
fn local_type(
    lowerer: &mut Lowerer<'_, '_>,
    name: &SmolStr,
    type_id: trust_hir::TypeId,
) -> Result<Ty, NativeError> {
    let ty = lowerer.model.ty(type_id)?;
    if lowerer.model.types.holds_class(ty) {
        return Err(NativeError::unsupported(format!(
            "the frame variable {name}, an instance of a class with inheritance or interfaces"
        )));
    }
    Ok(ty)
}

/// Declares a function's or a method's frame: its return variable first, its inputs copied from the call's arguments, its in-outs and outputs as the caller's
/// variables, its locals; the result, the outputs and the locals start at their starting values
/// (IEC §6.5.1, §6.5.2.1). Returns the Rust parameters after `w` (or `me`) and the Rust return
/// type, and sets the body's exit.
fn frame(
    lowerer: &mut Lowerer<'_, '_>,
    name: &SmolStr,
    ret: Option<trust_hir::TypeId>,
    params: &[Param],
    locals: &[VarDef],
) -> Result<(String, String), NativeError> {
    let signature = signature(lowerer.model, ret, params)?;
    if let (Some(ret_id), Some(ret)) = (ret, return_type(lowerer.model, ret)?) {
        let info = lowerer.info(ret).clone();
        let own = SmolStr::new(name.rsplit('.').next().unwrap_or(name));
        let variable = lowerer.declare(&own, ret);
        let place = Place {
            expr: variable.clone(),
            ty: ret,
            is_ref: false,
        };
        // A reference to an instance starts as `NULL`, as the declaration leaves it.
        if !matches!(info.kind, Kind::Ref { .. }) {
            crate::native::frame_init::start(lowerer, &place, ret_id, None, &own)?;
        }
        lowerer.result = Some(place.clone());
        if crate::native::lower::is_scalar(&info.kind) {
            let value = lowerer.load(&place);
            lowerer.exit = format!("return Ok({value});");
        } else {
            lowerer.exit = format!("Assign::assign(out, &{variable});\n        return Ok(());");
        }
    }
    for param in params {
        let ty = lowerer.model.ty(param.type_id)?;
        let info = lowerer.info(ty).clone();
        // `EN` reads TRUE in the body, which runs only when the call is enabled (specification 10
        // §7.5).
        if param.name.eq_ignore_ascii_case("EN") {
            let local = lowerer.declare(&param.name, ty);
            lowerer.line(&format!("{local}.set(1);"));
            continue;
        }
        let argument = crate::native::lower::parameter(&param.name);
        // An open array names the caller's array through its view, whatever the direction: an
        // input reads it, and an output or an in-out writes it in place (specification 03 §6).
        if matches!(info.kind, Kind::Open { .. }) {
            lowerer.declare_reference(&param.name, ty, &argument);
            if param.direction == trust_hir::symbols::ParamDirection::In {
                lowerer.read_only.push(param.name.clone());
            }
            continue;
        }
        if param.direction != trust_hir::symbols::ParamDirection::In {
            // An in-out is the caller's variable itself (IEC §6.6.1.2.2); an output is the
            // caller's place for it, which the caller copies to the output's variable after the
            // call, and which starts at the output's starting value.
            let output = param.direction == trust_hir::symbols::ParamDirection::Out;
            if param.default.is_some() && !output {
                return Err(NativeError::unsupported(format!(
                    "the initial value of in-out {} of {name}",
                    param.name
                )));
            }
            lowerer.declare_reference(&param.name, ty, &argument);
            if output && param.name.eq_ignore_ascii_case("ENO") {
                // `ENO` starts TRUE, and the body may set it FALSE (specification 10 §7.5).
                lowerer.line(&format!("{argument}.set(1);"));
            } else if output {
                let place = Place {
                    expr: argument,
                    ty,
                    is_ref: true,
                };
                crate::native::frame_init::start(
                    lowerer,
                    &place,
                    param.type_id,
                    param.default.as_ref(),
                    &param.name,
                )?;
            }
            continue;
        }
        // A function block instance given as an input is the caller's instance (§6.4).
        if info.kind.is_instance() {
            lowerer.declare_reference(&param.name, ty, &argument);
            continue;
        }
        let local = lowerer.declare(&param.name, ty);
        if crate::native::lower::is_scalar(&info.kind) {
            let elem = info.scalar().map_or("u8", |elem| elem.rust());
            let stored = if elem == "bool" {
                format!("u8::from({argument})")
            } else {
                argument.clone()
            };
            lowerer.line(&format!("{local}.set({stored});"));
        } else {
            lowerer.line(&format!("Assign::assign(&{local}, {argument});"));
        }
    }
    for local in locals {
        // A `VAR_EXTERNAL` names the global itself (IEC §6.5.2.2), which the frame does not hold.
        if local.external {
            continue;
        }
        let ty = local_type(lowerer, &local.name, local.type_id)?;
        let variable = lowerer.declare(&local.name, ty);
        let place = Place {
            expr: variable,
            ty,
            is_ref: false,
        };
        crate::native::frame_init::start(
            lowerer,
            &place,
            local.type_id,
            local.initializer.as_ref(),
            &local.name,
        )?;
    }
    Ok((signature.parameters, signature.returns))
}
