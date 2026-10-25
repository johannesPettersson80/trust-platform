//! The generated crate (specification 44 §3, step 3; §6.3): the types, the program structures and
//! the areas as `#[repr(C)]` structures of cells with an assertion of every offset, every POU as a
//! function, and the entry points the kernel calls.

mod bodies;
mod connections;

use std::fmt::Write as _;

use super::classes::Owner;
use super::entries::{entries, reference_initializers, Entry};
use super::front::{Front, StatementIds};
use super::lower::{FrameVar, Need, PRELUDE};
use super::model::{AreaLayout, Model};
use super::recursion::{CallGraph, Recursion};
use super::types::{mangle, rust_field, Kind, Ty};
use super::NativeError;
use crate::program_model::{FunctionDef, MethodDef, Param};
use bodies::{body, BodyContext};
use trust_runtime_core::native::image::Target;

/// The generated crate and what the image's tables need of it.
pub(super) struct Generated {
    pub(super) source: String,
    pub(super) entries: Vec<Entry>,
    /// The POU of each statement, by statement id; `u16::MAX` for a statement of no compiled POU.
    pub(super) statement_pous: Vec<u16>,
    /// The POUs of a debug image with their frames, by POU id, which its POU symbols and frame
    /// variable symbols name (specification 44 §5.7); empty for a release image.
    pub(super) pous: Vec<PouFrame>,
}

/// One POU of a debug image: its name, the type its instance has, and its frame's variables at
/// their offsets (specification 44 §5.7, §13.2).
pub(super) struct PouFrame {
    /// The POU's id.
    pub(super) id: u16,
    /// The POU's declared name, `Owner.Method` for a method.
    pub(super) name: String,
    /// The type of the instance the POU runs on: a program's or a function block's; `None` for a
    /// function.
    pub(super) instance: Option<PouInstance>,
    /// The frame variables with their offsets in the frame.
    pub(super) vars: Vec<(FrameVar, u32)>,
    /// The frame's size in bytes.
    pub(super) size: u32,
}

/// The type of the instance a POU runs on.
#[derive(Clone, Copy)]
pub(super) enum PouInstance {
    /// The program type of this index.
    Program(usize),
    /// A function block or class type.
    Block(Ty),
}

/// What a body is, for the generator's work list.
enum Work<'a> {
    Program(usize),
    FunctionBlock(Ty),
    Method(Owner<'a>, &'a MethodDef, Ty),
    Function(&'a FunctionDef),
}

