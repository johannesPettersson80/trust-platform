//! The generated crate (specification 44 §3, step 3; §6.3): the types, the program structures and
//! the areas as `#[repr(C)]` structures of cells with an assertion of every offset, every POU as a
//! function, and the entry points the kernel calls.

use std::fmt::Write as _;

use smol_str::SmolStr;

use super::front::{Front, StatementIds};
use super::lower::{Frame, Lowerer, Need, PRELUDE};
use super::model::{AreaLayout, Model};
use super::types::{mangle, rust_field, Kind, Ty};
use super::NativeError;
use crate::program_model::{FunctionBlockDef, FunctionDef, MethodDef, Param, VarDef};
use trust_runtime_core::native::image::{EntryKind, Target};

/// An entry point of the module.
pub(super) struct Entry {
    /// The exported symbol.
    pub(super) symbol: String,
    pub(super) kind: EntryKind,
    /// The program instance's index for a program body.
    pub(super) index: u32,
}

/// The generated crate and what the image's tables need of it.
pub(super) struct Generated {
    pub(super) source: String,
    pub(super) entries: Vec<Entry>,
    /// The POU of each statement, by statement id; `u16::MAX` for a statement of no compiled POU.
    pub(super) statement_pous: Vec<u16>,
}

/// What a body is, for the generator's work list.
enum Work<'a> {
    Program(usize),
    FunctionBlock(Ty),
    Method(&'a FunctionBlockDef, &'a MethodDef, Ty),
    Function(&'a FunctionDef),
}

