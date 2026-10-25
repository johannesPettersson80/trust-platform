//! Located variables in the process images (specification 10 §2.5; IEC §6.5.5): each binding of an
//! `AT` declaration or a `VAR_CONFIG` entry resolved to the variable it names, its symbol and the
//! member the path selects, and an array or a structure spread into its leaves, each at its own
//! place on the wire: a `BOOL` a bit, the 8- to 64-bit elementary types a byte, a word, a double
//! word or a long word, a `TIME` a double word of unsigned whole milliseconds, consecutively, or at
//! the relative addresses of a relatively located structure, whose extent is its greatest leaf end
//! rounded to bytes and an array's stride.

use smol_str::SmolStr;
use trust_runtime_core::io_address::{IoAddress, IoSize};
use trust_runtime_core::memory::IoArea;
use trust_runtime_core::native::image::{BIND_RAW, BIND_TIME_MILLIS};

use super::model::{Binding, Global, ProgramInstance, ProgramType};
use super::types::{Elem, Kind, Ty, Types};
use super::NativeError;

/// The bindings of the declared `(path, address)` pairs: each resolved to the symbol of its
/// variable and spread into the leaves of its type.
///
/// # Errors
///
/// [`NativeError`] for a path that names no variable or member, and for a type that a process
/// image does not hold.
pub(super) fn bindings(
    types: &Types<'_>,
    declared: Vec<(Vec<SmolStr>, IoAddress)>,
    globals: &[Global],
    instances: &[ProgramInstance],
    program_types: &[ProgramType<'_>],
) -> Result<Vec<Binding>, NativeError> {
    let mut bindings = Vec::new();
    for (path, address) in declared {
        let (symbol, offset, ty) = resolve(types, &path, globals, instances, program_types)?;
        // A hierarchical address (`%IX1.2.3`, IEC §6.5.5.2) binds one elementary variable, whose
        // value lies in a slot of the kernel's hierarchical image, not in a flat image
        // (specification 44 §5.6).
        if address.path.len() > 1 {
            let mut leaves = Vec::new();
            walk(types, ty, 0, offset, &mut leaves, &path)?;
            let [leaf] = leaves.as_slice() else {
                return Err(NativeError::unsupported(format!(
                    "the located variable {} at a hierarchical address, which is no elementary \
                     value",
                    path.join(".")
                )));
            };
            bindings.push(Binding {
                path: symbol,
                address,
                leaf: leaf.offset,
                codec: leaf.codec,
            });
            continue;
        }
        let mut leaves = Vec::new();
        walk(types, ty, 0, offset, &mut leaves, &path)?;
        for leaf in leaves {
            let bits = u64::from(address.byte) * 8 + u64::from(address.bit) + leaf.wire_bit;
            let byte = u32::try_from(bits / 8).map_err(|_| too_far(&path))?;
            let leaf_address = IoAddress {
                area: address.area,
                size: leaf.size,
                byte,
                bit: u8::try_from(bits % 8).unwrap_or(0),
                path: vec![byte],
                wildcard: false,
            };
            if leaf.offset >= 1 << 24 {
                return Err(NativeError::unsupported(format!(
                    "the located variable {}, a leaf of which lies 16 MiB into it",
                    path.join(".")
                )));
            }
            bindings.push(Binding {
                path: symbol.clone(),
                address: leaf_address,
                leaf: leaf.offset,
                codec: leaf.codec,
            });
        }
    }
    Ok(bindings)
}

/// The `AT` declarations of the members of the function block instances that the program
/// instances hold, at any depth, each as its path and address (IEC §6.5.5).
pub(super) fn block_members(
    types: &Types<'_>,
    instances: &[ProgramInstance],
    program_types: &[ProgramType<'_>],
) -> Vec<(Vec<SmolStr>, IoAddress)> {
    let mut declared = Vec::new();
    for instance in instances {
        let program = &program_types[instance.program];
        for member in program.members.iter().chain(&program.retained) {
            let path = vec![instance.name.clone(), member.name.clone()];
            block(types, member.ty, &path, &mut declared, 0);
        }
    }
    declared
}

/// Adds the located members of the instance of type `ty` at `path` to `declared`.
fn block(
    types: &Types<'_>,
    ty: Ty,
    path: &[SmolStr],
    declared: &mut Vec<(Vec<SmolStr>, IoAddress)>,
    depth: usize,
) {
    // A function block cannot hold itself; the bound guards a malformed table.
    if depth > 32 {
        return;
    }
    let Some(owner) = types.owner_of(ty) else {
        return;
    };
    let addresses = owner
        .params()
        .iter()
        .map(|param| (&param.name, param.address.as_ref()))
        .chain(
            owner
                .vars()
                .iter()
                .map(|var| (&var.name, var.address.as_ref())),
        );
    for (name, address) in addresses {
        if let Some(address) = address
            && !address.wildcard
        {
            let mut member_path = path.to_vec();
            member_path.push(name.clone());
            declared.push((member_path, address.clone()));
        }
    }
    for member in types.info(ty).members() {
        if matches!(types.info(member.ty).kind, Kind::Fb { .. }) {
            let mut member_path = path.to_vec();
            member_path.push(member.name.clone());
            block(types, member.ty, &member_path, declared, depth + 1);
        }
    }
}

/// The symbol path of the variable `path` names, the offset within it of the member the rest of
/// the path selects, and that member's type.
fn resolve(
    types: &Types<'_>,
    path: &[SmolStr],
    globals: &[Global],
    instances: &[ProgramInstance],
    program_types: &[ProgramType<'_>],
) -> Result<(Vec<SmolStr>, u32, Ty), NativeError> {
    let names_none = || {
        NativeError::Compile(format!(
            "the located variable {} names no variable",
            path.join(".")
        ))
    };
    let (first, rest) = path.split_first().ok_or_else(names_none)?;
    let (symbol, mut ty, rest) = if let Some(global) = globals
        .iter()
        .find(|global| global.name.eq_ignore_ascii_case(first))
    {
        (vec![global.name.clone()], global.ty, rest)
    } else {
        let instance = instances
            .iter()
            .find(|instance| instance.name.eq_ignore_ascii_case(first))
            .ok_or_else(names_none)?;
        let (variable, rest) = rest.split_first().ok_or_else(names_none)?;
        let (member, _) = program_types[instance.program]
            .member(variable)
            .ok_or_else(names_none)?;
        (
            vec![instance.name.clone(), member.name.clone()],
            member.ty,
            rest,
        )
    };
    let mut offset = 0u32;
    for name in rest {
        let member = types
            .info(ty)
            .members()
            .iter()
            .find(|member| member.name.eq_ignore_ascii_case(name))
            .ok_or_else(names_none)?;
        offset = offset.saturating_add(member.offset);
        ty = member.ty;
    }
    Ok((symbol, offset, ty))
}

/// One leaf on the wire: its bit from the binding's address, its size and codec, and its offset
/// within the variable.
struct Leaf {
    wire_bit: u64,
    size: IoSize,
    codec: u32,
    offset: u32,
}

/// Adds the leaves of a value of type `ty` that lies at `offset` in the variable and starts at
/// `bit` on the wire to `leaves`, and returns the bit after its last leaf.
fn walk(
    types: &Types<'_>,
    ty: Ty,
    bit: u64,
    offset: u32,
    leaves: &mut Vec<Leaf>,
    path: &[SmolStr],
) -> Result<u64, NativeError> {
    let refused = |what: &str| {
        NativeError::unsupported(format!("the located variable {} of {what}", path.join(".")))
    };
    let info = types.info(ty);
    let elem = match &info.kind {
        Kind::Elem(elem) | Kind::Enum { base: elem, .. } | Kind::Subrange { base: elem, .. } => {
            *elem
        }
        Kind::Array { elem, dims } => {
            let count: u64 = dims
                .iter()
                .map(|(lower, upper)| u64::try_from(upper - lower + 1).unwrap_or(0))
                .product();
            let elem_size = types.info(*elem).size;
            // The stride is the first element's extent: its bits, or its bytes for a structure.
            let mut stride = 0;
            for index in 0..count {
                let start = bit + index * stride;
                let element_offset = u32::try_from(u64::from(elem_size) * index)
                    .ok()
                    .and_then(|element| offset.checked_add(element))
                    .ok_or_else(|| too_far(path))?;
                let end = walk(types, *elem, start, element_offset, leaves, path)?;
                if index == 0 {
                    stride = if matches!(types.info(*elem).kind, Kind::Struct { .. }) {
                        (end - start).div_ceil(8) * 8
                    } else {
                        end - start
                    };
                }
            }
            return Ok(bit + count * stride);
        }
        Kind::Struct { members } => {
            // A structure starts at a whole byte; its members follow one another, or lie at their
            // relative addresses.
            let start = bit.div_ceil(8) * 8;
            let relative = types.relative.get(&ty);
            let mut end = start;
            let mut at = start;
            for (index, member) in members.iter().enumerate() {
                let member_bit = match relative.and_then(|bits| bits.get(index).copied().flatten())
                {
                    Some(relative) => start + relative,
                    None => at,
                };
                let member_end = walk(
                    types,
                    member.ty,
                    member_bit,
                    offset.saturating_add(member.offset),
                    leaves,
                    path,
                )?;
                at = member_end;
                end = end.max(member_end);
            }
            return Ok(end.div_ceil(8) * 8);
        }
        // A `STRUCT OVERLAP` lies on the wire as its backing, byte for byte (specification 02
        // §3.6).
        Kind::Overlap { .. } => {
            let start = bit.div_ceil(8) * 8;
            for index in 0..info.size {
                leaves.push(Leaf {
                    wire_bit: start + u64::from(index) * 8,
                    size: IoSize::Byte,
                    codec: BIND_RAW,
                    offset: offset.checked_add(index).ok_or_else(|| too_far(path))?,
                });
            }
            return Ok(start + u64::from(info.size) * 8);
        }
        Kind::Str(_) => return Err(refused("a STRING")),
        _ => return Err(refused("a type a process image does not hold")),
    };
    let (size, bits, codec) = match elem {
        Elem::Bool => (IoSize::Bit, 1, BIND_RAW),
        Elem::SInt | Elem::USInt | Elem::Byte | Elem::Char => (IoSize::Byte, 8, BIND_RAW),
        Elem::Int | Elem::UInt | Elem::Word | Elem::WChar => (IoSize::Word, 16, BIND_RAW),
        Elem::DInt | Elem::UDInt | Elem::DWord | Elem::Real => (IoSize::DWord, 32, BIND_RAW),
        Elem::Time => (IoSize::DWord, 32, BIND_TIME_MILLIS),
        Elem::LInt
        | Elem::ULInt
        | Elem::LWord
        | Elem::LReal
        | Elem::LTime
        | Elem::LDate
        | Elem::LTod
        | Elem::Ldt => (IoSize::LWord, 64, BIND_RAW),
        // IEC 61131-3 Table 10 gives these types no width, so no image holds them; their long
        // forms are 64 bits of nanoseconds (specification 10 §1.2).
        Elem::Date | Elem::Tod | Elem::Dt => {
            let (name, long) = match elem {
                Elem::Date => ("DATE", "LDATE"),
                Elem::Tod => ("TOD", "LTOD"),
                _ => ("DT", "LDT"),
            };
            return Err(NativeError::Compile(format!(
                "the located variable {}: {name} has no fixed direct-address width; declare it \
                 {long}",
                path.join(".")
            )));
        }
    };
    // Every leaf but a bit starts at a whole byte.
    let start = if bits == 1 { bit } else { bit.div_ceil(8) * 8 };
    leaves.push(Leaf {
        wire_bit: start,
        size,
        codec,
        offset,
    });
    Ok(start + bits)
}

/// The error of a located variable whose leaves lie beyond what an image can address.
/// The text of `address` as a declaration spells it: `%IX1.2.3`, `%QW2.5.7.1`; the name of the
/// directly represented variable at it (`Front::direct_addresses`).
pub(super) fn address_text(address: &IoAddress) -> String {
    let area = match address.area {
        IoArea::Input => 'I',
        IoArea::Output => 'Q',
        IoArea::Memory => 'M',
    };
    let size = match address.size {
        IoSize::Bit => 'X',
        IoSize::Byte => 'B',
        IoSize::Word => 'W',
        IoSize::DWord => 'D',
        IoSize::LWord | IoSize::Bytes(_) => 'L',
    };
    let mut parts: Vec<String> = address.path.iter().map(u32::to_string).collect();
    if matches!(address.size, IoSize::Bit) {
        parts.push(address.bit.to_string());
    }
    format!("%{area}{size}{}", parts.join("."))
}

fn too_far(path: &[SmolStr]) -> NativeError {
    NativeError::unsupported(format!(
        "the located variable {}, which reaches past 4 GiB",
        path.join(".")
    ))
}
