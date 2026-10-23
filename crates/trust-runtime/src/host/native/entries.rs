//! The module's entry points (specification 44 §6.3, §8.3): one for each program instance's body,
//! and the cold-start and warm-start initializations, which set the `REF` initializers that the
//! starting memory cannot hold.

use std::fmt::Write as _;

use super::front::Front;
use super::model::Model;
use super::types::{mangle, rust_field, Kind, Ty};
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

/// The entry points of the module for `target`, written to `source`, the initializations setting
/// `references`.
pub(super) fn entries(
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
