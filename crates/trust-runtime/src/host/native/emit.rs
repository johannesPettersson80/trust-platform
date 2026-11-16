//! The generated crate (specification 44 §3, step 3; §6.3): the types, the program structures and
//! the areas as `#[repr(C)]` structures of cells with an assertion of every offset, every POU as a
//! function, and the entry points the kernel calls.

mod bodies;
mod connections;
mod rust_glue;
mod rust_places;
mod st_bindings;
mod st_blocks;
mod st_defaults;
mod st_types;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;

use super::classes::Owner;
use super::entries::{entries, reference_initializers, Entry};
use super::front::{Front, StatementIds};
use super::layers::BodyPou;
use super::lower::{FrameVar, Need, PRELUDE_IMPORTS, PRELUDE_ITEMS};
use super::model::{AreaLayout, Model};
use super::recursion::{CallGraph, Recursion};
use super::rust_pous::StructuredTextUse;
use super::types::{mangle, rust_field, Kind, Ty};
use super::NativeError;
use crate::program_model::{FunctionBlockBase, FunctionDef, MethodDef, Param};
use bodies::{body, BodyContext};
pub(super) use bodies::{find_pou, method_body};
use smol_str::SmolStr;
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
    /// The methods a tool may call between scans, which the tool symbols name (specification 44
    /// §5.7, §13.3).
    pub(super) tools: Vec<super::tools::Tool>,
    /// Layer 1 of specification 48 §9.2, for a project whose crate calls Structured Text; this
    /// crate, layer 3, then takes its types and functions from it.
    pub(super) layer_one: Option<LayerOne>,
}

/// Layer 1 (specification 48 §9.2, RU-39): the crate `trust_st`, whose library `st` holds the
/// Structured Text that the project's crate calls, with everything it reaches, and the bindings.
pub(super) struct LayerOne {
    /// The generated module of this generation's image, the text of `__generated`.
    pub(super) module: String,
    /// The bindings, the same for every image.
    pub(super) bindings: String,
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
    /// A method or a textual action of the program type of this index.
    ProgramMethod(usize, &'a MethodDef),
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
    // Each body's function with its text, which goes to layer 1 or to this crate.
    let mut texts: Vec<(String, String)> = Vec::new();
    let mut pous = 0usize;
    let mut statement_pous = vec![u16::MAX; ids.order.len()];
    let mut queued: Vec<Need> = Vec::new();
    let mut dispatchers: Vec<Need> = Vec::new();
    let mut pou_frames: Vec<PouFrame> = Vec::new();
    let mut frame_types = String::new();
    let mut done_blocks: Vec<Ty> = Vec::new();
    let mut work: Vec<Work<'a>> = (0..model.program_types.len()).map(Work::Program).collect();
    // CODESYS's implicit `FB_init`: the initializations call the method of every instance whose
    // type declares it (specification 10 §4.8).
    let fb_init = super::fb_init::calls(front, model)?;
    for need in fb_init.needs {
        queue(model, need, &mut queued, &mut work)?;
    }
    // CODESYS's implicit `FB_exit`: the exit before a restart or a download calls the method of
    // every instance whose type declares it, in the reverse order.
    let fb_exit = super::fb_exit::calls(front, model)?;
    for need in fb_exit.needs {
        queue(model, need, &mut queued, &mut work)?;
    }
    // CODESYS's implicit `FB_reinit`: an online change calls the method of every instance it
    // copied.
    let fb_reinit = super::fb_reinit::calls(model)?;
    for need in fb_reinit.needs {
        queue(model, need, &mut queued, &mut work)?;
    }
    // The Structured Text functions that the crate calls, whether a program calls them or not
    // (specification 48 RU-40).
    let named = structured_text_calls(front, model, &mut queued, &mut work)?;
    let roots = named.roots;
    let named_types = named.types;
    let held_methods = named.methods;
    let mut bodies: BTreeMap<String, BodyPou> = BTreeMap::new();
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
            // once every body is generated; a method it needs may need more. A tool's call of a
            // method goes through its interface's dispatcher (specification 44 §13.3).
            for need in super::tools::needs(model)? {
                need.add_to(&mut dispatchers);
            }
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
            Work::ProgramMethod(program, method) => Work::ProgramMethod(*program, method),
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
        };
        let of = body_pou(front, model, &item);
        let (text, statements, needs, name, called, frame) = match body(&context, model, &item) {
            Ok(generated) => generated,
            // A body that holds a construct not compiled yet outside its statements is left out,
            // and the others are still lowered to find theirs (specification 11 §6.8.1c).
            Err(error) if error.is_unsupported() => {
                model.refused.push(error);
                continue;
            }
            Err(error) => return Err(error),
        };
        if let Some((pou_frame, layout)) = frame {
            frame_types.push_str(&layout);
            pou_frames.push(pou_frame);
        }
        graph.body(&name, called);
        bodies.insert(name.clone(), of);
        pous += 1;
        for statement in statements {
            if let Some(slot) = statement_pous.get_mut(statement as usize) {
                *slot = pou;
            }
        }
        texts.push((name, text));
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
    // A build that met constructs it does not compile yet fails with every one of them
    // (specification 44 NX-09, specification 11 §6.8.1c).
    if let Some(refused) = NativeError::refusals(std::mem::take(&mut model.refused)) {
        return Err(refused);
    }
    let tools = super::tools::tools(model)?;
    texts.extend(super::dispatch::emit(model, &dispatchers, &mut graph)?);
    // Layer 1 is what the Structured Text that the crate calls reaches (specification 48 §9.2);
    // the rest is this crate's.
    let layer_one = super::layers::layer_one(&graph, &roots, &bodies)?;
    // A crate that names a type of the project's Structured Text, and calls none of it, still needs
    // its binding in layer 1 (RU-39, RU-42).
    let layered = !layer_one.is_empty() || !named_types.is_empty();
    let (layer_one_functions, mut functions) = split(texts, &layer_one);
    // The places of the crate's structures that the Rust POUs' values reach (specification 48
    // RU-29).
    functions.push_str(&rust_places::places(
        &model.types,
        &model.rust_places,
        &front.rust_structures,
    )?);
    connections::connections(front, ids, model, &mut functions)?;

