//! The bodies of Rust POUs (specification 48 RU-28): the generated function of a Rust function
//! block reads the instance's variables from their cells, calls its Rust item with them in
//! declaration order and writes them back when the item returns `Ok`; the generated function of a
//! Rust function calls the Rust function with its inputs and returns its result. A fault the item
//! returns ends the scan as a fault of Structured Text does (RU-14).
//!
//! The call gets the SDK's own context, made from a clone of the generated code's, which takes back
//! only the record of the fault the call returns: a Rust POU reaches neither the statement budget,
//! the call depth nor the kernel's pointers (RU-11).
//!
//! A value goes through its place (`trust_plc::Place`, RU-25, RU-29): a scalar and a string in
//! the SDK's form of its type, a `TIME` read as `Time`, a `BYTE` as `Byte`; a structure of the
//! project's crate through the place the generated code gives its type (`rust_places`), field by
//! field; an array element by element; a reference as a `trust_plc::Ref` that reads its variable as
//! the type it lies as. A function's or a method's in-out is the place of the caller's variable
//! itself, which the call reads and writes at once.

use std::collections::{BTreeSet, HashMap};

use super::super::lower::{Lowerer, Place};
use super::super::rust_pous::{MethodLink, RustLink};
use super::super::types::{Kind, Ty, Types};
use super::super::NativeError;
use crate::program_model::Param;
use trust_hir::symbols::ParamDirection;
use trust_runtime_core::native::image::SYMBOL_IN_OUT;

/// The body of the Rust function block whose instance type is `ty`, called through `link`;
/// `structures` gives the path of each structure of the crate by its name in capitals.
pub(super) fn block(
    lowerer: &mut Lowerer<'_, '_>,
    ty: Ty,
    link: &RustLink,
    structures: &HashMap<String, String>,
) -> Result<(), NativeError> {
    let instance = instance(lowerer, ty, link, structures)?;
    let arguments: String = instance
        .values
        .iter()
        .map(|value| format!(", &mut {value}"))
        .collect();
    let (called, eno) = call_rust(
        lowerer,
        |cx| format!("{}::__trust_call({cx}{arguments})", link.call),
        true,
    );
    lowerer.line(&format!("{called}?;"));
    store_instance(lowerer, &instance, structures)?;
    let (place, eno_ty) = block_eno(lowerer, ty, link)?;
    store_eno(lowerer, place, eno_ty, &eno.unwrap_or_default());
    Ok(())
}

/// The body of the Rust program of the program type `index`, called through `link` (specification
/// 48 RU-35): its variables read from the instance's places, a retained one's from the retained
/// area's, in declaration order, and written back when the item returns `Ok`. A program has no
/// `ENO`.
pub(super) fn program(
    lowerer: &mut Lowerer<'_, '_>,
    index: usize,
    link: &RustLink,
    structures: &HashMap<String, String>,
) -> Result<(), NativeError> {
    let mut places = Vec::with_capacity(link.variables.len());
    for name in &link.variables {
        let Some((member, retained)) = lowerer.model.program_types[index].member(name) else {
            return Err(NativeError::Compile(format!(
                "the variable {name} of {} is not in its declaration",
                link.call
            )));
        };
        let owner = if retained { "mr" } else { "me" };
        places.push(Place {
            expr: format!("{owner}.{}", member.field()),
            ty: member.ty,
            is_ref: false,
        });
    }
    let in_outs = vec![false; places.len()];
    let instance = loaded(lowerer, places, in_outs, structures)?;
    let arguments: String = instance
        .values
        .iter()
        .map(|value| format!(", &mut {value}"))
        .collect();
    let (called, _) = call_rust(
        lowerer,
        |cx| format!("{}::__trust_call({cx}{arguments})", link.call),
        false,
    );
    lowerer.line(&format!("{called}?;"));
    store_instance(lowerer, &instance, structures)
}