/// Generates the crate of `model` for `target`, with the calls `recursion` names bounded by the call
/// depth, and returns it with its call graph (specification 44 §6.4).
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
    recursion: &Recursion,
) -> Result<(Generated, CallGraph), NativeError> {
    let mut graph = CallGraph::default();
    let mut functions = String::new();
    let mut pous = 0usize;
    let mut statement_pous = vec![u16::MAX; ids.order.len()];
    let mut queued: Vec<Need> = Vec::new();
    let mut dispatchers: Vec<Need> = Vec::new();
    let mut pou_frames: Vec<PouFrame> = Vec::new();
    let mut frame_types = String::new();
    let mut done_blocks: Vec<Ty> = Vec::new();
    let mut work: Vec<Work<'a>> = (0..model.program_types.len()).map(Work::Program).collect();
    let mut index = 0;
    loop {
        // Every function block type the bodies use needs its body; a class has none.
        let blocks: Vec<Ty> = model
            .types
            .all()
            .filter(|(_, info)| {
                matches!(
                    info.kind,
                    Kind::Fb {
                        is_class: false,
                        ..
                    }
                )
            })
            .map(|(ty, _)| ty)
            .filter(|ty| !done_blocks.contains(ty))
            .collect();
        for ty in blocks {
            done_blocks.push(ty);
            work.push(Work::FunctionBlock(ty));
        }
        let Some(item) = work.get(index) else {
            // A dispatcher calls the method of every type that can be there, and those are known
            // once every body is generated; a method it needs may need more.
            let mut added = false;
            for need in super::dispatch::targets(model, &dispatchers)? {
                added |= queue(model, need, &mut queued, &mut work)?;
            }
            if added {
                continue;
            }
            break;
        };
        let item = match item {
            Work::Program(program) => Work::Program(*program),
            Work::FunctionBlock(ty) => Work::FunctionBlock(*ty),
            Work::Method(owner, method, ty) => Work::Method(*owner, method, *ty),
            Work::Function(def) => Work::Function(def),
        };
        index += 1;
        let pou = u16::try_from(pous).unwrap_or(u16::MAX);
        let context = BodyContext {
            front,
            ids,
            hooks,
            recursion,
            pou,
            pointer_bytes: target.pointer_bytes(),
        };
        let (text, statements, needs, name, called, frame) = body(&context, model, &item)?;
        if let Some((pou_frame, layout)) = frame {
            frame_types.push_str(&layout);
            pou_frames.push(pou_frame);
        }
        graph.body(&name, called);
        pous += 1;
        for statement in statements {
            if let Some(slot) = statement_pous.get_mut(statement as usize) {
                *slot = pou;
            }
        }
        functions.push_str(&text);
        for need in needs {
            match need {
                Need::Virtual(..) | Need::Interface(..) | Need::Body(_) | Need::Cast(_) => {
                    need.add_to(&mut dispatchers);
                }
                other => {
                    queue(model, other, &mut queued, &mut work)?;
                }
            }
        }
    }
    functions.push_str(&super::dispatch::emit(model, &dispatchers, &mut graph)?);
    connections::connections(front, ids, model, target.pointer_bytes(), &mut functions)?;

    let mut source = String::new();
    source.push_str(
        "//! A truST program compiled for the kernel interface (specification 44). Generated; do not edit.\n\
         #![no_std]\n#![no_main]\n#![deny(unsafe_code)]\n\
         #![allow(non_snake_case, non_camel_case_types, unused_parens, unused_variables, unused_mut, \
         dead_code, unreachable_code, unused_labels, unused_braces, clippy::all)]\n\n",
    );
    source.push_str(PRELUDE);
    if target.is_computer() {
        // The memory functions a large copy or fill calls, which no C library brings
        // (specification 44 §4.5).
        source.push_str("extern crate trust_plc_mem;\n");
    }
    source.push('\n');
    types(&mut source, model);
    programs(&mut source, model);
    source.push_str(&frame_types);
    let images = model.images;
    areas(&mut source, model, images);
    source.push_str(&functions);
    let references = reference_initializers(front, model)?;
    let entries = entries(&mut source, model, target, &references)?;
    source.push_str(
        "\n#[panic_handler]\nfn panic(_: &core::panic::PanicInfo<'_>) -> ! {\n    loop {}\n}\n",
    );
    Ok((
        Generated {
            source,
            entries,
            statement_pous,
            pous: pou_frames,
        },
        graph,
    ))
}

/// Queues the body of the function or method `need` names, once; returns whether it is new.
fn queue<'a>(
    model: &Model<'a>,
    need: Need,
    queued: &mut Vec<Need>,
    work: &mut Vec<Work<'a>>,
) -> Result<bool, NativeError> {
    if queued.contains(&need) {
        return Ok(false);
    }
    match &need {
        Need::Function(name) => {
            let Some(def) = model.function(name) else {
                return Err(NativeError::Compile(format!(
                    "function {name} is not declared"
                )));
            };
            work.push(Work::Function(def));
        }
        Need::Method(owner, method) => {
            let Some(def) = model.types.owner(owner) else {
                return Err(NativeError::Compile(format!(
                    "function block {owner} is not declared"
                )));
            };
            let Some(found) = def
                .methods()
                .iter()
                .find(|candidate| candidate.name.eq_ignore_ascii_case(method))
            else {
                return Err(NativeError::Compile(format!(
                    "method {owner}.{method} is not declared"
                )));
            };
            let Some(ty) = model.types.by_rust(&def.rust()) else {
                return Err(NativeError::Compile(format!(
                    "function block {owner} has no layout"
                )));
            };
            work.push(Work::Method(def, found, ty));
        }
        _ => return Ok(false),
    }
    queued.push(need);
    Ok(true)
}

/// The Rust name of method `method` of `owner`.
pub(super) fn method_function(owner: Owner<'_>, method: &MethodDef) -> String {
    format!("m_{}__{}", mangle(owner.name()), mangle(&method.name))
}