    let mut source = head(target, layered);
    let shared = shared(model, &frame_types);
    let layer_one = if layered {
        let bindings = st_bindings::bindings(model, &roots, &named_types, &held_methods)?;
        Some(LayerOne {
            module: format!("{PRELUDE_IMPORTS}{PRELUDE_ITEMS}\n{shared}{layer_one_functions}"),
            bindings,
        })
    } else {
        source.push_str(&shared);
        None
    };
    source.push_str(&functions);
    let references = reference_initializers(front, model)?;
    let lifecycle = super::entries::Lifecycle {
        inits: &fb_init.variables,
        exits: &fb_exit.variables,
        reinits: &fb_reinit.variables,
    };
    let mut entries = entries(&mut source, model, target, &references, &lifecycle)?;
    entries.extend(super::tools::emit(&mut source, model, &tools, target));
    source.push_str(
        "\n#[panic_handler]\nfn panic(_: &core::panic::PanicInfo<'_>) -> ! {\n    loop {}\n}\n",
    );
    // No construct that a later step met and left out may reach the crate (NX-09).
    if let Some(refused) = NativeError::refusals(std::mem::take(&mut model.refused)) {
        return Err(refused);
    }
    Ok((
        Generated {
            source,
            entries,
            statement_pous,
            pous: pou_frames,
            tools,
            layer_one,
        },
        graph,
    ))
}

/// The head of the generated crate for `target`: its attributes, the prelude, and, in layer 3 of a
/// project whose crate calls Structured Text (`layered`), the use of layer 1, where the prelude's
/// items lie (specification 48 §9.2).
fn head(target: Target, layered: bool) -> String {
    let mut source = String::from(
        "//! A truST program compiled for the kernel interface (specification 44). Generated; do not edit.\n\
         #![no_std]\n#![no_main]\n#![deny(unsafe_code)]\n\
         #![allow(non_snake_case, non_camel_case_types, unused_parens, unused_variables, unused_mut, \
         dead_code, unreachable_code, unused_labels, unused_braces, clippy::all)]\n\n",
    );
    source.push_str(PRELUDE_IMPORTS);
    if layered {
        source.push_str("use ::st::__generated::*;\n");
    } else {
        source.push_str(PRELUDE_ITEMS);
    }
    if target.is_computer() {
        // The memory functions a large copy or fill calls, which no C library brings
        // (specification 44 §4.5).
        source.push_str("extern crate trust_plc_mem;\n");
    }
    source.push('\n');
    source
}