/// The body of the Rust function with the parameters `params`, called through `link`: its inputs
/// in declaration order, its result into the function's result.
pub(super) fn function(
    lowerer: &mut Lowerer<'_, '_>,
    params: &[Param],
    link: &RustLink,
    structures: &HashMap<String, String>,
) -> Result<(), NativeError> {
    let mut arguments = String::new();
    for name in &link.variables {
        let param = parameter_named(params, name, &link.call)?;
        arguments.push_str(&format!(", {}", argument(lowerer, param, structures)?));
    }
    let (called, eno) = call_rust(
        lowerer,
        |cx| format!("{}({cx}{arguments})", link.call),
        true,
    );
    store_result(lowerer, &format!("{called}?"), structures)?;
    let Some(output) = params.iter().find(|param| {
        param.direction == ParamDirection::Out && param.name.eq_ignore_ascii_case("ENO")
    }) else {
        return Err(NativeError::Compile(format!(
            "the output ENO of {} is not in its declaration",
            link.call
        )));
    };
    let ty = lowerer.model.ty(output.type_id)?;
    let place = format!("(*{})", crate::native::lower::parameter(&output.name));
    store_eno(lowerer, place, ty, &eno.unwrap_or_default());
    Ok(())
}

/// The body of the method `method` of the Rust function block whose instance type is `ty`, called
/// through `link` (specification 48 RU-32): the instance's variables as the block's body reads
/// them, the method's inputs and in-outs as a function's, its outputs from their places in its
/// frame, all written back with its result when it returns `Ok`.
pub(super) fn method(
    lowerer: &mut Lowerer<'_, '_>,
    ty: Ty,
    params: &[Param],
    (link, method): (&RustLink, &MethodLink),
    structures: &HashMap<String, String>,
) -> Result<(), NativeError> {
    let instance = instance(lowerer, ty, link, structures)?;
    let mut arguments = String::new();
    let mut outputs = Vec::new();
    for name in &method.variables {
        let param = parameter_named(params, name, &link.call)?;
        if param.direction != ParamDirection::Out {
            arguments.push_str(&format!(", {}", argument(lowerer, param, structures)?));
            continue;
        }
        // An output starts where the method's frame holds it, at its type's default.
        let ty = lowerer.model.ty(param.type_id)?;
        let place = format!("(*{})", crate::native::lower::parameter(&param.name));
        let local = format!("ro{}", outputs.len());
        let loaded = load_in(lowerer, ty, &place, structures)?;
        lowerer.line(&format!("let mut {local} = {loaded};"));
        arguments.push_str(&format!(", &mut {local}"));
        outputs.push((ty, place, local));
    }
    let fields: String = instance
        .values
        .iter()
        .map(|value| format!(", &mut {value}"))
        .collect();
    let (called, _) = call_rust(
        lowerer,
        |cx| {
            format!(
                "{}::__trust_with({cx}{fields}, |rme, rcx| rme.{}(rcx{arguments}))",
                link.call, method.item
            )
        },
        false,
    );
    store_result(lowerer, &format!("{called}?"), structures)?;
    store_instance(lowerer, &instance, structures)?;
    for (ty, place, local) in outputs {
        store_in(lowerer, ty, &place, &local, structures)?;
    }
    Ok(())
}

/// The variables of a Rust function block's instance as a call of it reads them (specification
/// 48 RU-28): their places, the locals they are loaded into, and which of them are in-outs, whose
/// references the call reads but never rebinds (RU-29).
struct Instance {
    places: Vec<Place>,
    values: Vec<String>,
    in_outs: Vec<bool>,
}

