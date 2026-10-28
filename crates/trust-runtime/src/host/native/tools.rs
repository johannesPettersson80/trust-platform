//! The methods a tool calls on a live instance between scans (specification 44 §5.4, §5.7, §7.1,
//! §13.3). Every method of every interface that a laid-out function block or class implements has
//! a tool symbol. A method whose parameters are inputs that hold values has a call block in the
//! stack area, which the tool symbol's parts name, and a tool entry: it reads the block, calls the
//! method through the interface's dispatcher on the instance the block names, as a call through an
//! interface reference binds it (§6.4), gives each input the block does not mark as given its
//! declared initial value (IEC 61131-3 §6.6.1), and writes the result into the block. A method that
//! takes an output, an in-out, a function block instance or an array whose bounds a caller supplies
//! has a tool symbol without parts and no entry: a block cannot hold the caller's variable such a
//! parameter names.

use std::fmt::Write as _;

use smol_str::SmolStr;
use trust_hir::symbols::ParamDirection;
use trust_runtime_core::native::image::{EntryKind, Target, SYMBOL_INPUT};
use trust_runtime_core::native::tool::{tool_symbol, GIVEN_PART, THIS_PART};

use super::dispatch::interface_function;
use super::emit::{return_type, structure};
use super::entries::{entry_abi, Entry};
use super::lower::{is_scalar, Need};
use super::model::Model;
use super::types::{lay_out, rust_field, Elem, Kind, Member, Ty, TypeInfo};
use super::NativeError;
use crate::program_model::{InterfaceDef, MethodDef, Param};

/// A method that a tool may call: its tool symbol and, when a tool can call it, its call block.
pub(super) struct Tool {
    /// The tool symbol's canonical name, `@TOOL.<INTERFACE>.<METHOD>`.
    pub(super) canonical: String,
    /// The tool symbol's display name, `<Interface>.<Method>` as declared.
    pub(super) display: String,
    /// The interface's type, which `THIS` has.
    pub(super) interface: Ty,
    /// The call block with its entry; `None` for a method that a tool cannot call.
    pub(super) block: Option<Block>,
}

/// A tool entry's call block in the stack area, and the entry that reads it.
pub(super) struct Block {
    /// The entry point's exported symbol.
    pub(super) entry: String,
    /// The Rust type of the block.
    rust: String,
    /// `THIS`, `__GIVEN`, the inputs, each with the input flag, and the result, at their offsets.
    pub(super) parts: Vec<Member>,
    /// The block's bytes.
    pub(super) size: u32,
    /// The statements of the entry's body.
    code: String,
}

/// The interfaces that a laid-out function block or class implements, itself, through a base or
/// through an interface that extends one, in the order of their declarations, each with its
/// methods: its own, then those of the interfaces it extends, each name once, as a call through it
/// finds them.
fn implemented<'a>(model: &Model<'a>) -> Vec<(&'a InterfaceDef, Vec<&'a MethodDef>)> {
    let defs = model.types.defs;
    let owners: Vec<_> = model
        .types
        .classes()
        .into_iter()
        .filter_map(|(ty, _)| model.types.owner_of(ty))
        .collect();
    let levels = defs.interfaces().count();
    defs.interfaces()
        .filter(|interface| {
            owners
                .iter()
                .any(|owner| defs.implements(*owner, &interface.name))
        })
        .map(|interface| {
            let mut methods: Vec<&'a MethodDef> = Vec::new();
            let mut current = Some(interface);
            // A chain of more interfaces than the project has is a cycle, which trust-hir refuses.
            for _ in 0..=levels {
                let Some(here) = current else {
                    break;
                };
                for method in &here.methods {
                    if !methods
                        .iter()
                        .any(|known| known.name.eq_ignore_ascii_case(&method.name))
                    {
                        methods.push(method);
                    }
                }
                current = here.base.as_deref().and_then(|base| defs.interface(base));
            }
            (interface, methods)
        })
        .collect()
}

/// The inputs of `method` that its call block holds: every parameter but `EN`, which reads TRUE
/// whenever the method runs (specification 10 §7.5).
fn inputs(method: &MethodDef) -> impl Iterator<Item = &Param> {
    method
        .params
        .iter()
        .filter(|param| !param.name.eq_ignore_ascii_case("EN"))
}