/// The types, the program structures, the frames `frame_types` and the areas, which layer 1 holds
/// when the project's crate calls Structured Text (specification 48 §9.2).
fn shared(model: &Model<'_>, frame_types: &str) -> String {
    let mut shared = String::new();
    types(&mut shared, model);
    programs(&mut shared, model);
    shared.push_str(frame_types);
    areas(&mut shared, model, model.images);
    shared
}

/// The functions of `texts` in layer 1, those `layer_one` names, and the others, this crate's.
fn split(texts: Vec<(String, String)>, layer_one: &BTreeSet<String>) -> (String, String) {
    let mut functions = (String::new(), String::new());
    for (name, text) in texts {
        if layer_one.contains(&name) {
            functions.0.push_str(&text);
        } else {
            functions.1.push_str(&text);
        }
    }
    functions
}

/// The library of layer 1 (specification 48 RU-39) from the generated module of a release image,
/// `release`, and of a debug image, `debug`, which its cargo feature `hooks` chooses, and the
/// `bindings`: one text for every target and every image.
pub(super) fn layer_one_library(release: &str, debug: &str, bindings: &str) -> String {
    format!(
        "//! The Structured Text that the project's crate calls (specification 48 RU-39). Generated \
         by truST from the project; do not edit.\n\
         #![no_std]\n#![deny(unsafe_code)]\n#![allow(warnings, clippy::all, clippy::pedantic)]\n\n\
         /// The generated code of the Structured Text in a release image, which the bindings and the \
         build call.\n\
         #[cfg(not(feature = \"hooks\"))]\n#[doc(hidden)]\npub mod __generated {{\n{release}}}\n\n\
         /// The generated code of the Structured Text in a debug image, with the debugger's statement \
         hooks (specification 44 §13).\n\
         #[cfg(feature = \"hooks\")]\n#[doc(hidden)]\npub mod __generated {{\n{debug}}}\n\n\
         {bindings}"
    )
}

/// Queues the Structured Text function of each call that the project's crate makes (specification
/// 48 RU-38) and gives the function's name with the call, and the project's data types the crate
/// names, whose bindings the crate writes values of; refuses a name of anything else.
fn structured_text_calls<'a>(
    front: &Front,
    model: &mut Model<'a>,
    queued: &mut Vec<Need>,
    work: &mut Vec<Work<'a>>,
) -> Result<Named, NativeError> {
    let mut roots = Vec::new();
    let mut types = Vec::new();
    let mut methods: HashMap<String, Vec<st_blocks::HeldMethod>> = HashMap::new();
    for used in &front.structured_text {
        let used = &resolved(front, model, used)?;
        let call = format!("st::{} at {}:{}", used.name, used.file, used.line);
        if front.rust.contains_key(&used.name.to_ascii_uppercase()) {
            return Err(NativeError::Compile(format!(
                "{call} names a Rust POU, which the crate calls as Rust (specification 48 RU-38)"
            )));
        }
        if let Some(def) = model.function(&used.name) {
            queue(model, Need::Function(def.name.clone()), queued, work)?;
            roots.push((format!("f_{}", mangle(&def.name)), used.clone()));
        } else if let Some(owner) = model.types.owner(&used.name) {
            // A function block or a class a Rust POU holds (RU-39), laid out as its field's type:
            // a block's body, which every function block type of the program has; a class has none
            // (IEC §6.6.5).
            let what = if owner.is_class() {
                "class"
            } else {
                "function block"
            };
            if model.types.by_rust(&owner.rust()).is_none() {
                return Err(NativeError::Compile(format!(
                    "{call} names the Structured Text {what} {}, which a Rust function block or \
                     program holds as a field, `name: st::{}`, and no field holds (specification \
                     48 RU-38)",
                    owner.name(),
                    used.name
                )));
            }
            if !owner.is_class() {
                roots.push((format!("fb_{}", mangle(owner.name())), used.clone()));
            }
            // Its `PUBLIC` methods, which its binding calls, lie in layer 1 with it (RU-39).
            if !methods.contains_key(owner.name().as_str()) {
                let held = held_methods(front, model, owner);
                for method in &held {
                    queue(
                        model,
                        Need::Method(method.declaring.clone(), method.method.clone()),
                        queued,
                        work,
                    )?;
                    let root = format!(
                        "m_{}__{}",
                        mangle(&method.declaring),
                        mangle(&method.method)
                    );
                    roots.push((root, used.clone()));
                }
                methods.insert(owner.name().to_string(), held);
            }
        } else if let Some(ty) = named_type(model, &used.name)? {
            // A structure, an enumeration or a type with named values of the project, whose
            // binding the crate writes values of (RU-39).
            if let Some(why) = st_bindings::unpassed(&model.types, ty) {
                let info = model.types.info(ty);
                let what = match &info.kind {
                    Kind::Struct { .. } => "structure",
                    Kind::Enum { named: true, .. } => "type with named values",
                    _ => "enumeration",
                };
                return Err(NativeError::Compile(format!(
                    "{call} names the {what} {}, which {why} (specification 48 RU-39, milestone R3)",
                    info.name
                )));
            }
            types.push(ty);
        } else {
            return Err(NativeError::Compile(format!(
                "{call} names no Structured Text function, function block, structure, \
                 enumeration or type with named values of the project (specification 48 RU-40)"
            )));
        }
    }
    Ok(Named {
        roots,
        types,
        methods,
    })
}