/// Loads the variables of the instance of type `ty` that `link` names into locals.
fn instance(
    lowerer: &mut Lowerer<'_, '_>,
    ty: Ty,
    link: &RustLink,
    structures: &HashMap<String, String>,
) -> Result<Instance, NativeError> {
    let members = lowerer.info(ty).members().to_vec();
    let mut places = Vec::with_capacity(link.variables.len());
    let mut in_outs = Vec::with_capacity(link.variables.len());
    for name in &link.variables {
        let Some(member) = members
            .iter()
            .find(|member| member.via.is_empty() && member.name.eq_ignore_ascii_case(name))
        else {
            return Err(NativeError::Compile(format!(
                "the variable {name} of {} is not in its declaration",
                link.call
            )));
        };
        places.push(Place {
            expr: format!("me.{}", member.field()),
            ty: member.ty,
            is_ref: false,
        });
        in_outs.push(member.flags & SYMBOL_IN_OUT != 0);
    }
    loaded(lowerer, places, in_outs, structures)
}

/// Loads the variables at `places` into locals, `in_outs` saying which of them are in-outs.
fn loaded(
    lowerer: &mut Lowerer<'_, '_>,
    places: Vec<Place>,
    in_outs: Vec<bool>,
    structures: &HashMap<String, String>,
) -> Result<Instance, NativeError> {
    let values: Vec<String> = (0..places.len())
        .map(|index| format!("rv{index}"))
        .collect();
    for (place, value) in places.iter().zip(&values) {
        let loaded = load_in(lowerer, place.ty, &place.expr, structures)?;
        lowerer.line(&format!("let mut {value} = {loaded};"));
    }
    Ok(Instance {
        places,
        values,
        in_outs,
    })
}

/// Writes the instance's variables back but its in-outs: a string's length and the 0 after its
/// text as Structured Text's writes leave them (DEV-039).
fn store_instance(
    lowerer: &mut Lowerer<'_, '_>,
    instance: &Instance,
    structures: &HashMap<String, String>,
) -> Result<(), NativeError> {
    for ((place, value), in_out) in instance
        .places
        .iter()
        .zip(&instance.values)
        .zip(&instance.in_outs)
    {
        if !in_out {
            store_in(lowerer, place.ty, &place.expr, value, structures)?;
        }
    }
    Ok(())
}

/// The place and type of the output `ENO` of the instance of type `ty` (RU-30).
fn block_eno(
    lowerer: &Lowerer<'_, '_>,
    ty: Ty,
    link: &RustLink,
) -> Result<(String, Ty), NativeError> {
    lowerer
        .info(ty)
        .members()
        .iter()
        .find(|member| member.via.is_empty() && member.name.eq_ignore_ascii_case("ENO"))
        .map(|member| (format!("me.{}", member.field()), member.ty))
        .ok_or_else(|| {
            NativeError::Compile(format!(
                "the output ENO of {} is not in its declaration",
                link.call
            ))
        })
}

/// The input or in-out `name` of `params`, of the POU `call`.
fn parameter_named<'p>(
    params: &'p [Param],
    name: &str,
    call: &str,
) -> Result<&'p Param, NativeError> {
    params
        .iter()
        .find(|param| param.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| {
            NativeError::Compile(format!(
                "the parameter {name} of {call} is not in its declaration"
            ))
        })
}

/// The argument a Rust function or method takes for its input or in-out `param`: a scalar input
/// its value, another input the value of the caller's that it is given a reference to
/// (specification 44 §6.2), an in-out `InOut` of the caller's variable itself, whose place the
/// generated code passes (specification 48 RU-29): a reference to it, or the view of a string's of
/// any capacity of its family (docs/IEC_DEVIATIONS.md DEV-035).
fn argument(
    lowerer: &mut Lowerer<'_, '_>,
    param: &Param,
    structures: &HashMap<String, String>,
) -> Result<String, NativeError> {
    let ty = lowerer.model.ty(param.type_id)?;
    let argument = crate::native::lower::parameter(&param.name);
    if param.direction == ParamDirection::InOut {
        if matches!(lowerer.info(ty).kind, Kind::Str(_) | Kind::WStr(_)) {
            return Ok(format!("::trust_plc::InOut::new(&{argument})"));
        }
        let model = &mut *lowerer.model;
        reach(&model.types, ty, structures, &mut model.rust_places)?;
        if through_places(&model.types, ty) {
            return Ok(format!(
                "::trust_plc::InOut::new(&::trust_plc::__private::PlaceVia::<Places, _>::new({argument}))"
            ));
        }
        return Ok(format!("::trust_plc::InOut::new({argument})"));
    }
    if lowerer.info(ty).scalar().is_some() {
        return Ok(format!("::trust_plc::Stored::from_raw({argument})"));
    }
    load_in(lowerer, ty, &format!("(*{argument})"), structures)
}