/// The Rust form of a function's or a method's call (specification 44 §6.4): the parameters after
/// `w` (or `me`), the names a dispatcher passes them on by, and the return type.
pub(super) struct Signature {
    pub(super) parameters: String,
    pub(super) names: Vec<String>,
    pub(super) returns: String,
}

/// The type a function or a method returns, none for `VOID` or no return type. A result of a
/// function block's type is a reference to the instance it names, as `THIS` is a reference to the
/// own instance (IEC §6.6.5.7.2) and the runtime returns the instance itself (§6.4).
pub(super) fn return_type(
    model: &mut Model<'_>,
    ret: Option<trust_hir::TypeId>,
) -> Result<Option<Ty>, NativeError> {
    match ret {
        Some(ret)
            if model
                .types
                .registry()
                .get(ret)
                .is_some_and(|ty| !matches!(ty, trust_hir::Type::Void)) =>
        {
            let ty = model.ty(ret)?;
            if model.types.info(ty).kind.is_instance() {
                return Ok(Some(model.types.reference(ty)));
            }
            Ok(Some(ty))
        }
        _ => Ok(None),
    }
}

/// The signature of a function or a method returning `ret` with `params`: a scalar result is
/// returned, any other written to `out`; a scalar input is passed by value, any other input, an
/// in-out and an output by reference.
pub(super) fn signature(
    model: &mut Model<'_>,
    ret: Option<trust_hir::TypeId>,
    params: &[Param],
) -> Result<Signature, NativeError> {
    let mut parameters = String::new();
    let mut names = Vec::new();
    let mut returns = "()".to_string();
    if let Some(ret) = return_type(model, ret)? {
        let info = model.types.info(ret);
        if super::lower::is_scalar(&info.kind) {
            returns = info.scalar().map_or("()", |elem| elem.rust()).to_string();
        } else {
            let _ = write!(parameters, ", out: &{}", info.rust);
            names.push("out".to_string());
        }
    }
    for param in params {
        // `EN` is TRUE whenever the body runs, since a call it disables does not call the
        // function (specification 10 §7.5); `ENO` is an output like any other.
        if param.name.eq_ignore_ascii_case("EN") {
            continue;
        }
        let ty = model.ty(param.type_id)?;
        let info = model.types.info(ty);
        let argument = super::lower::parameter(&param.name);
        if matches!(info.kind, Kind::Open { .. }) {
            // An open array is a view of the caller's array, passed as it is (§6.2).
            let _ = write!(parameters, ", {argument}: {}", info.rust);
        } else if param.direction == trust_hir::symbols::ParamDirection::In
            && super::lower::is_scalar(&info.kind)
        {
            let elem = info.scalar().map_or("u8", |elem| elem.rust());
            let _ = write!(parameters, ", {argument}: {elem}");
        } else {
            let _ = write!(parameters, ", {argument}: &{}", info.rust);
        }
        names.push(argument);
    }
    Ok(Signature {
        parameters,
        names,
        returns,
    })
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
                // A derived type's fields are its base, first, and its own variables; the base's
                // variables lie inside the base (specification 44 §6.2).
                let base = match info.kind {
                    Kind::Fb {
                        base: Some(base), ..
                    } => Some(("__BASE".to_string(), model.types.info(base).rust.clone(), 0)),
                    _ => None,
                };
                let fields: Vec<(String, String, u32)> =
                    base.into_iter()
                        .chain(members.iter().filter(|member| member.via.is_empty()).map(
                            |member| {
                                (
                                    member.field(),
                                    model.types.info(member.ty).rust.clone(),
                                    member.offset,
                                )
                            },
                        ))
                        .collect();
                structure(
                    source,
                    &info.rust,
                    &fields,
                    info.size,
                    matches!(info.kind, Kind::Struct { .. }),
                );
            }
            // A `STRUCT OVERLAP` is its byte backing, which the views of its members read and write
            // (specification 44 §6.2).
            Kind::Overlap { .. } => {
                let fields = [("B".to_string(), format!("[Cell<u8>; {}]", info.size), 0)];
                structure(source, &info.rust, &fields, info.size, true);
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
