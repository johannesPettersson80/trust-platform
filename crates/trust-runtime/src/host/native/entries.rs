//! The module's entry points (specification 44 §6.3, §8.3): one for each program instance's body,
//! and the cold-start and warm-start initializations, which set the `REF` initializers that the
//! starting memory cannot hold and, under the CODESYS profiles, call `FB_init` (specification 10
//! §4.8).

use std::fmt::Write as _;

use super::fb_init::VariableCalls;
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
    /// The canonical name of the top-level variable a change entry serves, and whether it serves
    /// the variable as copied rather than as dropped or added; the table writer turns them into
    /// the entry's index (specification 44 §5.4).
    pub(super) variable: Option<(String, bool)>,
}

/// The calls of CODESYS's implicit methods, each for the instances of one top-level variable
/// (specification 10 §4.8): `FB_init`'s in the initialization's order, `FB_exit`'s in the exit's
/// and `FB_reinit`'s in `FB_init`'s.
pub(super) struct Lifecycle<'c> {
    pub(super) inits: &'c [VariableCalls],
    pub(super) exits: &'c [VariableCalls],
    pub(super) reinits: &'c [VariableCalls],
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
/// `references` and then making the `FB_init` calls of `lifecycle`, the exit making its `FB_exit`
/// calls, and an online change's entries (specification 44 §6.7, §12.3).
///
/// # Errors
///
/// [`NativeError`] for a function block bound to a task that its program does not hold.
pub(super) fn entries(
    source: &mut String,
    model: &Model<'_>,
    target: Target,
    references: &[String],
    lifecycle: &Lifecycle<'_>,
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
            variable: None,
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
            variable: None,
        });
    }
    if !references.is_empty() {
        let _ = writeln!(
            source,
            "fn init_references(cx: &mut Cx, w: &World<'_>) -> R<()> {{\n    {}\n    Ok(())\n}}\n",
            references.join("\n    ")
        );
    }
    variable_functions(source, lifecycle);
    // `bInitRetains` is TRUE at a cold restart, which initializes the retained variables, and
    // FALSE at a warm one, which keeps them (specification 10 §4.8).
    for (symbol, kind, retains) in [
        ("trust_entry_cold", EntryKind::ColdInit, true),
        ("trust_entry_warm", EntryKind::WarmInit, false),
    ] {
        let mut steps = Vec::new();
        if !references.is_empty() {
            steps.push("init_references(cx, &w)?;".to_string());
        }
        if !lifecycle.inits.is_empty() {
            steps.push(format!("init_blocks(cx, &w, {retains})?;"));
        }
        let initialization = if steps.is_empty() {
            "run(cx, |_, _| Ok(()))".to_string()
        } else {
            format!(
                "run(cx, |cx, areas| {{\n        let w = World::new(cx, &areas)?;\n        {}\n        Ok(())\n    }})",
                steps.join("\n        ")
            )
        };
        let _ = writeln!(
            source,
            "#[allow(unsafe_code)]\n#[unsafe(no_mangle)]\npub extern \"{abi}\" fn {symbol}(cx: *mut Cx) -> u32 {{\n    {initialization}\n}}\n"
        );
        entries.push(Entry {
            symbol: symbol.to_string(),
            kind,
            index: 0,
            variable: None,
        });
    }
    entries.extend(lifecycle_entries(source, abi, references, lifecycle));
    Ok(entries)
}