/// Writes the value of `call`, an expression that ends the body on a fault, into the POU's result:
/// a result that is not a scalar into the result variable, which its exit copies out.
fn store_result(
    lowerer: &mut Lowerer<'_, '_>,
    call: &str,
    structures: &HashMap<String, String>,
) -> Result<(), NativeError> {
    match lowerer.result.clone() {
        Some(result) if lowerer.info(result.ty).scalar().is_none() => {
            let value = lowerer.bind(call);
            store_in(lowerer, result.ty, &result.expr, &value, structures)
        }
        Some(result) => {
            let value = lowerer.bind(&format!("::trust_plc::Stored::into_raw({call})"));
            lowerer.store_scalar(&result, &value);
            Ok(())
        }
        None => {
            lowerer.line(&format!("{call};"));
            Ok(())
        }
    }
}

/// Calls a Rust POU or method with `call`, which writes the call given the expression of its
/// context: the SDK's context, made from a clone of the generated code's `cx`, from which only the
/// record of the fault the call returns goes back (specification 48 RU-11). Returns the temporary
/// that holds the call's `Result` and, when `eno` asks for it, the one that holds the POU's `ENO`
/// (RU-30), which a method has not.
fn call_rust(
    lowerer: &mut Lowerer<'_, '_>,
    call: impl FnOnce(&str) -> String,
    eno: bool,
) -> (String, Option<String>) {
    let context = lowerer.temp();
    lowerer.line(&format!(
        "let mut {context} = ::trust_plc::__private::enter(cx);"
    ));
    let called = lowerer.bind(&call(&format!("&mut {context}")));
    let leave = format!("::trust_plc::__private::leave({context}, cx)");
    if eno {
        let eno = lowerer.bind(&leave);
        (called, Some(eno))
    } else {
        lowerer.line(&format!("let _ = {leave};"));
        (called, None)
    }
}

/// Writes the POU's `ENO`, the Rust expression `eno`, into its output `ENO`, the place `place` of
/// type `ty` (specification 48 RU-30).
fn store_eno(lowerer: &mut Lowerer<'_, '_>, place: String, ty: Ty, eno: &str) {
    let target = Place {
        expr: place,
        ty,
        is_ref: false,
    };
    lowerer.store_scalar(&target, eno);
}

/// The type a reference of type `ty` names, when `ty` is a reference that lies as one `RefValue`
/// cell, `REF_TO` of anything but an open array (specification 44 §7.3), which the SDK holds as a
/// `trust_plc::Ref`.
fn reference_target(types: &Types<'_>, ty: Ty) -> Option<Ty> {
    match types.info(ty).kind {
        Kind::Ref {
            target,
            pointer: false,
        } if !matches!(types.info(target).kind, Kind::Open { .. }) => Some(target),
        _ => None,
    }
}

/// [`load`] of the generated code's place `place` of type `ty`, which records in the model the
/// structures the value reaches.
fn load_in(
    lowerer: &mut Lowerer<'_, '_>,
    ty: Ty,
    place: &str,
    structures: &HashMap<String, String>,
) -> Result<String, NativeError> {
    let model = &mut *lowerer.model;
    load(
        &model.types,
        ty,
        place,
        structures,
        &mut model.rust_places,
        0,
    )
}

/// [`store`] of `value` into the generated code's place `place` of type `ty`, which records in the
/// model the structures the value reaches.
fn store_in(
    lowerer: &mut Lowerer<'_, '_>,
    ty: Ty,
    place: &str,
    value: &str,
    structures: &HashMap<String, String>,
) -> Result<(), NativeError> {
    let model = &mut *lowerer.model;
    let text = store(
        &model.types,
        ty,
        place,
        value,
        structures,
        &mut model.rust_places,
        0,
    )?;
    lowerer.line(&text);
    Ok(())
}