/// `used` named by what its path names (specification 48 RU-39, IEC §6.9): the longest prefix of
/// the path's names, joined with `.`, that names a Rust POU or a Structured Text function, function
/// block, class or data type (`Motion.MOVE_BY` itself, `MODE.RUN` the enumeration `MODE`); the whole
/// path when none does, which the call then refuses.
fn resolved(
    front: &Front,
    model: &mut Model<'_>,
    used: &StructuredTextUse,
) -> Result<StructuredTextUse, NativeError> {
    let names: Vec<&str> = used.name.split('.').collect();
    for end in (1..=names.len()).rev() {
        let name = names[..end].join(".");
        let named = front.rust.contains_key(&name.to_ascii_uppercase())
            || model.function(&name).is_some()
            || model.types.owner(&name).is_some()
            || named_type(model, &name)?.is_some();
        if named {
            return Ok(StructuredTextUse {
                name,
                ..used.clone()
            });
        }
    }
    Ok(used.clone())
}

/// The Structured Text that the project's crate names (specification 48 RU-38): the generated
/// function of each POU it calls, with the call, and the data types it writes values of.
struct Named {
    roots: Vec<(String, StructuredTextUse)>,
    types: Vec<Ty>,
    /// The `PUBLIC` methods of each held block, by the block's name.
    methods: HashMap<String, Vec<st_blocks::HeldMethod>>,
}

/// The `PUBLIC` methods a held block or class `owner` has (RU-39): its own and those it inherits
/// from the blocks and classes it extends and does not override (IEC §6.6.5.5), each with the
/// block or class that declares it and the path to that one's part of the instance.
fn held_methods<'m>(
    front: &Front,
    model: &Model<'m>,
    owner: Owner<'m>,
) -> Vec<st_blocks::HeldMethod> {
    let mut held = Vec::new();
    let mut seen: Vec<SmolStr> = Vec::new();
    let mut path = String::new();
    let mut current = Some(owner);
    while let Some(block) = current {
        for method in block.methods() {
            if seen
                .iter()
                .any(|name| name.eq_ignore_ascii_case(&method.name))
            {
                continue;
            }
            seen.push(method.name.clone());
            if is_public_method(front, block.name(), &method.name) {
                held.push(st_blocks::HeldMethod {
                    declaring: block.name().clone(),
                    method: method.name.clone(),
                    path: path.clone(),
                });
            }
        }
        // A block extends a block or a class, a class a class (IEC §6.6.5.5).
        current = match block {
            Owner::Fb(def) => match &def.base {
                Some(FunctionBlockBase::FunctionBlock(base) | FunctionBlockBase::Class(base)) => {
                    model.types.owner(base)
                }
                None => None,
            },
            Owner::Class(def) => def.base.as_deref().and_then(|base| model.types.owner(base)),
        };
        path.push_str(".__BASE");
    }
    held
}

/// Whether trust-hir declares the method `method` of the function block `block` `PUBLIC`, which
/// code outside the block may call (IEC §6.6.5.4, Table 48 features 5a to 5d: a method is
/// `PROTECTED` unless it says otherwise).
fn is_public_method(front: &Front, block: &str, method: &str) -> bool {
    front.sources.iter().any(|source| {
        let symbols = &source.analysis.symbols;
        symbols.iter().any(|symbol| {
            matches!(symbol.kind, trust_hir::symbols::SymbolKind::Method { .. })
                && symbol.name.eq_ignore_ascii_case(method)
                && symbol.visibility == trust_hir::symbols::Visibility::Public
                && symbol
                    .parent
                    .and_then(|parent| symbols.get(parent))
                    .is_some_and(|owner| owner.name.eq_ignore_ascii_case(block))
        })
    })
}