/// Whether a tool can call `method`: its parameters are inputs whose values a call block holds,
/// neither a function block instance, which a call passes as the caller's own instance, nor an
/// array whose bounds a caller supplies (§6.4).
fn callable(model: &mut Model<'_>, method: &MethodDef) -> Result<bool, NativeError> {
    if method
        .params
        .iter()
        .any(|param| param.direction != ParamDirection::In)
    {
        return Ok(false);
    }
    for param in inputs(method) {
        let ty = model.ty(param.type_id)?;
        let kind = &model.types.info(ty).kind;
        if kind.is_instance() || matches!(kind, Kind::Open { .. }) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The dispatchers the tool entries call: one for each method a tool can call (§6.4).
///
/// # Errors
///
/// [`NativeError`] for a parameter's type the generator does not compile.
pub(super) fn needs(model: &mut Model<'_>) -> Result<Vec<Need>, NativeError> {
    let mut needs = Vec::new();
    for (interface, methods) in implemented(model) {
        for method in methods {
            if callable(model, method)? {
                Need::Interface(interface.name.clone(), method.name.clone()).add_to(&mut needs);
            }
        }
    }
    Ok(needs)
}

/// Every tool symbol of the program, with the call block of each method a tool can call; the stack
/// area holds the largest block (§7.1).
///
/// # Errors
///
/// [`NativeError`] for an input's initial value the generator does not compile.
pub(super) fn tools(model: &mut Model<'_>) -> Result<Vec<Tool>, NativeError> {
    let mut tools = Vec::new();
    let mut entries = 0usize;
    for (interface, methods) in implemented(model) {
        let interface_ty = model.types.interface(&interface.name)?;
        for method in methods {
            let block = if callable(model, method)? {
                let block = block(model, &interface.name, interface_ty, method, entries)?;
                entries += 1;
                model.stack = model.stack.max(block.size);
                Some(block)
            } else {
                None
            };
            tools.push(Tool {
                canonical: tool_symbol(&interface.name, &method.name),
                display: format!("{}.{}", interface.name, method.name),
                interface: interface_ty,
                block,
            });
        }
    }
    Ok(tools)
}

/// The type of a mask of `bytes` bytes: `ARRAY[0..bytes - 1] OF BYTE`.
fn mask_type(model: &mut Model<'_>, bytes: usize) -> Ty {
    let byte = model.types.elem(Elem::Byte);
    model.types.intern(TypeInfo {
        kind: Kind::Array {
            elem: byte,
            dims: vec![(0, i64::try_from(bytes).unwrap_or(i64::MAX) - 1)],
        },
        size: u32::try_from(bytes).unwrap_or(u32::MAX),
        align: 1,
        name: SmolStr::default(),
        rust: format!("[Cell<u8>; {bytes}]"),
    })
}

/// The call block of `method` of `interface`, whose type is `interface_ty`, laid out as a structure
/// (§7.2): `THIS`, the mask `__GIVEN` of one bit per input and at least one byte, the inputs in
/// their order, and the result by the method's name; with the body of tool entry `number`.
fn block(
    model: &mut Model<'_>,
    interface: &SmolStr,
    interface_ty: Ty,
    method: &MethodDef,
    number: usize,
) -> Result<Block, NativeError> {
    let inputs: Vec<&Param> = inputs(method).collect();
    let mask = mask_type(model, inputs.len().div_ceil(8).max(1));
    let mut fields = vec![
        (SmolStr::new_static(THIS_PART), interface_ty, 0),
        (SmolStr::new_static(GIVEN_PART), mask, 0),
    ];
    for param in &inputs {
        fields.push((param.name.clone(), model.ty(param.type_id)?, SYMBOL_INPUT));
    }
    let result = return_type(model, method.return_type)?;
    if let Some(result) = result {
        fields.push((method.name.clone(), result, 0));
    }
    let (parts, size, _) = lay_out(fields, &model.types);
    let result = parts.last().filter(|_| result.is_some()).cloned();
    let code = entry_body(model, interface, method, &inputs, &parts, result.as_ref())?;
    Ok(Block {
        entry: format!("trust_entry_t{number}"),
        rust: format!("Tool{number}"),
        parts,
        size,
        code,
    })
}

/// The statements of the tool entry of `method` of `interface`, whose block `b` holds `parts`: the
/// instance from `THIS`, each input from the block when the mask marks it given and else at its
/// declared initial value, the call through the interface's dispatcher, and the result into the
/// block (§13.3).
fn entry_body(
    model: &mut Model<'_>,
    interface: &SmolStr,
    method: &MethodDef,
    inputs: &[&Param],
    parts: &[Member],
    result: Option<&Member>,
) -> Result<String, NativeError> {
    let mask = rust_field(GIVEN_PART);
    let mut lines = vec![format!("let r = b.{}.get();", rust_field(THIS_PART))];
    let mut arguments = vec!["cx".to_string(), "&w".to_string(), "&r".to_string()];
    let scalar_result = result.filter(|part| is_scalar(&model.types.info(part.ty).kind));
    if let Some(part) = result
        && scalar_result.is_none()
    {
        // A result that is no scalar is written into its place, here the block's (§6.4).
        arguments.push(format!("&b.{}", part.field()));
    }
    for (bit, param) in inputs.iter().enumerate() {
        let Some(part) = parts
            .iter()
            .find(|part| part.flags & SYMBOL_INPUT != 0 && part.name == param.name)
        else {
            return Err(NativeError::Compile(format!(
                "the input {} of {interface}.{} has no place in its call block",
                param.name, method.name
            )));
        };
        let field = part.field();
        let info = model.types.info(part.ty).clone();
        let given = format!("(b.{mask}[{}].get() & {}) != 0", bit / 8, 1u8 << (bit % 8));
        let argument = format!("a{bit}");
        if is_scalar(&info.kind) {
            let read = if info.scalar() == Some(Elem::Bool) {
                format!("b.{field}.get() != 0")
            } else {
                format!("b.{field}.get()")
            };
            let start = super::frame_init::omitted_scalar(
                model,
                part.ty,
                param.type_id,
                param.default.as_ref(),
                &param.name,
            )?;
            lines.push(format!(
                "let {argument} = if {given} {{ {read} }} else {{ {start} }};"
            ));
        } else {
            // The initial value in a temporary of its own, the given value where the block holds
            // it; the method copies either into its frame (§6.7).
            let bytes = super::frame_init::starting_bytes(
                model,
                part.ty,
                param.type_id,
                param.default.as_ref(),
                &param.name,
            )?;
            let mut start = Vec::new();
            super::frame_init::statements(
                model,
                &format!("d{bit}"),
                part.ty,
                &bytes,
                0,
                &mut start,
            )?;
            let storage = &info.rust;
            lines.push(format!(
                "let d{bit}: {storage} = <{storage} as Plain>::ZEROED;"
            ));
            lines.push(format!("let {argument}: &{storage} = if {given} {{"));
            lines.push(format!("    &b.{field}"));
            lines.push("} else {".to_string());
            lines.extend(start.into_iter().map(|line| format!("    {line}")));
            lines.push(format!("    &d{bit}"));
            lines.push("};".to_string());
        }
        arguments.push(argument);
    }
    let call = format!(
        "{}({})?",
        interface_function(interface, &method.name),
        arguments.join(", ")
    );
    match scalar_result {
        Some(part) => {
            let value = if model.types.info(part.ty).scalar() == Some(Elem::Bool) {
                format!("u8::from({call})")
            } else {
                call
            };
            lines.push(format!("b.{}.set({value});", part.field()));
        }
        None => lines.push(format!("{call};")),
    }
    Ok(lines
        .iter()
        .map(|line| format!("        {line}\n"))
        .collect())
}

/// Writes the call block structures, each with the assertions of its offsets (§6.3), and the tool
/// entries of `tools` for `target` into `source`, and returns the entries; each entry's index, its
/// tool symbol's, is the symbol table's to give (§5.4).
pub(super) fn emit(
    source: &mut String,
    model: &Model<'_>,
    tools: &[Tool],
    target: Target,
) -> Vec<Entry> {
    let abi = entry_abi(target);
    let mut entries = Vec::new();
    for block in tools.iter().filter_map(|tool| tool.block.as_ref()) {
        let fields: Vec<(String, String, u32)> = block
            .parts
            .iter()
            .map(|part| {
                (
                    part.field(),
                    model.types.info(part.ty).rust.clone(),
                    part.offset,
                )
            })
            .collect();
        structure(source, &block.rust, &fields, block.size, false);
        let _ = writeln!(
            source,
            "#[allow(unsafe_code)]\n#[unsafe(no_mangle)]\npub extern \"{abi}\" fn {}(cx: *mut Cx) -> u32 {{\n    run(cx, |cx, areas| {{\n        let w = World::new(cx, &areas)?;\n        let b: &{} = areas.view(cx, abi::AREA_STACK)?;\n{}        Ok(())\n    }})\n}}\n",
            block.entry, block.rust, block.code
        );
        entries.push(Entry {
            symbol: block.entry.clone(),
            kind: EntryKind::Method,
            index: 0,
        });
    }
    entries
}