/// Adds to `places` each structure of the project's crate that a value of `ty` holds or names:
/// itself, an array's elements' and a reference's target's, whose places the generated code gives
/// (`rust_places`).
fn reach(
    types: &Types<'_>,
    ty: Ty,
    structures: &HashMap<String, String>,
    places: &mut BTreeSet<Ty>,
) -> Result<(), NativeError> {
    let info = types.info(ty);
    match &info.kind {
        Kind::Struct { .. } => {
            structure_path(structures, &info.name)?;
            places.insert(ty);
            Ok(())
        }
        Kind::Array { elem, .. } => reach(types, *elem, structures, places),
        _ => match reference_target(types, ty) {
            Some(target) => reach(types, target, structures, places),
            None => Ok(()),
        },
    }
}

/// Whether a value of `ty` reaches a structure of the crate, which the generated code's `Places`
/// reads and writes (`rust_places`): the structure, or an array of it.
fn through_places(types: &Types<'_>, ty: Ty) -> bool {
    match &types.info(ty).kind {
        Kind::Struct { .. } => true,
        Kind::Array { elem, .. } => through_places(types, *elem),
        _ => false,
    }
}

/// Whether the SDK carries a value of `ty` in a place of its own type (`trust_plc::Place`): a
/// scalar of RU-25's table or of a `#[trust_enum]`, or a string.
fn placed_by_sdk(types: &Types<'_>, ty: Ty) -> bool {
    let info = types.info(ty);
    info.scalar().is_some() || matches!(info.kind, Kind::Str(_) | Kind::WStr(_))
}