/// The structure, enumeration or type with named values named `name`, laid out, when the project
/// declares one.
fn named_type(model: &mut Model<'_>, name: &str) -> Result<Option<Ty>, NativeError> {
    let Some(id) = model.types.registry().lookup(name) else {
        return Ok(None);
    };
    let is_data_type = matches!(
        model.types.registry().get(id),
        Some(trust_hir::Type::Struct { .. } | trust_hir::Type::Enum { .. })
    );
    if !is_data_type {
        return Ok(None);
    }
    let ty = model.ty(id)?;
    Ok(matches!(
        model.types.info(ty).kind,
        Kind::Struct { .. } | Kind::Enum { .. }
    )
    .then_some(ty))
}

/// The POU whose body `item` is, and whether it is a Rust POU (specification 48 RU-40).
fn body_pou(front: &Front, model: &Model<'_>, item: &Work<'_>) -> BodyPou {
    let (name, pou) = match item {
        Work::Program(index) => {
            let name = model.program_types[*index].def.name.to_string();
            (name.clone(), name)
        }
        Work::FunctionBlock(ty) => {
            let name = model.types.info(*ty).name.to_string();
            (name.clone(), name)
        }
        Work::Method(owner, method, _) => (
            format!("{}.{}", owner.name(), method.name),
            owner.name().to_string(),
        ),
        Work::ProgramMethod(index, method) => {
            let owner = model.program_types[*index].def.name.to_string();
            (format!("{owner}.{}", method.name), owner)
        }
        Work::Function(def) => (def.name.to_string(), def.name.to_string()),
    };
    BodyPou {
        rust: front.rust.contains_key(&pou.to_ascii_uppercase()),
        name,
    }
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
        Need::ProgramMethod(program, method) => {
            let def = model.program_types[*program].def;
            let Some(found) = def
                .methods
                .iter()
                .find(|candidate| candidate.name.eq_ignore_ascii_case(method))
            else {
                return Err(NativeError::Compile(format!(
                    "method {}.{method} is not declared",
                    def.name
                )));
            };
            work.push(Work::ProgramMethod(*program, found));
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
/// in-out and an output by reference, and a string in-out as a view of the caller's string.
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
        } else if param.direction == trust_hir::symbols::ParamDirection::InOut
            && matches!(info.kind, Kind::Str(_) | Kind::WStr(_))
        {
            // A string in-out is a view of the caller's string, of any capacity of its family
            // (docs/IEC_DEVIATIONS.md DEV-035, §7.3).
            let view = if matches!(info.kind, Kind::WStr(_)) {
                "WStrView"
            } else {
                "StrView"
            };
            let _ = write!(parameters, ", {argument}: abi::{view}<'_>");
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
pub(super) fn structure(
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
                // A structure that holds an instance gets no `Assign`: it is copied cell by cell
                // (`Plain::copy_cells`, `lower/copies.rs`).
                let assign = matches!(info.kind, Kind::Struct { .. })
                    && !members
                        .iter()
                        .any(|member| model.types.holds_instance(member.ty));
                structure(source, &info.rust, &fields, info.size, assign);
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
    let _ = writeln!(source, "pub type AreaI = [Cell<u8>; {}];", images[0]);
    let _ = writeln!(source, "pub type AreaQ = [Cell<u8>; {}];", images[1]);
    let _ = writeln!(source, "pub type AreaM = [Cell<u8>; {}];", images[2]);
    area(source, "AreaG", &model.area_g);
    area(source, "AreaR", &model.area_r);
    area(source, "AreaP", &model.area_p);
    source.push_str(
        "/// The areas of one entry call (specification 44 §7.1).
pub struct World<'a> {
    pub i: &'a AreaI,
    pub q: &'a AreaQ,
    pub m: &'a AreaM,
    pub g: &'a AreaG,
    pub r: &'a AreaR,
    pub p: &'a AreaP,
    /// The areas, for references (specification 44 §7.3). A reference, not a copy: a module has no
    /// `memcpy`.
    pub areas: &'a Areas<'a>,
}

impl<'a> World<'a> {
    pub fn new(cx: &mut Cx, areas: &'a Areas<'a>) -> R<Self> {
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
