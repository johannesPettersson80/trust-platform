//! The module's entry points (specification 44 §6.3, §8.3): one for each program instance's body,
//! and the cold-start and warm-start initializations, which set the `REF` initializers that the
//! starting memory cannot hold.

use std::fmt::Write as _;

use super::front::Front;
use super::model::Model;
use super::types::{rust_field, Kind, Ty};
use super::NativeError;
use trust_runtime_core::native::image::{EntryKind, Target};

/// An entry point of the module.
pub(super) struct Entry {
    /// The exported symbol.
    pub(super) symbol: String,
    pub(super) kind: EntryKind,
    /// The program instance's index for a program body.
    pub(super) index: u32,
}

/// The `REF(variable)` initializers of the globals and of the program instances' variables
/// (specification 44 §6.7) as Rust statements: a reference names where its variable lies, which the
/// starting memory cannot hold, so the cold-start and the warm-start initialization set it.
pub(super) fn reference_initializers(
    front: &Front,
    model: &Model<'_>,
) -> Result<Vec<String>, NativeError> {
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

/// The calling convention of an entry point on `target` (specification 44 §8.3): `sysv64` on
/// x86-64, whatever the computer's operating system, `C` elsewhere.
pub(super) fn entry_abi(target: Target) -> &'static str {
    if target == Target::X86_64 {
        "sysv64"
    } else {
        "C"
    }
}

/// The entry points of the module for `target`, written to `source`, the initializations setting
/// `references`.
///
/// # Errors
///
/// [`NativeError`] for a function block bound to a task that its program does not hold.
pub(super) fn entries(
    source: &mut String,
    model: &Model<'_>,
    target: Target,
    references: &[String],
) -> Result<Vec<Entry>, NativeError> {
    let abi = entry_abi(target);
    let mut entries = Vec::new();
    for (index, instance) in model.instances.iter().enumerate() {
        let program = &model.program_types[instance.program];
        let symbol = format!("trust_entry_p{index}");
        let field = rust_field(&instance.name);
        // The connections of the instance's inputs before its body and of its outputs after it
        // (IEC §6.8.2, Table 62 8b, 9b; `emit::connections`).
        let mut call = format!("{}(cx, &w, &w.p.{field}, &w.r.{field})", program.body);
        if instance.connects(false) {
            call = format!("connect_in_{index}(cx, &w)?;\n        {call}");
        }
        if instance.connects(true) {
            call = format!("{call}?;\n        connect_out_{index}(cx, &w)");
        }
        let _ = writeln!(
            source,
            "#[allow(unsafe_code)]\n#[unsafe(no_mangle)]\npub extern \"{abi}\" fn {symbol}(cx: *mut Cx) -> u32 {{\n    run(cx, |cx, areas| {{\n        let w = World::new(cx, &areas)?;\n        {call}\n    }})\n}}\n"
        );
        entries.push(Entry {
            symbol,
            kind: EntryKind::ProgramBody,
            index: u32::try_from(index).unwrap_or(u32::MAX),
        });
    }
    // A function block instance a task runs on its own: its body on the instance, an entry of the
    // program that holds it (IEC §6.8.2).
    for (index, block) in model.block_tasks.iter().enumerate() {
        let symbol = format!("trust_entry_b{index}");
        let call = block_body(model, block)?;
        let _ = writeln!(
            source,
            "#[allow(unsafe_code)]\n#[unsafe(no_mangle)]\npub extern \"{abi}\" fn {symbol}(cx: *mut Cx) -> u32 {{\n    run(cx, |cx, areas| {{\n        let w = World::new(cx, &areas)?;\n        {call}\n    }})\n}}\n"
        );
        entries.push(Entry {
            symbol,
            kind: EntryKind::ProgramBody,
            index: u32::try_from(block.instance).unwrap_or(u32::MAX),
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
    Ok(entries)
}

/// The call of the body of the function block instance `block` names, in an entry.
fn block_body(model: &Model<'_>, block: &super::model::BlockTask) -> Result<String, NativeError> {
    let instance = &model.instances[block.instance];
    let unknown = || {
        NativeError::Compile(format!(
            "{}.{} bound to a task is no function block instance",
            instance.name,
            block.path.join(".")
        ))
    };
    let (first, rest) = block.path.split_first().ok_or_else(unknown)?;
    let program = &model.program_types[instance.program];
    let (member, retained) = program.member(first).ok_or_else(unknown)?;
    let owner = if retained { "w.r" } else { "w.p" };
    let mut place = format!("{owner}.{}.{}", rust_field(&instance.name), member.field());
    let mut ty = member.ty;
    for name in rest {
        let member = model
            .types
            .info(ty)
            .members()
            .iter()
            .find(|member| member.name.eq_ignore_ascii_case(name))
            .ok_or_else(unknown)?;
        place = format!("{place}.{}", member.field());
        ty = member.ty;
    }
    match &model.types.info(ty).kind {
        Kind::Fb { def, .. } => Ok(format!(
            "fb_{}(cx, &w, &{place})",
            super::types::mangle(def)
        )),
        Kind::StdFb(fb) => {
            let (function, timed) = fb.body();
            let time = if timed { ", cx.now_ns" } else { "" };
            Ok(format!(
                "{{\n            {function}(&{place}{time});\n            Ok(())\n        }}"
            ))
        }
        _ => Err(unknown()),
    }
}