/// The functions of the calls of `lifecycle`, each variable's in a function of its own that the
/// starts, the exit and an online change's entries call: `init_var_<k>(cx, w, retains, copy)`,
/// `exit_var_<k>(cx, w, copy)` and `reinit_var_<k>(cx, w)`, then `init_blocks` and `exit_blocks`,
/// which call every variable's with `copy` FALSE.
fn variable_functions(source: &mut String, lifecycle: &Lifecycle<'_>) {
    let function = |source: &mut String, head: String, statements: &[String]| {
        let _ = writeln!(
            source,
            "#[allow(unused_variables)]\n{head} -> R<()> {{\n    {}\n    Ok(())\n}}\n",
            statements.join("\n    ")
        );
    };
    for (k, variable) in lifecycle.inits.iter().enumerate() {
        function(
            source,
            format!("fn init_var_{k}(cx: &mut Cx, w: &World<'_>, retains: bool, copy: bool)"),
            &variable.statements,
        );
    }
    for (k, variable) in lifecycle.exits.iter().enumerate() {
        function(
            source,
            format!("fn exit_var_{k}(cx: &mut Cx, w: &World<'_>, copy: bool)"),
            &variable.statements,
        );
    }
    for (k, variable) in lifecycle.reinits.iter().enumerate() {
        function(
            source,
            format!("fn reinit_var_{k}(cx: &mut Cx, w: &World<'_>)"),
            &variable.statements,
        );
    }
    if !lifecycle.inits.is_empty() {
        let calls: Vec<String> = (0..lifecycle.inits.len())
            .map(|k| format!("init_var_{k}(cx, w, retains, false)?;"))
            .collect();
        function(
            source,
            "fn init_blocks(cx: &mut Cx, w: &World<'_>, retains: bool)".into(),
            &calls,
        );
    }
    if !lifecycle.exits.is_empty() {
        let calls: Vec<String> = (0..lifecycle.exits.len())
            .map(|k| format!("exit_var_{k}(cx, w, false)?;"))
            .collect();
        function(
            source,
            "fn exit_blocks(cx: &mut Cx, w: &World<'_>)".into(),
            &calls,
        );
    }
}

/// The exit entry, which leaves the instances before a restart or a download starts or replaces
/// their memory (specification 44 §6.7), and an online change's entries (§12.3): for each
/// variable a change-exit and a change-init entry for it dropped or added and for it copied, a
/// change-reinit entry, and the change-start entry with the reference initializers `references`.
/// None of them when the program calls no `FB_init`, `FB_exit` or `FB_reinit`.
fn lifecycle_entries(
    source: &mut String,
    abi: &str,
    references: &[String],
    lifecycle: &Lifecycle<'_>,
) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut entry = |source: &mut String, symbol: String, call: &str, kind, variable| {
        let _ = writeln!(
            source,
            "#[allow(unsafe_code)]\n#[unsafe(no_mangle)]\npub extern \"{abi}\" fn {symbol}(cx: *mut Cx) -> u32 {{\n    run(cx, |cx, areas| {{\n        let w = World::new(cx, &areas)?;\n        {call}\n    }})\n}}\n"
        );
        entries.push(Entry {
            symbol,
            kind,
            index: 0,
            variable,
        });
    };
    if !lifecycle.exits.is_empty() {
        entry(
            source,
            "trust_entry_exit".into(),
            "exit_blocks(cx, &w)",
            EntryKind::Exit,
            None,
        );
    }
    for (k, variable) in lifecycle.exits.iter().enumerate() {
        for copied in [false, true] {
            entry(
                source,
                format!("trust_entry_cx{k}_{}", u8::from(copied)),
                &format!("exit_var_{k}(cx, &w, {copied})"),
                EntryKind::ChangeExit,
                Some((variable.canonical.clone(), copied)),
            );
        }
    }
    for (k, variable) in lifecycle.inits.iter().enumerate() {
        for copied in [false, true] {
            entry(
                source,
                format!("trust_entry_ci{k}_{}", u8::from(copied)),
                &format!("init_var_{k}(cx, &w, {}, {copied})", !copied),
                EntryKind::ChangeInit,
                Some((variable.canonical.clone(), copied)),
            );
        }
    }
    for (k, variable) in lifecycle.reinits.iter().enumerate() {
        entry(
            source,
            format!("trust_entry_cr{k}"),
            &format!("reinit_var_{k}(cx, &w)"),
            EntryKind::ChangeReinit,
            Some((variable.canonical.clone(), true)),
        );
    }
    let changes =
        !(lifecycle.inits.is_empty() && lifecycle.exits.is_empty() && lifecycle.reinits.is_empty());
    if changes {
        let start = if references.is_empty() {
            "Ok(())"
        } else {
            "init_references(cx, &w)"
        };
        entry(
            source,
            "trust_entry_cs".into(),
            start,
            EntryKind::ChangeStart,
            None,
        );
    }
    entries
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