/// The Rust expression of the value of type `ty` that the place `place` holds, an expression of
/// the generated code's type; `depth` names the closures' variables apart. Adds to `places` the
/// structures it reaches.
pub(super) fn load(
    types: &Types<'_>,
    ty: Ty,
    place: &str,
    structures: &HashMap<String, String>,
    places: &mut BTreeSet<Ty>,
    depth: usize,
) -> Result<String, NativeError> {
    if let Some(target) = reference_target(types, ty) {
        reach(types, target, structures, places)?;
        let rust = &types.info(target).rust;
        if through_places(types, target) {
            return Ok(format!(
                "::trust_plc::__private::reference_via::<_, {rust}, Places>({place}.get())"
            ));
        }
        return Ok(format!(
            "::trust_plc::__private::reference::<_, {rust}>({place}.get())"
        ));
    }
    if placed_by_sdk(types, ty) {
        return Ok(format!("::trust_plc::Place::load(&{place})"));
    }
    let info = types.info(ty);
    match &info.kind {
        Kind::Array { elem, .. } => {
            let cell = format!("e{depth}");
            let item = load(
                types,
                *elem,
                &format!("(*{cell})"),
                structures,
                places,
                depth + 1,
            )?;
            Ok(format!(
                "::trust_plc::Array::from_items({place}.each_ref().map(|{cell}| {item}))"
            ))
        }
        Kind::Struct { .. } => {
            reach(types, ty, structures, places)?;
            Ok(format!(
                "<Places as ::trust_plc::__private::PlaceOf<_, _>>::load(&{place})"
            ))
        }
        // A Structured Text function block the Rust POU holds (specification 48 RU-39): its
        // binding, with its instance's place and the values of its inputs and outputs.
        Kind::Fb {
            is_class: false, ..
        } => {
            let (inputs, outputs) = held(types, ty)?;
            let values = |list: &[super::st_bindings::HeldMember]| {
                list.iter()
                    .map(|member| {
                        format!(
                            "{}: ::trust_plc::Place::load(&{place}.{})",
                            member.field, member.cells
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let name = &info.name;
            Ok(format!(
                "::st::{name}::__held(w.areas.refer(cx, &{place}, abi::RefValue::INSTANCE)?, \
                 ::st::{name}_Input {{ {} }}, ::st::{name}_Output {{ {} }})",
                values(&inputs),
                values(&outputs)
            ))
        }
        _ => Err(not_carried(&info.name)),
    }
}

/// The inputs and the outputs of the Structured Text function block `ty` that a Rust POU holds
/// (specification 48 RU-39).
fn held(
    types: &Types<'_>,
    ty: Ty,
) -> Result<
    (
        Vec<super::st_bindings::HeldMember>,
        Vec<super::st_bindings::HeldMember>,
    ),
    NativeError,
> {
    super::st_bindings::held_members(types, ty).map_err(|what| {
        NativeError::Compile(format!(
            "the held block {} {what}, which a held block does not pass yet (specification 48 \
             RU-39, milestone R3)",
            types.info(ty).name
        ))
    })
}

/// The statements that write the value `value` of type `ty`, a Rust expression, into the place
/// `place`; `depth` names the loops' variables apart. Adds to `places` the structures it reaches.
pub(super) fn store(
    types: &Types<'_>,
    ty: Ty,
    place: &str,
    value: &str,
    structures: &HashMap<String, String>,
    places: &mut BTreeSet<Ty>,
    depth: usize,
) -> Result<String, NativeError> {
    if reference_target(types, ty).is_some() {
        return Ok(format!(
            "{place}.set(::trust_plc::__private::reference_value({value}));"
        ));
    }
    if placed_by_sdk(types, ty) {
        return Ok(format!("::trust_plc::Place::store(&{place}, &{value});"));
    }
    let info = types.info(ty);
    match &info.kind {
        Kind::Array { elem, .. } => {
            let (item, cell) = (format!("v{depth}"), format!("e{depth}"));
            let inner = store(
                types,
                *elem,
                &format!("(*{cell})"),
                &format!("(*{item})"),
                structures,
                places,
                depth + 1,
            )?;
            Ok(format!(
                "for ({item}, {cell}) in {value}.iter().zip({place}.iter()) {{ {inner} }}"
            ))
        }
        Kind::Struct { .. } => {
            reach(types, ty, structures, places)?;
            Ok(format!(
                "<Places as ::trust_plc::__private::PlaceOf<_, _>>::store(&{place}, &{value});"
            ))
        }
        // A held block's inputs, which the Rust POU gives it, go back; its outputs are its own
        // (specification 48 RU-39).
        Kind::Fb {
            is_class: false, ..
        } => {
            let (inputs, _) = held(types, ty)?;
            Ok(inputs
                .iter()
                .map(|member| {
                    format!(
                        "::trust_plc::Place::store(&{place}.{}, &{value}.input.{});",
                        member.cells, member.field
                    )
                })
                .collect::<Vec<_>>()
                .join(" "))
        }
        _ => Err(not_carried(&info.name)),
    }
}

/// The path the generated code builds the crate's structure `name` by: its full name's, else, for
/// a type the registry names without its namespaces, the one structure of that own name.
pub(super) fn structure_path<'s>(
    structures: &'s HashMap<String, String>,
    name: &str,
) -> Result<&'s str, NativeError> {
    let upper = name.to_ascii_uppercase();
    if let Some(path) = structures.get(&upper) {
        return Ok(path);
    }
    let suffix = format!(".{upper}");
    let mut matching = structures
        .iter()
        .filter(|(full, _)| full.ends_with(&suffix))
        .map(|(_, path)| path.as_str());
    match (matching.next(), matching.next()) {
        (Some(path), None) => Ok(path),
        _ => Err(not_carried(name)),
    }
}

/// The refusal of a Rust POU's variable whose type the SDK does not carry.
fn not_carried(name: &str) -> NativeError {
    NativeError::unsupported(format!(
        "a value of the type {name} in a Rust POU's variable, which is no type of specification 48 \
         RU-25 nor a structure of the project's crate"
    ))
}
