//! The retain map of a compiled program (specification 44 §5.10; specification 11 §6.7): each
//! retained variable with its checkpoint key, its area, its offset in the area and its type, in the
//! order the interpreter writes its checkpoint: the retained globals, the programs' retained
//! variables, and then the retained variables of the function block and class instances that the
//! globals and the programs hold, depth first. A variable whose type holds an instance, a reference
//! or an interface is never retained, so it has no record.

use smol_str::SmolStr;
use trust_plc_abi as abi;
use trust_runtime_core::native::image::{SYMBOL_INPUT, SYMBOL_OUTPUT};

use super::model::{retained, Model};
use super::types::{Kind, Ty};
use crate::RetainPolicy;

/// A retained variable: its checkpoint key, its area, its offset in the area and its type.
pub(super) struct Retained {
    pub(super) key: String,
    pub(super) area: u8,
    pub(super) offset: u32,
    pub(super) ty: Ty,
}

/// The retain map of `model`.
pub(super) fn retain_map(model: &mut Model<'_>) -> Vec<Retained> {
    let mut map = Vec::new();
    let globals: Vec<(SmolStr, Ty, bool)> = model
        .globals
        .iter()
        .map(|global| (global.name.clone(), global.ty, global.retained))
        .collect();
    let instances: Vec<(SmolStr, usize)> = model
        .instances
        .iter()
        .map(|instance| (instance.name.clone(), instance.program))
        .collect();
    // The retained globals, in declaration order.
    for (name, ty, kept) in &globals {
        if !kept {
            continue;
        }
        if let Some(field) = model.area_r.field(name) {
            let offset = field.offset;
            push(
                model,
                &mut map,
                name.to_string(),
                abi::AREA_RETAIN,
                offset,
                *ty,
            );
        }
    }
    // The programs' retained variables, in registration and declaration order.
    for (name, program) in &instances {
        let r_offset = model.area_r.field(name).map_or(0, |field| field.offset);
        let members: Vec<(SmolStr, u32, Ty)> = model.program_types[*program]
            .retained
            .iter()
            .map(|member| (member.name.clone(), member.offset, member.ty))
            .collect();
        for (member, offset, ty) in members {
            let key = format!("@program/{name}/{member}");
            push(
                model,
                &mut map,
                key,
                abi::AREA_RETAIN,
                r_offset + offset,
                ty,
            );
        }
    }
    // The instances the globals hold, in declaration order.
    for (name, ty, kept) in &globals {
        let (area, layout) = if *kept {
            (abi::AREA_RETAIN, &model.area_r)
        } else {
            (abi::AREA_GLOBALS, &model.area_g)
        };
        if let Some(offset) = layout.field(name).map(|field| field.offset) {
            let place = Place {
                area,
                offset,
                holder: *kept,
            };
            value(model, &mut map, name.to_string(), place, *ty);
        }
    }
    // The instances the programs hold, in registration order, each one's variables in declaration
    // order.
    for (name, program) in &instances {
        let p_offset = model.area_p.field(name).map_or(0, |field| field.offset);
        let r_offset = model.area_r.field(name).map_or(0, |field| field.offset);
        let variables: Vec<(SmolStr, RetainPolicy)> = model.program_types[*program]
            .def
            .vars
            .iter()
            .filter(|var| !var.external)
            .map(|var| (var.name.clone(), var.retain))
            .collect();
        for (variable, policy) in variables {
            // An edge-qualified input's hidden trigger follows it, retained as it is.
            let trigger = super::types::edge_trigger_name(&variable);
            for name_in_program in [variable, trigger] {
                let Some((member, in_retain)) =
                    model.program_types[*program].member(&name_in_program)
                else {
                    continue;
                };
                let (area, base) = if in_retain {
                    (abi::AREA_RETAIN, r_offset)
                } else {
                    (abi::AREA_INSTANCES, p_offset)
                };
                let (member_name, offset, ty) = (member.name.clone(), member.offset, member.ty);
                // A variable in the retained area is retained by its own qualifier or by its
                // program instance's (IEC §6.5.6.2), and so are the instances it holds.
                let place = Place {
                    area,
                    offset: base + offset,
                    holder: retained(policy) || in_retain,
                };
                value(
                    model,
                    &mut map,
                    format!("@program/{name}/{member_name}"),
                    place,
                    ty,
                );
            }
        }
    }
    map
}

/// Where a value lies, and whether the declaration that holds its instances is `RETAIN` or
/// `PERSISTENT` (IEC 61131-3 §6.5.6.2).
#[derive(Clone, Copy)]
struct Place {
    area: u8,
    offset: u32,
    holder: bool,
}