/// The source file of a POU: the file whose syntax tree holds its body.
fn find_pou(
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
            if !super::lower::ident_of(&found).eq_ignore_ascii_case(last) {
                continue;
            }
            if let Some(owner) = owner {
                let owner_last = owner.rsplit('.').next().unwrap_or(owner);
                let parent = node.ancestors().skip(1).find(|ancestor| {
                    matches!(
                        ancestor.kind(),
                        trust_syntax::syntax::SyntaxKind::FunctionBlock
                            | trust_syntax::syntax::SyntaxKind::Class
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
                if !super::lower::ident_of(&parent_name).eq_ignore_ascii_case(owner_last) {
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

/// Generates the crate of `model` for `target`.
///
/// # Errors
///
/// [`NativeError`] for a construct the generator does not compile.
pub(super) fn generate<'a>(
    front: &Front,
    model: &mut Model<'a>,
    ids: &StatementIds,
    target: Target,
    hooks: bool,
) -> Result<Generated, NativeError> {
    let mut functions = String::new();
    let mut pous = 0usize;
    let mut statement_pous = vec![u16::MAX; ids.order.len()];
    let mut done_methods: Vec<(SmolStr, SmolStr)> = Vec::new();
    let mut done_functions: Vec<SmolStr> = Vec::new();
    let mut done_blocks: Vec<Ty> = Vec::new();
    let mut work: Vec<Work<'a>> = (0..model.program_types.len()).map(Work::Program).collect();
    let mut index = 0;
    loop {
        // Every function block type the bodies use needs its body.
        let blocks: Vec<Ty> = model
            .types
            .all()
            .filter(|(_, info)| matches!(info.kind, Kind::Fb { .. }))
            .map(|(ty, _)| ty)
            .filter(|ty| !done_blocks.contains(ty))
            .collect();
        for ty in blocks {
            done_blocks.push(ty);
            work.push(Work::FunctionBlock(ty));
        }
        let Some(item) = work.get(index) else {
            break;
        };
        let item = match item {
            Work::Program(program) => Work::Program(*program),
            Work::FunctionBlock(ty) => Work::FunctionBlock(*ty),
            Work::Method(owner, method, ty) => Work::Method(owner, method, *ty),
            Work::Function(def) => Work::Function(def),
        };
        index += 1;
        let pou = u16::try_from(pous).unwrap_or(u16::MAX);
        let (text, statements, needs, name) = body(front, model, ids, &item, hooks)?;
        pous += 1;
        let _ = name;
        for statement in statements {
            if let Some(slot) = statement_pous.get_mut(statement as usize) {
                *slot = pou;
            }
        }
        functions.push_str(&text);
        for need in needs {
            match need {
                Need::Function(name) => {
                    if done_functions.contains(&name) {
                        continue;
                    }
                    done_functions.push(name.clone());
                    let Some(def) = model.function(&name) else {
                        return Err(NativeError::Compile(format!(
                            "function {name} is not declared"
                        )));
                    };
                    work.push(Work::Function(def));
                }
                Need::Method(owner, method) => {
                    if done_methods.contains(&(owner.clone(), method.clone())) {
                        continue;
                    }
                    done_methods.push((owner.clone(), method.clone()));
                    let Some(def) = model.types.function_block_def(&owner) else {
                        return Err(NativeError::Compile(format!(
                            "function block {owner} is not declared"
                        )));
                    };
                    let Some(found) = def
                        .methods
                        .iter()
                        .find(|candidate| candidate.name == method)
                    else {
                        return Err(NativeError::Compile(format!(
                            "method {owner}.{method} is not declared"
                        )));
                    };
                    let Some(ty) = model
                        .types
                        .all()
                        .find(|(_, info)| matches!(&info.kind, Kind::Fb { def: name, .. } if *name == def.name))
                        .map(|(ty, _)| ty)
                    else {
                        return Err(NativeError::Compile(format!("function block {owner} has no layout")));
                    };
                    work.push(Work::Method(def, found, ty));
                }
            }
        }
    }

    let mut source = String::new();
    source.push_str(
        "//! A truST program compiled for the kernel interface (specification 44). Generated; do not edit.\n\
         #![no_std]\n#![no_main]\n#![deny(unsafe_code)]\n\
         #![allow(non_snake_case, non_camel_case_types, unused_parens, unused_variables, unused_mut, \
         dead_code, unreachable_code, unused_labels, unused_braces, clippy::all)]\n\n",
    );
    source.push_str(PRELUDE);
    source.push('\n');
    types(&mut source, model);
    programs(&mut source, model);
    let images = model.images;
    areas(&mut source, model, images);
    source.push_str(&functions);
    let references = reference_initializers(front, model)?;
    let entries = entries(&mut source, model, target, &references);
    source.push_str(
        "\n#[panic_handler]\nfn panic(_: &core::panic::PanicInfo<'_>) -> ! {\n    loop {}\n}\n",
    );
    Ok(Generated {
        source,
        entries,
        statement_pous,
    })
}

/// The Rust of one body, its statements, the POUs it calls and its name.
fn body<'a>(
    front: &Front,
    model: &mut Model<'a>,
    ids: &StatementIds,
    item: &Work<'a>,
    hooks: bool,
) -> Result<(String, Vec<u32>, Vec<Need>, SmolStr), NativeError> {
    use trust_syntax::syntax::SyntaxKind as K;
    match item {
        Work::Program(index) => {
            let (def, rust, rust_retain) = {
                let program = &model.program_types[*index];
                (
                    program.def,
                    program.rust.clone(),
                    program.rust_retain.clone(),
                )
            };
            let signature = format!(
                "fn prog_{}(cx: &mut Cx, w: &World<'_>, me: &{rust}, mr: &{rust_retain}) -> R<()> {{\n",
                mangle(&def.name),
            );
            let Some((file, node)) = find_pou(front, K::Program, &def.name, None) else {
                return Err(NativeError::Compile(format!(
                    "program {} has no body",
                    def.name
                )));
            };
            let source = &front.sources[file];
            let mut lowerer = Lowerer::new(model, source, file, ids, Frame::Program(*index), hooks);
            declare_locals(&mut lowerer, &def.temps)?;
            if let Some(list) = body_of(&node) {
                lowerer.statements(list.children())?;
            }
            lowerer.line("Ok(())");
            let needs = lowerer.needs.clone();
            let (text, statements) = lowerer.finish();
            Ok((
                format!("{signature}{text}}}\n\n"),
                statements,
                needs,
                def.name.clone(),
            ))
        }
        Work::FunctionBlock(ty) => {
            let info = model.types.info(*ty).clone();
            let Kind::Fb { def, .. } = &info.kind else {
                return Err(NativeError::Compile(
                    "a function block without its definition".into(),
                ));
            };
            let Some(def) = model.types.function_block_def(def) else {
                return Err(NativeError::Compile(format!(
                    "function block {def} is not declared"
                )));
            };
            let signature = format!(
                "fn fb_{}(cx: &mut Cx, w: &World<'_>, me: &{}) -> R<()> {{\n",
                mangle(&def.name),
                info.rust
            );
            let Some((file, node)) = find_pou(front, K::FunctionBlock, &def.name, None) else {
                return Err(NativeError::Compile(format!(
                    "function block {} has no body",
                    def.name
                )));
            };
            let source = &front.sources[file];
            let mut lowerer =
                Lowerer::new(model, source, file, ids, Frame::FunctionBlock(*ty), hooks);
            declare_locals(&mut lowerer, &def.temps)?;
            if let Some(list) = body_of(&node) {
                lowerer.statements(list.children())?;
            }
            lowerer.line("Ok(())");
            let needs = lowerer.needs.clone();
            let (text, statements) = lowerer.finish();
            Ok((
                format!("{signature}{text}}}\n\n"),
                statements,
                needs,
                def.name.clone(),
            ))
        }
        Work::Method(owner, method, ty) => {
            let info = model.types.info(*ty).clone();
            let Some((file, node)) = find_pou(front, K::Method, &method.name, Some(&owner.name))
            else {
                return Err(NativeError::Compile(format!(
                    "method {}.{} has no body",
                    owner.name, method.name
                )));
            };
            let source = &front.sources[file];
            let mut lowerer = Lowerer::new(model, source, file, ids, Frame::Method(*ty), hooks);
            let (parameters, returns) = frame(
                &mut lowerer,
                &method.name,
                method.return_type,
                &method.params,
                &method.locals,
            )?;
            if !method.static_locals.is_empty() {
                return Err(NativeError::unsupported(format!(
                    "VAR_STAT of {}.{}",
                    owner.name, method.name
                )));
            }
            if let Some(list) = body_of(&node) {
                lowerer.statements(list.children())?;
            }
            let exit = lowerer.exit.clone();
            lowerer.line(&exit);
            let needs = lowerer.needs.clone();
            let (text, statements) = lowerer.finish();
            let signature = format!(
                "fn m_{}__{}(cx: &mut Cx, w: &World<'_>, me: &{}{parameters}) -> R<{returns}> {{\n",
                mangle(&owner.name),
                mangle(&method.name),
                info.rust
            );
            Ok((
                format!("{signature}{text}}}\n\n"),
                statements,
                needs,
                SmolStr::new(format!("{}.{}", owner.name, method.name)),
            ))
        }
        Work::Function(def) => {
            let Some((file, node)) = find_pou(front, K::Function, &def.name, None) else {
                return Err(NativeError::Compile(format!(
                    "function {} has no body",
                    def.name
                )));
            };
            let source = &front.sources[file];
            let mut lowerer = Lowerer::new(model, source, file, ids, Frame::Function, hooks);
            let (parameters, returns) = frame(
                &mut lowerer,
                &def.name,
                Some(def.return_type),
                &def.params,
                &def.locals,
            )?;
            if !def.static_locals.is_empty() {
                return Err(NativeError::unsupported(format!(
                    "VAR_STAT of {}",
                    def.name
                )));
            }
            if let Some(list) = body_of(&node) {
                lowerer.statements(list.children())?;
            }
            let exit = lowerer.exit.clone();
            lowerer.line(&exit);
            let needs = lowerer.needs.clone();
            let (text, statements) = lowerer.finish();
            let signature = format!(
                "fn f_{}(cx: &mut Cx, w: &World<'_>{parameters}) -> R<{returns}> {{\n",
                mangle(&def.name)
            );
            Ok((
                format!("{signature}{text}}}\n\n"),
                statements,
                needs,
                def.name.clone(),
            ))
        }
    }
}

/// Declares the `VAR_TEMP` of a program or a function block, zero at each call.
fn declare_locals(lowerer: &mut Lowerer<'_, '_>, temps: &[VarDef]) -> Result<(), NativeError> {
    for temp in temps {
        if temp.initializer.is_some() {
            return Err(NativeError::unsupported(format!(
                "the initial value of VAR_TEMP {}",
                temp.name
            )));
        }
        let ty = lowerer.model.ty(temp.type_id)?;
        lowerer.declare(&temp.name, ty);
    }
    Ok(())
}

/// Declares a function's or a method's frame: its return variable first, its inputs copied from
/// the call's arguments, its locals; returns the Rust parameters after `w` (or `me`) and the Rust
/// return type, and sets the body's exit.
fn frame(
    lowerer: &mut Lowerer<'_, '_>,
    name: &SmolStr,
    ret: Option<trust_hir::TypeId>,
    params: &[Param],
    locals: &[VarDef],
) -> Result<(String, String), NativeError> {
    let mut parameters = String::new();
    let mut returns = "()".to_string();
    let ret = match ret {
        Some(ret)
            if lowerer
                .model
                .types
                .registry()
                .get(ret)
                .is_some_and(|ty| !matches!(ty, trust_hir::Type::Void)) =>
        {
            Some(lowerer.model.ty(ret)?)
        }
        _ => None,
    };
    if let Some(ret) = ret {
        let info = lowerer.info(ret).clone();
        let own = SmolStr::new(name.rsplit('.').next().unwrap_or(name));
        let variable = lowerer.declare(&own, ret);
        if super::lower::is_scalar(&info.kind) {
            returns = info.scalar().map_or("()", |elem| elem.rust()).to_string();
            let value = lowerer.load(&super::lower::Place {
                expr: variable,
                ty: ret,
                is_ref: false,
            });
            lowerer.exit = format!("return Ok({value});");
        } else {
            let _ = write!(parameters, ", out: &{}", info.rust);
            lowerer.exit = format!("Assign::assign(out, &{variable});\n        return Ok(());");
        }
    }
    for param in params {
        if param.name.eq_ignore_ascii_case("EN") || param.name.eq_ignore_ascii_case("ENO") {
            continue;
        }
        let ty = lowerer.model.ty(param.type_id)?;
        let info = lowerer.info(ty).clone();
        let argument = super::lower::parameter(&param.name);
        if param.direction == trust_hir::symbols::ParamDirection::InOut {
            // The caller's variable itself (IEC §6.6.1.2.2).
            let _ = write!(parameters, ", {argument}: &{}", info.rust);
            lowerer.declare_reference(&param.name, ty, &argument);
            continue;
        }
        if param.direction != trust_hir::symbols::ParamDirection::In {
            return Err(NativeError::unsupported(format!(
                "the {:?} parameter {} of {name}",
                param.direction, param.name
            )));
        }
        let local = lowerer.declare(&param.name, ty);
        if super::lower::is_scalar(&info.kind) {
            let elem = info.scalar().map_or("u8", |elem| elem.rust());
            let _ = write!(parameters, ", {argument}: {elem}");
            let stored = if elem == "bool" {
                format!("u8::from({argument})")
            } else {
                argument.clone()
            };
            lowerer.line(&format!("{local}.set({stored});"));
        } else {
            let _ = write!(parameters, ", {argument}: &{}", info.rust);
            lowerer.line(&format!("Assign::assign(&{local}, {argument});"));
        }
    }
    for local in locals {
        if local.initializer.is_some() {
            return Err(NativeError::unsupported(format!(
                "the initial value of local {} of {name}",
                local.name
            )));
        }
        let ty = lowerer.model.ty(local.type_id)?;
        lowerer.declare(&local.name, ty);
    }
    Ok((parameters, returns))
}

/// Writes a `plain_struct!` of `fields`, its offset assertions and, for a structure that is
/// assigned as a whole, its `Assign`.
fn structure(
    source: &mut String,
    name: &str,
    fields: &[(String, String, u32)],
    size: u32,
    assign: bool,
) {
    let _ = writeln!(source, "plain_struct! {{\n    pub struct {name} {{");
    for (field, rust, _) in fields {
        let _ = writeln!(source, "        pub {field}: {rust},");
    }
    source.push_str("    }\n}\n");
    if assign {
        let _ = writeln!(
            source,
            "impl Assign for {name} {{\n    #[inline]\n    fn assign(&self, source: &Self) {{"
        );
        for (field, _, _) in fields {
            let _ = writeln!(
                source,
                "        Assign::assign(&self.{field}, &source.{field});"
            );
        }
        source.push_str("    }\n}\n");
    }
    for (field, _, offset) in fields {
        let _ = writeln!(
            source,
            "const _: () = assert!(core::mem::offset_of!({name}, {field}) == {offset});"
        );
    }
    let _ = writeln!(
        source,
        "const _: () = assert!(core::mem::size_of::<{name}>() == {size});\n"
    );
}

fn types(source: &mut String, model: &Model<'_>) {
    for (_, info) in model.types.all() {
        match &info.kind {
            Kind::Struct { members } | Kind::Fb { members, .. } => {
                let fields: Vec<(String, String, u32)> = members
                    .iter()
                    .map(|member| {
                        (
                            member.field(),
                            model.types.info(member.ty).rust.clone(),
                            member.offset,
                        )
                    })
                    .collect();
                structure(
                    source,
                    &info.rust,
                    &fields,
                    info.size,
                    matches!(info.kind, Kind::Struct { .. }),
                );
            }
            Kind::StdFb(_) => {
                let _ = writeln!(
                    source,
                    "const _: () = assert!(core::mem::size_of::<{}>() == {});",
                    info.rust, info.size
                );
            }
            _ => {}
        }
    }
}

fn programs(source: &mut String, model: &Model<'_>) {
    for program in &model.program_types {
        for (name, members, size) in [
            (&program.rust, &program.members, program.size),
            (&program.rust_retain, &program.retained, program.retain_size),
        ] {
            let fields: Vec<(String, String, u32)> = members
                .iter()
                .map(|member| {
                    (
                        member.field(),
                        model.types.info(member.ty).rust.clone(),
                        member.offset,
                    )
                })
                .collect();
            structure(source, name, &fields, size, false);
        }
    }
}

fn area(source: &mut String, name: &str, layout: &AreaLayout) {
    let fields: Vec<(String, String, u32)> = layout
        .fields
        .iter()
        .map(|field| (rust_field(&field.name), field.rust.clone(), field.offset))
        .collect();
    structure(source, name, &fields, layout.size, false);
}

fn areas(source: &mut String, model: &Model<'_>, images: [u32; 3]) {
    let _ = writeln!(source, "type AreaI = [Cell<u8>; {}];", images[0]);
    let _ = writeln!(source, "type AreaQ = [Cell<u8>; {}];", images[1]);
    let _ = writeln!(source, "type AreaM = [Cell<u8>; {}];", images[2]);
    area(source, "AreaG", &model.area_g);
    area(source, "AreaR", &model.area_r);
    area(source, "AreaP", &model.area_p);
    source.push_str(
        "/// The areas of one entry call (specification 44 §7.1).
struct World<'a> {
    i: &'a AreaI,
    q: &'a AreaQ,
    m: &'a AreaM,
    g: &'a AreaG,
    r: &'a AreaR,
    p: &'a AreaP,
    /// The areas, for references (specification 44 §7.3). A reference, not a copy: a module has no
    /// `memcpy`.
    areas: &'a Areas<'a>,
}

impl<'a> World<'a> {
    fn new(cx: &mut Cx, areas: &'a Areas<'a>) -> R<Self> {
        Ok(Self {
            i: areas.view(cx, abi::AREA_INPUTS)?,
            q: areas.view(cx, abi::AREA_OUTPUTS)?,
            m: areas.view(cx, abi::AREA_MARKERS)?,
            g: areas.view(cx, abi::AREA_GLOBALS)?,
            r: areas.view(cx, abi::AREA_RETAIN)?,
            p: areas.view(cx, abi::AREA_INSTANCES)?,
            areas,
        })
    }
}

",
    );
}

/// The `REF(variable)` initializers of the globals and of the program instances' variables
/// (specification 44 §6.7) as Rust statements: a reference names where its variable lies, which the
/// starting memory cannot hold, so the cold-start and the warm-start initialization set it.
fn reference_initializers(front: &Front, model: &Model<'_>) -> Result<Vec<String>, NativeError> {
    let mut lines = Vec::new();
    let configured = front
        .lowered
        .configuration
        .iter()
        .flat_map(|configuration| configuration.globals.iter());
    for global in &model.globals {
        let Some(init) = front
            .lowered
            .globals
            .iter()
            .chain(configured.clone())
            .find(|init| init.name.eq_ignore_ascii_case(&global.name))
        else {
            continue;
        };
        let area = if global.retained { "r" } else { "g" };
        let place = format!("w.{area}.{}", rust_field(&global.name));
        reference_line(
            model,
            None,
            &place,
            global.ty,
            init.initializer.as_ref(),
            &mut lines,
        )?;
    }
    for instance in &model.instances {
        let program = &model.program_types[instance.program];
        for var in program.def.vars.iter().filter(|var| !var.external) {
            let Some((member, retained)) = program.member(&var.name) else {
                continue;
            };
            let area = if retained { "r" } else { "p" };
            let place = format!("w.{area}.{}.{}", rust_field(&instance.name), member.field());
            let scope = Some((instance.name.as_str(), instance.program));
            reference_line(
                model,
                scope,
                &place,
                member.ty,
                var.initializer.as_ref(),
                &mut lines,
            )?;
        }
    }
    Ok(lines)
}

/// The statement that gives the variable at `place`, of type `ty`, its `REF` initializer.
fn reference_line(
    model: &Model<'_>,
    scope: Option<(&str, usize)>,
    place: &str,
    ty: Ty,
    initializer: Option<&crate::program_model::Expr>,
    lines: &mut Vec<String>,
) -> Result<(), NativeError> {
    use crate::program_model::Expr;
    use crate::value::Value;
    if !matches!(model.types.info(ty).kind, Kind::Ref { .. }) {
        return Ok(());
    }
    match initializer {
        None | Some(Expr::Literal(Value::Null | Value::Reference(None))) => Ok(()),
        Some(Expr::Ref(target)) => {
            let target = initializer_place(model, scope, target).ok_or_else(|| {
                NativeError::unsupported(format!("the reference initializer of {place}"))
            })?;
            lines.push(format!(
                "{place}.set(w.areas.refer(cx, &{target}, abi::RefValue::VARIABLE)?);"
            ));
            Ok(())
        }
        Some(_) => Err(NativeError::unsupported(format!(
            "the reference initializer of {place}"
        ))),
    }
}

/// The Rust place of the variable `target` names, seen from the program instance `scope` (its
/// name and its type's index) or from the globals: a variable, and members of it.
fn initializer_place(
    model: &Model<'_>,
    scope: Option<(&str, usize)>,
    target: &crate::program_model::LValue,
) -> Option<String> {
    use crate::program_model::LValue;
    fn walk(
        model: &Model<'_>,
        scope: Option<(&str, usize)>,
        target: &LValue,
    ) -> Option<(String, Ty)> {
        match target {
            LValue::Name(name) => {
                if let Some((instance, program)) = scope
                    && let Some((member, retained)) = model.program_types[program].member(name)
                {
                    let area = if retained { "r" } else { "p" };
                    return Some((
                        format!("w.{area}.{}.{}", rust_field(instance), member.field()),
                        member.ty,
                    ));
                }
                let global = model.global(name)?;
                let area = if global.retained { "r" } else { "g" };
                Some((format!("w.{area}.{}", rust_field(&global.name)), global.ty))
            }
            LValue::Field { target, field } => {
                let (base, ty) = walk(model, scope, target)?;
                let member = model
                    .types
                    .info(ty)
                    .members()
                    .iter()
                    .find(|member| member.name.eq_ignore_ascii_case(field))?;
                Some((format!("{base}.{}", member.field()), member.ty))
            }
            LValue::Index { .. } | LValue::Deref(_) => None,
        }
    }
    walk(model, scope, target).map(|(place, _)| place)
}

fn entries(
    source: &mut String,
    model: &Model<'_>,
    target: Target,
    references: &[String],
) -> Vec<Entry> {
    let abi = if target == Target::X86_64 {
        "sysv64"
    } else {
        "C"
    };
    let mut entries = Vec::new();
    for (index, instance) in model.instances.iter().enumerate() {
        let program = &model.program_types[instance.program];
        let symbol = format!("trust_entry_p{index}");
        let field = rust_field(&instance.name);
        let _ = writeln!(
            source,
            "#[allow(unsafe_code)]\n#[unsafe(no_mangle)]\npub extern \"{abi}\" fn {symbol}(cx: *mut Cx) -> u32 {{\n    run(cx, |cx, areas| {{\n        let w = World::new(cx, &areas)?;\n        prog_{}(cx, &w, &w.p.{field}, &w.r.{field})\n    }})\n}}\n",
            mangle(&program.def.name)
        );
        entries.push(Entry {
            symbol,
            kind: EntryKind::ProgramBody,
            index: u32::try_from(index).unwrap_or(u32::MAX),
        });
    }
    let initialization = if references.is_empty() {
        "run(cx, |_, _| Ok(()))".to_string()
    } else {
        let _ = writeln!(
            source,
            "fn init_references(cx: &mut Cx, w: &World<'_>) -> R<()> {{\n    {}\n    Ok(())\n}}\n",
            references.join("\n    ")
        );
        "run(cx, |cx, areas| {\n        let w = World::new(cx, &areas)?;\n        init_references(cx, &w)\n    })".to_string()
    };
    for (symbol, kind) in [
        ("trust_entry_cold", EntryKind::ColdInit),
        ("trust_entry_warm", EntryKind::WarmInit),
    ] {
        let _ = writeln!(
            source,
            "#[allow(unsafe_code)]\n#[unsafe(no_mangle)]\npub extern \"{abi}\" fn {symbol}(cx: *mut Cx) -> u32 {{\n    {initialization}\n}}\n"
        );
        entries.push(Entry {
            symbol: symbol.to_string(),
            kind,
            index: 0,
        });
    }
    entries
}