/// The retained variables of the instances that the value of type `ty` at `place` holds, under the
/// path `path`: an instance itself, the elements of an array in index order (the last index
/// fastest), the members of a structure in declaration order.
fn value(model: &mut Model<'_>, map: &mut Vec<Retained>, path: String, place: Place, ty: Ty) {
    if !holds_instances(model, ty) {
        return;
    }
    match model.types.info(ty).kind.clone() {
        Kind::Fb { .. } | Kind::StdFb(_) => instance(model, map, &path, place, ty),
        Kind::Array { elem, dims } => {
            let stride = model.types.info(elem).size;
            let mut indexes: Vec<i64> = dims.iter().map(|(lower, _)| *lower).collect();
            let mut offset = place.offset;
            loop {
                let element = format!(
                    "{path}[{}]",
                    indexes
                        .iter()
                        .map(i64::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                );
                value(model, map, element, Place { offset, ..place }, elem);
                offset += stride;
                // The next index, the last dimension fastest.
                let mut dimension = dims.len();
                loop {
                    if dimension == 0 {
                        return;
                    }
                    dimension -= 1;
                    if indexes[dimension] < dims[dimension].1 {
                        indexes[dimension] += 1;
                        break;
                    }
                    indexes[dimension] = dims[dimension].0;
                }
            }
        }
        Kind::Struct { members } => {
            for member in members {
                let place = Place {
                    offset: place.offset + member.offset,
                    ..place
                };
                value(
                    model,
                    map,
                    format!("{path}.{}", member.name),
                    place,
                    member.ty,
                );
            }
        }
        _ => {}
    }
}

/// The variables of the instance of type `ty` at `place`, under the path `path`: its inputs,
/// outputs and in-outs, then its other variables, each in declaration order, an instance a variable
/// holds where that variable lies, and then its base's variables.
fn instance(model: &mut Model<'_>, map: &mut Vec<Retained>, path: &str, place: Place, ty: Ty) {
    match model.types.info(ty).kind.clone() {
        Kind::StdFb(fb) => {
            // A library block's own state (`__LAST`) is no variable the program declares, but the
            // hidden trigger of a retained edge-qualified input keeps its memory with the input, so
            // that a warm restart fabricates no edge (IEC §6.5.2.2).
            let trigger = path
                .rsplit(['.', '/'])
                .next()
                .is_some_and(|last| last.starts_with("__TRIG_"));
            for member in model.types.std_fb_members(fb) {
                let declared = member.flags & (SYMBOL_INPUT | SYMBOL_OUTPUT) != 0
                    && !member.name.starts_with("__");
                if !(declared || trigger) || !place.holder {
                    continue;
                }
                let key = format!("{path}.{}", member.name);
                push(
                    model,
                    map,
                    key,
                    place.area,
                    place.offset + member.offset,
                    member.ty,
                );
            }
        }
        Kind::Fb {
            def, members, base, ..
        } => {
            let Some(owner) = model.types.owner(&def) else {
                return;
            };
            let declared: Vec<(SmolStr, RetainPolicy)> = owner
                .params()
                .iter()
                .map(|param| (param.name.clone(), param.retain))
                .chain(
                    owner
                        .vars()
                        .iter()
                        .filter(|var| !var.external)
                        .map(|var| (var.name.clone(), var.retain)),
                )
                .collect();
            for (name, policy) in declared {
                let Some(member) = members.iter().find(|member| {
                    member.via.is_empty() && member.name.eq_ignore_ascii_case(&name)
                }) else {
                    continue;
                };
                let key = format!("{path}.{}", member.name);
                let offset = place.offset + member.offset;
                if holds_instances(model, member.ty) {
                    // A variable that is an instance follows its own type's declarations; its own
                    // qualifier is its instances' holder (IEC 61131-3 §6.5.6.2).
                    let place = Place {
                        area: place.area,
                        offset,
                        holder: retained(policy),
                    };
                    value(model, map, key, place, member.ty);
                } else if kept(policy, place.holder) {
                    push(model, map, key, place.area, offset, member.ty);
                }
                // An edge-qualified input's hidden trigger follows it, retained as it is.
                let trigger = super::types::edge_trigger_name(&name);
                if let Some(member) = members.iter().find(|member| {
                    member.via.is_empty() && member.name.eq_ignore_ascii_case(&trigger)
                }) {
                    let place = Place {
                        area: place.area,
                        offset: place.offset + member.offset,
                        holder: kept(policy, place.holder),
                    };
                    value(
                        model,
                        map,
                        format!("{path}.{}", member.name),
                        place,
                        member.ty,
                    );
                }
            }
            if let Some(base) = base {
                // The base lies first in its instance (specification 44 §6.4).
                instance(model, map, path, place, base);
            }
        }
        _ => {}
    }
}

/// Whether a variable declared with `policy` is retained in an instance whose holder is retained or
/// not: its own qualifier decides, and without one the holder's does (IEC 61131-3 §6.5.6.2).
fn kept(policy: RetainPolicy, holder: bool) -> bool {
    match policy {
        RetainPolicy::Retain | RetainPolicy::Persistent => true,
        RetainPolicy::NonRetain => false,
        RetainPolicy::Unspecified => holder,
    }
}

/// Adds the variable to the map when a checkpoint can hold its value.
fn push(model: &Model<'_>, map: &mut Vec<Retained>, key: String, area: u8, offset: u32, ty: Ty) {
    if !holds_leaves(model, ty) {
        map.push(Retained {
            key,
            area,
            offset,
            ty,
        });
    }
}

/// Whether a value of type `ty` holds a function block or class instance.
fn holds_instances(model: &Model<'_>, ty: Ty) -> bool {
    match &model.types.info(ty).kind {
        Kind::Fb { .. } | Kind::StdFb(_) => true,
        Kind::Array { elem, .. } => holds_instances(model, *elem),
        Kind::Struct { members } => members
            .iter()
            .any(|member| holds_instances(model, member.ty)),
        _ => false,
    }
}

/// Whether a value of type `ty` holds an instance, a reference or an interface, which no checkpoint
/// holds.
fn holds_leaves(model: &Model<'_>, ty: Ty) -> bool {
    match &model.types.info(ty).kind {
        Kind::Fb { .. } | Kind::StdFb(_) | Kind::Ref { .. } | Kind::Interface => true,
        Kind::Array { elem, .. } => holds_leaves(model, *elem),
        Kind::Struct { members } => members.iter().any(|member| holds_leaves(model, member.ty)),
        _ => false,
    }
}
